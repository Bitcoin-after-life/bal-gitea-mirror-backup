# Project Overview

## Quick Reference
- **What this file contains:** vision, scope, system components, and mapping to existing documentation.
- **See also:** [02_glossary_and_bitcoin_domain.md](02_glossary_and_bitcoin_domain.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md)

## Vision and Scope

`bal_server` is a Rust-based Bitcoin transaction executor server. It receives raw Bitcoin transactions via HTTP, validates them, persists them in a local SQLite database, and coordinates their broadcast on-chain after a locktime condition expires. The system supports multiple Bitcoin networks (mainnet, testnet, regtest, testnet4, signet) and tracks extended public keys (xpub) for fee collection.

### Key Goals
1. **Receive and validate** raw Bitcoin transactions with locktime.
2. **Store** transactions, inputs, and outputs in a structured database.
3. **Monitor** new blocks via ZMQ and push transactions to the Bitcoin network when the locktime is satisfied.
4. **Track** derived addresses and extended public keys for fee accounting.
5. **Collect and report** statistics about the service.

## System Components

The project consists of three primary binaries and two shared libraries:

1. **`bal-server`**: Async HTTP server (hyper + tokio) that exposes the API for receiving transactions and serving statistics.
2. **`bal-pusher`**: Async daemon that listens for `hashblock` ZMQ messages and pushes pending transactions to a Bitcoin node via RPC.
4. **`lib.rs`**: Exports the shared modules `db` and `xpub`.
5. **`db.rs`**: All database operations and schema creation for SQLite `0.34.0`.
6. **`xpub.rs`**: Address derivation from xpub/zpub using BIP-84 and the `bitcoin` crate.

## Mapping to Existing Documentation

| Existing File | Subject | Covered in this KB |
|---------------|---------|-------------------|
| `README.md` | Installation, environment variables, ZMQ dependency | [`07_deployment_and_ops.md`](07_deployment_and_ops.md) |
| `RPC.md` | API endpoint specification | `05_api_reference.md` | [`05_api_reference.md`](05_api_reference.md) |
| `AGENTS.md` | Security guidelines for agents | `08_security_audit.md` | [`08_security_audit.md`](08_security_audit.md) |
