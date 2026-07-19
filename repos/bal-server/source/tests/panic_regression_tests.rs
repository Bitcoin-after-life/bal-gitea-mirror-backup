use sqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn test_mutex_poisoning_recovery() {
    let data = Arc::new(Mutex::new(0));
    let c = data.clone();
    let handle = thread::spawn(move || {
        let _guard = c.lock(); // Acquire lock
        panic!("test panic"); // Panic while holding the lock
        // _guard is dropped during panic unwinding, poisoning the mutex
    });
    let result = handle.join();
    assert!(result.is_err()); // Thread panicked

    // Recovery: the same pattern used in bal-server.rs
    let guard = match data.lock() {
        Ok(g) => g,
        Err(p) => {
            p.into_inner() // Should not panic
        }
    };
    assert_eq!(*guard, 0);
}

#[test]
fn test_db_null_unwrap_or() {
    let db = Connection::open(":memory:").unwrap();
    let _ = db.execute(
        "CREATE TABLE test_stats (report_date TEXT, chain TEXT, totals TEXT, waiting TEXT);",
    );
    let _ =
        db.execute("INSERT INTO test_stats (report_date, chain) VALUES ('2024-01-01', 'testnet');");

    let mut found_value = None;
    let _ = db.iterate("SELECT * FROM test_stats;", |pairs| {
        let row: HashMap<_, _> = pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.map(|s| s)))
            .collect();
        let totals = row["totals"].clone().unwrap_or("0").to_string();
        found_value = Some(totals);
        true
    });

    assert_eq!(found_value.unwrap(), "0");
}
