# Module Details

## Quick Reference
- **What this file contains:** detailed analysis of each Rust module and binary, including source code references.
- **See also:** [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [09_references_and_links.md](09_references_and_links.md)

---

## `lib.rs`

**Location:** `src/lib.rs`

This is the root of the library crate. It simply exports two public modules:
- `pub mod db;` — the database interface
- `pub mod xpub;` — the extended public key utilities

It contains no application logic.

---

## `db.rs` (Database Interface)

**Location:** `src/db.rs`

This module contains all the logic for interacting with the SQLite database.

### Key Functions

- `create_table`: Creates the full database schema if it does not exist. See `src/db.rs` for the `CREATE TABLE` statements.
- `execute_insert`: A batched, atomic SQL wrapper function that performs multiple insert operations inside a transaction.
- `insert_tx`: Inserts a transaction into `tbl_tx`.
- `insert_inp`: Inserts an input into `tbl_inp`.
- `insert_out`: Inserts an output into `tbl_out`.
- `insert_xpub`: Inserts an xpub into `tbl_xpub`.
- `insert_address`: Inserts a new derived address into `tbl_address`.
- `get_pending_txs`: Queries `tbl_tx` for transactions with `status=0` and valid locktime conditions.
- `update_tx_status`: Updates `status` to `1` (sent) or `2` (failed) after a broadcast attempt.
- `get_stats`: Aggregates statistics for the `tbl_stats` table.
- `get_address_by_ip`: A query that joins `tbl_address` with `tbl_xpub` to find addresses by IP for rate limiting or reuse logic.

### Design Notes
SQL queries are built using `format!` in many places. The `execute_insert` function attempts to batch inserts to reduce transaction overhead, but this is dependent on the SQLite version.

---

## `xpub.rs` (Extended Public Key Utilities)

**Location:** `src/xpub.rs`

This module handles the derivation of Bitcoin addresses from extended public keys (xpub/zpub) and the creation of P2WPKH descriptors.

### Key Functions

- `parse_xpub`: Parses a Base58-encoded xpub/zpub string into a `bitcoin::bip32::Xpub`.
- `derive_address`: Derives a P2WPKH (Bech32) address at a given address index from the xpub. Uses the BIP-84 path (`m/84'/coin_type'/account'/0/index`). Uses `Secp256k1` from the `secp256k1` crate for elliptic curve math.
- `get_descriptor`: Generates a Bitcoin descriptor string for the xpub (e.g., `wpkh(.../0/*)`), which is useful for wallet integration.
- `checksum_verify`: Verifies the Base58 checksum of an xpub/zpub string to prevent data corruption during entry.

### Dependencies
- `bitcoin::bip32::Xpub`
- `secp256k1::Secp256k1`
- `bs58` for Base58 decoding
- `bitcoin::Address::p2wpkh` for address creation

---

## `bal-server.rs` (HTTP Server / API)

**Location:** `src/bin/bal-server.rs`

The main application binary that provides an async HTTP server.

### Architecture
- **Runtime:** `tokio::main` with `rt-multi-thread`.
- **HTTP Framework:** `hyper` (low-level) + `hyper-util` + `http-body-util`. Each connection is spawned as a new `tokio::task`.
- **Routing:** Routes are matched using path regex and a simple match on the HTTP method. The router is implemented manually in `main`.

### Key Routes (implemented in source code)
- `GET /`, `GET /version`: Returns static strings (name and version).
- `GET /.pub_key.pem`: Returns the Ed25519 public key PEM file for signature verification.
- `GET /:network/info`: Returns JSON with fee, address, and chain info. Networks: `bitcoin`, `testnet`, `testnet4`, `signet`, `regtest`.
- `GET /:network/stats`: Returns per-chain statistics if `expose_stats` is enabled.
- `POST /:network/pushtxs`: Accepts one or more raw hex transactions. It validates them, checks the fee output to the `our_address` for that network, and stores the transaction in the database. See `src/bin/bal-server.rs` for the `pushtxs` request body parsing logic.
- `POST /searchtx`: Accepts a txid in the request body and returns the transaction details, status, and fee breakdown.

### Configuration
- The server reads environment variables and/or a config file (`confy`). Default config is hardcoded for `regtest` development.
- `db_file`: The path to the SQLite database (e.g., `bal.db`).
- `bind_address`: The address to listen on (e.g., `127.0.0.1:3031`).
- `expose_stats`: A boolean flag to enable/disable the stats endpoint.

### Error Handling
- **WARNING:** This binary uses `unwrap()` and `expect()` on many critical paths (e.g., `sqlite::open`, `Regex::new`, `req.collect()`). A malformed request could crash the async task or even the entire runtime. This is a known vulnerability.

### Static Public Key (`/.pub_key.pem`)
The server serves a static `public_key.pem` file. The corresponding private key (`privkey.pem`) is used by the pusher to sign statistics before sending them to the `welist` server. This file is located in the project root directory.

---

## `bal-pusher.rs` (Async Transaction Pusher)

**Location:** `src/bin/bal-pusher.rs`

This is the async daemon that monitors the blockchain and pushes pending transactions.

### Architecture
- **Runtime:** `tokio::main`.
- **ZMQ:** `zmq::Context` with a `SUB` socket that listens to `tcp://127.0.0.1:28332` (or similar per-network port). The topic is `hashblock` (32-byte block hash).
- **RPC:** It uses the `bitcoincore-rpc` client to call `getblockchaininfo` (to get the `mediantime`) and `sendrawtransaction` for each transaction.
- **HTTP Client:** `reqwest` with the `json` feature. It sends a signed JSON POST to the `welist` server.

### Key Logic
1. On every `hashblock` message, it calls `main_result()`.
2. `main_result` creates a `bitcoincore-rpc` client. If it fails, it **panics** (`panic!("impossible to get client {}", e)`), crashing the entire process.
3. It fetches `getblockchaininfo` to get the `mediantime`.
4. It queries the database for transactions with `status=0` and `locktime < mediantime`.
5. For each pending transaction, it calls `sendrawtransaction`.
6. If `send_stats` is enabled, it collects statistics, signs them with `privkey.pem`, and sends them to the configured `welist` URL via `reqwest`.
7. It updates the database with the new status.

### Configuration
- `zmq_endpoint`: The ZMQ endpoint (e.g., `tcp://127.0.0.1:28332`).
- `rpc_url`: The URL of the Bitcoin RPC (e.g., `http://127.0.0.1:18443`).
- `rpc_auth`: `user_pass` or `cookie_file`. The cookie path is constructed from the `HOME` environment variable (e.g., `~/.bitcoin/.cookie`).
- `send_stats`: A boolean that enables the remote server reporting.
- `welist_url`: The URL to POST to.
- `ssl_key_path`: The path to the Ed25519 private key (`privkey.pem`) for signing stats.

---