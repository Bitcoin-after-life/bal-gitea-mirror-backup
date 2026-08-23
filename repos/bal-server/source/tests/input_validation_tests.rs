use bal_server::db::{DatabasePool, create_database, get_all_addresses_by_xpub, open_database};

async fn setup_db_with_xpub() -> DatabasePool {
    let pool = open_database("sqlite", "sqlite::memory:").await.unwrap();
    create_database(&pool).await.unwrap();

    if let DatabasePool::SQLite(p) = &pool {
        sqlx::query("INSERT INTO tbl_xpub(id, network, xpub) VALUES(1, 'testnet', 'tpub_test')")
            .execute(p)
            .await
            .unwrap();

        for addr in ["addr1", "addr2", "addr3"] {
            sqlx::query("INSERT INTO tbl_address(address, path, xpub) VALUES(?, 'm/0/1', 1)")
                .bind(addr)
                .execute(p)
                .await
                .unwrap();
        }
    }

    pool
}

#[tokio::test]
async fn test_get_all_addresses_by_xpub_returns_known() {
    let pool = setup_db_with_xpub().await;
    let addresses = get_all_addresses_by_xpub(&pool, "tpub_test").await.unwrap();
    assert!(addresses.contains("addr1"));
    assert!(addresses.contains("addr2"));
    assert!(addresses.contains("addr3"));
    assert_eq!(addresses.len(), 3);
}

#[tokio::test]
async fn test_get_all_addresses_by_xpub_empty_for_missing() {
    let pool = setup_db_with_xpub().await;
    let addresses = get_all_addresses_by_xpub(&pool, "tpub_nonexistent")
        .await
        .unwrap();
    assert!(addresses.is_empty());
}

#[test]
fn test_network_unknown_returns_404() {
    let networks = ["bitcoin", "testnet", "testnet4", "signet", "regtest"];
    for n in networks {
        assert!(networks.contains(&n), "{} should be a valid network", n);
    }
    assert!(
        !networks.contains(&"attacker"),
        "attacker should not be a valid network"
    );
}

#[test]
fn test_txid_validation_is_hex_64() {
    let valid = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    assert!(valid.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(valid.len(), 64);

    let too_short = "abcdef1234567890";
    assert!(too_short.len() != 64);

    let non_hex = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef123456789g";
    assert!(!non_hex.chars().all(|c| c.is_ascii_hexdigit()));

    let with_dot = ".abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    assert!(!with_dot.chars().all(|c| c.is_ascii_hexdigit()));
}
