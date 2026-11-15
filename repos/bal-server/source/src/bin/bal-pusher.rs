extern crate bitcoincore_rpc;
extern crate zmq;
use bitcoin::Network;

use bitcoincore_rpc::{Auth, Client, Error, RpcApi, bitcoin};
use bitcoincore_rpc_json::GetBlockchainInfoResult;

use data_encoding::BASE32_NOPAD;
use ed25519_dalek::{
    Signature, Signer as _, SigningKey, VerifyingKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePublicKey},
};
use hickory_resolver::Resolver;
use hickory_resolver::config::ResolverOpts;
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::{Name, RData, RecordType};
use log::{debug, error, info, trace, warn};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use sha3::{Digest, Sha3_256};
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::error::Error as StdError;
use std::net::{IpAddr, SocketAddr};
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zmq::{Context, Socket};

use bal_server::db::{
    DatabasePool, ExecutorMetadataRow, calculate_and_upsert_stats, get_executor_metadata,
    get_pending_txs, open_database,
};
use bal_server::url_policy::canonical_executor_url;
use bal_server::validation::is_valid_welist_url;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client as rClient;
use url::Url;

// BIP-65: locktime values below this are block heights, at or above are UNIX timestamps.
const LOCKTIME_THRESHOLD: i64 = 500_000_000;
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Legacy ping signature: concatenation of URL, chain, height, median time and
/// block hash, verified against `/.pub_key.pem`.
const PING_SIG_VERSION_LEGACY: u8 = 1;
/// Canonical JSON ping signature verified against `/.pub_key.pem`.
///
/// v2 is identical to v3 except for how the WeTime service discovers the
/// signing key: it fetches `/.pub_key.pem` instead of using DNSSEC or the
/// onion identity key.
const PING_SIG_VERSION_PEM: u8 = 2;
/// Canonical JSON ping signature (`WELIST-PING-V3\n` + sorted compact JSON).
const PING_SIG_VERSION_CANONICAL: u8 = 3;
/// Domain-separation prefix signed before a v2 canonical JSON payload.
const PING_SIGNATURE_PREFIX_V2: &[u8] = b"WELIST-PING-V2\n";
/// Domain-separation prefix signed before a v3 canonical JSON payload.
const PING_SIGNATURE_PREFIX_V3: &[u8] = b"WELIST-PING-V3\n";
/// Maximum accepted `info` length, matching the WeTime `/ping` validation.
const MAX_INFO_BYTES: usize = 4096;
/// Maximum accepted `version` length, matching the WeTime `/ping` validation.
const MAX_VERSION_BYTES: usize = 64;
/// Default maximum onion delegation lifetime, in days.
const DEFAULT_MAX_DELEGATION_DAYS: u64 = 366;
/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// Onion delegation certificate issued offline by the onion identity key.
///
/// The field names and types match the WeTime service's `OnionDelegation`
/// wire format. The certificate is public data; only the onion identity key
/// and the delegated ping key are secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnionDelegation {
    /// Onion host the certificate is bound to.
    onion: String,
    /// Base64 SubjectPublicKeyInfo DER of the delegated Ed25519 key.
    key: String,
    /// Unix timestamp in seconds after which the certificate is invalid.
    valid_until: u64,
    /// Base64 Ed25519 signature by the onion identity key.
    signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyConfig {
    db_backend: String,
    db_file: String,
    pg_dsn: String,
    bitcoin_dir: String,
    regtest: NetworkParams,
    testnet: NetworkParams,
    testnet4: NetworkParams,
    signet: NetworkParams,
    mainnet: NetworkParams,
    send_stats: bool,
    url: String,
    ssl_key_path: String,
    /// WeTime ping signature version. Defaults to the legacy scheme so an
    /// unmodified deployment keeps working.
    ping_sig_version: u8,
    /// Days before onion delegation expiry at which the pusher starts warning.
    /// `0` disables the warning; expiry still disables delegation.
    delegation_warn_days: u64,
    /// Maximum accepted onion delegation lifetime, in days. `0` disables it.
    max_delegation_days: u64,
    /// Optional onion delegation certificate loaded from
    /// `WELIST_PING_DELEGATION_FILE`. When present and valid, onion pings are
    /// signed with `SSL_KEY_PATH` as the delegated key instead of the onion
    /// identity key. The certificate is checked at startup against the
    /// configured onion host, its expiry, and the signing key.
    onion_delegation: Option<OnionDelegation>,
}

impl Default for MyConfig {
    fn default() -> Self {
        MyConfig {
            db_backend: env::var("BAL_SERVER_DB_BACKEND").unwrap_or("sqlite".to_string()),
            db_file: env::var("BAL_SERVER_DB_FILE").unwrap_or("bal.db".to_string()),
            pg_dsn: env::var("BAL_SERVER_PG_DSN").unwrap_or_default(),
            bitcoin_dir: env::var("BAL_PUSHER_BITCOIN_DIR").unwrap_or("".to_string()),
            regtest: get_network_params_default(Network::Regtest),
            testnet: get_network_params_default(Network::Testnet),
            testnet4: get_network_params_default(Network::Testnet4),
            signet: get_network_params_default(Network::Signet),
            mainnet: get_network_params_default(Network::Bitcoin),
            send_stats: env::var("BAL_PUSHER_SEND_STATS")
                .unwrap_or("false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
            url: env::var("BAL_SERVER_URL").unwrap_or("http://127.0.0.1:9133".to_string()),
            ssl_key_path: env::var("SSL_KEY_PATH").unwrap_or("privkey.pem".to_string()),
            ping_sig_version: parse_ping_sig_version(),
            delegation_warn_days: parse_delegation_warn_days(),
            max_delegation_days: parse_max_delegation_days(),
            onion_delegation: None,
        }
    }
}

/// Load the onion delegation certificate from `WELIST_PING_DELEGATION_FILE`.
///
/// The file is a JSON object with `onion`, `key`, `valid_until` and
/// `signature`, produced offline with the onion identity key. A missing or
/// malformed file disables delegation, so the pusher falls back to signing with
/// `SSL_KEY_PATH` directly.
fn load_onion_delegation() -> Option<OnionDelegation> {
    let path = env::var("WELIST_PING_DELEGATION_FILE").ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            warn!(
                "Cannot read WELIST_PING_DELEGATION_FILE={} ({}), onion delegation disabled",
                path, err
            );
            return None;
        }
    };
    match serde_json::from_str::<OnionDelegation>(&text) {
        Ok(delegation) => Some(delegation),
        Err(err) => {
            warn!(
                "Cannot parse WELIST_PING_DELEGATION_FILE={} ({}), onion delegation disabled",
                path, err
            );
            None
        }
    }
}

/// Parse `WELIST_PING_SIG_VERSION`, falling back to the legacy scheme.
///
/// Unsupported or malformed values are reported and treated as `1` so a typo
/// cannot silently break stats reporting.
fn parse_ping_sig_version() -> u8 {
    match env::var("WELIST_PING_SIG_VERSION") {
        Ok(value) => match value.trim().parse::<u8>() {
            Ok(version)
                if version == PING_SIG_VERSION_LEGACY
                    || version == PING_SIG_VERSION_PEM
                    || version == PING_SIG_VERSION_CANONICAL =>
            {
                version
            }
            Ok(version) => {
                warn!(
                    "Unsupported WELIST_PING_SIG_VERSION={}, using legacy scheme",
                    version
                );
                PING_SIG_VERSION_LEGACY
            }
            Err(err) => {
                warn!(
                    "Invalid WELIST_PING_SIG_VERSION={:?} ({}), using legacy scheme",
                    value, err
                );
                PING_SIG_VERSION_LEGACY
            }
        },
        Err(_) => PING_SIG_VERSION_LEGACY,
    }
}

/// Parse `BAL_PUSHER_DELEGATION_WARN_DAYS`.
///
/// Defaults to 7 days. `0` disables the expiry warning; the certificate is
/// still not sent after `valid_until`.
fn parse_delegation_warn_days() -> u64 {
    match env::var("BAL_PUSHER_DELEGATION_WARN_DAYS") {
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(days) => days,
            Err(err) => {
                warn!(
                    "Invalid BAL_PUSHER_DELEGATION_WARN_DAYS={:?} ({}), using 7",
                    value, err
                );
                7
            }
        },
        Err(_) => 7,
    }
}

/// Parse `BAL_PUSHER_MAX_DELEGATION_DAYS`.
///
/// Defaults to 366 days, matching the WeTime service. `0` disables the cap.
fn parse_max_delegation_days() -> u64 {
    match env::var("BAL_PUSHER_MAX_DELEGATION_DAYS") {
        Ok(value) => match value.trim().parse::<u64>() {
            Ok(days) => days,
            Err(err) => {
                warn!(
                    "Invalid BAL_PUSHER_MAX_DELEGATION_DAYS={:?} ({}), using {}",
                    value, err, DEFAULT_MAX_DELEGATION_DAYS
                );
                DEFAULT_MAX_DELEGATION_DAYS
            }
        },
        Err(_) => DEFAULT_MAX_DELEGATION_DAYS,
    }
}

/// Return the canonical origin for `BAL_SERVER_URL`, if it is valid.
fn canonical_config_url(cfg: &MyConfig) -> Option<String> {
    canonical_executor_url(&cfg.url).ok()
}

/// Log a clear error when v2 or v3 is selected but `BAL_SERVER_URL` is not
/// canonical.
fn validate_canonical_url(cfg: &MyConfig) {
    if cfg.ping_sig_version != PING_SIG_VERSION_LEGACY && canonical_config_url(cfg).is_none() {
        error!(
            "BAL_SERVER_URL={} is not a canonical origin (scheme://host[:port]); v2/v3 pings will be skipped",
            cfg.url
        );
    }
}

/// Return whether `valid_until` is within the configured delegation lifetime.
fn delegation_lifetime_ok(max_delegation_days: u64, valid_until: u64) -> bool {
    let max_secs = max_delegation_days.saturating_mul(SECONDS_PER_DAY);
    max_secs == 0 || valid_until <= unix_now().saturating_add(max_secs)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkParams {
    /// Canonical chain name used as the key in `tbl_executor_metadata`.
    /// It is intentionally not overridable: `db_field` is a separate setting
    /// used only for the existing database queries.
    chain: String,
    host: String,
    port: u16,
    dir_path: String,
    db_field: String,
    cookie_file: String,
    rpc_user: String,
    rpc_pass: String,
    zmq_listener: String,
}
fn get_network_params(cfg: &MyConfig, network: Network) -> &NetworkParams {
    match network {
        Network::Testnet => &cfg.testnet,
        Network::Testnet4 => &cfg.testnet4,
        Network::Signet => &cfg.signet,
        Network::Regtest => &cfg.regtest,
        _ => &cfg.mainnet,
    }
}
fn get_network_params_default(network: Network) -> NetworkParams {
    match network {
        Network::Testnet => NetworkParams {
            chain: "testnet".to_string(),
            host: "http://127.0.0.1".to_string(),
            port: 18332,
            dir_path: "testnet3/".to_string(),
            db_field: "testnet".to_string(),
            cookie_file: "".to_string(),
            rpc_user: "".to_string(),
            rpc_pass: "".to_string(),
            zmq_listener: "tcp://127.0.0.1:23332".to_string(),
        },
        Network::Testnet4 => NetworkParams {
            chain: "testnet4".to_string(),
            host: "http://127.0.0.1".to_string(),
            port: 48332,
            dir_path: "testnet4/".to_string(),
            db_field: "testnet4".to_string(),
            cookie_file: "".to_string(),
            rpc_user: "".to_string(),
            rpc_pass: "".to_string(),
            zmq_listener: "tcp://127.0.0.1:24332".to_string(),
        },
        Network::Signet => NetworkParams {
            chain: "signet".to_string(),
            host: "http://127.0.0.1".to_string(),
            port: 18332,
            dir_path: "signet/".to_string(),
            db_field: "signet".to_string(),
            cookie_file: "".to_string(),
            rpc_user: "".to_string(),
            rpc_pass: "".to_string(),
            zmq_listener: "tcp://127.0.0.1:22332".to_string(),
        },
        Network::Regtest => NetworkParams {
            chain: "regtest".to_string(),
            host: "http://127.0.0.1".to_string(),
            port: 18443,
            dir_path: "regtest/".to_string(),
            db_field: "regtest".to_string(),
            cookie_file: "".to_string(),
            rpc_user: "".to_string(),
            rpc_pass: "".to_string(),
            zmq_listener: "tcp://127.0.0.1:21332".to_string(),
        },
        _ => NetworkParams {
            chain: "bitcoin".to_string(),
            host: "http://127.0.0.1".to_string(),
            port: 8332,
            dir_path: "".to_string(),
            db_field: "bitcoin".to_string(),
            cookie_file: "".to_string(),
            rpc_user: "".to_string(),
            rpc_pass: "".to_string(),
            zmq_listener: "tcp://127.0.0.1:28332".to_string(),
        },
    }
}

fn get_cookie_filename(network: &NetworkParams) -> Result<String, Box<dyn StdError>> {
    if !network.cookie_file.is_empty() {
        Ok(network.cookie_file.clone())
    } else {
        match env::var_os("HOME") {
            Some(home) => match home.to_str() {
                Some(home_str) => {
                    let cookie_file_path =
                        format!("{}/.bitcoin/{}.cookie", home_str, network.dir_path);

                    Ok(cookie_file_path)
                }
                None => Err("wrong HOME value".into()),
            },
            None => Err("Please Set HOME environment variable".into()),
        }
    }
}
fn get_client_from_username(
    url: &str,
    network: &NetworkParams,
) -> Result<(Client, GetBlockchainInfoResult), Box<dyn StdError>> {
    if !network.rpc_user.is_empty() {
        match Client::new(
            url,
            Auth::UserPass(network.rpc_user.to_string(), network.rpc_pass.to_string()),
        ) {
            Ok(client) => match client.get_blockchain_info() {
                Ok(bcinfo) => Ok((client, bcinfo)),
                Err(err) => Err(err.into()),
            },
            Err(err) => Err(err.into()),
        }
    } else {
        Err("Failed".into())
    }
}
fn get_client_from_cookie(
    url: &str,
    network: &NetworkParams,
) -> Result<(Client, GetBlockchainInfoResult), Box<dyn StdError>> {
    match get_cookie_filename(network) {
        Ok(cookie) => match Client::new(url, Auth::CookieFile(cookie.into())) {
            Ok(client) => match client.get_blockchain_info() {
                Ok(bcinfo) => Ok((client, bcinfo)),
                Err(err) => Err(err.into()),
            },
            Err(err) => Err(err.into()),
        },
        Err(err) => Err(err),
    }
}
fn get_client(
    network: &NetworkParams,
) -> Result<(Client, GetBlockchainInfoResult), Box<dyn StdError>> {
    let url = format!("{}:{}/", network.host, network.port);
    debug!("trying to connect to bitcoin daemon:{url}");
    match get_client_from_username(&url, network) {
        Ok(client) => Ok(client),
        Err(_) => match get_client_from_cookie(&url, network) {
            Ok(client) => Ok(client),
            Err(err) => Err(err),
        },
    }
}
async fn main_result(
    cfg: &MyConfig,
    network_params: &NetworkParams,
    txt_check: &TxtCheckCache,
) -> Result<(), Error> {
    match get_client(network_params) {
        Ok((rpc, bcinfo)) => {
            info!("connected");
            info!("median time: {}", bcinfo.median_time);
            info!("blocks: {}", bcinfo.blocks);
            debug!("best block hash: {}", bcinfo.best_block_hash);

            let average_time = bcinfo.median_time;

            let connection_string = match cfg.db_backend.as_str() {
                "sqlite" => cfg.db_file.clone(),
                "postgresql" => cfg.pg_dsn.clone(),
                other => {
                    error!("Unknown DB backend: {}", other);
                    return Ok(());
                }
            };

            let db = match open_database(&cfg.db_backend, &connection_string).await {
                Ok(pool) => pool,
                Err(e) => {
                    error!("Fatal: {}", e);
                    std::process::exit(1);
                }
            };
            info!("db open {}", &connection_string);

            let pending_txs = match get_pending_txs(
                &db,
                &network_params.db_field,
                LOCKTIME_THRESHOLD,
                bcinfo.blocks as i64,
                average_time as i64,
            )
            .await
            {
                Ok(txs) => txs,
                Err(e) => {
                    warn!("Failed to query pending transactions: {}", e);
                    return Ok(());
                }
            };

            let mut pushed_txs: Vec<String> = Vec::new();
            let mut invalid_txs: HashMap<String, String> = HashMap::new();

            for row in &pending_txs {
                let txid = &row.txid;
                let tx = &row.tx;
                let locktime = row.locktime;
                info!("to be pushed: {}: {}", txid, locktime);
                match rpc.send_raw_transaction(tx.as_str()) {
                    Ok(o) => {
                        info!("tx: {} pusshata PUSHED\n{}", txid, o);
                        pushed_txs.push(txid.clone());
                    }
                    Err(err) => {
                        warn!("Error: {}\n{}", err, txid);
                        invalid_txs.insert(txid.clone(), err.to_string());
                    }
                };
            }

            for txid in &pushed_txs {
                if let Err(e) = bal_server::db::update_tx_status(&db, txid, 1, None).await {
                    error!("Failed to update tx status: {}", e);
                }
            }
            for (txid, txerr) in &invalid_txs {
                if let Err(e) = bal_server::db::update_tx_status(&db, txid, 2, Some(txerr)).await {
                    error!("Failed to update tx status: {}", e);
                }
            }
            if let Err(e) = send_stats_report(cfg, network_params, &db, txt_check, bcinfo).await {
                error!("send_stats_report failed: {}", e);
            }
            if let Err(e) = calculate_and_upsert_stats(&db, &network_params.db_field).await {
                warn!("calculate_stats failed: {e}");
            }
        }
        Err(erx) => {
            error!("impossible to get client: {}, retrying on next block", erx);
            thread::sleep(Duration::from_secs(5));
            return Ok(());
        }
    }
    Ok(())
}

/// Parse the `(host, port)` pair from a base URL like `https://host[:port]`.
fn parse_host_port(base_url: &str) -> Option<(String, u16)> {
    let url = Url::parse(base_url).ok()?;
    let host = url
        .host_str()?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    Some((host, port))
}

/// Resolve `host:port` and return the first IPv6 (AAAA) address, if any.
async fn resolve_first_ipv6(host: &str, port: u16) -> Option<SocketAddr> {
    use std::net::ToSocketAddrs;
    let host = host.to_string();
    tokio::task::spawn_blocking(move || {
        format!("{}:{}", host, port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.find(|a| a.is_ipv6()))
    })
    .await
    .ok()
    .flatten()
}

async fn welist_http_client(welist_url: &str) -> rClient {
    let prefer_ipv6 = env::var("BAL_PUSHER_PREFER_IPV6")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    if !prefer_ipv6 {
        return new_welist_client();
    }
    let (host, port) = match parse_host_port(welist_url) {
        Some(hp) => hp,
        None => {
            warn!("BAL_PUSHER_PREFER_IPV6: cannot parse '{welist_url}', using default resolver");
            return new_welist_client();
        }
    };
    match resolve_first_ipv6(&host, port).await {
        Some(addr) => {
            debug!("BAL_PUSHER_PREFER_IPV6: pinning {host} to {addr}");
            rClient::builder()
                .timeout(Duration::from_secs(10))
                .resolve(&host, addr)
                .build()
                .unwrap_or_else(|_| new_welist_client())
        }
        None => {
            debug!("BAL_PUSHER_PREFER_IPV6: no IPv6 address for {host}, using default resolver");
            new_welist_client()
        }
    }
}

fn new_welist_client() -> rClient {
    rClient::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| rClient::new())
}

/// Metadata included in a v2 or v3 canonical ping.
///
/// The values come from `tbl_executor_metadata`, which bal-server writes from
/// its effective startup configuration. The pusher never fetches them over
/// HTTP, so a network attacker cannot choose what gets signed.
#[derive(Debug, Clone, Default)]
struct ExecutorMetadata {
    info: Option<String>,
    base_fee: Option<i64>,
    version: Option<String>,
}

/// Build the signing metadata from the database snapshot.
fn metadata_from_row(row: &ExecutorMetadataRow) -> ExecutorMetadata {
    sanitize_metadata(ExecutorMetadata {
        info: Some(row.info.clone()),
        base_fee: Some(row.base_fee),
        version: Some(row.version.clone()),
    })
}

/// Normalize an executor URL exactly like the WeTime `/ping` handler.
///
/// The service normalizes the request URL before rebuilding the v1 signed
/// message, so the pusher must sign the same representation. Otherwise a
/// trailing slash or surrounding whitespace in `BAL_SERVER_URL` produces a
/// signature mismatch and the ping is rejected with `"ko"`. v2 and v3 use the
/// stricter `canonical_executor_url` policy instead.
fn normalize_executor_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Drop metadata that the WeTime service would reject.
fn sanitize_metadata(metadata: ExecutorMetadata) -> ExecutorMetadata {
    ExecutorMetadata {
        info: metadata
            .info
            .filter(|value| !value.is_empty() && value.len() <= MAX_INFO_BYTES),
        base_fee: metadata.base_fee.filter(|value| *value >= 0),
        version: metadata
            .version
            .filter(|value| !value.is_empty() && value.len() <= MAX_VERSION_BYTES),
    }
}

/// Normalize a host the same way the WeTime service does: lowercase and no
/// trailing dot. The signed URL itself is never rewritten.
fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// Return the URL host exactly as the WeTime service extracts it from the
/// normalized executor URL.
fn executor_host(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
}

/// Return `true` when the URL host is a v3 `.onion` address.
///
/// The comparison is normalized so an uppercase host or a trailing dot is still
/// recognized as onion. This is used only to decide whether the delegation
/// certificate may be attached.
fn is_onion_url(url: &str) -> bool {
    executor_host(url).is_some_and(|host| normalize_host(&host).ends_with(".onion"))
}

/// Return the current Unix time in seconds, or `0` if the clock is before the
/// epoch.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Return whether `delegation.key` matches the public key derived from
/// `signing_key`.
///
/// `None` means the public key could not be encoded as SPKI DER. Only the
/// public half is compared; the private key is never logged or transmitted.
fn delegation_key_matches(signing_key: &SigningKey, delegation: &OnionDelegation) -> Option<bool> {
    let key_der = general_purpose::STANDARD
        .decode(delegation.key.trim())
        .ok()?;
    let delegated = VerifyingKey::from_public_key_der(&key_der).ok()?;
    Some(delegated.to_bytes() == signing_key.verifying_key().to_bytes())
}

/// Validate the structural fields of a delegation certificate.
///
/// Full signature verification needs the onion identity key and is performed by
/// the WeTime service. The pusher checks what it can do locally without
/// decoding the onion address.
fn validate_delegation_structure(delegation: &OnionDelegation) -> Result<(), String> {
    if delegation.valid_until == 0 {
        return Err("valid_until is zero".to_string());
    }

    let key_bytes = general_purpose::STANDARD
        .decode(delegation.key.trim())
        .map_err(|e| format!("key is not valid base64: {e}"))?;
    VerifyingKey::from_public_key_der(&key_bytes)
        .map_err(|e| format!("key is not a valid Ed25519 SPKI: {e}"))?;

    let signature = general_purpose::STANDARD
        .decode(delegation.signature.trim())
        .map_err(|e| format!("signature is not valid base64: {e}"))?;
    if signature.len() != 64 {
        return Err(format!(
            "signature is {} bytes, expected 64",
            signature.len()
        ));
    }

    Ok(())
}

/// Domain-separation prefix signed by the onion identity key for delegation.
const DELEGATION_SIGNATURE_PREFIX: &[u8] = b"WELIST-PING-DELEGATION-V1\n";
/// Prefix used by the v3 onion address checksum.
const ONION_CHECKSUM_PREFIX: &[u8] = b".onion checksum";
/// Supported v3 onion address version.
const ONION_VERSION: u8 = 3;

/// Decode the Ed25519 identity key embedded in a v3 `.onion` address.
fn decode_onion_public_key(host: &str) -> Result<[u8; 32], String> {
    let host = normalize_host(host);
    let address = host
        .strip_suffix(".onion")
        .ok_or_else(|| "not an onion address".to_string())?;

    if address.len() != 56 {
        return Err(format!(
            "unsupported onion address length {}",
            address.len()
        ));
    }
    if !address
        .chars()
        .all(|character| matches!(character, 'a'..='z' | '2'..='7'))
    {
        return Err("invalid onion base32 characters".to_string());
    }

    let decoded = BASE32_NOPAD
        .decode(address.to_ascii_uppercase().as_bytes())
        .map_err(|err| format!("invalid onion base32: {err}"))?;
    if decoded.len() != 35 {
        return Err(format!("invalid onion payload length {}", decoded.len()));
    }

    let public_key: [u8; 32] = decoded[..32]
        .try_into()
        .map_err(|_| "invalid onion public key length".to_string())?;
    let checksum = &decoded[32..34];
    let version = decoded[34];
    if version != ONION_VERSION {
        return Err(format!("unsupported onion version {version}"));
    }

    let mut hasher = Sha3_256::new();
    hasher.update(ONION_CHECKSUM_PREFIX);
    hasher.update(public_key);
    hasher.update([version]);
    let digest = hasher.finalize();
    if checksum != &digest[..2] {
        return Err("onion checksum mismatch".to_string());
    }

    Ok(public_key)
}

/// Build the exact bytes signed by the onion identity key.
fn delegation_canonical_bytes(delegation: &OnionDelegation) -> Result<Vec<u8>, serde_json::Error> {
    let mut fields: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    fields.insert("key", json!(delegation.key.trim()));
    fields.insert("onion", json!(normalize_host(&delegation.onion)));
    fields.insert("valid_until", json!(delegation.valid_until));

    let body = serde_json::to_vec(&fields)?;
    let mut message = Vec::with_capacity(DELEGATION_SIGNATURE_PREFIX.len() + body.len());
    message.extend_from_slice(DELEGATION_SIGNATURE_PREFIX);
    message.extend_from_slice(&body);
    Ok(message)
}

/// Verify the delegation certificate signature against the onion address.
fn verify_delegation_signature(delegation: &OnionDelegation) -> Result<(), String> {
    let public_key = decode_onion_public_key(&delegation.onion)?;
    let verifying_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|err| format!("invalid onion identity key: {err}"))?;
    let message = delegation_canonical_bytes(delegation)
        .map_err(|err| format!("cannot build delegation payload: {err}"))?;
    let signature_bytes = general_purpose::STANDARD
        .decode(delegation.signature.trim())
        .map_err(|err| format!("signature is not valid base64: {err}"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|err| format!("invalid signature: {err}"))?;

    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|err| format!("invalid delegation signature: {err}"))
}

/// Last Unix day at which the delegation expiry warning was logged.
static LAST_DELEGATION_WARN_DAY: AtomicU64 = AtomicU64::new(0);
/// Last Unix day at which the delegation expired error was logged.
static LAST_DELEGATION_ERROR_DAY: AtomicU64 = AtomicU64::new(0);

/// Warn once per day when the delegation certificate is close to expiry.
fn warn_delegation_expiry(delegation: &OnionDelegation, warn_days: u64) {
    if warn_days == 0 {
        return;
    }

    let now = unix_now();
    let warn_seconds = warn_days.saturating_mul(86_400);
    if delegation.valid_until > now.saturating_add(warn_seconds) {
        return;
    }

    let day = now / 86_400;
    if LAST_DELEGATION_WARN_DAY.swap(day, Ordering::Relaxed) != day {
        warn!(
            "Onion delegation expires at {} (now {}); re-issue it before then",
            delegation.valid_until, now
        );
    }
}

/// Log an expired delegation once per day.
fn log_delegation_expired(delegation: &OnionDelegation) {
    let now = unix_now();
    let day = now / 86_400;
    if LAST_DELEGATION_ERROR_DAY.swap(day, Ordering::Relaxed) != day {
        error!(
            "Onion delegation expired at {} (now {}); sending the ping without a certificate. Re-issue the certificate.",
            delegation.valid_until, now
        );
    }
}

/// Validate the loaded onion delegation certificate against the configured
/// executor URL and signing key.
///
/// The WeTime service rejects a present-but-invalid certificate instead of
/// falling back to the onion identity key, so a stale certificate would make
/// every onion ping fail. Check the parts the pusher can verify locally at
/// startup: the certificate is bound to the configured onion host, it has not
/// expired, and its delegated key matches `SSL_KEY_PATH`. On any mismatch,
/// delegation is disabled with a warning and the pusher signs with
/// `SSL_KEY_PATH` directly.
fn validate_onion_delegation(cfg: &mut MyConfig) {
    let Some(delegation) = cfg.onion_delegation.as_ref() else {
        return;
    };

    if let Err(reason) = validate_delegation_structure(delegation) {
        warn!(
            "Onion delegation certificate is malformed ({}); disabling onion delegation",
            reason
        );
        cfg.onion_delegation = None;
        return;
    }

    if let Err(reason) = verify_delegation_signature(delegation) {
        warn!(
            "Onion delegation signature is invalid ({}); disabling onion delegation",
            reason
        );
        cfg.onion_delegation = None;
        return;
    }

    let Some(normalized_url) = canonical_config_url(cfg) else {
        warn!(
            "WELIST_PING_DELEGATION_FILE is set but BAL_SERVER_URL={} is not a canonical origin; disabling onion delegation",
            cfg.url
        );
        cfg.onion_delegation = None;
        return;
    };
    let Some(host) = executor_host(&normalized_url) else {
        warn!(
            "WELIST_PING_DELEGATION_FILE is set but BAL_SERVER_URL={} is not a valid URL; disabling onion delegation",
            cfg.url
        );
        cfg.onion_delegation = None;
        return;
    };
    let host = normalize_host(&host);

    if !host.ends_with(".onion") {
        warn!(
            "WELIST_PING_DELEGATION_FILE is set but BAL_SERVER_URL host {} is not an onion address; disabling onion delegation",
            host
        );
        cfg.onion_delegation = None;
        return;
    }

    if normalize_host(&delegation.onion) != host {
        warn!(
            "Onion delegation is bound to {} but the executor URL host is {}; disabling onion delegation. Re-issue the certificate for the exact host.",
            delegation.onion, host
        );
        cfg.onion_delegation = None;
        return;
    }

    let now = unix_now();
    if delegation.valid_until <= now {
        warn!(
            "Onion delegation expired at {} (now {}); disabling onion delegation. Re-issue the certificate.",
            delegation.valid_until, now
        );
        cfg.onion_delegation = None;
        return;
    }
    if !delegation_lifetime_ok(cfg.max_delegation_days, delegation.valid_until) {
        warn!(
            "Onion delegation valid_until={} exceeds the {} day limit; disabling onion delegation. Re-issue a shorter certificate.",
            delegation.valid_until, cfg.max_delegation_days
        );
        cfg.onion_delegation = None;
        return;
    }

    let signing_key = match SigningKey::read_pkcs8_pem_file(&cfg.ssl_key_path) {
        Ok(key) => key,
        Err(err) => {
            warn!(
                "Cannot read SSL_KEY_PATH={} to validate onion delegation ({}); disabling onion delegation",
                cfg.ssl_key_path, err
            );
            cfg.onion_delegation = None;
            return;
        }
    };

    match delegation_key_matches(&signing_key, delegation) {
        Some(true) => {
            warn_delegation_expiry(delegation, cfg.delegation_warn_days);
        }
        Some(false) => {
            warn!(
                "Onion delegation delegates to a different key than SSL_KEY_PATH={}; disabling onion delegation. Re-issue the certificate for the current ping key.",
                cfg.ssl_key_path
            );
            cfg.onion_delegation = None;
        }
        None => {
            warn!(
                "Cannot encode the public key from SSL_KEY_PATH={}; disabling onion delegation",
                cfg.ssl_key_path
            );
            cfg.onion_delegation = None;
        }
    }
}

/// Return the delegation certificate to send for `url`, if any.
///
/// Delegation is onion-only: sending it for a DNS host would be rejected by
/// the WeTime service. An expired certificate is never sent, because the
/// service rejects a present-but-invalid certificate instead of falling back
/// to the onion identity key.
fn delegation_for_url<'a>(cfg: &'a MyConfig, url: &str) -> Option<&'a OnionDelegation> {
    if !is_onion_url(url) {
        return None;
    }

    let delegation = cfg.onion_delegation.as_ref()?;
    if delegation.valid_until <= unix_now() {
        log_delegation_expired(delegation);
        return None;
    }
    if !delegation_lifetime_ok(cfg.max_delegation_days, delegation.valid_until) {
        debug!(
            "Onion delegation valid_until={} exceeds the configured lifetime; not sending it",
            delegation.valid_until
        );
        return None;
    }

    warn_delegation_expiry(delegation, cfg.delegation_warn_days);
    Some(delegation)
}

/// Derive the base64 SPKI public key from a signing key.
fn signing_public_key(signing_key: &SigningKey) -> Result<String, Box<dyn StdError>> {
    let public_der = signing_key
        .verifying_key()
        .to_public_key_der()
        .map_err(|e| format!("cannot encode signing public key: {e}"))?;
    Ok(general_purpose::STANDARD.encode(public_der.as_bytes()))
}

/// Normalize a base64 SubjectPublicKeyInfo DER key from a TXT record.
fn normalize_spki_b64(key_b64: &str) -> Result<String, Box<dyn StdError>> {
    let der = general_purpose::STANDARD
        .decode(key_b64.trim())
        .map_err(|err| format!("key is not valid base64: {err}"))?;
    let key = VerifyingKey::from_public_key_der(&der)
        .map_err(|err| format!("key is not a valid Ed25519 SPKI: {err}"))?;
    let public_der = key
        .to_public_key_der()
        .map_err(|err| format!("cannot encode TXT public key: {err}"))?;
    Ok(general_purpose::STANDARD.encode(public_der.as_bytes()))
}

/// Return the base64 SPKI public key contained in the PEM served by bal-server.
fn server_public_key(pem: &str) -> Result<String, Box<dyn StdError>> {
    let key = VerifyingKey::from_public_key_pem(pem)
        .map_err(|e| format!("cannot parse server public key PEM: {e}"))?;
    let public_der = key
        .to_public_key_der()
        .map_err(|e| format!("cannot encode server public key: {e}"))?;
    Ok(general_purpose::STANDARD.encode(public_der.as_bytes()))
}

/// Return `Ok(())` when the public key served by bal-server matches the key
/// that signs pings.
fn verify_server_public_key(pem: &str, signing_key: &SigningKey) -> Result<(), String> {
    if pem.trim().is_empty() {
        return Err("server did not publish a public key".to_string());
    }

    let server = server_public_key(pem).map_err(|e| e.to_string())?;
    let local = signing_public_key(signing_key).map_err(|e| e.to_string())?;
    if server != local {
        return Err(format!(
            "server public key {server} does not match signing key {local}"
        ));
    }

    Ok(())
}

/// Last Unix hour at which a public-key gate error was logged.
static LAST_PUBKEY_ERROR_HOUR: AtomicU64 = AtomicU64::new(0);

/// Log a refused ping at most once per hour per process.
fn log_pubkey_error(chain: &str, reason: &str) {
    let hour = unix_now() / 3600;
    if LAST_PUBKEY_ERROR_HOUR.swap(hour, Ordering::Relaxed) != hour {
        error!("Refusing to send WeTime ping for {}: {}", chain, reason);
    } else {
        debug!("Refusing to send WeTime ping for {}: {}", chain, reason);
    }
}

/// Cached result of the one-time v3 DNS TXT consistency check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxtCheckResult {
    /// The record exists and matches, or the check does not apply.
    Match,
    /// The record exists but does not match, or is absent/malformed.
    Mismatch,
    /// DNS timed out or was otherwise unavailable; do not block.
    Unavailable,
}

#[derive(Default)]
struct TxtCheckState {
    key_b64: Option<String>,
    result: Option<TxtCheckResult>,
}

struct TxtCheckCache {
    state: tokio::sync::Mutex<TxtCheckState>,
}

impl TxtCheckCache {
    fn new() -> Self {
        Self {
            state: tokio::sync::Mutex::new(TxtCheckState::default()),
        }
    }

    /// Run the check once per signing key and cache the result for the process.
    async fn ensure_checked(&self, cfg: &MyConfig, signing_key_b64: &str) -> TxtCheckResult {
        let mut state = self.state.lock().await;
        if state.key_b64.as_deref() == Some(signing_key_b64)
            && let Some(result) = state.result
        {
            return result;
        }

        let result = run_txt_check(cfg, signing_key_b64).await;
        state.key_b64 = Some(signing_key_b64.to_string());
        state.result = Some(result);
        result
    }
}

enum TxtLookup {
    Match,
    Mismatch(String),
    Unavailable(String),
}

/// Perform the startup-only TXT check.
///
/// NXDOMAIN, empty answers, malformed records, and a mismatching key are
/// definitive mismatches. A timeout or resolver error is transient and does not
/// stop pings; the service remains the authoritative verifier.
async fn run_txt_check(cfg: &MyConfig, signing_key_b64: &str) -> TxtCheckResult {
    let Some(executor_url) = canonical_config_url(cfg) else {
        warn!("BAL_SERVER_URL is not a canonical v3 origin; skipping TXT check");
        return TxtCheckResult::Mismatch;
    };
    let Some(host) = executor_host(&executor_url) else {
        return TxtCheckResult::Match;
    };
    let host = normalize_host(&host);
    if host.ends_with(".onion") || host.parse::<IpAddr>().is_ok() {
        debug!("TXT check skipped for non-DNS host {}", host);
        return TxtCheckResult::Match;
    }

    match lookup_txt_key(&host, signing_key_b64).await {
        TxtLookup::Match => {
            debug!("TXT record matches the signing key for {}", host);
            TxtCheckResult::Match
        }
        TxtLookup::Unavailable(reason) => {
            warn!(
                "TXT lookup unavailable for {}: {}; continuing without blocking pings",
                host, reason
            );
            TxtCheckResult::Unavailable
        }
        TxtLookup::Mismatch(reason) => {
            warn!(
                "TXT record mismatch for {}: {}; retrying in 30 seconds",
                host, reason
            );
            tokio::time::sleep(Duration::from_secs(30)).await;

            match lookup_txt_key(&host, signing_key_b64).await {
                TxtLookup::Match => {
                    info!("TXT record matched on the second lookup for {}", host);
                    TxtCheckResult::Match
                }
                TxtLookup::Mismatch(reason) => {
                    error!(
                        "TXT record mismatch for {} after two lookups: {}; skipping WeTime pings until restart",
                        host, reason
                    );
                    TxtCheckResult::Mismatch
                }
                TxtLookup::Unavailable(reason) => {
                    warn!(
                        "second TXT lookup unavailable for {}: {}; continuing",
                        host, reason
                    );
                    TxtCheckResult::Unavailable
                }
            }
        }
    }
}

/// Query `_welist-key.<host>` and compare the key with the signing key.
async fn lookup_txt_key(host: &str, expected_key_b64: &str) -> TxtLookup {
    let mut options = ResolverOpts::default();
    options.cache_size = 0;
    options.timeout = Duration::from_secs(5);
    options.attempts = 1;

    let resolver: Resolver<TokioRuntimeProvider> = match Resolver::builder_tokio() {
        Ok(builder) => match builder.with_options(options).build() {
            Ok(resolver) => resolver,
            Err(err) => return TxtLookup::Unavailable(format!("resolver build: {err}")),
        },
        Err(err) => return TxtLookup::Unavailable(format!("system resolver: {err}")),
    };

    let query_name = format!("_welist-key.{host}");
    // Append a trailing dot so the resolver treats the query as an absolute
    // FQDN and does not apply the system search list or `ndots`.
    let fqdn = format!("{query_name}.");
    let name = match Name::from_ascii(&fqdn) {
        Ok(name) => name,
        Err(err) => {
            return TxtLookup::Mismatch(format!("invalid DNS name {fqdn}: {err}"));
        }
    };

    let lookup = match tokio::time::timeout(
        Duration::from_secs(5),
        resolver.lookup(name.clone(), RecordType::TXT),
    )
    .await
    {
        Ok(Ok(lookup)) => lookup,
        Ok(Err(err)) if err.is_nx_domain() || err.is_no_records_found() => {
            return TxtLookup::Mismatch(format!("no TXT records: {err}"));
        }
        Ok(Err(err)) => return TxtLookup::Unavailable(err.to_string()),
        Err(_) => return TxtLookup::Unavailable("DNS lookup timed out".to_string()),
    };

    let mut saw_txt = false;
    let mut malformed = false;
    let mut mismatched = Vec::new();

    for record in lookup.answers() {
        if record.record_type() != RecordType::TXT {
            continue;
        }
        let record_name = record.name.to_string();
        if !record_name
            .trim_end_matches('.')
            .eq_ignore_ascii_case(query_name.trim_end_matches('.'))
        {
            return TxtLookup::Mismatch(format!(
                "unexpected record name {} (aliases are not accepted)",
                record_name
            ));
        }

        let RData::TXT(txt) = &record.data else {
            continue;
        };
        saw_txt = true;

        let mut bytes = Vec::new();
        for chunk in txt.txt_data.iter() {
            bytes.extend_from_slice(chunk);
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(err) => {
                malformed = true;
                mismatched.push(format!("TXT is not UTF-8: {err}"));
                continue;
            }
        };

        match parse_txt_key(&text) {
            Some(key) => match normalize_spki_b64(&key) {
                Ok(normalized) if normalized == expected_key_b64 => return TxtLookup::Match,
                Ok(normalized) => mismatched.push(normalized),
                Err(err) => {
                    malformed = true;
                    mismatched.push(err.to_string());
                }
            },
            None => {
                malformed = true;
                mismatched.push(format!("malformed TXT record: {text}"));
            }
        }
    }

    if !saw_txt {
        return TxtLookup::Mismatch("no TXT records in the answer".to_string());
    }
    if malformed || !mismatched.is_empty() {
        return TxtLookup::Mismatch(format!(
            "no matching key (found: {})",
            mismatched.join(", ")
        ));
    }

    TxtLookup::Mismatch("no matching TXT key".to_string())
}

/// Parse `v=we1; alg=ed25519; key=<base64 SPKI>` and return the key.
fn parse_txt_key(record: &str) -> Option<String> {
    let mut version = None;
    let mut algorithm = None;
    let mut key = None;

    for field in record.split(';') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let (name, value) = field.split_once('=')?;
        match name.trim().to_ascii_lowercase().as_str() {
            "v" => version = Some(value.trim().to_string()),
            "alg" => algorithm = Some(value.trim().to_ascii_lowercase()),
            "key" => key = Some(value.trim().to_string()),
            _ => {}
        }
    }

    if version.as_deref() != Some("we1") || algorithm.as_deref() != Some("ed25519") {
        return None;
    }

    key
}

/// Fields covered by a v2 or v3 canonical WeTime ping signature.
#[derive(Clone, Copy, Debug)]
struct PingPayload<'a> {
    /// Ping signature version: `PING_SIG_VERSION_PEM` (2) or
    /// `PING_SIG_VERSION_CANONICAL` (3).
    sig_version: u8,
    /// Canonical executor URL.
    url: &'a str,
    /// Canonical lowercase chain name.
    chain: &'a str,
    /// Self-reported block height.
    height: u64,
    /// Self-reported median time.
    median_time: u64,
    /// Self-reported block hash.
    last_block_hash: &'a str,
    /// Optional executor description.
    info: Option<&'a str>,
    /// Optional executor base fee.
    base_fee: Option<i64>,
    /// Optional executor version.
    version: Option<&'a str>,
}

impl PingPayload<'_> {
    /// Build the exact bytes covered by a v2 or v3 ping signature.
    ///
    /// The output is the domain-separation prefix for the selected version
    /// followed by compact JSON with sorted keys, every field present, and
    /// `null` for absent optional values. This must match the canonicalization
    /// performed by the WeTime `/ping` handler byte for byte.
    fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let prefix = if self.sig_version == PING_SIG_VERSION_PEM {
            PING_SIGNATURE_PREFIX_V2
        } else {
            PING_SIGNATURE_PREFIX_V3
        };

        let mut fields: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        fields.insert("base_fee", json!(self.base_fee));
        fields.insert("chain", json!(self.chain));
        fields.insert("height", json!(self.height));
        fields.insert("info", json!(self.info));
        fields.insert("last_block_hash", json!(self.last_block_hash));
        fields.insert("median_time", json!(self.median_time));
        fields.insert("sig_version", json!(self.sig_version));
        fields.insert("url", json!(self.url));
        fields.insert("version", json!(self.version));

        let body = serde_json::to_vec(&fields)?;
        let mut message = Vec::with_capacity(prefix.len() + body.len());
        message.extend_from_slice(prefix);
        message.extend_from_slice(&body);
        Ok(message)
    }
}

/// Build the exact bytes signed by a v1 ping.
///
/// The URL is normalized the same way the service normalizes the request URL,
/// so a trailing slash or surrounding whitespace in `BAL_SERVER_URL` cannot
/// invalidate the signature.
fn legacy_ping_message(
    url: &str,
    chain: &str,
    height: u64,
    median_time: u64,
    last_block_hash: &str,
) -> String {
    format!(
        "{}{}{}{}{}",
        normalize_executor_url(url),
        chain,
        height,
        median_time,
        last_block_hash
    )
}

async fn send_stats_report(
    cfg: &MyConfig,
    network_params: &NetworkParams,
    db: &DatabasePool,
    txt_check: &TxtCheckCache,
    bcinfo: GetBlockchainInfoResult,
) -> Result<(), reqwest::Error> {
    if !cfg.send_stats {
        debug!("Not sending stats");
        return Ok(());
    }

    debug!("sending report to welist");
    let welist_url =
        env::var("WELIST_SERVER_URL").unwrap_or("https://welist.bitcoin-after.life".to_string());
    let skip_validation = env::var("WELIST_SKIP_URL_VALIDATION")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap_or(false);
    if !skip_validation && !is_valid_welist_url(&welist_url) {
        warn!(
            "Invalid or unsafe WELIST_SERVER_URL: {}. Skipping stats report.",
            welist_url
        );
        return Ok(());
    }

    let signing_key = match SigningKey::read_pkcs8_pem_file(&cfg.ssl_key_path) {
        Ok(key) => key,
        Err(err) => {
            error!(
                "Cannot read SSL_KEY_PATH={}, skipping stats report: {}",
                cfg.ssl_key_path, err
            );
            return Ok(());
        }
    };

    // Metadata is best effort: v1 and v2 must keep working against databases
    // that predate `tbl_executor_metadata`. Missing values are signed as null.
    let (metadata, pubkey_pem) = match get_executor_metadata(db, &network_params.chain).await {
        Ok(Some(row)) => (metadata_from_row(&row), row.pubkey_pem),
        Ok(None) => {
            debug!(
                "No executor metadata row for {}; signing null metadata",
                network_params.chain
            );
            (ExecutorMetadata::default(), None)
        }
        Err(err) => {
            warn!(
                "Cannot read executor metadata for {} ({}); signing null metadata",
                network_params.chain, err
            );
            (ExecutorMetadata::default(), None)
        }
    };

    // When bal-server published a key, verify that it matches SSL_KEY_PATH.
    // The legacy v1/v2 verifier fetches this key from `/.pub_key.pem`. If the
    // row or key is unavailable, continue with null metadata instead of
    // blocking the ping.
    if let Some(pem) = pubkey_pem.as_deref()
        && let Err(reason) = verify_server_public_key(pem, &signing_key)
    {
        log_pubkey_error(&network_params.chain, &reason);
        return Ok(());
    }

    let signing_key_b64 = match signing_public_key(&signing_key) {
        Ok(key) => key,
        Err(err) => {
            error!(
                "Cannot derive the signing public key, skipping stats report: {}",
                err
            );
            return Ok(());
        }
    };

    let canonical_url = if cfg.ping_sig_version != PING_SIG_VERSION_LEGACY {
        match canonical_config_url(cfg) {
            Some(url) => Some(url),
            None => {
                log_pubkey_error(
                    &network_params.chain,
                    "BAL_SERVER_URL is not a canonical origin",
                );
                return Ok(());
            }
        }
    } else {
        None
    };

    if cfg.ping_sig_version == PING_SIG_VERSION_CANONICAL {
        let result = txt_check.ensure_checked(cfg, &signing_key_b64).await;
        if result == TxtCheckResult::Mismatch {
            debug!(
                "Skipping WeTime ping for {}: TXT check reported a mismatch",
                network_params.chain
            );
            return Ok(());
        }
    }

    let client = welist_http_client(&welist_url).await;
    let url = format!("{}/ping", welist_url);
    debug!("welist url: {}", url);
    let chain = bcinfo.chain.to_string().to_lowercase();
    let last_block_hash = bcinfo.best_block_hash.to_string();
    // v1 signs the normalized URL sent in the request body. Use the same
    // normalized form the service derives, so a trailing slash or whitespace in
    // `BAL_SERVER_URL` cannot invalidate the signature.
    let executor_url = normalize_executor_url(&cfg.url);

    let request = client
        .post(url)
        .header("User-Agent", format!("bal-pusher/{}", VERSION));

    let request = if cfg.ping_sig_version == PING_SIG_VERSION_LEGACY {
        // The v1 signature authenticates only the legacy liveness
        // concatenation. `info`, `base_fee` and `version` are intentionally
        // not sent: they are outside the signature, and the WeTime service
        // ignores them for v1 pings. Use v2 or v3 when metadata must be
        // published and authenticated.
        let message = legacy_ping_message(
            &cfg.url,
            &chain,
            bcinfo.blocks,
            bcinfo.median_time,
            &last_block_hash,
        );
        trace!("message to be sent: {}", message);
        let sign =
            general_purpose::STANDARD.encode(signing_key.sign(message.as_bytes()).to_bytes());
        request.json(&json!(
        {
            "url":              executor_url,
            "chain":            chain,
            "height":           bcinfo.blocks,
            "median_time":      bcinfo.median_time,
            "last_block_hash":  last_block_hash,
            "signature":        sign,
        }))
    } else {
        // v2 and v3 share the same canonical JSON payload. They differ only in
        // how the WeTime service discovers the signing key: v2 fetches
        // `/.pub_key.pem`, while v3 uses DNSSEC or the onion identity key.
        let Some(executor_url) = canonical_url.as_deref() else {
            error!("Canonical URL missing after validation, skipping stats report");
            return Ok(());
        };
        let sig_version = cfg.ping_sig_version;
        let payload = PingPayload {
            sig_version,
            url: executor_url,
            chain: &chain,
            height: bcinfo.blocks,
            median_time: bcinfo.median_time,
            last_block_hash: &last_block_hash,
            info: metadata.info.as_deref(),
            base_fee: metadata.base_fee,
            version: metadata.version.as_deref(),
        };
        let message = match payload.canonical_bytes() {
            Ok(message) => message,
            Err(err) => {
                error!("Cannot build canonical ping payload, skipping: {}", err);
                return Ok(());
            }
        };
        trace!(
            "canonical message to be sent: {}",
            String::from_utf8_lossy(&message)
        );
        let sign = general_purpose::STANDARD.encode(signing_key.sign(&message).to_bytes());
        let mut body = json!({
            "url":              executor_url,
            "chain":            chain,
            "height":           bcinfo.blocks,
            "median_time":      bcinfo.median_time,
            "last_block_hash":  last_block_hash,
            "signature":        sign,
            "sig_version":      sig_version,
            "info":             metadata.info,
            "base_fee":         metadata.base_fee,
            "version":          metadata.version,
        });
        if sig_version == PING_SIG_VERSION_CANONICAL
            && let Some(delegation) = delegation_for_url(cfg, executor_url)
        {
            body["delegation"] = json!(delegation);
        }
        request.json(&body)
    };

    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;
    info!(
        "Report to welist({}) status={} body={}",
        welist_url, status, body
    );

    Ok(())
}

fn parse_env(cfg: &mut MyConfig) {
    if let Ok(value) = env::var("BAL_SERVER_DB_BACKEND") {
        cfg.db_backend = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_DB_FILE") {
        cfg.db_file = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_PG_DSN") {
        cfg.pg_dsn = value;
    }
    cfg.regtest = parse_env_netconfig(cfg, "regtest");
    cfg.signet = parse_env_netconfig(cfg, "signet");
    cfg.testnet = parse_env_netconfig(cfg, "testnet");
    cfg.testnet4 = parse_env_netconfig(cfg, "testnet4");
    drop(parse_env_netconfig(cfg, "bitcoin"));
}
fn parse_env_netconfig(cfg_lock: &mut MyConfig, chain: &str) -> NetworkParams {
    let cfg = match chain {
        "regtest" => &mut cfg_lock.regtest,
        "signet" => &mut cfg_lock.signet,
        "testnet" => &mut cfg_lock.testnet,
        "testnet4" => &mut cfg_lock.testnet4,
        &_ => &mut cfg_lock.mainnet,
    };
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_HOST", chain.to_uppercase())) {
        cfg.host = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_PORT", chain.to_uppercase()))
        && let Ok(port_num) = value.parse::<u64>()
    {
        if let Ok(port) = u16::try_from(port_num) {
            cfg.port = port;
        } else {
            error!(
                "Port value {} exceeds u16 range for chain {}",
                port_num, chain
            );
        }
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_DIR_PATH", chain.to_uppercase())) {
        cfg.dir_path = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_DB_FIELD", chain.to_uppercase())) {
        cfg.db_field = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_COOKIE_FILE", chain.to_uppercase())) {
        cfg.cookie_file = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_RPC_USER", chain.to_uppercase())) {
        cfg.rpc_user = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_RPC_PASSWORD", chain.to_uppercase())) {
        cfg.rpc_pass = value;
    }
    if let Ok(value) = env::var(format!("BAL_PUSHER_{}_ZMQ_HASHBLOCK", chain.to_uppercase())) {
        cfg.zmq_listener = value;
    }
    cfg.clone()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let mut cfg = MyConfig::default();

    parse_env(&mut cfg);
    validate_canonical_url(&cfg);
    cfg.onion_delegation = load_onion_delegation();
    validate_onion_delegation(&mut cfg);
    let mut args = std::env::args();
    let _exe_name = args.next().unwrap();
    let arg_network = match args.next() {
        Some(nargs) => nargs,
        None => "bitcoin".to_string(),
    };
    let network = match arg_network.as_str() {
        "testnet" => Network::Testnet,
        "testnet4" => Network::Testnet4,
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        _ => Network::Bitcoin,
    };

    info!("Network: {}", arg_network);
    let network_params = get_network_params(&cfg, network);
    let txt_check = TxtCheckCache::new();

    let context = Context::new();
    let socket: Socket = context.socket(zmq::SUB).unwrap();

    let zmq_address = network_params.zmq_listener.clone();
    info!("zmq listening on: {}", zmq_address);
    loop {
        match socket.connect(&zmq_address) {
            Ok(_) => break,
            Err(e) => {
                error!("ZMQ connect failed: {}, retrying in 5s...", e);
                thread::sleep(Duration::from_secs(5));
            }
        }
    }

    match socket.set_subscribe(b"") {
        Ok(_) => {
            info!("ZMQ subscribed to all topics on {}", zmq_address);
        }
        Err(e) => {
            error!("ZMQ subscribe failed: {}, exiting", e);
            return Ok(());
        }
    }

    if let Err(e) = main_result(&cfg, network_params, &txt_check).await {
        error!("main_result failed on startup: {}", e);
    }
    info!("waiting new blocks..");
    socket.set_rcvtimeo(5000).unwrap(); // 5 seconds timeout
    let mut consecutive_timeouts: u32 = 0;
    loop {
        let message = match socket.recv_multipart(0) {
            Ok(m) => m,
            Err(e) => {
                consecutive_timeouts += 1;
                if consecutive_timeouts.is_multiple_of(720) {
                    error!(
                        "No ZMQ messages for {}s ({} consecutive timeouts), is bitcoind ZMQ active on {}?",
                        consecutive_timeouts * 5,
                        consecutive_timeouts,
                        zmq_address
                    );
                } else {
                    trace!("ZMQ recv timeout or error: {}, retrying...", e);
                }
                continue;
            }
        };
        if consecutive_timeouts > 0 {
            info!(
                "ZMQ connection restored after {} consecutive timeouts",
                consecutive_timeouts
            );
        }
        consecutive_timeouts = 0;
        let Some(topic) = message.first().cloned() else {
            warn!("ZMQ: received empty multipart message, skipping");
            continue;
        };
        let body = message.get(1).cloned().unwrap_or_default();
        debug!("ZMQ:GET TOPIC: {}", String::from_utf8_lossy(&topic));
        trace!("ZMQ:GET BODY: {}", hex::encode(&body));
        if topic == b"hashblock" {
            info!("NEW BLOCK: {}", hex::encode(&body));
            if let Err(e) = main_result(&cfg, network_params, &txt_check).await {
                error!("main_result failed on new block: {}", e);
            }
        }
        thread::sleep(Duration::from_millis(100)); // Sleep for 100ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_port_https_default_port() {
        assert_eq!(
            parse_host_port("https://welist.bitcoin-after.life"),
            Some(("welist.bitcoin-after.life".to_string(), 443))
        );
    }

    #[test]
    fn parse_host_port_explicit_port_and_path() {
        assert_eq!(
            parse_host_port("https://example.com:8443/ping"),
            Some(("example.com".to_string(), 8443))
        );
    }

    #[test]
    fn parse_host_port_http_default_port() {
        assert_eq!(
            parse_host_port("http://example.com"),
            Some(("example.com".to_string(), 80))
        );
    }

    #[test]
    fn parse_host_port_ipv6_literal_brackets_stripped() {
        assert_eq!(
            parse_host_port("https://[2a13:2c0::1]:443"),
            Some(("2a13:2c0::1".to_string(), 443))
        );
    }

    #[test]
    fn parse_host_port_invalid_url() {
        assert_eq!(parse_host_port("not a url"), None);
    }

    #[test]
    fn normalize_executor_url_trims_and_strips_trailing_slashes() {
        assert_eq!(
            normalize_executor_url("  https://we.example.com/  "),
            "https://we.example.com"
        );
        assert_eq!(
            normalize_executor_url("https://we.example.com"),
            "https://we.example.com"
        );
    }

    #[test]
    fn legacy_ping_message_normalizes_the_configured_url() {
        let with_slash = legacy_ping_message("  https://we.example.com/  ", "bitcoin", 1, 2, "abc");
        let normalized = legacy_ping_message("https://we.example.com", "bitcoin", 1, 2, "abc");
        assert_eq!(with_slash, "https://we.example.combitcoin12abc");
        assert_eq!(with_slash, normalized);
    }

    #[test]
    fn canonical_config_url_uses_the_origin_form() {
        let cfg = MyConfig {
            url: "  https://We.Example.com/  ".to_string(),
            ..MyConfig::default()
        };
        assert_eq!(
            canonical_config_url(&cfg).as_deref(),
            Some("https://we.example.com")
        );

        let cfg = MyConfig {
            url: "https://user@example.com".to_string(),
            ..MyConfig::default()
        };
        assert!(canonical_config_url(&cfg).is_none());
    }

    #[test]
    fn delegation_lifetime_is_capped() {
        let now = unix_now();
        assert!(delegation_lifetime_ok(
            DEFAULT_MAX_DELEGATION_DAYS,
            now + 365 * SECONDS_PER_DAY
        ));
        assert!(!delegation_lifetime_ok(
            DEFAULT_MAX_DELEGATION_DAYS,
            now + 367 * SECONDS_PER_DAY
        ));
        assert!(delegation_lifetime_ok(0, u64::MAX));
    }

    #[test]
    fn delegation_over_max_lifetime_is_not_sent() {
        let identity = SigningKey::from_bytes(&[11u8; 32]);
        let delegated = SigningKey::from_bytes(&[12u8; 32]);
        let onion = onion_address_from_public_key(&identity.verifying_key().to_bytes());
        let url = format!("http://{onion}");
        let delegation =
            signed_test_delegation(&identity, &delegated, unix_now() + 2 * SECONDS_PER_DAY);
        let cfg = MyConfig {
            url: url.clone(),
            max_delegation_days: 1,
            onion_delegation: Some(delegation),
            ..MyConfig::default()
        };

        assert!(delegation_for_url(&cfg, &url).is_none());
    }

    #[test]
    fn onion_urls_are_detected_after_normalization() {
        assert!(is_onion_url(
            "http://abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrstuv.onion"
        ));
        assert!(is_onion_url(
            "http://ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUV.onion."
        ));
        assert!(!is_onion_url("https://we.example.com"));
        assert!(!is_onion_url("not a url"));
    }

    #[test]
    fn normalize_host_lowercases_and_strips_trailing_dot() {
        assert_eq!(normalize_host("  Example.COM.  "), "example.com");
        assert_eq!(normalize_host("example.com"), "example.com");
        assert_eq!(normalize_host("EXAMPLE.ONION."), "example.onion");
    }

    fn test_delegation(onion: &str) -> OnionDelegation {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let public_der = signing_key
            .verifying_key()
            .to_public_key_der()
            .expect("public key encodes as SPKI DER");
        OnionDelegation {
            onion: onion.to_string(),
            key: general_purpose::STANDARD.encode(public_der.as_bytes()),
            valid_until: unix_now() + 3600,
            signature: general_purpose::STANDARD.encode([0u8; 64]),
        }
    }

    #[test]
    fn delegation_structure_accepts_valid_fields() {
        assert!(validate_delegation_structure(&test_delegation("example.onion")).is_ok());
    }

    #[test]
    fn delegation_structure_rejects_invalid_fields() {
        let mut bad_signature = test_delegation("example.onion");
        bad_signature.signature = general_purpose::STANDARD.encode([0u8; 63]);
        assert!(validate_delegation_structure(&bad_signature).is_err());

        let mut bad_key = test_delegation("example.onion");
        bad_key.key = "not base64!".to_string();
        assert!(validate_delegation_structure(&bad_key).is_err());

        let mut zero_expiry = test_delegation("example.onion");
        zero_expiry.valid_until = 0;
        assert!(validate_delegation_structure(&zero_expiry).is_err());
    }

    #[test]
    fn delegation_for_url_does_not_return_an_expired_certificate() {
        let cfg = MyConfig {
            onion_delegation: Some(OnionDelegation {
                valid_until: unix_now().saturating_sub(1),
                ..test_delegation("example.onion")
            }),
            ..MyConfig::default()
        };
        assert!(delegation_for_url(&cfg, "http://example.onion").is_none());
    }

    #[test]
    fn server_public_key_matches_the_signing_key() {
        use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;

        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("public key encodes as PEM");

        assert!(verify_server_public_key(&pem, &signing_key).is_ok());

        let other_key = SigningKey::from_bytes(&[8u8; 32]);
        assert!(verify_server_public_key(&pem, &other_key).is_err());
        assert!(verify_server_public_key("not a pem", &signing_key).is_err());
        assert!(verify_server_public_key("", &signing_key).is_err());
    }

    #[test]
    fn normalize_spki_b64_matches_the_signing_key() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let spki = general_purpose::STANDARD.encode(
            signing_key
                .verifying_key()
                .to_public_key_der()
                .unwrap()
                .as_bytes(),
        );

        assert_eq!(
            normalize_spki_b64(&spki).unwrap(),
            signing_public_key(&signing_key).unwrap()
        );
        assert!(normalize_spki_b64("not base64!").is_err());
    }

    #[test]
    fn metadata_from_row_sanitizes_values() {
        let row = ExecutorMetadataRow {
            info: "ok".to_string(),
            base_fee: 1500,
            version: VERSION.to_string(),
            pubkey_pem: None,
        };
        let metadata = metadata_from_row(&row);
        assert_eq!(metadata.info.as_deref(), Some("ok"));
        assert_eq!(metadata.base_fee, Some(1500));
        assert_eq!(metadata.version.as_deref(), Some(VERSION));

        let row = ExecutorMetadataRow {
            info: "x".repeat(MAX_INFO_BYTES + 1),
            base_fee: -1,
            version: String::new(),
            pubkey_pem: None,
        };
        let metadata = metadata_from_row(&row);
        assert_eq!(metadata.info, None);
        assert_eq!(metadata.base_fee, None);
        assert_eq!(metadata.version, None);
    }

    #[test]
    fn network_params_use_canonical_chain_names() {
        assert_eq!(
            get_network_params_default(Network::Bitcoin).chain,
            "bitcoin"
        );
        assert_eq!(
            get_network_params_default(Network::Testnet).chain,
            "testnet"
        );
        assert_eq!(
            get_network_params_default(Network::Testnet4).chain,
            "testnet4"
        );
        assert_eq!(get_network_params_default(Network::Signet).chain, "signet");
        assert_eq!(
            get_network_params_default(Network::Regtest).chain,
            "regtest"
        );
    }

    #[test]
    fn delegation_is_only_returned_for_onion_urls() {
        let cfg = MyConfig {
            onion_delegation: Some(test_delegation("example.onion")),
            ..MyConfig::default()
        };

        assert!(delegation_for_url(&cfg, "http://example.onion").is_some());
        assert!(delegation_for_url(&cfg, "https://we.example.com").is_none());

        let cfg = MyConfig::default();
        assert!(delegation_for_url(&cfg, "http://example.onion").is_none());
    }

    #[test]
    fn delegation_bound_to_another_onion_is_disabled() {
        let mut cfg = MyConfig {
            url: "http://expected.onion".to_string(),
            onion_delegation: Some(test_delegation("other.onion")),
            ..MyConfig::default()
        };

        validate_onion_delegation(&mut cfg);

        assert!(cfg.onion_delegation.is_none());
    }

    #[test]
    fn delegation_on_non_onion_url_is_disabled() {
        let mut cfg = MyConfig {
            url: "https://we.example.com".to_string(),
            onion_delegation: Some(test_delegation("example.onion")),
            ..MyConfig::default()
        };

        validate_onion_delegation(&mut cfg);

        assert!(cfg.onion_delegation.is_none());
    }

    #[test]
    fn delegation_key_must_match_the_signing_key() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let public_der = signing_key
            .verifying_key()
            .to_public_key_der()
            .expect("public key encodes as SPKI DER");
        let matching = OnionDelegation {
            key: general_purpose::STANDARD.encode(public_der.as_bytes()),
            ..test_delegation("example.onion")
        };

        assert_eq!(delegation_key_matches(&signing_key, &matching), Some(true));

        let wrapped = OnionDelegation {
            key: format!("\n  {}\n", matching.key),
            ..matching.clone()
        };
        assert_eq!(delegation_key_matches(&signing_key, &wrapped), Some(true));

        let other_key = SigningKey::from_bytes(&[8u8; 32]);
        let other_der = other_key
            .verifying_key()
            .to_public_key_der()
            .expect("public key encodes as SPKI DER");
        let mismatching = OnionDelegation {
            key: general_purpose::STANDARD.encode(other_der.as_bytes()),
            ..matching
        };
        assert_eq!(
            delegation_key_matches(&signing_key, &mismatching),
            Some(false)
        );
    }

    #[test]
    fn canonical_ping_payload_is_sorted_compact_and_complete() {
        let payload = PingPayload {
            sig_version: PING_SIG_VERSION_CANONICAL,
            url: "https://example.com",
            chain: "bitcoin",
            height: 100,
            median_time: 1000,
            last_block_hash: "abc",
            info: None,
            base_fee: None,
            version: None,
        };
        let payload = payload.canonical_bytes().unwrap();
        assert_eq!(
            String::from_utf8(payload).unwrap(),
            "WELIST-PING-V3\n{\"base_fee\":null,\"chain\":\"bitcoin\",\"height\":100,\"info\":null,\"last_block_hash\":\"abc\",\"median_time\":1000,\"sig_version\":3,\"url\":\"https://example.com\",\"version\":null}"
        );
    }

    #[test]
    fn v2_ping_payload_is_canonical_json_like_v3() {
        let v3 = PingPayload {
            sig_version: PING_SIG_VERSION_CANONICAL,
            url: "https://example.com",
            chain: "bitcoin",
            height: 100,
            median_time: 1000,
            last_block_hash: "abc",
            info: Some("info"),
            base_fee: Some(1500),
            version: Some("0.3.0"),
        };
        let v2 = PingPayload {
            sig_version: PING_SIG_VERSION_PEM,
            ..v3
        };

        let v3_bytes = String::from_utf8(v3.canonical_bytes().unwrap()).unwrap();
        let v2_bytes = String::from_utf8(v2.canonical_bytes().unwrap()).unwrap();

        assert!(v2_bytes.starts_with("WELIST-PING-V2\n"));
        assert!(v2_bytes.contains("\"sig_version\":2"));
        assert_eq!(
            v2_bytes
                .replacen("WELIST-PING-V2\n", "WELIST-PING-V3\n", 1)
                .replacen("\"sig_version\":2", "\"sig_version\":3", 1),
            v3_bytes
        );
    }

    #[test]
    fn canonical_ping_payload_includes_metadata_and_escapes_strings() {
        let payload = PingPayload {
            sig_version: PING_SIG_VERSION_CANONICAL,
            url: "https://example.com",
            chain: "bitcoin",
            height: 1,
            median_time: 2,
            last_block_hash: "abc",
            info: Some("line\n\"quoted\""),
            base_fee: Some(1500),
            version: Some("0.3.0"),
        };
        let payload = payload.canonical_bytes().unwrap();
        let message = String::from_utf8(payload).unwrap();
        assert!(message.contains("\"base_fee\":1500"));
        assert!(message.contains("\"info\":\"line\\n\\\"quoted\\\"\""));
        assert!(message.contains("\"version\":\"0.3.0\""));
    }

    fn onion_address_from_public_key(public_key: &[u8; 32]) -> String {
        let mut payload = Vec::new();
        payload.extend_from_slice(public_key);

        let mut hasher = Sha3_256::new();
        hasher.update(ONION_CHECKSUM_PREFIX);
        hasher.update(public_key);
        hasher.update([ONION_VERSION]);
        let digest = hasher.finalize();
        payload.extend_from_slice(&digest[..2]);
        payload.push(ONION_VERSION);

        format!(
            "{}.onion",
            BASE32_NOPAD.encode(&payload).to_ascii_lowercase()
        )
    }

    fn signed_test_delegation(
        identity: &SigningKey,
        delegated: &SigningKey,
        valid_until: u64,
    ) -> OnionDelegation {
        let onion = onion_address_from_public_key(&identity.verifying_key().to_bytes());
        let key = general_purpose::STANDARD.encode(
            delegated
                .verifying_key()
                .to_public_key_der()
                .expect("public key encodes as SPKI DER")
                .as_bytes(),
        );
        let mut delegation = OnionDelegation {
            onion,
            key,
            valid_until,
            signature: String::new(),
        };
        let message = delegation_canonical_bytes(&delegation).unwrap();
        delegation.signature = general_purpose::STANDARD.encode(identity.sign(&message).to_bytes());
        delegation
    }

    #[test]
    fn onion_address_round_trip_and_normalization() {
        let identity = SigningKey::from_bytes(&[1u8; 32]);
        let public_key = identity.verifying_key().to_bytes();
        let onion = onion_address_from_public_key(&public_key);

        assert_eq!(decode_onion_public_key(&onion).unwrap(), public_key);
        assert_eq!(
            decode_onion_public_key(&onion.to_ascii_uppercase()).unwrap(),
            public_key
        );
        assert_eq!(
            decode_onion_public_key(&format!("{onion}.")).unwrap(),
            public_key
        );
    }

    #[test]
    fn onion_address_rejects_bad_checksum_and_version() {
        let identity = SigningKey::from_bytes(&[1u8; 32]);
        let public_key = identity.verifying_key().to_bytes();

        let mut hasher = Sha3_256::new();
        hasher.update(ONION_CHECKSUM_PREFIX);
        hasher.update(public_key);
        hasher.update([ONION_VERSION]);
        let digest = hasher.finalize();
        let mut payload = Vec::new();
        payload.extend_from_slice(&public_key);
        payload.extend_from_slice(&[digest[0] ^ 0xff, digest[1] ^ 0xff]);
        payload.push(ONION_VERSION);
        let bad_checksum = format!(
            "{}.onion",
            BASE32_NOPAD.encode(&payload).to_ascii_lowercase()
        );
        assert!(decode_onion_public_key(&bad_checksum).is_err());

        let mut hasher = Sha3_256::new();
        hasher.update(ONION_CHECKSUM_PREFIX);
        hasher.update(public_key);
        hasher.update([2u8]);
        let digest = hasher.finalize();
        let mut payload = Vec::new();
        payload.extend_from_slice(&public_key);
        payload.extend_from_slice(&digest[..2]);
        payload.push(2);
        let bad_version = format!(
            "{}.onion",
            BASE32_NOPAD.encode(&payload).to_ascii_lowercase()
        );
        assert!(decode_onion_public_key(&bad_version).is_err());
    }

    #[test]
    fn onion_delegation_signature_round_trip_and_exact_payload() {
        let identity = SigningKey::from_bytes(&[1u8; 32]);
        let delegated = SigningKey::from_bytes(&[2u8; 32]);
        let delegation = signed_test_delegation(&identity, &delegated, 1_800_000_000);

        assert!(verify_delegation_signature(&delegation).is_ok());

        let message = delegation_canonical_bytes(&delegation).unwrap();
        let text = String::from_utf8(message).unwrap();
        assert!(text.starts_with("WELIST-PING-DELEGATION-V1\n{\"key\":\""));
        assert!(text.contains("\"onion\":\""));
        assert!(text.ends_with(",\"valid_until\":1800000000}"));
    }

    #[test]
    fn onion_delegation_signature_rejects_tampering() {
        let identity = SigningKey::from_bytes(&[1u8; 32]);
        let delegated = SigningKey::from_bytes(&[2u8; 32]);
        let valid = signed_test_delegation(&identity, &delegated, 1_800_000_000);

        let mut tampered = valid.clone();
        tampered.valid_until += 1;
        assert!(verify_delegation_signature(&tampered).is_err());

        let other_delegated = SigningKey::from_bytes(&[3u8; 32]);
        let mut tampered = valid.clone();
        tampered.key = general_purpose::STANDARD.encode(
            other_delegated
                .verifying_key()
                .to_public_key_der()
                .unwrap()
                .as_bytes(),
        );
        assert!(verify_delegation_signature(&tampered).is_err());

        let mut tampered = valid.clone();
        let other_identity = SigningKey::from_bytes(&[4u8; 32]);
        tampered.onion = onion_address_from_public_key(&other_identity.verifying_key().to_bytes());
        assert!(verify_delegation_signature(&tampered).is_err());

        let mut tampered = valid;
        let wrong_signer = SigningKey::from_bytes(&[5u8; 32]);
        let message = delegation_canonical_bytes(&tampered).unwrap();
        tampered.signature =
            general_purpose::STANDARD.encode(wrong_signer.sign(&message).to_bytes());
        assert!(verify_delegation_signature(&tampered).is_err());
    }

    #[test]
    fn parse_txt_key_accepts_only_we1_ed25519() {
        assert_eq!(
            parse_txt_key("v=we1; alg=ed25519; key=ABC"),
            Some("ABC".to_string())
        );
        assert_eq!(
            parse_txt_key("alg=ED25519; v=we1; key=ABC; extra=ignored"),
            Some("ABC".to_string())
        );
        assert_eq!(parse_txt_key("v=we2; alg=ed25519; key=ABC"), None);
        assert_eq!(parse_txt_key("v=we1; alg=rsa; key=ABC"), None);
        assert_eq!(parse_txt_key("v=we1; alg=ed25519"), None);
        assert_eq!(parse_txt_key("not a txt record"), None);
    }

    #[test]
    fn sanitize_metadata_drops_values_the_service_rejects() {
        let metadata = sanitize_metadata(ExecutorMetadata {
            info: Some("x".repeat(MAX_INFO_BYTES + 1)),
            base_fee: Some(-1),
            version: Some(String::new()),
        });
        assert_eq!(metadata.info, None);
        assert_eq!(metadata.base_fee, None);
        assert_eq!(metadata.version, None);

        let metadata = sanitize_metadata(ExecutorMetadata {
            info: Some("ok".to_string()),
            base_fee: Some(0),
            version: Some(VERSION.to_string()),
        });
        assert_eq!(metadata.info.as_deref(), Some("ok"));
        assert_eq!(metadata.base_fee, Some(0));
        assert_eq!(metadata.version.as_deref(), Some(VERSION));
    }
}
