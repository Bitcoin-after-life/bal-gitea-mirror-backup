use bal_server::db::open_db;
use sqlite::State;
use std::fs;
use std::path::Path;

#[test]
fn test_open_db_blocks_traversal() {
    let res = open_db("../etc/passwd");
    assert!(res.is_err(), "Path with '..' should be rejected");
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("Expected error for traversal path"),
    };
    assert!(
        err.contains("'..'"),
        "Error should mention directory traversal: {}",
        err
    );
}

#[test]
fn test_open_db_blocks_forbidden_absolute() {
    for path in ["/etc/passwd", "/proc/self/mem", "/dev/null", "/usr/bin/ls"] {
        let res = open_db(path);
        assert!(res.is_err(), "Absolute path {} should be rejected", path);
        let err = match res {
            Err(e) => e,
            Ok(_) => panic!("Expected error for forbidden path {}", path),
        };
        assert!(
            err.contains("forbidden"),
            "Error should mention forbidden prefix: {}",
            err
        );
    }
}

#[test]
fn test_open_db_allows_relative() {
    let test_path = "tmp_test_bal.db";
    let _ = fs::remove_file(test_path);
    let res = open_db(test_path);
    assert!(res.is_ok(), "Valid relative path should be allowed");
    let db = res.unwrap();
    drop(db);
    let _ = fs::remove_file(test_path);
}

#[test]
fn test_open_db_wal_pragmas_set() {
    let test_path = "tmp_test_wal.db";
    let _ = fs::remove_file(test_path);
    let _ = fs::remove_file(format!("{}-shm", test_path));
    let _ = fs::remove_file(format!("{}-wal", test_path));

    let db = open_db(test_path).expect("Should open DB");
    let mut stmt = db.prepare("PRAGMA journal_mode;").unwrap();
    if let Ok(State::Row) = stmt.next() {
        let mode: String = stmt.read(0).unwrap();
        assert_eq!(mode, "wal", "SQLite journal mode should be WAL");
    } else {
        panic!("Could not read journal_mode pragma");
    }

    let _ = fs::remove_file(test_path);
    let _ = fs::remove_file(format!("{}-shm", test_path));
    let _ = fs::remove_file(format!("{}-wal", test_path));
}

#[test]
fn test_open_db_rejects_symlink() {
    let real = "tmp_test_real.db";
    let link = "tmp_test_link.db";
    let _ = fs::remove_file(real);
    let _ = fs::remove_file(link);
    fs::File::create(real).unwrap();
    fs::soft_link(real, link).unwrap();

    let res = open_db(link);
    assert!(res.is_err(), "Symlink DB path should be rejected");
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("Expected error for symlink"),
    };
    assert!(
        err.contains("symlink"),
        "Error should mention symlink: {}",
        err
    );

    let _ = fs::remove_file(real);
    let _ = fs::remove_file(link);
}
