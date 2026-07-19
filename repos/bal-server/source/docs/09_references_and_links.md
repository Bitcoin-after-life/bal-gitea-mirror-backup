# References and Links

## Quick Reference
- **What this file contains:** links to source code, dependencies, and existing documentation, plus a machine-readable dependency map.
- **See also:** [INDEX.md](INDEX.md), [01_project_overview.md](01_project_overview.md), [04_modules_detail.md](04_modules_detail.md)

---

## Source Code References

| Module/Component | File Path | Key Lines/Details |
|---|---|---|
| Library | `src/lib.rs` | Exports `db` and `xpub` modules |
| Database | `src/db.rs` | SQL schema, `execute_insert`, batched inserts |
| XPub/Address Derivation | `src/xpub.rs` | `parse_xpub`, `derive_address`, `get_descriptor`, BIP-84 |
| HTTP Server | `src/bin/bal-server.rs` | Hyper + Tokio, routes, handlers, `pushtxs` logic |
| Async Pusher | `src/bin/bal-pusher.rs` | ZMQ `hashblock`, RPC + Reqwest, `send_stats` |

| Stats (broken) | `src/bin/bal-stats.rs.dontcompile` | Not compiled, incomplete HTML report generator |
| Release script | `make_release.sh` | Hardcoded token, `cargo install` |
| DB download script | `download_bal_db.sh` | `scp` from remote |
| Server dev script | `bal-server.sh` | Sources `bal-server.env`, `cargo run` |
| Pusher dev script | `bal-pusher.sh` | Sources `bal-pusher.env`, `cargo run` |
| Send transaction script | `sendtx.sh` | `bitcoin-cli` wrapper |
| Utility scripts | `lib.sh` | Colored echo functions |
| Contrib (install) | `contrib/download_and_install_bal.sh` | Nginx, Certbot, systemd setup, xpub via argument |
| Contrib (install bitcoind) | `contrib/download_and_install_bitcoincore.sh` | Bitcoind download, GPG verify, systemd, config |
| Contrib (install Tor) | `contrib/install_tor.sh` | Tor repository, `ControlPort 9051` || Systemd service | `bal-server.service` | Runs as `bal` user, `ProtectSystem`, `MemoryDenyWriteExecute` |
| Systemd service | `bitcoind.service` | `zmqpubhashblock` setup |
| Systemd service | `tbitcoind.service` | Testnet `bitcoind` |

---

## Dependency Map (from `Cargo.toml`)

| Dependency | Version | Purpose |
|---|---|---|
| `base64` | `0.22.1` | Encoding/decoding in `pushtxs` / xpub |
| `bs58` | `0.4.0` | Base58 encoding for Bitcoin addresses / xpubs |
| `bytes` | `1.2` | Byte handling for `hyper`/`reqwest` |
| `bitcoin` | `0.32.5` | Transaction parsing, `ScriptBuf`, `Address`, `Xpub`, `Transaction` |
| `bitcoincore-rpc` | `0.19.0` | RPC client for `bitcoin-cli` methods (`sendrawtransaction`, `getblockchaininfo`) |
| `bitcoincore-rpc-json` | `0.19.0` | JSON types for Bitcoin RPC responses |
| `byteorder` | `1.5.0` | Reading block timestamp from raw block header (big-endian) `u32` |
| `confy` | `0.6.1` | Loading `.toml` configuration files (default config) |
| `chrono` | `0.4.40` | `Date` and `DateTime` handling for timestamps and `report` |
| `env_logger` | `0.11.5` | Log level configuration via `RUST_LOG` environment variable |
| `hex` | `0.4.3` | Hex encoding for transaction serialization and raw bytes |
| `hex-conservative` | `0.1.1` | Hex parsing (used for Bitcoin hex strings) |
| `hyper` | `1.3.1` | Async HTTP server (features: `http1`, `server`) |
| `hyper-util` | `0.1.3` | Hyper utilities, `TokioIo` |
| `http-body-util` | `0.1` | HTTP body collection and streaming utilities |
| `log` | `0.4.21` | Logging facade (used by `env_logger`) |
| `openssl` | `0.10.74` | TLS/SSL, `vendored` feature to avoid system dependency |
| `sha2` | `0.10.8` | SHA-256 hashing (used in transaction validation or address generation) |
| `serde` | `1.0.152` | Serialization of config objects and JSON responses (`derive` feature) |
| `serde_json` | `1.0.116` | JSON parsing for HTTP request bodies and API responses |
| `sqlite` | `0.34.0` | Direct SQLite C bindings, raw SQL queries, no ORM |
| `regex` | `1.10.4` | `RegExp` parsing for URL matching (e.g., `network` regex) in `bal-server` |
| `reqwest` | `0.12.24` | HTTP client (`json` + `socks` features) for `welist` stats POST |
| `tokio` | `1` | Async runtime (`rt`, `net`, `macros`, `rt-multi-thread`) |
| `zmq` | `0.10.0` | ZeroMQ for `hashblock`/`rawblock` notifications |

---

## Mapping to Existing Documentation

| Existing File | Description | Replaced/Managed By KB |
|---|---|---|
| `README.md` | Installation, environment variables, ZMQ dependency | `07_deployment_and_ops.md` |
| `RPC.md` | API endpoint specification (HTTP methods, paths) | `05_api_reference.md` |
| `AGENTS.md` | Security guidelines, audit rules, baseline commands | `08_security_audit.md` |
| `update` | Irrelevant saved conversation (Diesel/Axum) | Ignored, not mapped |
| `valid_txs` / `invalid_txs` | Logs of past transaction push results | `08_security_audit.md` (Information Leakage) |
| `Cargo.toml` | Dependency versions and features | `09_references_and_links.md` (Dependency Map) |
| `bal-server.service` | `systemd` unit file | `07_deployment_and_ops.md` (Systemd) |
| `bitcoind.service` | `systemd` unit for `bitcoin` node | `07_deployment_and_ops.md` (Systemd) |
| `tbitcoind.service` | `systemd` unit for testnet node | `07_deployment_and_ops.md` (Systemd) |
| `bal-server.env` | `bal-server` environment variables | `07_deployment_and_ops.md` (Environment Variables) |
| `bal-pusher.env` | `bal-pusher` environment variables | `07_deployment_and_ops.md` (Environment Variables) |
| `bal-server.sh` | Dev server startup script | `07_deployment_and_ops.md` (Bash Scripts) |
| `bal-pusher.sh` | Dev pusher startup script | `07_deployment_and_ops.md` (Bash Scripts) |
| `sendtx.sh` | Test transaction sender | `07_deployment_and_ops.md` (Bash Scripts) |
| `make_release.sh` | Release script with hardcoded secret | `08_security_audit.md` (Secret Leakage) |
| `download_bal_db.sh` | `scp` from remote | `07_deployment_and_ops.md` (Bash Scripts) |
| `generate_keys.sh` | Generate public key from `private_key.pem` | `07_deployment_and_ops.md` (Bash Scripts) |
| `public_key.pem` | Ed25519 public key for stats verification | `05_api_reference.md` (GET `/.pub_key.pem` endpoint) |
| `private_key.pem` / `privkey.pem` / `ec.key` / `chiave_privata.key` | Private keys for stats signing | `08_security_audit.md` (Secret Leakage) |
| `contrib/download_and_install_bal.sh` | Full deployment setup | `07_deployment_and_ops.md` (Nginx, SSL) and `08_security_audit.md` (Hardcoded Secret) |
| `contrib/download_and_install_bitcoincore.sh` | Bitcoin Core install/verify | `07_deployment_and_ops.md` (Systemd) |
| `contrib/install_tor.sh` | Tor installation script | `07_deployment_and_ops.md` (Tor) |
| `contrib` | Various helper scripts | `07_deployment_and_ops.md` |

---
