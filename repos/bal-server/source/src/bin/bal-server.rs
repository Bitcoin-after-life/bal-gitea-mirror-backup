use actix_governor::{Governor, GovernorConfigBuilder, KeyExtractor, SimpleKeyExtractionError};
use actix_web::dev::ServiceRequest;
use actix_web::middleware;
use actix_web::web::Bytes;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use bitcoin::{Network, Transaction, consensus};
use chrono::Utc;
use hex_conservative::FromHex;
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::net::IpAddr;
use std::str::FromStr;

use bal_server::db::{
    DatabasePool, InsertInpData, InsertOutData, InsertTxData, check_duplicate_txids,
    create_database, get_all_addresses_by_xpub, get_last_used_address_by_ip,
    get_next_address_index, get_stats, insert_xpub, open_database, save_new_address, search_tx,
};
use bal_server::xpub::new_address_from_xpub;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NETWORKS: [&str; 5] = ["bitcoin", "testnet", "testnet4", "signet", "regtest"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetConfig {
    address: String,
    fixed_fee: u64,
    xpub: bool,
    network: Network,
    name: String,
    enabled: bool,
}

impl NetConfig {
    fn default_network(name: String, network: Network) -> Self {
        NetConfig {
            address: "".to_string(),
            fixed_fee: 50000,
            xpub: false,
            name,
            network,
            enabled: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MyConfig {
    regtest: NetConfig,
    signet: NetConfig,
    testnet: NetConfig,
    testnet4: NetConfig,
    mainnet: NetConfig,
    info: String,
    bind_address: String,
    bind_port: u16,
    db_backend: String,
    db_file: String,
    pg_dsn: String,
    pub_key_path: String,
    expose_stats: bool,
}

impl Default for MyConfig {
    fn default() -> Self {
        MyConfig {
            regtest: NetConfig::default_network("regtest".to_string(), Network::Regtest),
            signet: NetConfig::default_network("signet".to_string(), Network::Signet),
            testnet: NetConfig::default_network("testnet".to_string(), Network::Testnet),
            testnet4: NetConfig::default_network("testnet4".to_string(), Network::Testnet4),
            mainnet: NetConfig::default_network("bitcoin".to_string(), Network::Bitcoin),
            bind_address: "127.0.0.1".to_string(),
            bind_port: 9137,
            db_backend: "sqlite".to_string(),
            db_file: "bal.db".to_string(),
            pg_dsn: String::new(),
            info: "Will Executor Server".to_string(),
            pub_key_path: "public_key.pem".to_string(),
            expose_stats: env::var("BAL_SERVER_EXPOSE_STATS")
                .unwrap_or("false".to_string())
                .parse::<bool>()
                .unwrap_or(false),
        }
    }
}

impl MyConfig {
    fn get_net_config(&self, param: &str) -> &NetConfig {
        match param {
            "regtest" => &self.regtest,
            "testnet" => &self.testnet,
            "testnet4" => &self.testnet4,
            "signet" => &self.signet,
            _ => &self.mainnet,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InfoResponse {
    pub address: String,
    pub base_fee: u64,
    pub chain: String,
    pub info: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub report_date: String,
    pub chain: String,
    pub totals: i64,
    pub waiting: i64,
    pub sent: i64,
    pub failed: i64,
    pub waiting_profit: i64,
    pub sent_profit: i64,
    pub missed_profit: i64,
    pub unique_inputs: i64,
}

#[derive(Debug, Clone)]
#[expect(dead_code)]
struct ActixConfig {
    max_body_size: usize,
    timeout_secs: u64,
    rate_limit_pushtxs: (u64, u32),
    rate_limit_searchtx: (u64, u32),
    rate_limit_info: (u64, u32),
    rate_limit_default: (u64, u32),
    workers: usize,
    max_connections: usize,
    trusted_proxy: IpAddr,
}

fn parse_actix_config() -> ActixConfig {
    ActixConfig {
        max_body_size: env::var("BAL_SERVER_ACTIX_MAX_BODY_SIZE")
            .unwrap_or("1048576".to_string())
            .parse::<usize>()
            .unwrap_or(1_048_576),
        timeout_secs: env::var("BAL_SERVER_ACTIX_TIMEOUT_SECS")
            .unwrap_or("5".to_string())
            .parse::<u64>()
            .unwrap_or(5),
        rate_limit_pushtxs: (
            env::var("BAL_SERVER_ACTIX_PUSHTXS_PER_SEC")
                .unwrap_or("1".to_string())
                .parse::<u64>()
                .unwrap_or(1),
            env::var("BAL_SERVER_ACTIX_PUSHTXS_BURST")
                .unwrap_or("3".to_string())
                .parse::<u32>()
                .unwrap_or(3),
        ),
        rate_limit_searchtx: (
            env::var("BAL_SERVER_ACTIX_SEARCHTX_PER_SEC")
                .unwrap_or("5".to_string())
                .parse::<u64>()
                .unwrap_or(5),
            env::var("BAL_SERVER_ACTIX_SEARCHTX_BURST")
                .unwrap_or("10".to_string())
                .parse::<u32>()
                .unwrap_or(10),
        ),
        rate_limit_info: (
            env::var("BAL_SERVER_ACTIX_INFO_PER_SEC")
                .unwrap_or("20".to_string())
                .parse::<u64>()
                .unwrap_or(20),
            env::var("BAL_SERVER_ACTIX_INFO_BURST")
                .unwrap_or("30".to_string())
                .parse::<u32>()
                .unwrap_or(30),
        ),
        rate_limit_default: (
            env::var("BAL_SERVER_ACTIX_DEFAULT_PER_SEC")
                .unwrap_or("50".to_string())
                .parse::<u64>()
                .unwrap_or(50),
            env::var("BAL_SERVER_ACTIX_DEFAULT_BURST")
                .unwrap_or("100".to_string())
                .parse::<u32>()
                .unwrap_or(100),
        ),
        workers: env::var("BAL_SERVER_ACTIX_WORKERS")
            .unwrap_or("4".to_string())
            .parse::<usize>()
            .unwrap_or(4),
        max_connections: env::var("BAL_SERVER_ACTIX_MAX_CONNECTIONS")
            .unwrap_or("100".to_string())
            .parse::<usize>()
            .unwrap_or(100),
        trusted_proxy: env::var("BAL_SERVER_TRUSTED_PROXY")
            .unwrap_or_else(|_| "127.0.0.1".to_string())
            .parse::<IpAddr>()
            .unwrap_or(IpAddr::from_str("127.0.0.1").unwrap()),
    }
}

struct AppState {
    db: DatabasePool,
    cfg: MyConfig,
}

async fn echo_home(data: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().body(data.cfg.info.clone())
}

async fn echo_pub_key(data: web::Data<AppState>) -> impl Responder {
    match fs::read_to_string(&data.cfg.pub_key_path) {
        Ok(pub_key) => HttpResponse::Ok().body(pub_key),
        Err(e) => {
            error!(
                "Failed to read public key file {}: {}",
                data.cfg.pub_key_path, e
            );
            HttpResponse::InternalServerError().body("error")
        }
    }
}

async fn echo_version() -> impl Responder {
    HttpResponse::Ok().body(VERSION)
}

fn extract_real_ip(req: &actix_web::HttpRequest, trusted_proxy: IpAddr) -> String {
    let peer_ip = req.peer_addr().map(|socket| socket.ip());
    let connection_info = req.connection_info();

    let ip = match peer_ip {
        Some(peer) if peer == trusted_proxy => {
            connection_info.realip_remote_addr().unwrap_or("unknown")
        }
        _ => connection_info.peer_addr().unwrap_or("unknown"),
    };

    debug!("client IP: {}", ip);
    ip.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RealIpKeyExtractor;

impl KeyExtractor for RealIpKeyExtractor {
    type Key = IpAddr;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        let proxy_ip = req
            .app_data::<web::Data<IpAddr>>()
            .map(|ip| *ip.get_ref())
            .unwrap_or_else(|| IpAddr::from_str("0.0.0.0").unwrap());

        let peer_ip = req.peer_addr().map(|socket| socket.ip());
        let connection_info = req.connection_info();

        match peer_ip {
            Some(peer) if peer == proxy_ip => connection_info
                .realip_remote_addr()
                .ok_or_else(|| {
                    SimpleKeyExtractionError::new("Could not extract real IP address from request")
                })
                .and_then(|str| {
                    str.parse::<IpAddr>().map_err(|_| {
                        SimpleKeyExtractionError::new(
                            "Could not extract real IP address from request",
                        )
                    })
                }),
            _ => connection_info
                .peer_addr()
                .ok_or_else(|| {
                    SimpleKeyExtractionError::new("Could not extract peer IP address from request")
                })
                .and_then(|str| {
                    str.parse::<IpAddr>().map_err(|_| {
                        SimpleKeyExtractionError::new(
                            "Could not extract peer IP address from request",
                        )
                    })
                }),
        }
    }
}

async fn echo_info(
    path: web::Path<String>,
    data: web::Data<AppState>,
    proxy: web::Data<IpAddr>,
    req: actix_web::HttpRequest,
) -> impl Responder {
    let param = path.into_inner();
    if !NETWORKS.contains(&param.as_str()) {
        return HttpResponse::NotFound().body("error");
    }
    info!("echo info!!!{}", param);
    let netconfig = data.cfg.get_net_config(&param);
    if !netconfig.enabled {
        debug!("network disabled {}", param);
        return HttpResponse::BadRequest().body("error");
    }
    let remote_addr = extract_real_ip(&req, **proxy);
    let address = match netconfig.xpub {
        false => {
            let address = netconfig.address.to_string();
            trace!("is address: {}", &address);
            address
        }
        true => {
            if let Some(address) = get_last_used_address_by_ip(
                &data.db,
                &netconfig.name,
                &netconfig.address,
                &remote_addr,
            )
            .await
            {
                return HttpResponse::Ok().json(InfoResponse {
                    address,
                    base_fee: netconfig.fixed_fee,
                    chain: netconfig.network.to_string(),
                    info: data.cfg.info.to_string(),
                    version: VERSION.to_string(),
                });
            }

            let next_idx =
                get_next_address_index(&data.db, &netconfig.name, &netconfig.address).await;

            let derived =
                match new_address_from_xpub(&netconfig.address, next_idx.1, netconfig.network) {
                    Ok(address) => address,
                    Err(e) => {
                        error!("Failed to derive address from xpub: {}", e);
                        return HttpResponse::BadRequest().body("error");
                    }
                };

            save_new_address(&data.db, next_idx.0, &derived.0, &derived.1, &remote_addr).await;
            debug!("save new address {} {}", derived.0, derived.1);
            trace!("next {} {}", next_idx.0, next_idx.1);
            derived.0
        }
    };
    let info = InfoResponse {
        address,
        base_fee: netconfig.fixed_fee,
        chain: netconfig.network.to_string(),
        info: data.cfg.info.to_string(),
        version: VERSION.to_string(),
    };
    trace!("address: {:#?}", info);
    match serde_json::to_string(&info) {
        Ok(json_data) => {
            debug!("echo info reply: {}", json_data);
            HttpResponse::Ok().json(info)
        }
        Err(_err) => HttpResponse::InternalServerError().body("error"),
    }
}

async fn echo_stats(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let param = path.into_inner();
    if !NETWORKS.contains(&param.as_str()) {
        return HttpResponse::NotFound().body("error");
    }
    info!("echo stats!!! {}", data.cfg.expose_stats);
    let netconfig = data.cfg.get_net_config(&param);
    if !netconfig.enabled {
        debug!("network disabled {}", param);
        return HttpResponse::BadRequest().body("error");
    }
    if !data.cfg.expose_stats {
        return HttpResponse::Forbidden().body("error");
    }

    let stats_rows = match get_stats(&data.db, &netconfig.name).await {
        Ok(rows) => rows,
        Err(e) => {
            error!("Failed to query stats: {}", e);
            return HttpResponse::InternalServerError().body("error");
        }
    };

    let stats: Vec<StatsResponse> = stats_rows
        .into_iter()
        .map(|row| StatsResponse {
            report_date: row.report_date,
            chain: row.chain,
            totals: row.totals,
            waiting: row.waiting,
            sent: row.sent,
            failed: row.failed,
            waiting_profit: row.waiting_profit,
            sent_profit: row.sent_profit,
            missed_profit: row.missed_profit,
            unique_inputs: row.unique_inputs,
        })
        .collect();

    debug!("echo stats reply for chain: {}", netconfig.name);
    HttpResponse::Ok().json(stats)
}

async fn echo_search(body: Bytes, data: web::Data<AppState>) -> impl Responder {
    info!("echo search!!!");
    let strbody = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return HttpResponse::BadRequest().body("error");
        }
    };
    info!("{}", strbody);

    if strbody.is_empty() || strbody.len() != 64 || !strbody.chars().all(|c| c.is_ascii_hexdigit())
    {
        return HttpResponse::BadRequest().body("error");
    }

    match search_tx(&data.db, strbody).await {
        Ok(Some(row)) => {
            let mut response_data = HashMap::new();
            response_data.insert("status", row.status);
            response_data.insert("tx", row.tx);
            response_data.insert("our_address", row.our_address);
            response_data.insert("our_fees", row.our_fees);
            response_data.insert("time", row.reqid);

            match serde_json::to_string(&response_data) {
                Ok(json_data) => {
                    debug!("echo search reply: {}", json_data);
                    HttpResponse::Ok().json(&response_data)
                }
                Err(_) => HttpResponse::BadRequest().body("error"),
            }
        }
        Ok(None) => HttpResponse::BadRequest().body("error"),
        Err(e) => {
            error!("Failed to search tx: {}", e);
            HttpResponse::InternalServerError().body("error")
        }
    }
}

#[derive(Clone)]
struct ParsedTx {
    txid: String,
    wtxid: String,
    ntxid: String,
    raw_hex: String,
    locktime: String,
    inputs: Vec<(String, String)>,
    outputs: Vec<(usize, String, u64)>,
}

fn parse_request_transactions(
    strbody: &str,
    _req_time: i64,
    netconfig: &NetConfig,
    known_addresses: &HashSet<String>,
) -> Vec<(ParsedTx, String, u64)> {
    let mut result: Vec<(ParsedTx, String, u64)> = Vec::new();

    for line in strbody.split('\n') {
        if line.is_empty() {
            continue;
        }
        let raw_hex = line.to_string();
        let raw_tx = match Vec::<u8>::from_hex(line) {
            Ok(v) => v,
            Err(e) => {
                error!("rawtx error: {} for line {}", e, line);
                continue;
            }
        };
        if raw_tx.is_empty() {
            continue;
        }
        let tx: Transaction = match consensus::deserialize(&raw_tx) {
            Ok(t) => t,
            Err(e) => {
                error!("Deserialize error: {} for line {}", e, line);
                continue;
            }
        };

        let txid = tx.compute_txid().to_string();
        let ntxid = tx.compute_ntxid();
        let wtxid = tx.compute_wtxid();
        let locktime = tx.lock_time.to_string();

        let mut inputs: Vec<(String, String)> = Vec::with_capacity(tx.input.len());
        for input in tx.input {
            inputs.push((
                input.previous_output.txid.to_string(),
                input.previous_output.vout.to_string(),
            ));
        }

        let mut outputs: Vec<(usize, String, u64)> = Vec::with_capacity(tx.output.len());
        let mut found = false;
        let mut our_address = String::new();
        let mut our_fees = 0u64;

        for (idx, output) in tx.output.into_iter().enumerate() {
            let script = output.script_pubkey.to_string();
            let amount = output.value.to_sat();
            outputs.push((idx, script.clone(), amount));

            let address = match bitcoin::Address::from_script(
                output.script_pubkey.as_script(),
                netconfig.network,
            ) {
                Ok(addr) => addr.to_string(),
                Err(_) => continue,
            };

            let expected_ours = if netconfig.xpub {
                if known_addresses.contains(&address) {
                    trace!(
                        "output {} address {} found in known_addresses (total: {})",
                        idx,
                        &address,
                        known_addresses.len()
                    );
                    address.clone()
                } else {
                    trace!(
                        "output {} address {} NOT in known_addresses (total: {}), skipping",
                        idx,
                        &address,
                        known_addresses.len()
                    );
                    continue;
                }
            } else {
                netconfig.address.clone()
            };

            if address == expected_ours && amount >= netconfig.fixed_fee {
                our_address = expected_ours;
                our_fees = amount;
                found = true;
                trace!("address and fees are correct {}: {}", our_address, our_fees);
            } else if address == expected_ours {
                trace!(
                    "output {} address matches but amount {} < fixed_fee {}, skipping",
                    idx, amount, netconfig.fixed_fee
                );
            }
        }

        if netconfig.fixed_fee == 0 {
            found = true;
        }

        if !found {
            trace!("willexecutor output not found for tx {}, skipping", txid);
            continue;
        }
        result.push((
            ParsedTx {
                txid,
                wtxid: wtxid.to_string(),
                ntxid: ntxid.to_string(),
                raw_hex,
                locktime,
                inputs,
                outputs,
            },
            our_address,
            our_fees,
        ));
    }

    result
}

async fn echo_push(
    body: Bytes,
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> HttpResponse {
    trace!("echo_push");
    let strbody = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return HttpResponse::BadRequest().body("error");
        }
    };

    let param = path.into_inner();
    if !NETWORKS.contains(&param.as_str()) {
        return HttpResponse::NotFound().body("error");
    }
    let netconfig = data.cfg.get_net_config(&param);
    if !netconfig.enabled {
        trace!("network not enabled {}", &netconfig.name);
        return HttpResponse::BadRequest().body("error");
    }
    let req_time = match Utc::now().timestamp_nanos_opt() {
        Some(t) => t,
        None => {
            error!("Invalid timestamp");
            return HttpResponse::BadRequest().body("error");
        }
    };

    // ===== PHASE 1: parse all transactions WITHOUT the DB lock =====
    let known_addresses: HashSet<String> = if netconfig.xpub {
        match get_all_addresses_by_xpub(&data.db, &netconfig.address).await {
            Ok(addrs) => addrs,
            Err(e) => {
                error!("Failed to load addresses from xpub: {}", e);
                return HttpResponse::InternalServerError().body("error");
            }
        }
    } else {
        HashSet::new()
    };

    // Parse all transactions (CPU-bound, no DB needed)
    let parsed = parse_request_transactions(strbody, req_time, netconfig, &known_addresses);
    if parsed.is_empty() {
        return HttpResponse::BadRequest().body("error");
    }

    let all_txids: Vec<String> = parsed.iter().map(|(p, _, _)| p.txid.clone()).collect();

    // ===== PHASE 2: check duplicates in a single batch query =====
    let duplicates = match check_duplicate_txids(&data.db, &all_txids).await {
        Ok(dups) => dups,
        Err(e) => {
            error!("Duplicate check failed: {}", e);
            return HttpResponse::InternalServerError().body("error");
        }
    };

    let all_present = all_txids.iter().all(|t| duplicates.contains(t));
    if all_present {
        return HttpResponse::Ok().body("already present");
    }

    // ===== PHASE 3: build insert data and execute (single transaction, minimal time) =====
    let mut tx_data = Vec::new();
    let mut inp_data = Vec::new();
    let mut out_data = Vec::new();

    for (parsed, our_address, our_fees) in &parsed {
        if duplicates.contains(&parsed.txid) {
            continue;
        }

        tx_data.push(InsertTxData {
            txid: parsed.txid.clone(),
            wtxid: parsed.wtxid.clone(),
            ntxid: parsed.ntxid.clone(),
            raw_hex: parsed.raw_hex.clone(),
            locktime: parsed.locktime.clone(),
            reqid: req_time.to_string(),
            network: netconfig.name.clone(),
            our_address: our_address.clone(),
            our_fees: our_fees.to_string(),
        });

        for (in_txid, in_vout) in &parsed.inputs {
            inp_data.push(InsertInpData {
                txid: parsed.txid.clone(),
                in_txid: in_txid.clone(),
                in_vout: in_vout.clone(),
            });
        }

        for (idx, script, amount) in &parsed.outputs {
            out_data.push(InsertOutData {
                txid: parsed.txid.clone(),
                vout: i64::try_from(*idx).unwrap_or(-1),
                script_pubkey: script.clone(),
                amount: i64::try_from(*amount).unwrap_or(0),
            });
        }
    }

    if tx_data.is_empty() {
        return HttpResponse::Ok().body("already present");
    }

    if let Err(err) = bal_server::db::execute_insert(&data.db, &tx_data, &inp_data, &out_data).await
    {
        error!("execute_insert failed: {}", err);
        return HttpResponse::BadRequest().body("error");
    }

    HttpResponse::Ok().body("thx")
}

fn parse_env(data: &MyConfig) -> MyConfig {
    let mut cfg = data.clone();
    if let Ok(value) = env::var("BAL_SERVER_DB_BACKEND") {
        debug!("BAL_SERVER_DB_BACKEND: {}", value);
        cfg.db_backend = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_DB_FILE") {
        debug!("BAL_SERVER_DB_FILE: {}", value);
        cfg.db_file = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_PG_DSN") {
        debug!("BAL_SERVER_PG_DSN: {}", value);
        cfg.pg_dsn = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_BIND_ADDRESS") {
        debug!("BAL_SERVER_BIND_ADDRESS: {}", value);
        cfg.bind_address = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_BIND_PORT") {
        debug!("BAL_SERVER_BIND_PORT: {}", value);
        if let Ok(v) = value.parse::<u16>() {
            cfg.bind_port = v;
        }
    }
    if let Ok(value) = env::var("BAL_SERVER_PUB_KEY_PATH") {
        debug!("BAL_SERVER_PUB_KEY_PATH: {}", value);
        cfg.pub_key_path = value;
    }
    if let Ok(value) = env::var("BAL_SERVER_INFO") {
        debug!("BAL_SERVER_INFO: {}", value);
        cfg.info = value;
    }
    parse_env_netconfig(&mut cfg, "regtest");
    parse_env_netconfig(&mut cfg, "signet");
    parse_env_netconfig(&mut cfg, "testnet");
    parse_env_netconfig(&mut cfg, "testnet4");
    parse_env_netconfig(&mut cfg, "bitcoin");

    cfg
}

fn parse_env_netconfig(cfg: &mut MyConfig, chain: &str) {
    let c = match chain {
        "regtest" => &mut cfg.regtest,
        "signet" => &mut cfg.signet,
        "testnet" => &mut cfg.testnet,
        "testnet4" => &mut cfg.testnet4,
        _ => &mut cfg.mainnet,
    };
    if let Ok(value) = env::var(format!("BAL_SERVER_{}_ADDRESS", chain.to_uppercase())) {
        debug!("BAL_SERVER_{}_ADDRESS: {}", chain.to_uppercase(), value);
        c.address = value;
        if c.address.len() > 5 && &c.address[1..4] == "pub" {
            c.xpub = true;
            trace!("is_xpub");
        }
        c.enabled = true;
    }
    if let Ok(value) = env::var(format!("BAL_SERVER_{}_FIXED_FEE", chain.to_uppercase())) {
        debug!("BAL_SERVER_{}_FIXED_FEE: {}", chain.to_uppercase(), value);
        if let Ok(v) = value.parse::<u64>() {
            c.fixed_fee = v;
        }
    }
}

async fn init_network(pool: &DatabasePool, cfg: &MyConfig) {
    for network in NETWORKS {
        let netconfig = cfg.get_net_config(network);
        insert_xpub(pool, &netconfig.name.to_string(), &netconfig.address).await;
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let cfg = MyConfig::default();
    let actix_cfg = parse_actix_config();

    let cfg = parse_env(&cfg);

    let connection_string = match cfg.db_backend.as_str() {
        "sqlite" => cfg.db_file.clone(),
        "postgresql" => cfg.pg_dsn.clone(),
        other => {
            return Err(std::io::Error::other(format!(
                "Unknown DB backend: {}",
                other
            )));
        }
    };

    let db = match open_database(&cfg.db_backend, &connection_string).await {
        Ok(pool) => pool,
        Err(e) => {
            return Err(std::io::Error::other(e));
        }
    };

    // Create database tables
    create_database(&db)
        .await
        .map_err(|e| std::io::Error::other(format!("Failed to create database: {}", e)))?;

    // Initialize networks
    init_network(&db, &cfg).await;

    let data = web::Data::new(AppState {
        db,
        cfg: cfg.clone(),
    });

    let bind_address = data.cfg.bind_address.clone();
    let bind_port = data.cfg.bind_port;

    let governor_conf = GovernorConfigBuilder::default()
        .seconds_per_request(actix_cfg.rate_limit_pushtxs.0)
        .burst_size(actix_cfg.rate_limit_pushtxs.1)
        .key_extractor(RealIpKeyExtractor)
        .finish()
        .unwrap();

    println!("Starting server on http://{}:{}", bind_address, bind_port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::PayloadConfig::default().limit(actix_cfg.max_body_size))
            .app_data(data.clone())
            .app_data(web::Data::new(actix_cfg.trusted_proxy))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .wrap(Governor::new(&governor_conf))
            .service(web::resource("/").route(web::get().to(echo_home)))
            .service(web::resource("/.pub_key.pem").route(web::get().to(echo_pub_key)))
            .service(web::resource("/version").route(web::get().to(echo_version)))
            .service(web::resource("/{network}/info").route(web::get().to(echo_info)))
            .service(web::resource("/{network}/stats").route(web::get().to(echo_stats)))
            .service(web::resource("/{network}/pushtxs").route(web::post().to(echo_push)))
            .service(web::resource("/searchtx").route(web::post().to(echo_search)))
    })
    .workers(actix_cfg.workers)
    .max_connections(actix_cfg.max_connections)
    .bind((bind_address, bind_port))?
    .run()
    .await
}
