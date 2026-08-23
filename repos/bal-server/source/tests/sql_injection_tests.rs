use bal_server::db::{DatabasePool, open_database};
use sqlx::Row;

async fn setup_db() -> DatabasePool {
    open_database("sqlite", "sqlite::memory:").await.unwrap()
}

#[tokio::test]
async fn test_sql_injection_via_push_err_update() {
    let pool = setup_db().await;
    if let DatabasePool::SQLite(p) = &pool {
        sqlx::query("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER, push_err TEXT);")
            .execute(p)
            .await
            .unwrap();

        sqlx::query("INSERT INTO tbl_tx (txid, status, push_err) VALUES (?, ?, ?)")
            .bind("dummy_txid")
            .bind(0_i64)
            .bind("")
            .execute(p)
            .await
            .unwrap();

        let malicious_error = "'; DROP TABLE tbl_tx; --";
        let txid = "dummy_txid";

        sqlx::query("UPDATE tbl_tx SET status = 2, push_err = ? WHERE txid = ?")
            .bind(malicious_error)
            .bind(txid)
            .execute(p)
            .await
            .unwrap();

        let row = sqlx::query("SELECT status, push_err FROM tbl_tx WHERE txid = ?")
            .bind("dummy_txid")
            .fetch_one(p)
            .await
            .unwrap();

        let status: i64 = row.try_get("status").unwrap();
        let push_err: String = row.try_get("push_err").unwrap();
        assert_eq!(status, 2);
        assert_eq!(push_err, malicious_error);

        let row = sqlx::query("SELECT COUNT(*) as cnt FROM tbl_tx")
            .fetch_one(p)
            .await
            .unwrap();
        let count: i64 = row.try_get("cnt").unwrap();
        assert_eq!(count, 1);
    }
}

#[tokio::test]
async fn test_sql_injection_via_txid_update() {
    let pool = setup_db().await;
    if let DatabasePool::SQLite(p) = &pool {
        sqlx::query("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER);")
            .execute(p)
            .await
            .unwrap();

        for i in 0..3 {
            sqlx::query("INSERT INTO tbl_tx (txid, status) VALUES (?, ?)")
                .bind(format!("txid_{}", i))
                .bind(0_i64)
                .execute(p)
                .await
                .unwrap();
        }

        let malicious_txid = "' OR '1'='1";

        sqlx::query("UPDATE tbl_tx SET status = 1 WHERE txid = ?")
            .bind(malicious_txid)
            .execute(p)
            .await
            .unwrap();

        for i in 0..3 {
            let row = sqlx::query("SELECT status FROM tbl_tx WHERE txid = ?")
                .bind(format!("txid_{}", i))
                .fetch_one(p)
                .await
                .unwrap();
            let status: i64 = row.try_get("status").unwrap();
            assert_eq!(
                status, 0,
                "Row txid_{} should not be updated by malicious txid",
                i
            );
        }
    }
}

#[tokio::test]
async fn test_sql_injection_via_txid_with_comment() {
    let pool = setup_db().await;
    if let DatabasePool::SQLite(p) = &pool {
        sqlx::query("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER);")
            .execute(p)
            .await
            .unwrap();

        sqlx::query("INSERT INTO tbl_tx (txid, status) VALUES (?, ?)")
            .bind("safe_txid")
            .bind(0_i64)
            .execute(p)
            .await
            .unwrap();

        let malicious_txid = "safe_txid'; UPDATE tbl_tx SET status = 99; --";

        sqlx::query("UPDATE tbl_tx SET status = 1 WHERE txid = ?")
            .bind(malicious_txid)
            .execute(p)
            .await
            .unwrap();

        let row = sqlx::query("SELECT status FROM tbl_tx WHERE txid = ?")
            .bind("safe_txid")
            .fetch_one(p)
            .await
            .unwrap();
        let status: i64 = row.try_get("status").unwrap();
        assert_eq!(status, 0, "Original row should not be updated");

        let row = sqlx::query("SELECT COUNT(*) as cnt FROM tbl_tx WHERE status = 99")
            .fetch_one(p)
            .await
            .unwrap();
        let count: i64 = row.try_get("cnt").unwrap();
        assert_eq!(count, 0, "No rows should have status 99");
    }
}
