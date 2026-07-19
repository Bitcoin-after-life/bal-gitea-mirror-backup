# Architecture and Data Flow

## Quick Reference
- **What this file contains:** high-level architecture, data flow, state machine, and error handling strategy.
- **See also:** [01_project_overview.md](01_project_overview.md), [04_modules_detail.md](04_modules_detail.md), [05_api_reference.md](05_api_reference.md), [06_database_schema.md](06_database_schema.md)

## High-Level Architecture

```
User
 |
 | HTTP POST (raw hex transactions)
 v
+-----------------+
| bal-server      |  (hyper + tokio, async)
| (src/bin/bal-server.rs) |
+-----------------+
 | SQLite insert (db.rs)
 v
 bal.db
 | (transactions with status=0, waiting locktime)
 |
 | ZMQ (hashblock / rawblock)
 v
+-----------------+
+-----------------+
| bal-pusher      |  (async, ZMQ + RPC + reqwest)
| (src/bin/bal-pusher.rs)
+-----------------+
+-----------------+
 | bitcoincore-rpc
 | sendrawtransaction
 v
 Bitcoin Network
```

## Data Flow (Transaction Lifecycle)

1. **Submission**: A client sends one or more raw hex transactions to the `pushtxs` endpoint.
2. **Validation**: The `bal-server` parses each transaction using `bitcoin::Transaction`. It checks for the fee output, extracts inputs/outputs, and validates the locktime.
3. **Storage**: Valid transactions are stored in `tbl_tx` with `status = 0` (waiting). The inputs and outputs are stored in `tbl_inp` and `tbl_out`.
4. **Monitoring**: The `bal-pusher` listens to the ZMQ `hashblock` topic. When a new block is detected, it fetches the `mediantime` via `getblockchaininfo` (or via the block's median time in the enhanced version).
5. **Evaluation**: The pusher queries the database for transactions with `status=0` and compares their locktime to the current blockchain median time.
6. **Broadcast**: If the locktime is satisfied, the pusher sends the transaction via `sendrawtransaction` and updates the status to `1` (sent) or `2` (failed if the RPC returns an error).
7. **Statistics**: The pusher periodically sends statistics to a remote server (`welist`) using a signed POST request. The server also collects stats on its own.

## State Machine

```
[Submitted] -> status=0 (waiting)
      |
      | locktime satisfied
      v
[Push attempt] -> status=1 (sent) or status=2 (failed)
```

The `status` field in `tbl_tx` is an integer:
- `0`: Waiting for locktime.
- `1`: Successfully sent to the network.
- `2`: Failed (e.g., RPC error `-25 bad-txns-inputs-missingorspent`).

## Error Handling Strategy

The codebase is currently inconsistent with error handling. The `bal-server` uses `unwrap()` on many critical paths (e.g., `sqlite::open`, `Regex::new`, body parsing), which causes panics in the async runtime. The `bal-pusher` also panics on RPC connection failures (`panic!("impossible to get client {}", e)`) which crashes the entire ZMQ loop.

## Logging and Monitoring

The project uses `env_logger` and `log`. By default, `RUST_LOG=info` is set. The `bal-pusher` sends signed statistics to a remote server. The server exposes a `stats` endpoint (`/<network>/stats`) if `expose_stats` is enabled.
