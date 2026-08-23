use sqlx::Row;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn test_mutex_poisoning_recovery() {
    let data = Arc::new(Mutex::new(0));
    let c = data.clone();
    let handle = thread::spawn(move || {
        let _guard = c.lock();
        panic!("test panic");
    });
    let result = handle.join();
    assert!(result.is_err());

    let guard = match data.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    assert_eq!(*guard, 0);
}

#[tokio::test]
async fn test_db_null_unwrap_or() {
    let pool = bal_server::db::open_database("sqlite", "sqlite::memory:")
        .await
        .unwrap();
    if let bal_server::db::DatabasePool::SQLite(p) = &pool {
        sqlx::query(
            "CREATE TABLE test_stats (report_date TEXT, chain TEXT, totals TEXT, waiting TEXT);",
        )
        .execute(p)
        .await
        .unwrap();

        sqlx::query("INSERT INTO test_stats (report_date, chain) VALUES ('2024-01-01', 'testnet')")
            .execute(p)
            .await
            .unwrap();

        let row = sqlx::query("SELECT * FROM test_stats")
            .fetch_one(p)
            .await
            .unwrap();

        let totals: Option<String> = row.try_get("totals").unwrap_or(None);
        let totals_value = totals.unwrap_or_else(|| "0".to_string());
        assert_eq!(totals_value, "0");
    }
}
