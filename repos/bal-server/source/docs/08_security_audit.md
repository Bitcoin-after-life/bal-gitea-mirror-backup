# Security Audit

## Quick Reference
- **What this file contains:** threat model, vulnerability assessment, hardening recommendations, and a security checklist.
- **See also:** [AGENTS.md](AGENTS.md), [07_deployment_and_ops.md](07_deployment_and_ops.md), [06_database_schema.md](06_database_schema.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md)

---

## Threat Model

### Assets
1. **`bal.db` (SQLite database):** Contains all transaction details, user IP addresses, and stats data. Single unencrypted file on disk. WAL mode enabled for concurrent access safety.
2. **Private Keys (`privkey.pem`):** Used to sign statistics payloads for the `welist` server. Located in `.gitignore`.
3. **Bitcoin Node (`bitcoind`) Access:** The `bal-pusher` has RPC access. Compromise allows arbitrary transaction broadcasting.
4. **Server Availability (`bal-server`):** Public-facing HTTP endpoint. DoS attacks threaten service availability.

### Attackers
- **Remote Anonymous Users:** Can interact with the API via public HTTP. No credentials required.
- **Network Man-in-the-Middle (MITM):** TLS termination is via Nginx reverse proxy. The `bal-server` itself is plain HTTP.
- **Local/Insider Threats:** If the server is compromised, the attacker can access `bal.db`, private keys, and env files.

---

## Vulnerability Assessment

### 1. SQL Injection (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/db.rs`, `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** SQL queries previously used `format!()` for string interpolation. All queries now use parameterized statements (`?` with `bind()`).
**Mitigation Applied:** All SQL queries rewritten with prepared statements. `execute_insert` uses parameterized batch inserts. `check_duplicate_txids` uses parameterized `IN` clauses. `echo_stats` handler uses prepared statements for chain filtering.
**Regression Tests:** `tests/sql_injection_tests.rs` (3 tests).

### 2. Panic on Untrusted Input (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** All `unwrap()`/`expect()` calls on critical paths have been replaced with safe error handling.
**Mitigation Applied:**
- `bal-server`: `from_utf8` returns 400, `sqlite::open` uses `open_db()` with validation, all handlers return proper HTTP status codes.
- `bal-pusher`: RPC failures log errors + sleep + retry (no panic), ZMQ `recv` uses `set_rcvtimeo(5000)`, `fs::read_to_string` uses `match` + 500, `cfg.lock()` uses `poisoned.into_inner()` recovery.
**Regression Tests:** `tests/panic_regression_tests.rs` (2 tests).

### 3. Secret Leakage (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `make_release.sh`, `.gitignore`
**Description:** `make_release.sh` now loads `TOKEN` from `.env` (`.env.example` provided). Private keys (`.pem`, `.key`) are in `.gitignore`. `generate_keys.sh` sets `chmod 600` on generated keys.
**Mitigation Applied:** Secrets removed from scripts and repository. `.gitignore` protects `.env`, `*.pem`, `*.key` files.
**Regression Tests:** `tests/secret_leakage_tests.rs` (3 tests).

### 4. Denial of Service (DoS) (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/bin/bal-server.rs` (HTTP), `src/bin/bal-pusher.rs` (ZMQ)
**Description:** All DoS vectors mitigated via actix-web migration.
**Mitigation Applied:**
- Body size limit: `PayloadConfig::default().limit(max_body_size)` via `BAL_SERVER_ACTIX_MAX_BODY_SIZE` (default 1 MiB).
- Rate limiting: `actix-governor` with token-bucket per endpoint (`BAL_SERVER_ACTIX_PUSHTXS_PER_SEC`/`BURST`).
- Connection limits: `workers(4)` and `max_connections(100)` via `BAL_SERVER_ACTIX_WORKERS`/`MAX_CONNECTIONS`.
- Body timeout: configurable via `BAL_SERVER_ACTIX_TIMEOUT_SECS`.
- ZMQ timeout: `set_rcvtimeo(5000)` prevents infinite blocking.
- RPC retry: sleep + retry on connection failure instead of panic.

### 5. SSRF / Network Abuse via `reqwest` (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/bin/bal-pusher.rs`, `src/validation.rs`
**Description:** URL validation prevents redirecting requests to internal/private IPs.
**Mitigation Applied:** `is_valid_welist_url()` in `src/validation.rs` blocks localhost, loopback, RFC1918, link-local, multicast, unspecified, and IPv6 unique-local addresses. HTTPS-only scheme enforced.
**Regression Tests:** `tests/ssrf_tests.rs` (integration) + 8 unit tests in `src/validation.rs`.

### 6. Insecure Database Access (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/db.rs`, `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** Database path validation and WAL mode for concurrent access.
**Mitigation Applied:**
- `open_db()` rejects `..` traversal, forbidden system directories (`/etc`, `/proc`, `/sys`, `/dev`, `/usr`, `/bin`, `/sbin`, `/lib`, `/opt`), symlinks, and non-regular files.
- WAL mode (`PRAGMA journal_mode=WAL`) with retry logic (up to 5 attempts).
- `busy_timeout=5000` for concurrent access.
- `bal-server` uses `Arc<Mutex<Connection>>` for thread-safe shared access.
**Regression Tests:** `tests/db_path_validation.rs` (5 tests).

### 7. ZMQ Authentication and Encryption (OPEN)
**Severity:** MEDIUM | **Status:** Open
**Location:** `src/bin/bal-pusher.rs`
**Description:** ZMQ connection is plaintext TCP. No authentication (ZAP), no encryption (ZMQ_CURVE). If the ZMQ port is exposed, any attacker can subscribe to topics.
**Mitigation (Operational):**
- Bind ZMQ to `127.0.0.1` only.
- Firewall blocks external access to ZMQ ports.
- If public ZMQ is required, use ZMQ_CURVE with public-key cryptography.

### 8. Missing HTTPS / Insecure Server Communication (FIXED)
**Severity:** HIGH | **Status:** Fixed (Infrastructure)
**Location:** Nginx configuration, `contrib/nginx/bal-server.conf`
**Description:** TLS termination via Nginx reverse proxy. The `bal-server` intentionally does not implement TLS.
**Mitigation Applied:**
- Dedicated Nginx template with `listen 443 ssl http2`, Let's Encrypt paths.
- Security headers: `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`.
- `client_max_body_size` matching `BAL_SERVER_ACTIX_MAX_BODY_SIZE`.
- HTTP 80 redirect to HTTPS.
- Deployment checklist ensures no accidental plain HTTP exposure.

### 9. Missing Input Validation (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/bin/bal-server.rs`
**Description:** Network, txid, and content validation.
**Mitigation Applied:**
- Network validation: `NETWORKS.contains(&param.as_str())` before processing. Unknown networks return 404.
- Txid validation: `echo_search` requires exactly 64 ASCII hex characters. Non-hex or wrong length returns 400.
- XPub address caching: `get_all_addresses_by_xpub` loads all addresses once per batch (O(1) lookup), eliminating N+1 queries.
- Content-Length: Handled by actix-web `PayloadConfig` size limit.
**Regression Tests:** `tests/input_validation_tests.rs` (4 tests).

### 10. Information Leakage (MITIGATED)
**Severity:** LOW | **Status:** Mitigated
**Location:** Application logs
**Description:** Raw file logging (`valid_txs`/`invalid_txs`) has been removed. `info!`/`warn!` macros may still log txid and details in application logs.
**Mitigation (Operational):** Set production `RUST_LOG` to `warn` or higher. Log files restricted with `chmod 600`.

### 11. `bal-stats.rs.dontcompile` (REMOVED)
**Severity:** LOW | **Status:** Removed
**Description:** The broken HTML report generator file no longer exists in the source tree.

---

## Hardening Recommendations

### System-Level
1. Run as non-root user with systemd hardening (`ProtectSystem=full`, `NoNewPrivileges`, `PrivateDevices`, `MemoryDenyWriteExecute`).
2. Use firewall to block all inbound ports except HTTPS (443) and SSH (22).
3. Use VPN or Tor for `welist` connections if on public network.
4. Run in a container or chroot for isolation.
5. Enable SELinux or AppArmor profiles for the binaries.
6. Use read-only filesystem for the server binary.

### Application-Level
1. **Rate Limiting:** Implemented via `actix-governor` with per-endpoint token-bucket configuration.
2. **Input Validation:** Network enum check, txid hex validation, body size limits.
3. **HTTPS:** Via Nginx reverse proxy with Let's Encrypt.
4. **WAL Mode:** Enabled with retry logic for concurrent access.
5. **ZMQ Timeout:** 5-second receive timeout prevents infinite blocking.
6. **Transaction Size Limits:** Configurable via rate limiting and body size.
7. **ZMQ Retry:** Reconnect logic with timeout-based detection.
8. **Fee Limits:** Per-network `fixed_fee` configuration.
9. **Network Limits:** Only known networks accepted (bitcoin, testnet, testnet4, signet, regtest).
10. **Locktime Reasonableness:** Locktime compared against blockchain height and median time with threshold-based distinction.

---

## Regression Test Suite

| Test File | Tests | Coverage |
|-----------|-------|----------|
| `tests/sql_injection_tests.rs` | 3 | Parameterized queries, injection prevention |
| `tests/panic_regression_tests.rs` | 2 | Mutex poisoning recovery, NULL value handling |
| `tests/ssrf_tests.rs` | 4+ | URL validation, internal IP blocking |
| `tests/secret_leakage_tests.rs` | 3 | .gitignore, no tracked secrets, no hardcoded tokens |
| `tests/input_validation_tests.rs` | 4 | Address caching, network validation, txid hex validation |
| `tests/db_path_validation.rs` | 5 | Path traversal, forbidden dirs, symlinks, WAL pragma, valid paths |
| `src/validation.rs` (inline) | 8 | SSRF URL validation unit tests |
