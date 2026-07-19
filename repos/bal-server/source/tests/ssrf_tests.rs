use bal_server::validation::is_valid_welist_url;

#[test]
fn test_ssrf_blocks_internal_urls() {
    // Blocked: internal loopback
    assert!(
        !is_valid_welist_url("https://127.0.0.1"),
        "IPv4 loopback should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://localhost"),
        "localhost hostname should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://[::1]"),
        "IPv6 loopback should be blocked"
    );

    // Blocked: private RFC1918 ranges
    assert!(
        !is_valid_welist_url("https://192.168.1.1"),
        "RFC1918 private IP should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://10.0.0.1"),
        "RFC1918 private IP should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://172.16.0.1"),
        "RFC1918 private IP should be blocked"
    );

    // Blocked: AWS metadata link-local
    assert!(
        !is_valid_welist_url("https://169.254.169.254"),
        "AWS metadata link-local IP should be blocked"
    );

    // Blocked: non-HTTPS schemes
    assert!(
        !is_valid_welist_url("http://welist.bitcoin-after.life"),
        "HTTP plaintext should be blocked"
    );
    assert!(
        !is_valid_welist_url("ftp://welist.bitcoin-after.life"),
        "FTP scheme should be blocked"
    );

    // Blocked: malformed URLs
    assert!(
        !is_valid_welist_url("not a url"),
        "Malformed URL should be blocked"
    );
    assert!(
        !is_valid_welist_url("welist.bitcoin-after.life"),
        "URL missing scheme should be blocked"
    );

    // Allowed: valid public domain on HTTPS
    assert!(
        is_valid_welist_url("https://welist.bitcoin-after.life"),
        "Known production domain should be allowed"
    );
    assert!(
        is_valid_welist_url("https://example.com/ping"),
        "Public domain on HTTPS should be allowed"
    );

    // Allowed: valid public IP on HTTPS
    assert!(
        is_valid_welist_url("https://8.8.8.8"),
        "Public IP on HTTPS should be allowed"
    );
    assert!(
        is_valid_welist_url("https://1.2.3.4"),
        "Public IP on HTTPS should be allowed"
    );
}

#[test]
fn test_ssrf_case_insensitive_localhost() {
    assert!(
        !is_valid_welist_url("https://LOCALHOST"),
        "Uppercase localhost should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://LocalHost"),
        "Mixed case localhost should be blocked"
    );
}

#[test]
fn test_ssrf_ipv6_unique_local() {
    assert!(
        !is_valid_welist_url("https://[fc00::1]"),
        "IPv6 unique local fc00 should be blocked"
    );
    assert!(
        !is_valid_welist_url("https://[fd00::1]"),
        "IPv6 unique local fd00 should be blocked"
    );
}

#[test]
fn test_ssrf_port_presence_ok() {
    assert!(
        is_valid_welist_url("https://welist.bitcoin-after.life:443"),
        "HTTPS with explicit port should be allowed"
    );
}
