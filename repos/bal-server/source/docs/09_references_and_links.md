# References and Links

## Quick Reference
- **What this file contains:** links to source code, dependencies, and existing documentation, plus a machine-readable dependency map.
- **See also:** [INDEX.md](INDEX.md), [01_project_overview.md](01_project_overview.md), [04_modules_detail.md](04_modules_detail.md)

---

## Source Code References

| Module/Component | File Path | Key Details |
|---|---|---|
| Library | `src/lib.rs` | Exports `db`, `validation`, `xpub` modules |
| Database | `src/db.rs` | SQL schema, `open_db`, `execute_insert`, WAL mode, path validation |
| XPub/Address Derivation | `src/xpub.rs` | `new_address_from_xpub`, `get_bitcoincore_descriptor`, `calculate_fingerprint`, BIP-84 |
| SSRF Validation | `src/validation.rs` | `is_valid_welist_url`, blocks private/internal IPs |
| HTTP Server | `src/bin/bal-server.rs` | Actix-web 4.9.0, rate limiting, 7 routes, `Arc<Mutex<Connection>>` |
| Async Pusher | `src/bin/bal-pusher.rs` | ZMQ `hashblock`, RPC + Reqwest, Ed25519 signing, `calculate_stats` |

| Script/Config | Path | Purpose |
|---|---|---|
| Release script | `make_release.sh` | Builds release, creates tag, uploads to Gitea (token from `.env`) |
| DB download script | `download_bal_db.sh` | `scp` from remote |
| Server dev script | `bal-server.sh` | Sources `bal-server.env`, `cargo run` |
| Pusher dev script | `bal-pusher.sh` | Sources `bal-pusher.env`, `cargo run` |
| Send transaction script | `sendtx.sh` | `bitcoin-cli` wrapper for testing |
| Utility scripts | `lib.sh` | Colored echo functions |
| Contrib (install) | `contrib/download_and_install_bal.sh` | Nginx, Certbot, systemd setup |
| Contrib (install bitcoind) | `contrib/download_and_install_bitcoincore.sh` | Bitcoind download, GPG verify, systemd, config |
| Contrib (install Tor) | `contrib/install_tor.sh` | Tor repository, `ControlPort 9051` |
| Nginx template | `contrib/nginx/bal-server.conf` | TLS termination, security headers, rate limiting |
| Dockerfile | `Dockerfile` | Multi-stage build from source, non-root user, tini, healthcheck |
| Dockerfile.release | `Dockerfile.release` | Download latest release from Gitea, SHA-256 verification |

| Systemd Service | File | Purpose |
|---|---|---|
| `bal-server.service` | `bal-server.service` | Runs as `bal` user, hardened |
| `bitcoind.service` | `bitcoind.service` | `zmqpubhashblock` setup for mainnet |
| `tbitcoind.service` | `tbitcoind.service` | Testnet `bitcoind` |

---

## Dependency Map (from `Cargo.toml`)

### Core Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| `bitcoin` | `0.32.5` | Transaction parsing, `ScriptBuf`, `Address`, `Xpub`, `Transaction` |
| `bitcoincore-rpc` | `0.19.0` | RPC client (`sendrawtransaction`, `getblockchaininfo`) |
| `bitcoincore-rpc-json` | `0.19.0` | JSON types for Bitcoin RPC responses |
| `sqlite` | `0.34.0` | Direct SQLite C bindings, raw SQL queries |
| `serde` | `1.0.152` | Serialization (`derive` feature) |
| `serde_json` | `1.0.116` | JSON parsing for HTTP request/response |
| `tokio` | `1` | Async runtime (`rt`, `net`, `macros`, `rt-multi-thread`) |
| `sha2` | `0.10.8` | SHA-256 hashing |
| `bs58` | `0.4.0` | Base58 encoding for xpubs |
| `hex` | `0.4.3` | Hex encoding for transaction serialization |
| `regex` | `1.10.4` | Regular expressions |
| `log` | `0.4.21` | Logging facade |
| `env_logger` | `0.11.5` | Log level via `RUST_LOG` |
| `url` | `2` | URL parsing for SSRF validation |

### Server-Only Dependencies (feature: `server`)

| Dependency | Version | Purpose |
|---|---|---|
| `actix-web` | `4.9.0` | Async HTTP server framework |
| `actix-governor` | `0.6.0` | Rate limiting middleware (token-bucket) |
| `actix-rt` | `2.10.0` | Actix async runtime |
| `chrono` | `0.4.40` | Date/Time handling for timestamps |
| `hex-conservative` | `0.1.1` | Hex parsing for Bitcoin hex strings |

### Pusher-Only Dependencies (feature: `pusher`)

| Dependency | Version | Purpose |
|---|---|---|
| `zmq` | `0.10.0` | ZeroMQ for `hashblock` notifications |
| `reqwest` | `0.12.24` | HTTP client (`json` + `socks` features) for `welist` stats POST |
| `byteorder` | `1.5.0` | Reading block timestamp from raw block header |
| `base64` | `0.22.1` | Encoding/decoding for Ed25519 signatures |
| `ed25519-dalek` | `2` | Ed25519 signing (`pem` + `pkcs8` features) |
| `bytes` | `1.2` | Byte handling |

---

## Test Files

| Test File | Tests | Coverage |
|-----------|-------|----------|
| `tests/sql_injection_tests.rs` | 3 | SQL injection prevention |
| `tests/panic_regression_tests.rs` | 2 | Panic recovery, NULL handling |
| `tests/ssrf_tests.rs` | 4+ | SSRF URL validation |
| `tests/secret_leakage_tests.rs` | 3 | Secret protection, .gitignore |
| `tests/input_validation_tests.rs` | 4 | Input validation, address caching |
| `tests/db_path_validation.rs` | 5 | DB path validation, WAL mode |
| `tests/test_endpoints.sh` | Bash | Integration tests for HTTP endpoints |

---

## Mapping to Existing Documentation

| Existing File | Description | Replaced/Managed By KB |
|---|---|---|
| `README.md` | Installation, env vars, Docker, per-network config | `07_deployment_and_ops.md` |
| `RPC.md` | API endpoint specification (HTTP methods, paths) | `05_api_reference.md` |
| `AGENTS.md` | Security guidelines, audit rules, baseline commands | `08_security_audit.md` |
| `Cargo.toml` | Dependency versions and features | `09_references_and_links.md` (Dependency Map) |
| `bal-server.service` | `systemd` unit file | `07_deployment_and_ops.md` (Systemd) |
| `bitcoind.service` | `systemd` unit for `bitcoin` node | `07_deployment_and_ops.md` (Systemd) |
| `tbitcoind.service` | `systemd` unit for testnet node | `07_deployment_and_ops.md` (Systemd) |
| `bal-server.env` | `bal-server` environment variables | `07_deployment_and_ops.md` (Environment Variables) |
| `bal-pusher.env` | `bal-pusher` environment variables | `07_deployment_and_ops.md` (Environment Variables) |
| `bal-server.sh` | Dev server startup script | `07_deployment_and_ops.md` (Bash Scripts) |
| `bal-pusher.sh` | Dev pusher startup script | `07_deployment_and_ops.md` (Bash Scripts) |
| `sendtx.sh` | Test transaction sender | `07_deployment_and_ops.md` (Bash Scripts) |
| `make_release.sh` | Release script | `07_deployment_and_ops.md` (Bash Scripts) |
| `download_bal_db.sh` | `scp` from remote | `07_deployment_and_ops.md` (Bash Scripts) |
| `generate_keys.sh` | Generate public key from `privkey.pem` | `07_deployment_and_ops.md` (Bash Scripts) |
| `public_key.pem` | Ed25519 public key for stats verification | `05_api_reference.md` (GET `/.pub_key.pem` endpoint) |
| `privkey.pem` | Private key for stats signing | `08_security_audit.md` (Secret Leakage) |
| `contrib/download_and_install_bal.sh` | Full deployment setup | `07_deployment_and_ops.md` (Nginx, SSL) |
| `contrib/download_and_install_bitcoincore.sh` | Bitcoin Core install/verify | `07_deployment_and_ops.md` (Systemd) |
| `contrib/install_tor.sh` | Tor installation script | `07_deployment_and_ops.md` (Tor) |
| `contrib/nginx/bal-server.conf` | Nginx TLS config template | `07_deployment_and_ops.md` (Nginx) |
| `Dockerfile` | Multi-stage Docker build | `07_deployment_and_ops.md` (Docker) |

---

## Build and Release Profile

```toml
[profile.release]
opt-level = "z"      # Optimize for binary size
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for maximum optimization
strip = true         # Strip debug symbols
panic = "abort"      # Abort on panic (smaller binary)
```

## Feature Flags

```toml
[features]
default = ["server", "pusher"]
server = ["dep:actix-web", "dep:actix-governor", "dep:actix-rt", "dep:chrono", "dep:hex-conservative"]
pusher = ["dep:zmq", "dep:reqwest", "dep:byteorder", "dep:base64", "dep:ed25519-dalek"]
```

Build individual binaries:
```bash
cargo build --bin bal-server --features server
cargo build --bin bal-pusher --features pusher
```
