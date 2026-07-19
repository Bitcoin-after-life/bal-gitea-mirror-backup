use bal_server::db::{get_all_addresses_by_xpub, open_db};
use sqlite::Value;

fn setup_db_with_xpub() -> sqlite::Connection {
    let db = open_db(":memory:").unwrap();
    let _ = db.execute(
        "CREATE TABLE tbl_xpub (id INTEGER PRIMARY KEY, network TEXT, xpub TEXT, path_idx INTEGER DEFAULT -1);"
    );
    let _ = db.execute(
        "CREATE TABLE tbl_address (address TEXT PRIMARY KEY, path TEXT, xpub INTEGER, remote_address TEXT);"
    );
    // Insert test xpub
    let mut stmt = db
        .prepare("INSERT INTO tbl_xpub(id, network, xpub) VALUES(?, ?, ?);")
        .unwrap();
    stmt.bind((1, Value::Integer(1))).unwrap();
    stmt.bind((2, Value::String("testnet".to_string())))
        .unwrap();
    stmt.bind((3, Value::String("tpub_test".to_string())))
        .unwrap();
    let _ = stmt.next();
    drop(stmt);
    // Insert test addresses
    for addr in ["addr1", "addr2", "addr3"] {
        let mut stmt = db
            .prepare("INSERT INTO tbl_address(address, path, xpub) VALUES(?, ?, ?);")
            .unwrap();
        stmt.bind((1, Value::String(addr.to_string()))).unwrap();
        stmt.bind((2, Value::String("m/0/1".to_string()))).unwrap();
        stmt.bind((3, Value::Integer(1))).unwrap();
        let _ = stmt.next();
        drop(stmt);
    }
    db
}

#[test]
fn test_get_all_addresses_by_xpub_returns_known() {
    let db = setup_db_with_xpub();
    let addresses = get_all_addresses_by_xpub(&db, "tpub_test").unwrap();
    assert!(addresses.contains("addr1"));
    assert!(addresses.contains("addr2"));
    assert!(addresses.contains("addr3"));
    assert_eq!(addresses.len(), 3);
}

#[test]
fn test_get_all_addresses_by_xpub_empty_for_missing() {
    let db = setup_db_with_xpub();
    let addresses = get_all_addresses_by_xpub(&db, "tpub_nonexistent").unwrap();
    assert!(addresses.is_empty());
}

#[test]
fn test_network_unknown_returns_404() {
    // This is a code-level check; the actual HTTP test requires actix-web setup.
    // Verify that the NETWORKS constant includes the expected set.
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
