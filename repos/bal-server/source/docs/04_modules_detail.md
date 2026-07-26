# Module Details

## Quick Reference
- **What this file contains:** detailed analysis of each Rust module and binary, including source code references.
- **See also:** [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [09_references_and_links.md](09_references_and_links.md)

---

## `lib.rs`

**Location:** `src/lib.rs`

This is the root of the library crate. It exports three public modules:
- `pub mod db;` — the database interface
- `pub mod validation;` — SSRF URL validation
- `pub mod xpub;` — the extended public key utilities

It contains no application logic.

---

## `db.rs` (Database Interface)

**Location:** `src/db.rs`

This module contains all the logic for interacting with the SQLite database.

### Key Functions

| Function | Signature | Purpose |
|---|---|---|
| `open_db` | `pub fn open_db(path: &str) -> Result<Connection, String>` | Validates path (blocks `..` traversal, forbidden system dirs, symlinks), opens SQLite, sets `busy_timeout=5000`, retries WAL mode up to 5 times, sets `synchronous=NORMAL`. |
| `create_database` | `pub fn create_database(db: &Connection)` | Creates all tables and indexes (idempotent via `IF NOT EXISTS`). |
| `check_duplicate_txids` | `pub fn check_duplicate_txids(db: &Connection, txids: &[String]) -> Result<HashSet<String>, Error>` | Batch check which txids already exist. Chunks in groups of 500 for SQLite parameter limit safety. |
| `insert_xpub` | `pub fn insert_xpub(db: &Connection, network: &str, xpub: &str)` | INSERT OR IGNORE into tbl_xpub. |
| `get_last_used_address_by_ip` | `pub fn get_last_used_address_by_ip(db: &Connection, network: &String, xpub: &String, address: &String) -> Option<String>` | Finds most recent address previously assigned to a remote IP for an xpub. |
| `get_next_address_index` | `pub fn get_next_address_index(db: &Connection, network: &String, xpub: &String) -> (i64, i64)` | Atomically increments `path_idx` and returns `(xpub_id, new_index)` using `RETURNING`. |
| `save_new_address` | `pub fn save_new_address(db: &Connection, xpub: i64, address: &String, path: &String, remote_addr: &String)` | INSERT into tbl_address. |
| `execute_insert` | `pub fn execute_insert(db: &Connection, sqltxs: String, ptx: Vec<(usize, Value)>, sqlinp: String, pinp: Vec<(usize, Value)>, sqlout: String, pout: Vec<(usize, Value)>) -> Result<(), Error>` | Executes a transaction: BEGIN, insert txs, insert inputs, insert outputs, COMMIT (with ROLLBACK on error). |
| `get_total_transaction_number` | `pub fn get_total_transaction_number(db: Connection, network: &String) -> Result<i64, Error>` | Counts transactions for a network. |
| `get_all_addresses_by_xpub` | `pub fn get_all_addresses_by_xpub(db: &Connection, xpub: &str) -> Result<HashSet<String>, Error>` | Fetches all addresses for an xpub via JOIN on tbl_xpub/tbl_address. Used for O(1) fee validation in the push handler. |

### Design Notes
- All SQL queries use parameterized statements (`?` placeholders with `bind()`). No string formatting is used for user-controlled values.
- The `open_db` function validates paths before opening, rejecting directory traversal, system directories, and symlinks.
- WAL mode (`PRAGMA journal_mode=WAL`) is enabled with retry logic for concurrent access safety.

---

## `xpub.rs` (Extended Public Key Utilities)

**Location:** `src/xpub.rs`

This module handles the derivation of Bitcoin addresses from extended public keys (xpub/zpub/ypub) and the creation of P2WPKH descriptors with Bitcoin Core checksums.

### Key Functions

| Function | Signature | Purpose |
|---|---|---|
| `new_address_from_xpub` | `pub fn new_address_from_xpub(zpub: &str, index: i64, network: Network) -> Result<(String, String), Box<dyn std::error::Error>>` | Derives a P2WPKH (native SegWit) address at path `m/0/{index}` from an xpub. Returns `(address, path)`. |
| `get_bitcoincore_descriptor` | `pub fn get_bitcoincore_descriptor(xpub: &str) -> String` | Generates a Bitcoin Core descriptor with checksum (e.g., `wpkh([fingerprint/84h/0h/0h]xpub/0/*)#checksum`). |
| `calculate_fingerprint` | `pub fn calculate_fingerprint(tpub: &str) -> Result<String, String>` | Returns the hex fingerprint of an xpub (converts to standard xpub first). |

### Private Functions
- `poly_mod(c, val)` / `calc_checksum(desc)` — Bitcoin Core descriptor checksum calculation.
- `convert_xpub(xpub)` — Detects prefix (xpub/ypub/zpub or tpub/vpub/upub) and converts to target format.
- `base58check_decode(s)` / `base58check_encode(data)` — Base58Check encoding/decoding.
- `convert_to(zpub, prefix)` — Converts xpub between different prefix formats.

### Supported Prefixes
| Prefix | Type | Network |
|--------|------|---------|
| `xpub` | Legacy P2PKH | Mainnet |
| `ypub` | Nested SegWit P2SH-P2WPKH | Mainnet |
| `zpub` | Native SegWit P2WPKH | Mainnet |
| `tpub` | Legacy P2PKH | Testnet |
| `vpub` | Nested SegWit | Testnet |
| `upub` | Nested SegWit | Regtest |

### Dependencies
- `bitcoin::bip32::{DerivationPath, Xpub}`
- `bitcoin::key::Secp256k1`
- `bitcoin::{Address, Network, ScriptBuf, WPubkeyHash}`
- `sha2::{Digest, Sha256}`

---

## `validation.rs` (SSRF Protection)

**Location:** `src/validation.rs`

This module provides URL validation to prevent SSRF attacks via the `welist` stats reporting feature.

### Key Functions

| Function | Signature | Purpose |
|---|---|---|
| `is_valid_welist_url` | `pub fn is_valid_welist_url(url_str: &str) -> bool` | Validates a URL against SSRF: checks scheme is HTTPS, blocks localhost/loopback/private/link-local/multicast/unspecified IPs for both IPv4 and IPv6. |

### Validation Rules
1. URL must be well-formed and parsable.
2. Scheme must be `https://` (plain HTTP is rejected).
3. Host must not be `localhost`, `127.0.0.1`, `::1`, or any loopback/private/link-local/multicast/unspecified IP address.
4. IPv4 private RFC1918 ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16) and AWS metadata link-local (169.254.169.254) are blocked.
5. IPv6 Unique Local (fc00::/7) and link-local (fe80::/10) are blocked.

### Inline Tests
8 unit tests cover valid domains, invalid schemes, localhost/loopback, private IPs, unspecified/multicast, IPv6 link-local, IPv6 unique local, malformed URLs, and valid public IPs.

---

## `bal-server.rs` (HTTP Server / API)

**Location:** `src/bin/bal-server.rs`

The main application binary that provides an async HTTP server.

### Architecture
- **Runtime:** `actix-web 4.9.0` with `actix-rt` (`#[actix_web::main]`).
- **Rate Limiting:** `actix-governor` middleware with token-bucket algorithm per endpoint.
- **Response Compression:** `actix_web::middleware::Compress`.
- **Request Logging:** `actix_web::middleware::Logger::default()`.
- **Shared State:** `Arc<Mutex<Connection>>` for database access, `MyConfig` for configuration.

### Configuration Structs

**`MyConfig`** (server configuration):
- `regtest`, `signet`, `testnet`, `testnet4`, `mainnet`: `NetConfig` per network
- `info`, `bind_address`, `bind_port`, `db_file`, `pub_key_path`, `expose_stats`

**`NetConfig`** (per-network):
- `address` (xpub or address), `fixed_fee` (sats), `xpub` (bool), `network` (bitcoin::Network), `name`, `enabled`

**`ActixConfig`** (server tuning):
- `max_body_size`, `timeout_secs`, per-endpoint rate limits (`pushtxs`, `searchtx`, `info`, `default`), `workers`, `max_connections`

### Key Routes
| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/` | `echo_home` | Returns `cfg.info` string |
| GET | `/.pub_key.pem` | `echo_pub_key` | Returns public key PEM file |
| GET | `/version` | `echo_version` | Returns VERSION constant (0.3.2) |
| GET | `/{network}/info` | `echo_info` | Returns `InfoResponse` JSON. In xpub mode, derives/returns per-IP address |
| GET | `/{network}/stats` | `echo_stats` | Returns `Vec<StatsResponse>` JSON (requires `expose_stats=true`) |
| POST | `/{network}/pushtxs` | `echo_push` | Accepts newline-separated raw tx hex. 3-phase: parse (no lock), check duplicates (lock), insert (lock) |
| POST | `/searchtx` | `echo_search` | Searches by txid (body = 64 hex chars). Returns status, tx, our_address, our_fees, reqid |

### Handler Details

**`echo_info`**: If xpub mode is enabled, first checks `get_last_used_address_by_ip` for an existing address for that IP. If none, atomically claims next index via `get_next_address_index`, derives address via `new_address_from_xpub`, and saves it. Two separate DB lock acquisitions (lookup + save) with CPU-bound derivation in between (no lock held).

**`echo_push`**: Three-phase approach:
1. Load all known addresses (for xpub validation) with DB lock, release lock
2. Parse all transactions from request body (CPU-bound, no lock) using `parse_request_transactions`
3. Batch check duplicates with DB lock, release lock
4. Build bulk INSERT statements using `UNION ALL SELECT` and execute in single transaction

**`parse_request_transactions`**: Splits body by newlines, hex-decodes each line, deserializes via `consensus::deserialize`, computes txid/wtxid/ntxid, checks if any output matches the expected address (or is in known_addresses for xpub mode) with amount >= fixed_fee.

### Error Handling
All `unwrap()`/`expect()` calls have been replaced with safe `match`/`if let` error propagation, returning appropriate HTTP status codes (400, 404, 500). The server does not panic on untrusted input.

### Static Public Key (`/.pub_key.pem`)
The server serves a static `public_key.pem` file. The corresponding `privkey.pem` is used by the pusher to sign statistics before sending them to the `welist` server.

---

## `bal-pusher.rs` (Async Transaction Pusher)

**Location:** `src/bin/bal-pusher.rs`

This is the async daemon that monitors the blockchain and pushes pending transactions.

### Architecture
- **Runtime:** `tokio::main` with `rt-multi-thread`.
- **ZMQ:** `zmq::Context` with a `SUB` socket. Subscribes to all topics. Uses `set_rcvtimeo(5000)` for 5-second receive timeout.
- **RPC:** `bitcoincore-rpc` client. Tries username/password auth first, falls back to cookie file auth.
- **HTTP Client:** `reqwest` with `json` and `socks` features. Sends Ed25519-signed JSON POST to the `welist` server.
- **IPv6 Preference:** Optional `BAL_PUSHER_PREFER_IPV6` flag pins the HTTP connection to the first IPv6 address.

### Key Logic
1. On startup and every `hashblock` message, it calls `main_result()`.
2. `main_result` creates a `bitcoincore-rpc` client. If it fails, it logs an error and returns (no panic).
3. It fetches `getblockchaininfo` to get `mediantime` and `blocks` height.
4. It queries the database for transactions with `status=0` and locktime satisfied (block height < best block, or timestamp < mediantime for timestamps > `LOCKTIME_THRESHOLD`).
5. For each pending transaction, it calls `sendrawtransaction`.
6. If `send_stats` is enabled, it collects statistics, signs them with `privkey.pem`, and sends them to the configured `welist` URL via `reqwest`.
7. It updates the database with the new status (`1` = sent, `2` = failed with `push_err`).

### Statistics Reporting
- Statistics are aggregated from the database (total, waiting, sent, failed, profits, unique inputs).
- The chain name is validated (alphanumeric, `-`, `_` only).
- Stats are inserted into `tbl_stats` with `ON CONFLICT(chain) DO UPDATE`.
- The stats payload is signed with Ed25519 and POSTed to `{welist_url}/ping`.
- The `WELIST_SERVER_URL` is validated via `is_valid_welist_url()` before sending (can be bypassed with `WELIST_SKIP_URL_VALIDATION=true`).

### ZMQ Timeout Handling
- Uses `set_rcvtimeo(5000)` (5-second timeout).
- Logs a warning every ~720 consecutive timeouts (~1 hour of no blocks).
- Does not block forever or panic on connection loss.

### Configuration
All configuration is via environment variables (no config files):
- `BAL_PUSHER_DB_FILE`: Path to SQLite database.
- `BAL_PUSHER_BITCOIN_DIR`: Bitcoin data directory (for cookie file path).
- `BAL_PUSHER_SEND_STATS`: Enable/disable remote stats reporting.
- `BAL_SERVER_URL`: URL of the bal-server for internal communication.
- `SSL_KEY_PATH`: Path to Ed25519 private key for signing stats.
- `WELIST_SERVER_URL`: URL to POST stats to (validated against SSRF).
- `WELIST_SKIP_URL_VALIDATION`: Bypass URL validation (for testing).
- `BAL_PUSHER_PREFER_IPV6`: Pin HTTP connection to IPv6 address.
- Per-network: `BAL_PUSHER_{NETWORK}_HOST`, `_PORT`, `_DIR_PATH`, `_DB_FIELD`, `_COOKIE_FILE`, `_RPC_USER`, `_RPC_PASSWORD`, `_ZMQ_HASHBLOCK`.
