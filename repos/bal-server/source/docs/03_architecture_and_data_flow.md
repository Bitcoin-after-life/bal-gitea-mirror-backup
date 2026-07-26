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
| bal-server      |  (actix-web 4.9.0 + actix-governor, async)
| (src/bin/bal-server.rs) |
+-----------------+
 | SQLite insert (db.rs, Arc<Mutex<Connection>>)
 v
 bal.db (WAL mode)
 | (transactions with status=0, waiting locktime)
 |
 | ZMQ (hashblock)
 v
+-----------------+
| bal-pusher      |  (tokio, ZMQ + RPC + reqwest)
| (src/bin/bal-pusher.rs)
+-----------------+
 | bitcoincore-rpc
 | sendrawtransaction
 v
 Bitcoin Network
```

## Data Flow (Transaction Lifecycle)

1. **Submission**: A client sends one or more raw hex transactions to the `pushtxs` endpoint (newline-separated).
2. **Validation**: The `bal-server` parses each transaction using `bitcoin::Transaction` via `consensus::deserialize`. It checks for the fee output, extracts inputs/outputs, and validates the locktime.
3. **Storage**: Valid transactions are stored in `tbl_tx` with `status = 0` (waiting). The inputs and outputs are stored in `tbl_inp` and `tbl_out`. Batch inserts use `UNION ALL SELECT` for efficiency.
4. **Monitoring**: The `bal-pusher` listens to the ZMQ `hashblock` topic with a 5-second receive timeout. When a new block is detected, it fetches blockchain info via RPC.
5. **Evaluation**: The pusher queries the database for transactions with `status=0` and compares their locktime against the blockchain's best block height or median time (for timestamp-based locktimes above `LOCKTIME_THRESHOLD`).
6. **Broadcast**: If the locktime is satisfied, the pusher sends the transaction via `sendrawtransaction` and updates the status to `1` (sent) or `2` (failed with error stored in `push_err`).
7. **Statistics**: The pusher periodically calculates statistics and sends them to a remote `welist` server using an Ed25519-signed POST request. The server also exposes stats via the `GET /:network/stats` endpoint if `expose_stats` is enabled.

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
- `2`: Failed (e.g., RPC error `-25 bad-txns-inputs-missingorspent`). Error details stored in `push_err`.

## Error Handling Strategy

The codebase has been hardened with comprehensive error handling:
- **`bal-server`**: Uses `actix-web`'s built-in error handling. All `unwrap()`/`expect()` calls have been replaced with safe `match`/`if let` error propagation, returning appropriate HTTP status codes (400, 404, 500).
- **`bal-pusher`**: ZMQ `recv` uses `set_rcvtimeo(5000)` with a match/timeout handler. RPC connection failures log errors and retry with a sleep interval instead of panicking. The pusher logs warnings for consecutive ZMQ timeouts (~1 hour threshold).
- **`db.rs`**: Database operations use `Result` types. The `open_db` function validates paths before opening. WAL mode is set with retry logic.

## Logging and Monitoring

The project uses `env_logger` and `log`. By default, `RUST_LOG=info` is set. The `bal-pusher` sends signed statistics to a remote server. The server exposes a `stats` endpoint (`/<network>/stats`) if `expose_stats` is enabled. The actix-web `Logger::default()` middleware logs all HTTP requests/responses.
