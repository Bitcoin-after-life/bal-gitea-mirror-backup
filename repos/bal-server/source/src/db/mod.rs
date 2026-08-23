pub mod schema;

use log::{error, info, trace};
use sqlx::Row;
use std::collections::HashSet;
use std::path::Path;

#[derive(Clone)]
pub enum DatabasePool {
    SQLite(sqlx::SqlitePool),
    PostgreSQL(sqlx::PgPool),
}

fn validate_sqlite_path(dsn: &str) -> Result<(), String> {
    // Extract file path from DSN formats like "sqlite:path" or "file:path?mode=rwc"
    let path_str = if let Some(rest) = dsn.strip_prefix("sqlite:") {
        rest.split('?').next().unwrap_or(rest)
    } else if let Some(rest) = dsn.strip_prefix("file:") {
        rest.split('?').next().unwrap_or(rest)
    } else {
        dsn
    };

    // Skip validation for in-memory databases
    if path_str == ":memory:" || path_str.is_empty() {
        return Ok(());
    }

    let p = Path::new(path_str);

    // Prevent directory traversal
    for component in p.components() {
        if component == std::path::Component::ParentDir {
            return Err("Database path may not contain '..'".to_string());
        }
    }

    // If absolute, block known sensitive system directories
    if p.is_absolute() {
        let forbidden = [
            "/etc", "/proc", "/sys", "/dev", "/usr", "/bin", "/sbin", "/lib", "/opt",
        ];
        for prefix in &forbidden {
            if path_str.starts_with(prefix) {
                return Err(format!(
                    "Absolute database path under {} is forbidden",
                    prefix
                ));
            }
        }
    }

    // If file exists, must be a regular file (not a symlink, device, etc.)
    if p.exists() {
        if p.is_symlink() {
            return Err("Database path must not be a symlink".to_string());
        }
        let metadata = std::fs::metadata(p)
            .map_err(|e| format!("Cannot access database file metadata: {}", e))?;
        if !metadata.is_file() {
            return Err(
                "Database path must point to a regular file, not a directory or device".to_string(),
            );
        }
    }

    Ok(())
}

pub async fn open_database(backend: &str, connection_string: &str) -> Result<DatabasePool, String> {
    match backend {
        "sqlite" => {
            validate_sqlite_path(connection_string)?;
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect(connection_string)
                .await
                .map_err(|e| format!("Failed to open SQLite database: {}", e))?;
            sqlx::query("PRAGMA journal_mode=WAL")
                .execute(&pool)
                .await
                .map_err(|e| format!("Failed to set WAL mode: {}", e))?;
            sqlx::query("PRAGMA busy_timeout=5000")
                .execute(&pool)
                .await
                .map_err(|e| format!("Failed to set busy_timeout: {}", e))?;
            Ok(DatabasePool::SQLite(pool))
        }
        "postgresql" => {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(10)
                .connect(connection_string)
                .await
                .map_err(|e| format!("Failed to open PostgreSQL database: {}", e))?;
            Ok(DatabasePool::PostgreSQL(pool))
        }
        other => Err(format!("Unknown database backend: {}", other)),
    }
}

pub async fn create_database(pool: &DatabasePool) -> Result<(), sqlx::Error> {
    info!("database sanity check");
    match pool {
        DatabasePool::SQLite(p) => schema::create_sqlite_schema(p).await,
        DatabasePool::PostgreSQL(p) => schema::create_pg_schema(p).await,
    }
}

pub async fn check_duplicate_txids(
    pool: &DatabasePool,
    txids: &[String],
) -> Result<HashSet<String>, sqlx::Error> {
    if txids.is_empty() {
        return Ok(HashSet::new());
    }

    let mut duplicates = HashSet::new();
    let chunk_size = 500;

    for chunk in txids.chunks(chunk_size) {
        match pool {
            DatabasePool::SQLite(p) => {
                let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
                let sql = format!(
                    "SELECT txid FROM tbl_tx WHERE txid IN ({})",
                    placeholders.join(",")
                );
                let mut query = sqlx::query(&sql);
                for txid in chunk {
                    query = query.bind(txid);
                }
                let rows = query.fetch_all(p).await?;
                for row in rows {
                    if let Ok(txid) = row.try_get::<String, _>("txid") {
                        duplicates.insert(txid);
                    }
                }
            }
            DatabasePool::PostgreSQL(p) => {
                let placeholders: Vec<String> = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("${}", i + 1))
                    .collect();
                let sql = format!(
                    "SELECT txid FROM tbl_tx WHERE txid IN ({})",
                    placeholders.join(",")
                );
                let mut query = sqlx::query(&sql);
                for txid in chunk {
                    query = query.bind(txid);
                }
                let rows = query.fetch_all(p).await?;
                for row in rows {
                    if let Ok(txid) = row.try_get::<String, _>("txid") {
                        duplicates.insert(txid);
                    }
                }
            }
        }
    }

    Ok(duplicates)
}

pub async fn get_all_addresses_by_xpub(
    pool: &DatabasePool,
    xpub: &str,
) -> Result<HashSet<String>, sqlx::Error> {
    let mut addresses = HashSet::new();

    trace!("get_all_addresses_by_xpub: querying for xpub={}", xpub);

    match pool {
        DatabasePool::SQLite(p) => {
            let rows = sqlx::query(
                "SELECT a.address FROM tbl_address a JOIN tbl_xpub x ON a.xpub = x.id WHERE x.xpub = ?",
            )
            .bind(xpub)
            .fetch_all(p)
            .await?;
            trace!(
                "get_all_addresses_by_xpub: SQLite returned {} rows",
                rows.len()
            );
            for row in rows {
                if let Ok(addr) = row.try_get::<String, _>("address") {
                    trace!("get_all_addresses_by_xpub:   address={}", &addr);
                    addresses.insert(addr);
                }
            }
        }
        DatabasePool::PostgreSQL(p) => {
            let rows = sqlx::query(
                "SELECT a.address FROM tbl_address a JOIN tbl_xpub x ON a.xpub = x.id WHERE x.xpub = $1",
            )
            .bind(xpub)
            .fetch_all(p)
            .await?;
            trace!(
                "get_all_addresses_by_xpub: PostgreSQL returned {} rows",
                rows.len()
            );
            for row in rows {
                if let Ok(addr) = row.try_get::<String, _>("address") {
                    trace!("get_all_addresses_by_xpub:   address={}", &addr);
                    addresses.insert(addr);
                }
            }
        }
    }

    trace!(
        "get_all_addresses_by_xpub: returning {} addresses",
        addresses.len()
    );
    Ok(addresses)
}

pub async fn insert_xpub(pool: &DatabasePool, network: &str, xpub: &str) {
    if xpub.is_empty() {
        return;
    }
    trace!("going to insert: {} xpub:{}", network, xpub);

    match pool {
        DatabasePool::SQLite(p) => {
            if let Err(e) =
                sqlx::query("INSERT OR IGNORE INTO tbl_xpub(network, xpub) VALUES(?, ?)")
                    .bind(network)
                    .bind(xpub)
                    .execute(p)
                    .await
            {
                error!("Failed to insert xpub: {}", e);
            }
        }
        DatabasePool::PostgreSQL(p) => {
            if let Err(e) = sqlx::query(
                "INSERT INTO tbl_xpub(network, xpub) VALUES($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(network)
            .bind(xpub)
            .execute(p)
            .await
            {
                error!("Failed to insert xpub: {}", e);
            }
        }
    }
}

pub async fn get_last_used_address_by_ip(
    pool: &DatabasePool,
    network: &str,
    xpub: &str,
    address: &str,
) -> Option<String> {
    match pool {
        DatabasePool::SQLite(p) => {
            let result = sqlx::query(
                "SELECT tbl_address.address FROM tbl_xpub JOIN tbl_address ON(tbl_xpub.id = tbl_address.xpub) WHERE tbl_xpub.network = ? AND tbl_address.remote_address = ? AND tbl_xpub.xpub = ? ORDER BY tbl_address.date_create DESC LIMIT 1",
            )
            .bind(network)
            .bind(address)
            .bind(xpub)
            .fetch_optional(p)
            .await;

            match result {
                Ok(Some(row)) => row.try_get::<String, _>("address").ok(),
                Ok(None) => None,
                Err(e) => {
                    error!("Failed to query last used address: {}", e);
                    None
                }
            }
        }
        DatabasePool::PostgreSQL(p) => {
            let result = sqlx::query(
                "SELECT tbl_address.address FROM tbl_xpub JOIN tbl_address ON(tbl_xpub.id = tbl_address.xpub) WHERE tbl_xpub.network = $1 AND tbl_address.remote_address = $2 AND tbl_xpub.xpub = $3 ORDER BY tbl_address.date_create DESC LIMIT 1",
            )
            .bind(network)
            .bind(address)
            .bind(xpub)
            .fetch_optional(p)
            .await;

            match result {
                Ok(Some(row)) => row.try_get::<String, _>("address").ok(),
                Ok(None) => None,
                Err(e) => {
                    error!("Failed to query last used address: {}", e);
                    None
                }
            }
        }
    }
}

pub async fn get_next_address_index(pool: &DatabasePool, network: &str, xpub: &str) -> (i64, i64) {
    match pool {
        DatabasePool::SQLite(p) => {
            let result = sqlx::query(
                "UPDATE tbl_xpub SET path_idx = path_idx + 1 WHERE network = ? AND xpub = ? RETURNING path_idx, id",
            )
            .bind(network)
            .bind(xpub)
            .fetch_optional(p)
            .await;

            match result {
                Ok(Some(row)) => {
                    let idx = row.try_get::<i64, _>("path_idx").unwrap_or(0);
                    let id = row.try_get::<i64, _>("id").unwrap_or(0);
                    (id, idx)
                }
                Ok(None) => (0, 0),
                Err(e) => {
                    error!("Failed to get next address index: {}", e);
                    (0, 0)
                }
            }
        }
        DatabasePool::PostgreSQL(p) => {
            let result = sqlx::query(
                "UPDATE tbl_xpub SET path_idx = path_idx + 1 WHERE network = $1 AND xpub = $2 RETURNING path_idx, id",
            )
            .bind(network)
            .bind(xpub)
            .fetch_optional(p)
            .await;

            match result {
                Ok(Some(row)) => {
                    let idx = row.try_get::<i32, _>("path_idx").unwrap_or(0) as i64;
                    let id = row.try_get::<i32, _>("id").unwrap_or(0) as i64;
                    (id, idx)
                }
                Ok(None) => (0, 0),
                Err(e) => {
                    error!("Failed to get next address index: {}", e);
                    (0, 0)
                }
            }
        }
    }
}

pub async fn save_new_address(
    pool: &DatabasePool,
    xpub: i64,
    address: &str,
    path: &str,
    remote_addr: &str,
) {
    match pool {
        DatabasePool::SQLite(p) => {
            if let Err(e) = sqlx::query(
                "INSERT INTO tbl_address(address, path, xpub, remote_address) VALUES(?, ?, ?, ?)",
            )
            .bind(address)
            .bind(path)
            .bind(xpub)
            .bind(remote_addr)
            .execute(p)
            .await
            {
                error!("Failed to save address: {}", e);
            }
        }
        DatabasePool::PostgreSQL(p) => {
            if let Err(e) = sqlx::query(
                "INSERT INTO tbl_address(address, path, xpub, remote_address) VALUES($1, $2, $3, $4)",
            )
            .bind(address)
            .bind(path)
            .bind(xpub as i32)
            .bind(remote_addr)
            .execute(p)
            .await
            {
                error!("Failed to save address: {}", e);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InsertTxData {
    pub txid: String,
    pub wtxid: String,
    pub ntxid: String,
    pub raw_hex: String,
    pub locktime: String,
    pub reqid: String,
    pub network: String,
    pub our_address: String,
    pub our_fees: String,
}

#[derive(Debug, Clone)]
pub struct InsertInpData {
    pub txid: String,
    pub in_txid: String,
    pub in_vout: String,
}

#[derive(Debug, Clone)]
pub struct InsertOutData {
    pub txid: String,
    pub vout: i64,
    pub script_pubkey: String,
    pub amount: i64,
}

pub async fn execute_insert(
    pool: &DatabasePool,
    txs: &[InsertTxData],
    inps: &[InsertInpData],
    outs: &[InsertOutData],
) -> Result<(), sqlx::Error> {
    match pool {
        DatabasePool::SQLite(p) => {
            let mut tx = p.begin().await?;

            for item in txs {
                sqlx::query(
                    "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, reqid, network, our_address, our_fees) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(&item.txid)
                .bind(&item.wtxid)
                .bind(&item.ntxid)
                .bind(&item.raw_hex)
                .bind(&item.locktime)
                .bind(&item.reqid)
                .bind(&item.network)
                .bind(&item.our_address)
                .bind(&item.our_fees)
                .execute(&mut *tx)
                .await?;
            }

            for item in inps {
                sqlx::query("INSERT INTO tbl_inp (txid, in_txid, in_vout) VALUES (?, ?, ?)")
                    .bind(&item.txid)
                    .bind(&item.in_txid)
                    .bind(&item.in_vout)
                    .execute(&mut *tx)
                    .await?;
            }

            for item in outs {
                sqlx::query(
                    "INSERT INTO tbl_out (txid, vout, script_pubkey, amount) VALUES (?, ?, ?, ?)",
                )
                .bind(&item.txid)
                .bind(item.vout)
                .bind(&item.script_pubkey)
                .bind(item.amount)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
        }
        DatabasePool::PostgreSQL(p) => {
            let mut tx = p.begin().await?;

            for item in txs {
                let locktime_i64: i64 = item.locktime.parse().unwrap_or(0);
                sqlx::query(
                    "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, reqid, network, our_address, our_fees) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
                )
                .bind(&item.txid)
                .bind(&item.wtxid)
                .bind(&item.ntxid)
                .bind(&item.raw_hex)
                .bind(locktime_i64)
                .bind(&item.reqid)
                .bind(&item.network)
                .bind(&item.our_address)
                .bind(&item.our_fees)
                .execute(&mut *tx)
                .await?;
            }

            for item in inps {
                let in_vout_i32: i32 = item.in_vout.parse().unwrap_or(0);
                sqlx::query("INSERT INTO tbl_inp (txid, in_txid, in_vout) VALUES ($1, $2, $3)")
                    .bind(&item.txid)
                    .bind(&item.in_txid)
                    .bind(in_vout_i32)
                    .execute(&mut *tx)
                    .await?;
            }

            for item in outs {
                sqlx::query(
                    "INSERT INTO tbl_out (txid, vout, script_pubkey, amount) VALUES ($1, $2, $3, $4)",
                )
                .bind(&item.txid)
                .bind(item.vout as i32)
                .bind(&item.script_pubkey)
                .bind(item.amount.to_string())
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
        }
    }

    Ok(())
}

pub async fn get_total_transaction_number(
    pool: &DatabasePool,
    network: &str,
) -> Result<i64, sqlx::Error> {
    match pool {
        DatabasePool::SQLite(p) => {
            let row = sqlx::query("SELECT COUNT(*) as total_number FROM tbl_tx WHERE network = ?")
                .bind(network)
                .fetch_one(p)
                .await?;
            Ok(row.try_get::<i64, _>("total_number").unwrap_or(0))
        }
        DatabasePool::PostgreSQL(p) => {
            let row = sqlx::query("SELECT COUNT(*) as total_number FROM tbl_tx WHERE network = $1")
                .bind(network)
                .fetch_one(p)
                .await?;
            Ok(row.try_get::<i64, _>("total_number").unwrap_or(0))
        }
    }
}

pub async fn update_tx_status(
    pool: &DatabasePool,
    txid: &str,
    status: i32,
    push_err: Option<&str>,
) -> Result<(), sqlx::Error> {
    match pool {
        DatabasePool::SQLite(p) => {
            if let Some(err) = push_err {
                sqlx::query("UPDATE tbl_tx SET status = ?, push_err = ? WHERE txid = ?")
                    .bind(status)
                    .bind(err)
                    .bind(txid)
                    .execute(p)
                    .await?;
            } else {
                sqlx::query("UPDATE tbl_tx SET status = ? WHERE txid = ?")
                    .bind(status)
                    .bind(txid)
                    .execute(p)
                    .await?;
            }
        }
        DatabasePool::PostgreSQL(p) => {
            if let Some(err) = push_err {
                sqlx::query("UPDATE tbl_tx SET status = $1, push_err = $2 WHERE txid = $3")
                    .bind(status)
                    .bind(err)
                    .bind(txid)
                    .execute(p)
                    .await?;
            } else {
                sqlx::query("UPDATE tbl_tx SET status = $1 WHERE txid = $2")
                    .bind(status)
                    .bind(txid)
                    .execute(p)
                    .await?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct TxRow {
    pub txid: String,
    pub tx: String,
    pub locktime: i64,
    pub network: String,
    pub status: i64,
}

pub async fn get_pending_txs(
    pool: &DatabasePool,
    network: &str,
    locktime_threshold: i64,
    bestblock_height: i64,
    bestblock_time: i64,
) -> Result<Vec<TxRow>, sqlx::Error> {
    let mut results = Vec::new();

    match pool {
        DatabasePool::SQLite(p) => {
            let rows = sqlx::query(
                "SELECT txid, tx, locktime, network, status FROM tbl_tx WHERE network = ? AND status = 0 AND (locktime < ? OR (locktime > ? AND locktime < ?))",
            )
            .bind(network)
            .bind(bestblock_height)
            .bind(locktime_threshold)
            .bind(bestblock_time)
            .fetch_all(p)
            .await?;

            for row in rows {
                results.push(TxRow {
                    txid: row.try_get("txid")?,
                    tx: row.try_get("tx")?,
                    locktime: row.try_get("locktime")?,
                    network: row.try_get("network")?,
                    status: row.try_get("status")?,
                });
            }
        }
        DatabasePool::PostgreSQL(p) => {
            let rows = sqlx::query(
                "SELECT txid, tx, locktime, network, status FROM tbl_tx WHERE network = $1 AND status = 0 AND (locktime < $2 OR (locktime > $3 AND locktime < $4))",
            )
            .bind(network)
            .bind(bestblock_height)
            .bind(locktime_threshold)
            .bind(bestblock_time)
            .fetch_all(p)
            .await?;

            for row in rows {
                results.push(TxRow {
                    txid: row.try_get("txid")?,
                    tx: row.try_get("tx")?,
                    locktime: row.try_get::<i64, _>("locktime").unwrap_or(0),
                    network: row.try_get("network")?,
                    status: row.try_get::<i32, _>("status").unwrap_or(0) as i64,
                });
            }
        }
    }

    Ok(results)
}

#[derive(Debug, Clone)]
pub struct StatsRow {
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

pub async fn get_stats(pool: &DatabasePool, chain: &str) -> Result<Vec<StatsRow>, sqlx::Error> {
    let mut results = Vec::new();

    match pool {
        DatabasePool::SQLite(p) => {
            let rows = sqlx::query(
                "SELECT report_date, chain, totals, waiting, sent, failed, waiting_profit, sent_profit, missed_profit, unique_inputs FROM tbl_stats WHERE chain = ?",
            )
            .bind(chain)
            .fetch_all(p)
            .await?;

            for row in rows {
                results.push(StatsRow {
                    report_date: row.try_get("report_date").unwrap_or_default(),
                    chain: row.try_get("chain").unwrap_or_default(),
                    totals: row.try_get("totals").unwrap_or(0),
                    waiting: row.try_get("waiting").unwrap_or(0),
                    sent: row.try_get("sent").unwrap_or(0),
                    failed: row.try_get("failed").unwrap_or(0),
                    waiting_profit: row.try_get("waiting_profit").unwrap_or(0),
                    sent_profit: row.try_get("sent_profit").unwrap_or(0),
                    missed_profit: row.try_get("missed_profit").unwrap_or(0),
                    unique_inputs: row.try_get("unique_inputs").unwrap_or(0),
                });
            }
        }
        DatabasePool::PostgreSQL(p) => {
            let rows = sqlx::query(
                "SELECT report_date, chain, totals, waiting, sent, failed, waiting_profit, sent_profit, missed_profit, unique_inputs FROM tbl_stats WHERE chain = $1",
            )
            .bind(chain)
            .fetch_all(p)
            .await?;

            for row in rows {
                results.push(StatsRow {
                    report_date: row.try_get("report_date").unwrap_or_default(),
                    chain: row.try_get("chain").unwrap_or_default(),
                    totals: row.try_get::<i32, _>("totals").unwrap_or(0) as i64,
                    waiting: row.try_get::<i32, _>("waiting").unwrap_or(0) as i64,
                    sent: row.try_get::<i32, _>("sent").unwrap_or(0) as i64,
                    failed: row.try_get::<i32, _>("failed").unwrap_or(0) as i64,
                    waiting_profit: row.try_get::<i32, _>("waiting_profit").unwrap_or(0) as i64,
                    sent_profit: row.try_get::<i32, _>("sent_profit").unwrap_or(0) as i64,
                    missed_profit: row.try_get::<i32, _>("missed_profit").unwrap_or(0) as i64,
                    unique_inputs: row.try_get::<i32, _>("unique_inputs").unwrap_or(0) as i64,
                });
            }
        }
    }

    Ok(results)
}

#[derive(Debug, Clone)]
pub struct SearchTxRow {
    pub status: String,
    pub tx: String,
    pub our_address: String,
    pub our_fees: String,
    pub reqid: String,
}

pub async fn search_tx(
    pool: &DatabasePool,
    txid: &str,
) -> Result<Option<SearchTxRow>, sqlx::Error> {
    match pool {
        DatabasePool::SQLite(p) => {
            let result = sqlx::query("SELECT * FROM tbl_tx WHERE txid = ? LIMIT 1")
                .bind(txid)
                .fetch_optional(p)
                .await?;

            Ok(result.map(|row| SearchTxRow {
                status: row.try_get::<i64, _>("status").unwrap_or(0).to_string(),
                tx: row.try_get::<String, _>("tx").unwrap_or_default(),
                our_address: row.try_get::<String, _>("our_address").unwrap_or_default(),
                our_fees: row.try_get::<String, _>("our_fees").unwrap_or_default(),
                reqid: row.try_get::<String, _>("reqid").unwrap_or_default(),
            }))
        }
        DatabasePool::PostgreSQL(p) => {
            let result = sqlx::query("SELECT * FROM tbl_tx WHERE txid = $1 LIMIT 1")
                .bind(txid)
                .fetch_optional(p)
                .await?;

            Ok(result.map(|row| SearchTxRow {
                status: row.try_get::<i32, _>("status").unwrap_or(0).to_string(),
                tx: row.try_get::<String, _>("tx").unwrap_or_default(),
                our_address: row.try_get::<String, _>("our_address").unwrap_or_default(),
                our_fees: row.try_get::<String, _>("our_fees").unwrap_or_default(),
                reqid: row.try_get::<String, _>("reqid").unwrap_or_default(),
            }))
        }
    }
}

pub async fn calculate_and_upsert_stats(
    pool: &DatabasePool,
    chain: &str,
) -> Result<(), sqlx::Error> {
    if !chain
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        || chain.is_empty()
    {
        error!("Invalid chain name: {chain}");
        return Ok(());
    }

    match pool {
        DatabasePool::SQLite(p) => {
            sqlx::query("DELETE FROM tbl_stats WHERE chain = ?")
                .bind(chain)
                .execute(p)
                .await?;

            sqlx::query(
                "INSERT INTO tbl_stats (
                    report_date, chain, totals, waiting, sent, failed,
                    waiting_profit, sent_profit, missed_profit, unique_inputs
                )
                SELECT
                    CURRENT_TIMESTAMP,
                    ?,
                    (SELECT COUNT(*) FROM tbl_tx WHERE network = ?),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 0 AND network = ?),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 1 AND network = ?),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 2 AND network = ?),
                    (SELECT IFNULL(SUM(our_fees),0) FROM tbl_tx WHERE status = 0 AND network = ?),
                    (SELECT IFNULL(SUM(our_fees),0) FROM tbl_tx WHERE status = 1 AND network = ?),
                    (SELECT IFNULL(SUM(our_fees),0) FROM tbl_tx WHERE status = 2 AND network = ?),
                    (SELECT COUNT(DISTINCT tbl_inp.in_txid)
                        FROM tbl_inp
                        JOIN tbl_tx ON tbl_inp.txid = tbl_tx.txid
                        WHERE tbl_tx.status = 0 AND tbl_tx.network = ?)
                ON CONFLICT(chain) DO UPDATE SET
                    report_date = excluded.report_date,
                    totals = excluded.totals,
                    waiting = excluded.waiting,
                    sent = excluded.sent,
                    failed = excluded.failed,
                    waiting_profit = excluded.waiting_profit,
                    sent_profit = excluded.sent_profit,
                    missed_profit = excluded.missed_profit,
                    unique_inputs = excluded.unique_inputs",
            )
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .bind(chain)
            .execute(p)
            .await?;
        }
        DatabasePool::PostgreSQL(p) => {
            sqlx::query("DELETE FROM tbl_stats WHERE chain = $1")
                .bind(chain)
                .execute(p)
                .await?;

            sqlx::query(
                "INSERT INTO tbl_stats (
                    report_date, chain, totals, waiting, sent, failed,
                    waiting_profit, sent_profit, missed_profit, unique_inputs
                )
                SELECT
                    CURRENT_TIMESTAMP,
                    $1,
                    (SELECT COUNT(*) FROM tbl_tx WHERE network = $1),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 0 AND network = $1),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 1 AND network = $1),
                    (SELECT COUNT(*) FROM tbl_tx WHERE status = 2 AND network = $1),
                    (SELECT COALESCE(SUM(CAST(our_fees AS BIGINT)),0) FROM tbl_tx WHERE status = 0 AND network = $1),
                    (SELECT COALESCE(SUM(CAST(our_fees AS BIGINT)),0) FROM tbl_tx WHERE status = 1 AND network = $1),
                    (SELECT COALESCE(SUM(CAST(our_fees AS BIGINT)),0) FROM tbl_tx WHERE status = 2 AND network = $1),
                    (SELECT COUNT(DISTINCT tbl_inp.in_txid)
                        FROM tbl_inp
                        JOIN tbl_tx ON tbl_inp.txid = tbl_tx.txid
                        WHERE tbl_tx.status = 0 AND tbl_tx.network = $1)
                ON CONFLICT(chain) DO UPDATE SET
                    report_date = excluded.report_date,
                    totals = excluded.totals,
                    waiting = excluded.waiting,
                    sent = excluded.sent,
                    failed = excluded.failed,
                    waiting_profit = excluded.waiting_profit,
                    sent_profit = excluded.sent_profit,
                    missed_profit = excluded.missed_profit,
                    unique_inputs = excluded.unique_inputs",
            )
            .bind(chain)
            .execute(p)
            .await?;
        }
    }

    info!("tbl_stats creation success");
    Ok(())
}
