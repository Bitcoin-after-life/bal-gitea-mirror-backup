use bal_server::db::{
    DatabasePool, calculate_and_upsert_stats, check_duplicate_txids, create_database,
    get_all_addresses_by_xpub, get_next_address_index, get_pending_txs, get_stats, insert_xpub,
    open_database, save_new_address, search_tx, update_tx_status,
};
use sqlx::Row;

fn pg_dsn() -> Option<String> {
    std::env::var("BAL_TEST_PG_DSN").ok()
}

async fn setup_pg() -> Option<DatabasePool> {
    let dsn = pg_dsn()?;
    let pool = open_database("postgresql", &dsn).await.ok()?;
    // Drop and recreate schema for clean test
    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query("DROP SCHEMA public CASCADE; CREATE SCHEMA public")
            .execute(p)
            .await
            .ok()?;
    }
    create_database(&pool).await.ok()?;
    Some(pool)
}

#[tokio::test]
async fn test_pg_open_database() {
    let Some(dsn) = pg_dsn() else {
        eprintln!("skipped: BAL_TEST_PG_DSN not set");
        return;
    };
    let pool = open_database("postgresql", &dsn).await;
    assert!(
        pool.is_ok(),
        "Opening PostgreSQL should succeed: {:?}",
        pool.err()
    );
}

#[tokio::test]
async fn test_pg_create_schema() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };
    // Second call should also succeed (idempotent)
    let result = create_database(&pool).await;
    assert!(
        result.is_ok(),
        "Creating PG schema twice should be idempotent: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_pg_insert_xpub() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };
    insert_xpub(&pool, "testnet", "tpub_test123").await;
    insert_xpub(&pool, "testnet", "tpub_test123").await; // duplicate should be ignored

    if let DatabasePool::PostgreSQL(p) = &pool {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM tbl_xpub WHERE xpub = 'tpub_test123'")
            .fetch_one(p)
            .await
            .unwrap();
        let count: i64 = row.try_get("cnt").unwrap();
        assert_eq!(count, 1, "INSERT OR IGNORE should prevent duplicates");
    }
}

#[tokio::test]
async fn test_pg_get_next_address_index() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };
    insert_xpub(&pool, "testnet", "tpub_addr_test").await;
    let (id, idx) = get_next_address_index(&pool, "testnet", "tpub_addr_test").await;
    assert!(id > 0, "Should return valid xpub id, got {}", id);
    assert_eq!(idx, 0, "First index should be 0, got {}", idx);

    let (id2, idx2) = get_next_address_index(&pool, "testnet", "tpub_addr_test").await;
    assert_eq!(id, id2, "xpub id should be stable");
    assert_eq!(idx2, 1, "Second index should be 1, got {}", idx2);
}

#[tokio::test]
async fn test_pg_save_and_get_address() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };
    insert_xpub(&pool, "testnet", "tpub_addr_test2").await;
    let (xpub_id, _idx) = get_next_address_index(&pool, "testnet", "tpub_addr_test2").await;
    save_new_address(&pool, xpub_id, "tb1qtestaddr", "m/0/0", "1.2.3.4").await;

    let addrs = get_all_addresses_by_xpub(&pool, "tpub_addr_test2")
        .await
        .unwrap();
    assert!(
        addrs.contains("tb1qtestaddr"),
        "Should find saved address: {:?}",
        addrs
    );
}

#[tokio::test]
async fn test_pg_check_duplicate_txids() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    // Insert a transaction first via raw SQL (to have a txid to check)
    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status)
             VALUES ('txid_dup_test', 'wtx1', 'ntx1', 'rawtx', 100, 'testnet', 0)",
        )
        .execute(p)
        .await
        .unwrap();
    }

    let dups = check_duplicate_txids(
        &pool,
        &["txid_dup_test".to_string(), "txid_new".to_string()],
    )
    .await
    .unwrap();

    assert!(
        dups.contains("txid_dup_test"),
        "Should detect existing txid"
    );
    assert!(
        !dups.contains("txid_new"),
        "Should not report non-existing txid"
    );
}

#[tokio::test]
async fn test_pg_search_tx() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status, our_address, our_fees, reqid)
             VALUES ('txid_search', 'wtx', 'ntx', 'rawhex', 500, 'testnet', 1, 'tb1ouraddr', '0.0001', 'req123')"
        )
        .execute(p)
        .await
        .unwrap();
    }

    let result = search_tx(&pool, "txid_search").await.unwrap();
    assert!(result.is_some(), "Should find the transaction");
    let row = result.unwrap();
    assert_eq!(row.status, "1", "Status should be read as string '1'");
    assert_eq!(row.tx, "rawhex");
    assert_eq!(row.our_address, "tb1ouraddr");
    assert_eq!(row.our_fees, "0.0001");
    assert_eq!(row.reqid, "req123");

    let not_found = search_tx(&pool, "txid_nonexistent").await.unwrap();
    assert!(not_found.is_none(), "Should return None for missing txid");
}

#[tokio::test]
async fn test_pg_update_tx_status() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status)
             VALUES ('txid_status', 'wtx', 'ntx', 'raw', 100, 'testnet', 0)",
        )
        .execute(p)
        .await
        .unwrap();
    }

    update_tx_status(&pool, "txid_status", 1, None)
        .await
        .unwrap();

    if let DatabasePool::PostgreSQL(p) = &pool {
        let row = sqlx::query("SELECT status FROM tbl_tx WHERE txid = 'txid_status'")
            .fetch_one(p)
            .await
            .unwrap();
        let status: i64 = row.try_get("status").unwrap();
        assert_eq!(status, 1, "Status should be updated to 1");
    }

    update_tx_status(&pool, "txid_status", 2, Some("test error"))
        .await
        .unwrap();

    if let DatabasePool::PostgreSQL(p) = &pool {
        let row = sqlx::query("SELECT status, push_err FROM tbl_tx WHERE txid = 'txid_status'")
            .fetch_one(p)
            .await
            .unwrap();
        let status: i64 = row.try_get("status").unwrap();
        let push_err: Option<String> = row.try_get("push_err").unwrap();
        assert_eq!(status, 2);
        assert_eq!(push_err.as_deref(), Some("test error"));
    }
}

#[tokio::test]
async fn test_pg_get_pending_txs() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    if let DatabasePool::PostgreSQL(p) = &pool {
        // Insert pending tx (status=0, locktime < height)
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status)
             VALUES ('pending1', 'w', 'n', 'rawtx1', 100, 'testnet', 0)",
        )
        .execute(p)
        .await
        .unwrap();

        // Insert already pushed tx (status=1)
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status)
             VALUES ('pushed1', 'w', 'n', 'rawtx2', 100, 'testnet', 1)",
        )
        .execute(p)
        .await
        .unwrap();
    }

    let txs = get_pending_txs(&pool, "testnet", 5000000, 200, 1000)
        .await
        .unwrap();
    assert_eq!(txs.len(), 1, "Should only return pending txs");
    assert_eq!(txs[0].txid, "pending1");
    assert_eq!(txs[0].tx, "rawtx1");
}

#[tokio::test]
async fn test_pg_stats() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    // Insert test data
    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query(
            "INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, status, our_fees)
             VALUES
                ('stx1', 'w', 'n', 'r', 100, 'testnet', 0, '0.0001'),
                ('stx2', 'w', 'n', 'r', 100, 'testnet', 1, '0.0002'),
                ('stx3', 'w', 'n', 'r', 100, 'testnet', 2, '0.0003')",
        )
        .execute(p)
        .await
        .unwrap();
    }

    calculate_and_upsert_stats(&pool, "testnet").await.unwrap();

    let stats = get_stats(&pool, "testnet").await.unwrap();
    assert_eq!(stats.len(), 1, "Should have one stats row");
    assert_eq!(stats[0].totals, 3);
    assert_eq!(stats[0].waiting, 1);
    assert_eq!(stats[0].sent, 1);
    assert_eq!(stats[0].failed, 1);
}

#[tokio::test]
async fn test_pg_sql_injection_via_push_err() {
    let Some(pool) = setup_pg().await else {
        eprintln!("skipped: PostgreSQL not available");
        return;
    };

    if let DatabasePool::PostgreSQL(p) = &pool {
        sqlx::query(
            "CREATE TABLE test_inject (txid TEXT PRIMARY KEY, status INTEGER, push_err TEXT)",
        )
        .execute(p)
        .await
        .unwrap();

        sqlx::query("INSERT INTO test_inject (txid, status, push_err) VALUES ($1, $2, $3)")
            .bind("dummy")
            .bind(0_i64)
            .bind("")
            .execute(p)
            .await
            .unwrap();

        let malicious = "'; DROP TABLE test_inject; --";
        sqlx::query("UPDATE test_inject SET status = 2, push_err = $1 WHERE txid = $2")
            .bind(malicious)
            .bind("dummy")
            .execute(p)
            .await
            .unwrap();

        let row = sqlx::query("SELECT status, push_err FROM test_inject WHERE txid = 'dummy'")
            .fetch_one(p)
            .await
            .unwrap();
        let status: i64 = row.try_get("status").unwrap();
        let push_err: String = row.try_get("push_err").unwrap();
        assert_eq!(status, 2);
        assert_eq!(push_err, malicious);

        // Table should still exist
        let cnt = sqlx::query("SELECT COUNT(*) as cnt FROM test_inject")
            .fetch_one(p)
            .await
            .unwrap();
        let count: i64 = cnt.try_get("cnt").unwrap();
        assert_eq!(count, 1);
    }
}
