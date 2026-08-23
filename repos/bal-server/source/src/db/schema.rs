use sqlx::{PgPool, SqlitePool};

pub async fn create_sqlite_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_tx (
            txid TEXT PRIMARY KEY,
            date_creation TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            date_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            wtxid TEXT,
            ntxid TEXT,
            tx TEXT,
            locktime INTEGER,
            network TEXT,
            network_fees TEXT,
            reqid TEXT,
            our_fees TEXT,
            our_address TEXT,
            status INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    let _ = sqlx::query("ALTER TABLE tbl_tx ADD COLUMN push_err TEXT")
        .execute(pool)
        .await;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_inp (
            id INTEGER,
            txid TEXT,
            in_txid TEXT,
            in_vout INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_inp_unique ON tbl_inp(txid, in_txid, in_vout)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_out (
            id INTEGER,
            txid TEXT,
            script_pubkey TEXT,
            amount TEXT,
            vout INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_out_unique ON tbl_out(txid, script_pubkey, amount, vout)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_xpub (
            id INTEGER PRIMARY KEY,
            network TEXT,
            xpub TEXT,
            date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            path_idx INTEGER DEFAULT -1
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_xpub ON tbl_xpub(network, xpub)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_address (
            address TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            xpub INTEGER,
            remote_address TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_stats (
            report_date TEXT,
            chain TEXT,
            totals INTEGER,
            waiting INTEGER,
            sent INTEGER,
            failed INTEGER,
            waiting_profit INTEGER,
            sent_profit INTEGER,
            missed_profit INTEGER,
            unique_inputs INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("DROP INDEX IF EXISTS idx_stats_chain")
        .execute(pool)
        .await?;
    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_stats_chain ON tbl_stats(chain)")
        .execute(pool)
        .await?;

    sqlx::query("UPDATE tbl_tx SET network='bitcoin' WHERE network='mainnet'")
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn create_pg_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_tx (
            txid TEXT PRIMARY KEY,
            date_creation TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            date_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            wtxid TEXT,
            ntxid TEXT,
            tx TEXT,
            locktime BIGINT,
            network TEXT,
            network_fees TEXT,
            reqid TEXT,
            our_fees TEXT,
            our_address TEXT,
            status INTEGER DEFAULT 0,
            push_err TEXT
        )",
    )
    .execute(pool)
    .await?;

    // Migrate pre-existing deployments where locktime was created as INTEGER.
    // nLockTime is a u32 (up to 4_294_967_295); INTEGER (i32) cannot hold
    // timestamp-based locktimes after 2038-01-19.
    let _ = sqlx::query("ALTER TABLE tbl_tx ALTER COLUMN locktime TYPE BIGINT")
        .execute(pool)
        .await;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_inp (
            id SERIAL PRIMARY KEY,
            txid TEXT,
            in_txid TEXT,
            in_vout INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_inp_unique ON tbl_inp(txid, in_txid, in_vout)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_out (
            id SERIAL PRIMARY KEY,
            txid TEXT,
            script_pubkey TEXT,
            amount TEXT,
            vout INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_out_unique ON tbl_out(txid, script_pubkey, amount, vout)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_xpub (
            id SERIAL PRIMARY KEY,
            network TEXT,
            xpub TEXT,
            date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            path_idx INTEGER DEFAULT -1
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_xpub ON tbl_xpub(network, xpub)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_address (
            address TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            xpub INTEGER,
            remote_address TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tbl_stats (
            report_date TEXT,
            chain TEXT,
            totals INTEGER,
            waiting INTEGER,
            sent INTEGER,
            failed INTEGER,
            waiting_profit INTEGER,
            sent_profit INTEGER,
            missed_profit INTEGER,
            unique_inputs INTEGER
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_stats_chain ON tbl_stats(chain)")
        .execute(pool)
        .await?;

    sqlx::query("UPDATE tbl_tx SET network='bitcoin' WHERE network='mainnet'")
        .execute(pool)
        .await?;

    Ok(())
}
