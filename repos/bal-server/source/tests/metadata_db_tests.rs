//! Tests for the shared executor metadata table.

use bal_server::db::{
    create_database, get_executor_metadata, open_database, upsert_executor_metadata,
};

#[tokio::test]
async fn executor_metadata_upsert_and_get() {
    let pool = open_database("sqlite", "sqlite::memory:").await.unwrap();
    create_database(&pool).await.unwrap();

    assert!(
        get_executor_metadata(&pool, "bitcoin")
            .await
            .unwrap()
            .is_none()
    );

    upsert_executor_metadata(&pool, "bitcoin", "info", 50000, "0.3.2", Some("PEM"))
        .await
        .unwrap();

    let row = get_executor_metadata(&pool, "bitcoin")
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.info, "info");
    assert_eq!(row.base_fee, 50000);
    assert_eq!(row.version, "0.3.2");
    assert_eq!(row.pubkey_pem.as_deref(), Some("PEM"));

    upsert_executor_metadata(&pool, "bitcoin", "info2", 1234, "0.4.0", None)
        .await
        .unwrap();

    let row = get_executor_metadata(&pool, "bitcoin")
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.info, "info2");
    assert_eq!(row.base_fee, 1234);
    assert_eq!(row.version, "0.4.0");
    assert!(row.pubkey_pem.is_none());
}

#[tokio::test]
async fn executor_metadata_is_per_chain() {
    let pool = open_database("sqlite", "sqlite::memory:").await.unwrap();
    create_database(&pool).await.unwrap();

    upsert_executor_metadata(&pool, "bitcoin", "main", 1, "1", Some("PEM1"))
        .await
        .unwrap();
    upsert_executor_metadata(&pool, "regtest", "reg", 2, "2", Some("PEM2"))
        .await
        .unwrap();

    let bitcoin = get_executor_metadata(&pool, "bitcoin")
        .await
        .unwrap()
        .expect("bitcoin row");
    let regtest = get_executor_metadata(&pool, "regtest")
        .await
        .unwrap()
        .expect("regtest row");

    assert_eq!(bitcoin.info, "main");
    assert_eq!(bitcoin.base_fee, 1);
    assert_eq!(regtest.info, "reg");
    assert_eq!(regtest.base_fee, 2);

    assert!(
        get_executor_metadata(&pool, "signet")
            .await
            .unwrap()
            .is_none()
    );
}
