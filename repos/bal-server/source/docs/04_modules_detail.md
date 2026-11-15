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

## `url_policy.rs` (Canonical v3 Executor URL)

**Location:** `src/url_policy.rs` (identical copy in `bal-welist`)

Defines the single canonical origin form signed by v3 WeTime pings. The service
and the pusher must produce the same string; otherwise every v3 signature fails.

### Key Function

| Function | Signature | Purpose |
|---|---|---|
| `canonical_executor_url` | `pub fn canonical_executor_url(raw: &str) -> Result<String, UrlPolicyError>` | Returns `scheme://host[:port]` or a policy error. |

### Rules

1. ASCII only; raw Unicode is rejected. Use the `xn--` A-label form.
2. Scheme must be `http` or `https`.
3. No userinfo, path (other than `/`), query, or fragment.
4. No explicit default port (`:80` for http, `:443` for https); non-default
   ports are preserved.
5. No IP literal and no `localhost`/`.localhost`.
6. Host is lowercase, has no trailing dot, and uses valid LDH labels
   (1–63 bytes, total ≤253).
7. The caller must require `data.url == canonical_executor_url(data.url)`; the
   function normalizes, the caller rejects non-canonical input.

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
- **Rate Limiting:** `actix-governor` middleware with token-bucket algorithm. Uses `RealIpKeyExtractor` to identify clients by real IP behind reverse proxy (via `X-Real-IP` / `X-Forwarded-For` headers).
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
- `max_body_size`, `timeout_secs`, rate limits (`pushtxs` per sec/burst), `workers`, `max_connections`, `trusted_proxy`

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
- `WELIST_PING_SIG_VERSION` selects the signature scheme:
  - `1` (default): legacy concatenation of URL, chain, height, median time and
    block hash. The WeTime service verifies it against `/.pub_key.pem`. The
    signature authenticates liveness only: the pusher does not send `info`,
    `base_fee` or `version`, and the service refreshes `last_update` without
    persisting executor metadata. Use v2 or v3 when metadata must be published
    and authenticated.
  - `2`: canonical JSON signed with the `WELIST-PING-V2\n` prefix. The payload
    is identical to v3 except for the prefix and `sig_version`; the WeTime
    service verifies it against `/.pub_key.pem` instead of using DNSSEC or the
    onion identity key. The pusher reads `info`, `base_fee` and `version` from
    `tbl_executor_metadata`; absent metadata is signed as `null`. v2 does not
    use DNSSEC, onion keys or delegation; a `delegation` object is never sent.
  - `3`: canonical JSON signed with the `WELIST-PING-V3\n` prefix. The pusher
    reads `info`, `base_fee` and `version` from `tbl_executor_metadata` and
    signs them together with the block metadata. The lookup is keyed by the
    canonical `NetworkParams.chain` name (`bitcoin`, `testnet`, `testnet4`,
    `signet`, `regtest`), not by the overridable `db_field`. Onion pings receive
    the same metadata as DNS hosts. The WeTime service discovers the public key
    from a DNSSEC-validated `_welist-key.<host>` TXT record, from a v3 onion
    address, or from a ping key delegated by that onion identity key, so no HTTP
    callback is needed.
  - The v1 message signs the URL after trimming surrounding whitespace and
    trailing slashes, which is exactly what the service does before rebuilding
    the message. v2 and v3 sign the canonical origin computed from
    `BAL_SERVER_URL` by the shared `url_policy` module.
  - Metadata is best effort. If `tbl_executor_metadata` or the row is missing,
    v1/v2/v3 proceed with `null` metadata instead of blocking the ping. When
    `pubkey_pem` is present, the pusher verifies that it matches
    `SSL_KEY_PATH`; a malformed PEM or key mismatch skips the ping. `bal-server`
    reads the PEM once at startup and serves that cached value, so the endpoint
    and the database cannot disagree; rotating the key requires a server
    restart.
  - For v3 DNS hosts, the pusher performs one TXT consistency check before the
    first WeList ping after startup. It queries `_welist-key.<host>` through the
    system resolver without local DNSSEC validation (the service remains
    authoritative), parses `v=we1; alg=ed25519; key=<base64>`, and compares it
    with `SSL_KEY_PATH`. NXDOMAIN, empty answers, malformed records, or a
    mismatching key trigger one retry after 30 seconds; two definitive
    mismatches skip WeTime pings until restart. Timeout/SERVFAIL is transient
    and does not block. The result is cached per signing key for the process
    lifetime.
  - When onion delegation is configured, the pusher sends the certificate only
    with v3 pings and verifies at startup that it is bound to the configured
    onion host, is structurally valid, is not expired, is within
    `BAL_PUSHER_MAX_DELEGATION_DAYS` (default 366), verifies its Ed25519
    signature against the identity key embedded in the onion address, and
    delegates to the key at `SSL_KEY_PATH`. The expiry is also checked again
    on every ping, and
    `BAL_PUSHER_DELEGATION_WARN_DAYS` controls an advance warning. If a check
    fails the pusher disables delegation and signs directly, which the service
    rejects unless `SSL_KEY_PATH` is the onion identity key. A leaked key is
    stopped with `contrib/make_ping_key_revocation.py` (onion:
    identity-signed; DNS: self-signed), submitted to the WeTime
    `/ping-keys/revoke` endpoint.
- `bal-pusher.sh` defaults to `WELIST_PING_SIG_VERSION=1` for the development
  setup; a `localhost` URL cannot use the canonical URL policy required by
  v2/v3, so the dev pusher uses the legacy scheme verified against
  `/.pub_key.pem`. v2 uses canonical JSON with `/.pub_key.pem` key discovery; v3
  uses canonical JSON with a public DNS hostname or a v3 onion address.
  Development launchers
  require `-ok-dev` or `BAL_DEV=true`; otherwise they ask for interactive
  confirmation and refuse to start without a terminal. The scripts resolve their
  own directory with `${BASH_SOURCE[0]}`, so they also work when sourced from a
  tmux launcher.
- To publish the v3 public key, derive the base64 SubjectPublicKeyInfo DER:
  `openssl pkey -in private_key.pem -pubout -outform DER | base64 -w0` and
  publish it in a DNSSEC-signed TXT record named `_welist-key.<host>` with value
  `v=we1; alg=ed25519; key=<base64>`. A v3 onion URL needs no DNS record: its
  Ed25519 identity key is embedded in the address, and the recommended setup
  keeps that key offline while it certifies a dedicated ping key
  (`WELIST_PING_DELEGATION_FILE`). See
  [`10_dnssec_key_publishing.md`](10_dnssec_key_publishing.md) for the full
  operator guide.

### ZMQ Timeout Handling
- Uses `set_rcvtimeo(5000)` (5-second timeout).
- Logs a warning every ~720 consecutive timeouts (~1 hour of no blocks).
- Does not block forever or panic on connection loss.

### Configuration
All configuration is via environment variables (no config files):
- `BAL_SERVER_DB_FILE`: Path to SQLite database.
- `BAL_PUSHER_BITCOIN_DIR`: Bitcoin data directory (for cookie file path).
- `BAL_PUSHER_SEND_STATS`: Enable/disable remote stats reporting.
- `BAL_SERVER_URL`: URL of the bal-server for internal communication.
- `SSL_KEY_PATH`: Path to Ed25519 private key for signing stats.
- `WELIST_SERVER_URL`: URL to POST stats to (validated against SSRF).
- `WELIST_SKIP_URL_VALIDATION`: Bypass URL validation (for testing).
- `WELIST_PING_SIG_VERSION`: WeTime ping signature scheme (`1` legacy, `2` canonical JSON).
- `BAL_PUSHER_PREFER_IPV6`: Pin HTTP connection to IPv6 address.
- Per-network: `BAL_PUSHER_{NETWORK}_HOST`, `_PORT`, `_DIR_PATH`, `_DB_FIELD`, `_COOKIE_FILE`, `_RPC_USER`, `_RPC_PASSWORD`, `_ZMQ_HASHBLOCK`.
