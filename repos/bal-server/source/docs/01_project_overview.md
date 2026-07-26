# Project Overview

## Quick Reference
- **What this file contains:** vision, scope, system components, and mapping to existing documentation.
- **See also:** [02_glossary_and_bitcoin_domain.md](02_glossary_and_bitcoin_domain.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md)

## Vision and Scope

`bal_server` is a Rust-based Bitcoin transaction executor server (v0.3.2, edition 2024). It receives raw Bitcoin transactions via HTTP, validates them, persists them in a local SQLite database, and coordinates their broadcast on-chain after a locktime condition expires. The system supports multiple Bitcoin networks (mainnet, testnet, regtest, testnet4, signet) and tracks extended public keys (xpub) for fee collection.

### Key Goals
1. **Receive and validate** raw Bitcoin transactions with locktime.
2. **Store** transactions, inputs, and outputs in a structured database.
3. **Monitor** new blocks via ZMQ and push transactions to the Bitcoin network when the locktime is satisfied.
4. **Track** derived addresses and extended public keys for fee accounting.
5. **Collect and report** statistics about the service.

## System Components

The project consists of two binaries and one shared library:

1. **`bal-server`**: Async HTTP server (**actix-web 4.9.0** + actix-rt) that exposes the API for receiving transactions and serving statistics. Includes rate limiting via `actix-governor`.
2. **`bal-pusher`**: Async daemon (tokio) that listens for `hashblock` ZMQ messages and pushes pending transactions to a Bitcoin node via RPC.
3. **`lib.rs`**: Exports the shared modules `db`, `xpub`, and `validation`.

### Library Modules
4. **`db.rs`**: All database operations, schema creation, path validation, and WAL mode for SQLite `0.34.0`.
5. **`xpub.rs`**: Address derivation from xpub/zpub/ypub using BIP-84 and the `bitcoin` crate.
6. **`validation.rs`**: SSRF protection for the `welist` URL, blocking private/internal IP ranges.

## Feature Flags

The project uses Cargo feature flags to build each binary independently:

| Feature | Dependencies | Binary |
|---------|-------------|--------|
| `server` (default) | `actix-web`, `actix-governor`, `actix-rt`, `chrono`, `hex-conservative` | `bal-server` |
| `pusher` (default) | `zmq`, `reqwest`, `byteorder`, `base64`, `ed25519-dalek` | `bal-pusher` |

## Docker Support

Two Dockerfiles are provided:

- **`Dockerfile.release`** (recommended for production): Downloads the latest pre-built release from the Gitea server. No Rust toolchain needed. Verifies SHA-256 checksum. Supports pinning a specific version via `BAL_VERSION` build arg.
- **`Dockerfile`** (for development/custom builds): Multi-stage build using `rust:1.95-bookworm` as the builder and `debian:bookworm-slim` as the runtime. Each binary is compiled with only its required features (`--no-default-features --features server` / `--features pusher`).

Both run as a non-root `bal` user (uid 1000) with `tini` as PID 1 and include a healthcheck endpoint.

## Mapping to Existing Documentation

| Existing File | Subject | Covered in this KB |
|---------------|---------|-------------------|
| `README.md` | Installation, environment variables, ZMQ dependency, Docker | [`07_deployment_and_ops.md`](07_deployment_and_ops.md) |
| `RPC.md` | API endpoint specification | [`05_api_reference.md`](05_api_reference.md) |
| `AGENTS.md` | Security guidelines for agents | [`08_security_audit.md`](08_security_audit.md) |
