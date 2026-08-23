extern crate bitcoincore_rpc;
extern crate zmq;
use bitcoin::Network;

use bitcoincore_rpc::{Auth, Client, Error, RpcApi, bitcoin};
use bitcoincore_rpc_json::GetBlockchainInfoResult;

use ed25519_dalek::{Signer as _, SigningKey, pkcs8::DecodePrivateKey};
use log::{debug, error, info, trace, warn};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::error::Error as StdError;
use std::str;
use std::{thread, time::Duration};
use zmq::{Context, Socket};

use bal_server::db::{calculate_and_upsert_stats, get_pending_txs, open_database};
use bal_server::validation::is_valid_welist_url;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client as rClient;
use std::net::SocketAddr;
use url::Url;

// BIP-65: locktime values below this are block heights, at or above are UNIX timestamps.
const LOCKTIME_THRESHOLD: i64 = 500_000_000;
const VERSION: &str = env!("CARGO_PKG_VERSION");
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
}

impl Default for MyConfig {
    fn default() -> Self {
        MyConfig {
            db_backend: env::var("BAL_PUSHER_DB_BACKEND").unwrap_or("sqlite".to_string()),
            db_file: env::var("BAL_PUSHER_DB_FILE").unwrap_or("bal.db".to_string()),
            pg_dsn: env::var("BAL_PUSHER_PG_DSN").unwrap_or_default(),
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
            url: env::var("BAL_SERVER_URL").unwrap_or("http://localhost/".to_string()),
            ssl_key_path: env::var("SSL_KEY_PATH").unwrap_or("privkey.pem".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkParams {
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
async fn main_result(cfg: &MyConfig, network_params: &NetworkParams) -> Result<(), Error> {
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
            if let Err(e) = send_stats_report(cfg, bcinfo).await {
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

async fn send_stats_report(
    cfg: &MyConfig,
    bcinfo: GetBlockchainInfoResult,
) -> Result<(), reqwest::Error> {
    if cfg.send_stats {
        debug!("sending report to welist");
        let welist_url = env::var("WELIST_SERVER_URL")
            .unwrap_or("https://welist.bitcoin-after.life".to_string());
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
        let client = welist_http_client(&welist_url).await;
        let url = format!("{}/ping", welist_url);
        debug!("welist url: {}", url);
        let chain = bcinfo.chain.to_string().to_lowercase();
        let message = format!(
            "{0}{1}{2}{3}{4}",
            cfg.url, chain, bcinfo.blocks, bcinfo.median_time, bcinfo.best_block_hash
        );
        trace!("message to be sent: {}", message);
        let sign = sign_message(cfg.ssl_key_path.as_str(), message.as_str());
        let response = client
            .post(url)
            .header("User-Agent", format!("bal-pusher/{}", VERSION))
            .json(&json!(
            {
                "url":              cfg.url,
                "chain":            chain,
                "height":           bcinfo.blocks,
                "median_time":      bcinfo.median_time,
                "last_block_hash":  bcinfo.best_block_hash,
                "signature":        sign,
            }))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        info!(
            "Report to welist({}) status={} body={}",
            welist_url, status, body
        );
    } else {
        debug!("Not sending stats");
    }
    Ok(())
}
fn sign_message(private_key_path: &str, message: &str) -> String {
    let signing_key =
        SigningKey::read_pkcs8_pem_file(private_key_path).expect("failed to parse private key PEM");
    let signature = signing_key.sign(message.as_bytes());

    general_purpose::STANDARD.encode(signature.to_bytes())
}

fn parse_env(cfg: &mut MyConfig) {
    if let Ok(value) = env::var("BAL_PUSHER_DB_BACKEND") {
        cfg.db_backend = value;
    }
    if let Ok(value) = env::var("BAL_PUSHER_DB_FILE") {
        cfg.db_file = value;
    }
    if let Ok(value) = env::var("BAL_PUSHER_PG_DSN") {
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

    if let Err(e) = main_result(&cfg, network_params).await {
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
        let topic = message[0].clone();
        let body = message[1].clone();
        debug!(
            "ZMQ:GET TOPIC: {}",
            String::from_utf8(topic.clone()).expect("invalid topic")
        );
        trace!("ZMQ:GET BODY: {}", hex::encode(&body));
        if topic == b"hashblock" {
            info!("NEW BLOCK: {}", hex::encode(&body));
            if let Err(e) = main_result(&cfg, network_params).await {
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
}
