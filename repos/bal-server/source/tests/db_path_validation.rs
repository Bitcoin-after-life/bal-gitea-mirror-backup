use bal_server::db::{DatabasePool, create_database, open_database};
use sqlx::Row;

#[tokio::test]
async fn test_open_database_sqlite() {
    let pool = open_database("sqlite", "sqlite::memory:").await;
    assert!(pool.is_ok(), "Opening SQLite in-memory should succeed");
}

#[tokio::test]
async fn test_open_database_invalid_backend() {
    let pool = open_database("oracle", "connection_string").await;
    assert!(pool.is_err(), "Unknown backend should fail");
}

#[tokio::test]
async fn test_create_database_sqlite() {
    let pool = open_database("sqlite", "sqlite::memory:").await.unwrap();
    let result = create_database(&pool).await;
    assert!(result.is_ok(), "Creating SQLite schema should succeed");
}

#[tokio::test]
async fn test_create_database_is_idempotent() {
    let pool = open_database("sqlite", "sqlite::memory:").await.unwrap();
    create_database(&pool).await.unwrap();
    let result = create_database(&pool).await;
    assert!(
        result.is_ok(),
        "Creating schema twice should succeed (idempotent)"
    );
}

#[tokio::test]
async fn test_sqlite_wal_mode() {
    let tmp_path = std::env::temp_dir().join("tmp_test_wal_mode.db");
    let path_str = tmp_path.to_str().unwrap();
    let _ = std::fs::remove_file(&tmp_path);
    let dsn = format!("sqlite:{}?mode=rwc", path_str);
    let pool = open_database("sqlite", &dsn).await.unwrap();
    if let DatabasePool::SQLite(p) = &pool {
        let row = sqlx::query("PRAGMA journal_mode")
            .fetch_one(p)
            .await
            .unwrap();
        let mode: String = row.try_get(0).unwrap();
        assert_eq!(mode, "wal", "SQLite journal mode should be WAL");
    }
    drop(pool);
    let _ = std::fs::remove_file(&tmp_path);
    let _ = std::fs::remove_file(format!("{}-shm", path_str));
    let _ = std::fs::remove_file(format!("{}-wal", path_str));
}

#[tokio::test]
async fn test_open_database_blocks_traversal() {
    let res = open_database("sqlite", "sqlite:../etc/passwd").await;
    assert!(res.is_err(), "Path with '..' should be rejected");
    let err = res.err().unwrap();
    assert!(
        err.contains("'..'"),
        "Error should mention directory traversal: {}",
        err
    );
}

#[tokio::test]
async fn test_open_database_blocks_forbidden_absolute() {
    for path in [
        "sqlite:/etc/passwd",
        "sqlite:/proc/self/mem",
        "sqlite:/dev/null",
        "sqlite:/usr/bin/ls",
    ] {
        let res = open_database("sqlite", path).await;
        assert!(res.is_err(), "Absolute path {} should be rejected", path);
        let err = res.err().unwrap();
        assert!(
            err.contains("forbidden"),
            "Error should mention forbidden prefix: {}",
            err
        );
    }
}

#[tokio::test]
async fn test_open_database_rejects_symlink() {
    let tmp_dir = std::env::temp_dir();
    let real = tmp_dir.join("tmp_test_real_symlink.db");
    let link = tmp_dir.join("tmp_test_link_symlink.db");
    let _ = std::fs::remove_file(&real);
    let _ = std::fs::remove_file(&link);
    std::fs::File::create(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let dsn = format!("sqlite:{}?mode=rwc", link.to_str().unwrap());
    let res = open_database("sqlite", &dsn).await;
    assert!(res.is_err(), "Symlink DB path should be rejected");
    let err = res.err().unwrap();
    assert!(
        err.contains("symlink"),
        "Error should mention symlink: {}",
        err
    );

    let _ = std::fs::remove_file(&real);
    let _ = std::fs::remove_file(&link);
}
