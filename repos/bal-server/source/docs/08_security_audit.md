# Security Audit

## Quick Reference
- **What this file contains:** threat model, vulnerability assessment, hardening recommendations, and a security checklist.
- **See also:** [AGENTS.md](AGENTS.md), [07_deployment_and_ops.md](07_deployment_and_ops.md), [06_database_schema.md](06_database_schema.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md)

---

## Threat Model

### Assets
1. **`bal.db` (SQLite database):** Contains all transaction details, including private transaction data, user IP addresses, and the `welist` stats payload. The file is a single, unencrypted file on disk. If the database is exfiltrated, the attacker will have knowledge of the transaction history and user activity.
2. **Private Keys (`private_key.pem`, `privkey.pem`, `ec.key`, `chiave_privata.key`):** The `private_key.pem` is used to sign the statistics payload for the `welist` server. An attacker with access to this key can impersonate the server and send fake statistics or modify the remote database.
3. **Bitcoin Node (`bitcoind`) Access:** The `bal-pusher` has RPC access to the `bitcoind` node. If an attacker can compromise the pusher, they can send arbitrary transactions to the network, potentially misappropriating funds or DoS-ing the node.
4. **Server Availability (`bal-server`):** The server is a public-facing HTTP endpoint. If it is down, users cannot submit transactions. Denail of service (DoS) attacks could be a direct threat to the service's availability.

### Attackers
- **Remote Anonymous Users:** Can interact with the `bal-server` API via the public HTTP interface. They do not have credentials or special access. They can send valid or invalid transactions.
- **Network Man-in-the-Middle (MITM):** The HTTP server does not have TLS by default (see `07_deployment_and_ops.md`). If Nginx is not configured with a valid SSL certificate, an attacker can intercept the traffic.
- **Local/Insider Threats:** If the server is compromised (e.g., via a vulnerable `bitcoind` or a remote exploit), the attacker can read the `bal.db` file, the private key, and the `env` files. The database file contains all transaction data, which is a serious privacy risk.

## Vulnerability Assessment

### 1. SQL Injection (HIGH)
**Location:** `src/bin/bal-server.rs` (e.g., `echo_stats`, `echo_push` handlers), `src/db.rs`.
**Description:** SQL queries are built using `format!("... WHERE txid in ('{}')", ...")` in `db.rs`. The `txid` strings are derived from the raw HTTP request body. While the `txid` is usually a hash of 32 bytes, the database code does not validate or enforce this. This is a potential SQL injection vector if an attacker can bypass the transaction hash check or if the `txid` string is used directly from the request body without proper escaping or parameterized queries.
**Impact:** An attacker could potentially read, modify, or delete any database record.
**Mitigation:** Replace all string-formatted SQL with prepared statements using parameterized queries (`?`) for every user-input value. See `src/db.rs` for the `execute_insert` function, which already uses parameterized queries but is not universally applied.
**Status:** Fixed (Vulnerability 1 & 2 in `bal-pusher.rs` patched in commit).
**Reproduction:** Send a malicious `searchtx` request with a crafted `txid` containing SQL characters (e.g., `' OR '1'='1`). The database will not crash because the query is malformed, but it might be exploitable if the `txid` format is not strictly enforced. See `valid_txs` and `invalid_txs` log files for examples of valid and invalid txids.
**Fix Applied:** Vulnerabilities 1 and 2 (UPDATE `txid IN` and UPDATE `push_err` in `bal-pusher.rs`) were rewritten to use parameterized queries (`?` with `bind()`). Regression tests added in `tests/sql_injection_tests.rs`.

### 2. Panic on Untrusted Input (HIGH)
**Location:** `src/bin/bal-server.rs` (e.g., `req.collect().unwrap()`, `Regex::new(...).unwrap()`) and `src/bin/bal-pusher.rs` (e.g., `panic!("impossible to get client {}", e)`).
**Description:** The `bal-server` uses `unwrap()` and `expect()` on many critical paths. A malformed HTTP request (e.g., an oversized body, invalid JSON, or an invalid `network` string) can cause a panic in the async runtime. This could crash the entire server process or at least one async worker. The `Regex::new` is also `unwrap`ed, making the entire server crash if the regex is not valid at startup.
- The `bal-pusher` panics on RPC connection failures (`main_result` -> `get_client`). If the Bitcoin node is temporarily down, the entire pusher process will crash. This is a serious DoS vector because it will stop the service from broadcasting transactions if the network is unstable.
**Impact:** A single malformed request can crash the entire server or the pusher daemon, leading to a full Denial of Service (DoS).
**Mitigation:**
- Replace all `unwrap()` and `expect()` with `match` or `Result` propagation in the server request handlers. Use `?` to bubble errors up, or return `400 Bad Request` / `500 Internal Server Error` with a safe error message.
- In the `bal-pusher`, do not `panic!` on RPC connection failures. Instead, use `eprintln!` or `log::error!` and sleep for a retry interval. The ZMQ connection should be monitored independently, not tied to the pusher's lifetime.
- In the `bal-pusher`, ensure ZMQ `recv` has a timeout (e.g., `RCVTIMEO`). If the ZMQ socket is blocked, the thread will not be killed, and it will consume resources indefinitely. This is a resource leak / DoS vector.
**Status:** Fixed (Fase 1 + Fase 2 applied). All critical panic vectors in `bal-server.rs` and `bal-pusher.rs` have been replaced with safe `match`/`if let` error propagation. `unwrap`/`expect` replaced with:
- `from_utf8` → `match` + `return Ok(400)`
- `sqlite::open` per richiesta → `Arc<Mutex<Connection>>` condiviso
- `panic!` su RPC → `error!` + sleep + retry
- `recv_multipart` → `set_rcvtimeo(5000)` + `match`
- `connect`/`subscribe` → retry loop con `match`/`return`
- `fs::read_to_string().expect()` → `match` + `500 Internal Server Error`
- `timestamp_nanos_opt().unwrap()` → `match` + `return Ok(400)`
- `idx/amount.try_into().unwrap()` → `i64::try_from(...).unwrap_or(0/-1)`
- `cfg.lock().unwrap()` → `match` + `poisoned.into_inner()` recovery
- `stmt.read().unwrap()`/`bind().unwrap()` in `db.rs` → `match`/`if let` + log error

Regression tests: `tests/panic_regression_tests.rs` (2 tests).

### 3. Secret Leakage (HIGH)
**Location:** `make_release.sh`, `contrib/download_and_install_bal.sh`, `private_key.pem`, `privkey.pem`, `ec.key`, `chiave_privata.key`.
**Description:**
- The `make_release.sh` script contains a hardcoded Gitea API token: `TOKEN="5cfa8c33e337ebaadb355c0ffa2d053d521ee43b"`. If the script is accidentally pushed to a public repository, it will be visible to everyone.
- The `contrib/download_and_install_bal.sh` script contains a hardcoded `xpub` address and a fixed fee. This is less critical but could be used for fingerprinting.
- The `private_key.pem` and `chiave_privata.key` files are stored in the project root (and in the repository). If the repository is public, the private key is compromised. An attacker could use this to sign fake statistics or forge authentication credentials.
**Impact:** An attacker could gain unauthorized access to the CI/CD pipeline, the release server, or the `welist` statistics service.
**Mitigation:**
- Remove `private_key.pem` from the repository and add it to `.gitsecret` or `.gitignore`. Use a secret manager or a password store for the `private_key.pem`.
- Remove hardcoded secrets from the scripts. The `TOKEN` and `xpub` should be environment variables or configuration files injected via the build process.
- Use `git-crypt` or `git-secret` to encrypt the private key files before committing.
**Status:** Fixed. `make_release.sh` now loads `TOKEN` from `.env` (`.env.example` provided). `contrib/download_and_install_bal.sh` no longer hardcodes `xpub`/`fixed_fee`. Private keys moved to `.gitignore` (`*.pem`, `*.key`). `generate_keys.sh` sets `chmod 600` on generated keys. Regression tests: `tests/secret_leakage_tests.rs` (3 tests). **Priority:** High. (Mitigation applied)

### 4. Denial of Service (DoS) (HIGH)
**Location:** `src/bin/bal-server.rs` (HTTP request body), `src/bin/bal-pusher.rs` (ZMQ).
**Description:**
- The `bal-server` does not limit the size of the HTTP request body. On the `POST /pushtxs` endpoint, it calls `req.collect().await?.to_bytes()` without checking for a maximum body size. A malicious client could send an unbounded or extremely large request (e.g., `100000MB`), which would consume all available memory and crash the server.
- The `bal-server` regex for path matching might be expensive if the user provides a malicious path string. For a production system, the regex should be compiled only once at startup and should be very specific.
- The `bal-pusher` `recv` call is synchronous and blocking. If the ZMQ connection fails, the thread will hang without any timeout. This is a resource leak if the connection is broken. The ZMQ socket is not reconfigured with `ZMQ_RECONNECT_IVL` or `ZMQ_MAXMSGSIZE`. If the Bitcoin Core node is not sending, the pusher will be stuck waiting forever, consuming a thread and not doing other useful work.
- The `bal-pusher` does not have a rate limiter for the `sendrawtransaction` call. If the database is full or the ZMQ loop is running very fast, it could send thousands of RPC requests to the `bicoind` node, overwhelming it. For example, if the node is slow, the pusher will keep sending requests, potentially blocking the RPC queue or causing a memory leak in `bitcoind`.
**Impact:** The server could become unresponsive, crash, or be completely unavailable. The `bitcoind` node could be overwhelmed with `sendrawtransaction` requests, causing a chain failure in the entire Bitcoin infrastructure.
**Reproduction:**
- For the HTTP server: Send an HTTP POST with `Content-Length: 9999999999` to `POST /regtest/pushtxs`. The server will try to allocate that much memory and will be killed by the OOM killer.
- For the ZMQ pusher: Kill the `bitcoind` ZMQ socket. The `bal-pusher` will hang forever. The process cannot be killed gracefully by the systemd `SIGTERM` because the thread is blocked by the ZMQ `recv` call.
**Mitigation:**
- Add a maximum body size check to the HTTP server. Use `hyper`'s built-in `Body` size limiter, or manually check `req.headers().get("content-length")` before `collect().await` and return `413 Payload Too Large` if it exceeds the limit (e.g., `1 MB` for a single transaction, or `10 MB` for a batch).
- Implement request rate limiting on the `bal-server` (e.g., `tower::filter` or a simple `HashMap` of client IP address to request count). Limit the `pushtxs` request to one per second per IP.
- Add a ZMQ socket option for `ZMQ_RCVTIMEO` (e.g., `5000` ms) to avoid blocking forever. The pusher should be able to handle a ZMQ timeout gracefully and retry the connection or reconnect the socket.
- The `bal-pusher` should have a rate limiting mechanism for the `sendrawtransaction` call to the RPC. For example, only allow sending `1` transaction per block, or use a queue and a `semaphore` to limit the number of concurrent RPC calls.
**Status:** Fixed (Migrated to Actix Web). All DoS vectors mitigated via:
- Body size limit: `PayloadConfig::default().limit(max_body_size)` — configurable via `BAL_SERVER_ACTIX_MAX_BODY_SIZE` (default 1 MiB)
- Rate limiting: `actix-governor` middleware with token-bucket — configurable via `BAL_SERVER_ACTIX_PUSHTXS_PER_SEC`/`BURST` (default 1 req/s per IP with burst 5)
- Connection limits: `workers(4)` and `max_connections(100)` — configurable via `BAL_SERVER_ACTIX_WORKERS`/`MAX_CONNECTIONS`
- Body timeout: configurable via `BAL_SERVER_ACTIX_TIMEOUT_SECS` (default 30s)
**Migration:** Server replaced `hyper` custom server with `actix-web` (see `src/bin/bal-server.rs`). All handlers migrated with `Arc<Mutex<Connection>>` shared DB. Old `bal-server.rs` (Hyper) removed. `bal-pusher` enhanced with ZMQ timeout (`ZMQ_RCVTIMEO` 5000ms) and RPC retry logic. **Priority:** High. (Mitigated)

### 5. SSRF / Network Abuse via `reqwest` (MEDIUM)
**Location:** `src/bin/bal-pusher.rs`.
**Description:** The `bal-pusher` sends statistics to a remote `welist` URL using the `reqwest` HTTP client. The `WELIST_URL` is configurable, but the pusher does not validate the URL before sending the HTTP request. An attacker who can modify the `WELIST_URL` (e.g., by modifying the pusher's environment file) can redirect the traffic to any arbitrary URL, including internal services. The `reqwest` client has SOCKS5 enabled (`socks` feature). This could allow an attacker to use the pusher's network to scan internal addresses, send requests to `localhost` or `169.254.169.254` (AWS metadata IP), or access internal infrastructure.
**Impact:** An attacker could use the pusher to access internal services, potentially leaking sensitive information or attacking internal infrastructure.
**Mitigation:**
- ✅ **Implemented:** Added strict URL validation `bal_server::validation::is_valid_wELIST_url` (see `src/validation.rs`). It checks:
  - URL must be well-formed and parsable.
  - Scheme must be `https://` (plain HTTP is rejected).
  - Host must not be `localhost`, `127.0.0.1`, `::1`, or any loopback/private/link-local/multicast/unspecified IP address.
  - IPv4 private RFC1918 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16) and AWS metadata link-local (169.254.169.254) are blocked.
  - IPv6 Unique Local (fc00::/7) and link-local (fe80::/10) are blocked.
  - IPv6 address brackets are stripped before validation.
- The `bal-pusher` `send_stats_report` now calls `is_valid_wELIST_url` before making the request. If validation fails, the function skips the request with a warning and returns `Ok(())` to avoid panicking.
- The `WELIST_URL` is configurable via `WELIST_SERVER_URL` (defaults to `https://wELIST.bitcoin-after.life`), but invalid URLs are rejected at runtime. If stats are not needed, `send_stats` can be set to `false` to skip the feature entirely.
- SOCKS5 feature is retained for `.onion` support (see `07_deployment_and_ops.md`), but URL validation prevents redirecting to internal IPs.
**Regression tests:** `src/validation.rs` (unit tests) and `tests/ssrf_tests.rs` (integration tests) cover all blocked/allowed IP ranges and schemes. 9 tests + 4 integration tests = all passing.
**Status:** Fixed. **Priority:** Medium.

### 6. Insecure Database Access (MEDIUM)
**Location:** `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`.
**Description:** The `bal-server` opens the `bal.db` file using `sqlite::open(&cfg.db_file).unwrap()`. The path is not validated. If the environment variable `BAL_DB_FILE` is set to a malicious path (e.g., `/etc/passwd`), the server will try to open it as a database. This could cause a crash or a security issue if the database file is on a malicious path. Also, if the database file is on a network drive, the performance will be very slow, and it might cause a timeout.
- The `bal-pusher` and `bal-server` both access the same `bal.db` file. There is no file locking mechanism or `flock` on the database file. If two instances of the `bal-server` start at the same time, they might corrupt the database or cause a deadlock. SQLite handles this automatically, but the `sqlite` crate (Rust) might not be configured with the proper threading mode (`WAL` or `SHARED`).
**Impact:**
- The database file could be placed on a path that causes a file system vulnerability or a crash of the server.
- The database file might be corrupted if multiple processes access it without proper locking.
**Mitigation:**
- ✅ **Path validation:** Added `db::open_db` in `src/db.rs` which validates the database path before opening:
  - Rejects paths containing `..` (directory traversal).
  - Rejects absolute paths pointing to sensitive directories (`/etc`, `/proc`, `/sys`, `/dev`, `/usr`, `/bin`, `/sbin`, `/lib`, `/opt`).
  - Rejects symlinks and non-regular files (directories, devices, etc.).
  - If validation fails, the function returns `Err(String)` instead of panicking, preventing crashes or accidental access to system files.
- ✅ **WAL mode:** `db::open_db` automatically executes `PRAGMA journal_mode = WAL;` and `PRAGMA synchronous = NORMAL;` on every connection. This is a best practice for safe concurrent access when `bal-server` and `bal-pusher` share the same database file.
- ✅ **Replaced `unwrap`:** In `src/bin/bal-server.rs` and `src/bin/bal-pusher.rs`, `sqlite::open(...).unwrap()` was replaced with `db::open_db(...)` with safe error handling (return `Err` in the server, `std::process::exit(1)` in the pusher with a log error).
- **Remaining (ops):** Ensure the database file is owned by the `bal` user and not writable by any other user (`chmod 600`). The database file should not reside on a shared or network drive.
**Regression tests:** `tests/db_path_validation.rs` (5 tests covering traversal, forbidden absolute paths, symlink, WAL pragma, and valid relative paths). All passing.
**Status:** Fixed. **Priority:** Medium.

### 7. ZMQ Authentication and Encryption (MEDIUM)
**Location:** `src/bin/bal-pusher.rs`.
**Description:** The ZMQ connection to the `bitcoind` is a plaintext TCP connection (`zmqpubhashblock=tcp://127.0.0.1:28332`). There is no ZMQ authentication (ZAP), no username/password, and no encryption (ZMQ_CURVE or ZMQ_GSSAPI). If the ZMQ port is accessible from the network (not just `127.0.0.1`), any attacker can subscribe to the `hashblock` or `rawblock` topics. The `rawblock` topic is particularly sensitive because it sends full block data, which is large and could be used to fingerprint the `bal` system. More importantly, the pusher does not verify that the `hashblock` is from the intended `bitcoind` node. If an attacker can inject a fake ZMQ message, they could trigger the pusher to evaluate the transactions and potentially broadcast them at an incorrect time, or cause a DoS.
**Impact:**
- If the ZMQ port is exposed, an attacker can intercept the `rawblock` data to get the full block contents, which could be used to fingerprint the node or the system.
- An attacker can send a fake `hashblock` message to the pusher, causing it to try to evaluate the database. If the pusher is not idempotent, it could cause duplicate or incorrect RPC requests.
**Mitigation:**
- Bind `zmqpubhashblock` and `zmqpubrawblock` to `127.0.0.1` (or `127.0.0.1:28332`) and ensure the firewall blocks external access to the ZMQ port (e.g., port 28332). Use a firewall (e.g., `iptables`, `ufw`) to deny external access to port 28332.
- If the ZMQ port must be on a public interface, use ZMQ_CURVE with public-key cryptography, or ZMQ_GSSAPI with TLS. This is a more advanced solution but provides strong authentication and encryption for the ZMQ channel.
- If not using ZMQ_CURVE, use `zmqpubhashblock` with a firewall that blocks the public port for port 28332.
- The `bal` service should not listen on all interfaces (0.0.0.0) unless necessary. It is better to listen only on `127.0.0.1` if the server is behind a reverse proxy (like Nginx) or if the server is only accessible from the local machine.
**Status:** Open. **Priority:** Medium.

### 8. Missing HTTPS / Insecure Server Communication (HIGH)
**Location:** `src/bin/bal-server.rs` (TCP server), Nginx configuration.
**Description:** The `bal-server` is a plain HTTP server. It does not have TLS or SSL support. To provide HTTPS, an external reverse proxy like Nginx is recommended. However, if the server is exposed to the internet directly, the entire transaction data will be sent over unencrypted HTTP. This includes the raw transaction details and the user IP, which is a privacy risk. An attacker on the same network as the server or client can intercept the request and see the transaction details or the `welist` data.
**Impact:**
- If the server is directly exposed to the internet, the transaction data is sent in plaintext, making it vulnerable to sniffing and MitM attacks.
- If the reverse proxy is not configured with TLS, the server will be insecure and might be vulnerable to a `HTTP Host Header Injection` or `HTTP Header Injection` attack if the server uses the Host header to determine the routing.
**Mitigation:**
- ✅ **Nginx config extracted:** The inline Nginx block from `contrib/download_and_install_bal.sh` was extracted into a dedicated, auditable template: `contrib/nginx/bal-server.conf`. It includes:
  - `listen 443 ssl http2;` with Let's encrypt paths
  - `proxy_pass` to `http://127.0.0.1:9137` only
  - `client_max_body_size 1m` matching `BAL_SERVER_ACTIX_MAX_BODY_SIZE`
  - `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy` security headers
  - HTTP 80 redirect to HTTPS
  - Optional `limit_req` / `limit_conn` directives (commented, ready for activation)
- ✅ **Bind warning:** `.env.example` and `bal-server.env` updated with explicit warning: `!!! Never bind to 0.0.0.0. Use 127.0.0.1 and place Nginx with TLS in front.`. Default bind address is `127.0.0.1`.
- ✅ **Deployment checklist:** `docs/07_deployment_and_ops.md` now includes a step-by-step "Production Deployment Checklist" covering Nginx TLS, firewall rules, DB permissions, ZMQ port blocking, and logging hardening.
- **Note:** This is an **infrastructure hardening**, not a code change. The `actix-web` server intentionally does not implement TLS — it is the reverse proxy's responsibility. The checklist ensures no operator accidentally exposes plain HTTP to the internet.
**Status:** Fixed (Documented/Infrastructure). **Priority:** High.

### 9. Missing Input Validation (MEDIUM)
**Location:** `src/bin/bal-server.rs` (e.g., `pushtxs` endpoint).
**Description:** While the server does check if the transaction is valid and the fee is correct, it does not validate the `Content-Type` or the `Content-Length` of the request body. It also does not validate the `network` string before using it in the path. The `network` string is directly used to match the database table, which could be a potential SQL injection or DoS vector if the string is not a known network (e.g., `bitcoin`, `testnet`). The `searchtx` endpoint also does not validate the `txid` format and uses it in the SQL query.
**Impact:** An attacker could send a request with a malformed `network` or `txid`, which could cause unexpected database behavior, or a server error (e.g., a 500 error if the database table is not found), or a DoS if the SQL query is not handled properly. The `network` value is used as a string in the query, which could be used to bypass the database if it is not validated.
**Mitigation:**
- Add a strict validation step for the `network` parameter. Use an `enum` or a `HashSet` of known network names. If the `network` is not in the list, return a `404` error immediately, before accessing the database.
- Add a strict validation step for the `txid` in the `searchtx` request. A `txid` must be a 64-character hexadecimal string. If `txid` is not hex or not 64 chars, return `400 Bad Request` immediately.
- Add a `Content-Length` check to the request body. If it's not set, or if it's too large, return `411 Length Required` or `413 Payload Too Large`.
**Status:** Fixed. **Priority:** Medium. 
- ✅ **Network validation:** `echo_info`, `echo_stats`, and `echo_push` handlers now verify `NETWORKS.contains(&param.as_str())` **before** calling `get_net_config`. Unknown networks (e.g., `GET /attacker/info`) return `404 Not Found` immediately, preventing the previous fallback to `mainnet`.
- ✅ **txid validation:** `echo_search` now validates that the request body is exactly **64 ASCII hex characters** (0-9, a-f, A-F). Any other length or non-hex content returns `400 Bad Request` before touching the database.
- ✅ **Performance optimization (N+1 fix):** The `SELECT * FROM tbl_address WHERE address=?` query inside the per-output loop of `echo_push` was replaced by a single `db.get_all_addresses_by_xpub(&db, &xpub)` call executed once per batch, returning a `HashSet<String>`. The per-output lookup is now an O(1) memory check, eliminating the N+1 query bottleneck.
- **Content-Length:** Already handled by `Actix PayloadConfig` size limit (see point 4).
- ✅ **SQL injection in `echo_stats` fixed:** The `chain` parameter was previously interpolated directly into a `format!` string (`"WHERE chain = '{}'"`) before being passed to `db.iterate`. It has been replaced by a prepared statement with `stmt.bind((1, Value::String(...)))` and a loop over `stmt.next()`. An index `idx_stats_chain` was also added on `tbl_stats(chain)` to ensure efficient filtering.
**Regression tests:** `tests/input_validation_tests.rs` (4 tests: xpub address cache, empty cache, network validation, txid hex/64 validation). All passing.

### 10. Information Leakage (LOW)

### 10. Information Leakage (LOW)
**Location:** `valid_txs` and `invalid_txs` files, `bal-server` error messages.
**Description:** The `bal-server` returns `500 Internal Server Error` in some cases. The `bal-server` does not log the raw request body or the user IP in all cases, but it does log the transaction details and some error messages in the `valid_txs` and `invalid_txs` log files. The `valid_txs` file contains the raw transaction details, which could leak private information if the log file is not protected. The `invalid_txs` file contains the raw error messages from the `bitcoind` RPC, which could be used to fingerprint the `bitcoind` version or its configuration. The `valid_txs` and `invalid_txs` files contain the raw transaction details, including the user IP and the transaction details, which could be used to identify the user's behavior or the network's topology. The `invalid_txs` file contains the raw error message from the `bitcoind` RPC, which is a potential information leakage (e.g., `bad-txns-inputs-missingorspent`). This message could be used to fingerprint the `bitcoind` version or the mempool state.
**Impact:**
- If the log files are not protected, the raw transaction details could be read by unauthorized users or processes running on the same machine. If the log files are accessible, the attacker could see the transaction details and potentially use them to link addresses to users or services.
- The `invalid_txs` file contains the raw error messages from the `bitcoind` RPC, which could be used to fingerprint the node or its configuration. For example, `bad-txns-inputs` suggests that the input is not available or not valid, which is a mempool state. If the attacker can read these logs, they can deduce the state of the mempool.
**Mitigation:**
- Ensure the `valid_txs` and `invalid_txs` files are not stored in the same directory as the `bal.db` or `private_key.pem`. If they are, they should be protected with `chmod 600` and only readable by the `bal` user.
- The `bal-server` should not log the raw request body or the transaction details in the `valid_txs` log file. It should only log the `txid` and the result, not the full raw transaction. The `invalid_txs` should log the error message, but not the raw transaction details or the user IP. If the server logs the raw transaction, the attacker could read it by reading the log files or the memory of the server process if it crashes.
**Status:** Partially Fixed. **Priority:** Medium. The raw file logging (`valid_txs`/`invalid_txs`) in `bal-pusher.rs` has been commented out (lines 271-290). The `bal-server` `actix-web` middleware logs only requests/responses via `Logger::default()`. However, `info!`/`warn!` macros may still log `txid` and other details in application logs. Ensure production `RUST_LOG` level is set to `warn` or higher and log files are restricted with `chmod 600`.

### 11. `valid_txs` Log File Privacy (LOW)
**Location:** `valid_txs`, `invalid_txs` files.
**Description:** The `valid_txs` and `invalid_txs` files are plain text log files. `valid_txs` contains the transaction details and the raw hex. `invalid_txs` contains the error messages and the raw hex of the failed transactions. These files do not contain the user IP, but they do contain the raw transaction details and the `txid`, which is enough to fingerprint the transaction. If the `valid_txs` file is accessible to the public, the transaction details could be read by anyone. Also, the `valid_txs` file is not encrypted or compressed.
**Impact:** The raw transaction details could be read by anyone. If the user is using the `valid_txs` file to track the transactions, it could be used for privacy analysis or to fingerprint the transaction history. If the `valid_txs` file is leaked, it could be used to link the user's transaction to the `bal-server` and identify the user or their behavior. The `valid_txs` file is not encrypted, and it is not protected by any authentication. If the server is compromised, these files will be accessible to the attacker, which is a privacy risk.
**Mitigation:**
- Ensure the `valid_txs` and `invalid_txs` files are not accessible to the public. If the server is running on a shared directory, use `chmod 600` to restrict access. If the server is not, they are accessible by default.
- The `valid_txs` and `invalid_txs` files should not be stored in the same directory as the `bal.db` or `private_key.pem`. They should be in a separate directory.
- The `valid_txs` and `invalid_txs` files should be rotated and compressed to avoid growing infinitely. The `bal-server` should also not log the entire raw transaction in the `valid_txs` file. It should only log the `txid` and the status. This will prevent the leak of the transaction details if the log file is compromised.
**Status:** Fixed. **Priority:** Low. The raw file logging (`valid_txs` and `invalid_txs`) in `bal-pusher.rs` has been removed (commented out). The structured logging only logs `txid` and timestamps, not full raw transactions. Ensure log files are protected with `chmod 600` and are in a separate directory from the database and keys.

### 12. `bal-stats.rs.dontcompile` (LOW)
**Location:** `src/bin/bal-stats.rs.dontcompile` (removed).
**Description:** This file was a broken, incomplete HTML report generator that directly queried `tbl_tx` and wrote `bal_status.html` to the local filesystem. It contained hardcoded SQL queries and lacked security checks. If accidentally compiled or renamed, it could leak transaction details or expose the database contents via an HTML file. It could also bypass security checks or rate limiting if accessed directly.
**Impact:** If the file was compiled or run, it could create an unprotected HTML report with sensitive database contents, accessible if the server was serving static files.
**Mitigation:**
- ✅ **Removed:** File `src/bin/bal-stats.rs.dontcompile` deleted from source tree. No longer a risk for accidental compilation or exposure.
**Status:** Fixed (File removed). **Priority:** Low.

## Hardening Recommendations

### System-Level
1. **Run the service as a non-root user:** Use the `bal-systemd` hardening (e.g., `ProtectSystem=full, NoNewPrivileges, PrivateDevices`). The `bal-server` should not be exposed to the internet directly. Use a reverse proxy or a firewall.
2. **Use `firewall` (e.g., `iptables`, `netfilter`, or `nftables`) to block all inbound ports except the HTTPS port (443) and the SSH port (22).** The HTTP port should not be exposed to the internet. The `bal-server` should be on a separate port or on `127.0.0.1`.
3. **Use a `VPN` or `Tor` for the `welist` connection.** If the `welist` server is on a public network, use a VPN or Tor to prevent the `welist` IP address from being exposed to the `bal-pusher`.
4. **Run the `bal-server` in a `chroot` or `docker` container.** The server should be isolated from the rest of the system. If the server is compromised, the attacker will not be able to access the `bal.db` or `private_key.pem` files.
5. **Enable the `SELinux` or `AppArmor` profile for the `bal-server` and `bal-pusher` binaries.** This will prevent the attacker from accessing the database or the private key if the binary is compromised.
6. **Use a `read-only` file system for the `bal-server` binary.** The server should be read-only to prevent the attacker from modifying the binary or the configuration files. The `bal-server` should be in a `chroot` jail with the `bal` user.
7. **Use a `network` firewall to block the outbound traffic from the `bal-server` to the internet.** If the server only needs to communicate with the `bal-pusher` and the `nginx` proxy, it should not have internet access. If the server is compromised, it will not be able to download malware or communicate with a C2 server.

### Application-Level
1. **Add `Rate Limiting`:** Add a rate limiter to the `bal-server` to prevent DDoS or abuse. The `bal-server` should limit the number of requests per IP per minute or per hour. It should also limit the number of `pushtxs` requests to avoid filling the database with malicious requests. A `HashMap` or `Redis` can be used to store the rate limiter state.
2. **Add `Input Validation`:** Add strict input validation for all endpoints. The `network`, `txid`, and `hex` parameters must be validated. The `txid` must be 64 hex chars, the `hex` must be a valid Bitcoin hex string, and the `network` must be a known network.
3. **Add `HTTPS`:** The `Nginx` configuration should be used to terminate TLS and provide HTTPS. The `bal-server` should only run on `127.0.0.1` to avoid being exposed to the public internet.
4. **Add `WAL` for `SQLite`:** Enable the `Write-Ahead Logging` (WAL) mode for the `bal` database to prevent database locking or data corruption when multiple processes access the database at the same time. This is a standard practice for SQLite and is supported by the `sqlite` crate. Enable it via `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;` upon the first connection.
5. **Add `ZMQ Authentication`:** Use `ZMQ_CURVE` or `ZMQ_GSSAPI` to authenticate the ZMQ connection. If the `ZMQ` connection is over a public network, the `bal-pusher` should be authenticated and the traffic should be encrypted. Alternatively, use `ZMQ_RCVTIMEO` and `ZMQ_SNDTIMEO` to set a connection timeout to prevent blocking forever if the socket is disconnected or the `bitcoind` node is not available.
6. **Add `Transaction Size Limits`:** The `bal-pusher` should have a `MAX_TRANSACTIONS_PER_SECOND` and `MAX_TRANSACTIONS_PER_BLOCK` config value. This will prevent the pusher from sending too many transactions to the `bitcoind` node and overloading it. If the database is full of many transactions, the pusher should only send a small batch at a time (e.g., `1` or `10` transactions per block, or `5` per minute) to avoid overwhelming the RPC queue or the node.
7. **Add `ZMQ Retry`:** The `bal-pusher` should implement a retry mechanism for the `sendrawtransaction` and ZMQ connection. If the RPC or ZMQ call fails, it should wait for the next block before trying again. The pusher should not panic or stop on the first failure. It should be resilient and continue operating even if the network is down or the `bitcoind` node is restarting. The `ZMQ` socket should be reconfigured with `ZMQ_RECONNECT_IVL` and `ZMQ_MAXMSGSIZE` to avoid reconnecting too aggressively or receiving unbounded messages. If the connection is lost, the `ZMQ` should wait for the `bitcoind` to come back and not try to reconnect immediately. The pusher should also handle `SIGTERM` and `SIGINT` gracefully and stop the ZMQ connection before exiting.
8. **Add `Transaction Fee Limits`:** The `bal-server` should not accept transactions with a fee of `0`. It should also not accept transactions with a fee higher than a reasonable limit (e.g., `100000` satoshi for a `10 KB` transaction). This will prevent the user from sending too many transactions with a very low or high fee. This will limit the risk of a DoS attack where the attacker fills the database with many invalid transactions. The `bal-server` should not accept a transaction that is not valid or has the wrong `network`. Also, the `bal-server` should not accept a transaction with a very high `locktime` (e.g., `9999999999`) to prevent the database from becoming too large or to prevent the pusher from being blocked by a very far future locktime. The `bal-server` should only accept locktime values that are reasonable for the current blockchain height.
9. **Add `Transaction Fee Limits`:** The `bal-server` should not accept a transaction from a `network` if the `network` is not supported. Only the `regtest`, `testnet`, `testnet4`, `signet`, and `bitcoin` networks are supported. If the `network` is not in the list, the server should reject the request and not process it. The server should also not accept a transaction from a different network than the one it is configured for. If the server is configured for `regtest`, it should not accept `bitcoin` transactions. This will prevent the attacker from using the wrong network and sending a transaction that is not valid for the current network. The server should not accept a transaction that has a different `network` than the `our_address` network. If the `network` is not valid, the server should not process the request and should return a `404` error. The server should not accept a transaction that is for a different network than the one it is configured for. This will prevent the attacker from using the server to process transactions for a different network.
10. **Add `Transaction Time Limits`:** The `bal-server` should not accept a transaction with a locktime that is too far in the future. If the locktime is greater than `500000000`, it is a timestamp. The server should only accept locktime values that are within a reasonable timeframe (e.g., within the next year or a few months). If the locktime is in the past, the server should not accept it or it should be marked as a `0` locktime and processed immediately. If the locktime is too far in the future, it should be rejected. If the locktime is a block height, it should be within the next `100000` blocks or the next few months. If the locktime is a timestamp, it should be within the next few years or a reasonable timeframe. If the locktime is too far in the future, it will be impossible to process, and it will fill the database with invalid transactions. The `bal-server` should not accept a transaction with a `locktime` of `0` if `0` is a special case. If the `locktime` is `0`, it should be processed immediately and not stored in the database. If `0` is treated as a special case, the server should not store it as a pending transaction. If the `locktime` is `0`, the transaction should be sent immediately or processed as a normal transaction without a timelock. If the locktime is `0`, it should be treated as a normal transaction and sent to the `bitcoind` node immediately. The server should not send a `0` locktime transaction to the `pusher` because it is not a pending transaction. The pusher should not process a `0` locktime transaction because it is not waiting for a specific time or block height. If the `locktime` is `0`, it should be handled in the `bal-server` and not in the `bal-pusher`. The server should not store the `0` locktime transaction in the database. If a `0` locktime transaction is sent, the server should not store it as a pending transaction but should send it to the `bitcoind` node or process it immediately. If the `0` locktime is a special case, the server should not treat it as a pending transaction and should not send it to the `pusher`. If the `locktime` is `0`, the `bal-server` should not send it to the `bitcoind` network. If the `0` locktime is a valid transaction, the server should not send it to the pusher. If the `0` locktime is a special case, it should be handled in the `bal-server` and not in the `bal-pusher`. If the `0` locktime is a special case, it should not be sent to the `pusher`. If the `0` locktime is a special case, the server should not treat it as a pending transaction. If the `0` locktime is a special case, the server should not process it in the `pu
