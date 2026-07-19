use sqlite::{Connection, Value};

#[test]
fn test_sql_injection_via_push_err_update() {
    // Create an in-memory database and the required table
    let db = Connection::open(":memory:").unwrap();
    let _ =
        db.execute("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER, push_err TEXT);");

    // Insert a dummy transaction
    let mut stmt = db
        .prepare("INSERT INTO tbl_tx (txid, status, push_err) VALUES (?, ?, ?);")
        .unwrap();
    stmt.bind((1, Value::String("dummy_txid".to_string())))
        .unwrap();
    stmt.bind((2, Value::Integer(0))).unwrap();
    stmt.bind((3, Value::String("".to_string()))).unwrap();
    let _ = stmt.next();
    drop(stmt);

    // Malicious error payload containing a single quote (SQL injection attempt)
    let malicious_error = "'; DROP TABLE tbl_tx; --";
    let txid = "dummy_txid";

    // Execute the fixed query using parameter binding (safe)
    let sql = "UPDATE tbl_tx SET status = 2, push_err = ? WHERE txid = ?";
    let mut stmt = db.prepare(sql).unwrap();
    stmt.bind((1, Value::String(malicious_error.to_string())))
        .unwrap();
    stmt.bind((2, Value::String(txid.to_string()))).unwrap();
    let _ = stmt.next();

    // Verify the table still exists and the row was updated correctly
    let mut check = db
        .prepare("SELECT status, push_err FROM tbl_tx WHERE txid = ?;")
        .unwrap();
    check
        .bind((1, Value::String("dummy_txid".to_string())))
        .unwrap();
    assert!(check.next().unwrap() == sqlite::State::Row);
    let status: i64 = check.read("status").unwrap();
    let push_err: String = check.read("push_err").unwrap();
    assert_eq!(status, 2);
    assert_eq!(push_err, malicious_error);

    // Ensure no second row was created (injection would have failed or produced extra rows)
    let mut count_stmt = db.prepare("SELECT COUNT(*) FROM tbl_tx;").unwrap();
    assert!(count_stmt.next().unwrap() == sqlite::State::Row);
    let count: i64 = count_stmt.read(0).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_sql_injection_via_txid_update() {
    let db = Connection::open(":memory:").unwrap();
    let _ = db.execute("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER);");

    // Insert multiple dummy transactions
    for i in 0..3 {
        let mut stmt = db
            .prepare("INSERT INTO tbl_tx (txid, status) VALUES (?, ?);")
            .unwrap();
        stmt.bind((1, Value::String(format!("txid_{}", i))))
            .unwrap();
        stmt.bind((2, Value::Integer(0))).unwrap();
        let _ = stmt.next();
    }

    // Malicious txid payload
    let malicious_txid = "' OR '1'='1";

    // The fixed query parameterizes the txid, so this should only update zero rows
    let sql = "UPDATE tbl_tx SET status = 1 WHERE txid = ?";
    let mut stmt = db.prepare(sql).unwrap();
    stmt.bind((1, Value::String(malicious_txid.to_string())))
        .unwrap();
    let _ = stmt.next();

    // Verify no rows were updated (status should still be 0 for all)
    for i in 0..3 {
        let mut check = db
            .prepare("SELECT status FROM tbl_tx WHERE txid = ?;")
            .unwrap();
        check
            .bind((1, Value::String(format!("txid_{}", i))))
            .unwrap();
        assert!(check.next().unwrap() == sqlite::State::Row);
        let status: i64 = check.read("status").unwrap();
        assert_eq!(
            status, 0,
            "Row txid_{} should not be updated by malicious txid",
            i
        );
    }
}

#[test]
fn test_sql_injection_via_txid_with_comment() {
    let db = Connection::open(":memory:").unwrap();
    let _ = db.execute("CREATE TABLE tbl_tx (txid TEXT PRIMARY KEY, status INTEGER);");

    let mut stmt = db
        .prepare("INSERT INTO tbl_tx (txid, status) VALUES (?, ?);")
        .unwrap();
    stmt.bind((1, Value::String("safe_txid".to_string())))
        .unwrap();
    stmt.bind((2, Value::Integer(0))).unwrap();
    let _ = stmt.next();

    // Another common injection pattern
    let malicious_txid = "safe_txid'; UPDATE tbl_tx SET status = 99; --";

    let sql = "UPDATE tbl_tx SET status = 1 WHERE txid = ?";
    let mut stmt = db.prepare(sql).unwrap();
    stmt.bind((1, Value::String(malicious_txid.to_string())))
        .unwrap();
    let _ = stmt.next();

    // Verify the original row was NOT updated (because it was looking for the full malicious string)
    // and no rows have status 99 (the injected update did not execute)
    let mut check = db
        .prepare("SELECT status FROM tbl_tx WHERE txid = ?;")
        .unwrap();
    check
        .bind((1, Value::String("safe_txid".to_string())))
        .unwrap();
    assert!(check.next().unwrap() == sqlite::State::Row);
    let status: i64 = check.read("status").unwrap();
    assert_eq!(status, 0, "Original row should not be updated");

    let mut count_stmt = db
        .prepare("SELECT COUNT(*) FROM tbl_tx WHERE status = 99;")
        .unwrap();
    assert!(count_stmt.next().unwrap() == sqlite::State::Row);
    let count: i64 = count_stmt.read(0).unwrap();
    assert_eq!(count, 0, "No rows should have status 99");
}
