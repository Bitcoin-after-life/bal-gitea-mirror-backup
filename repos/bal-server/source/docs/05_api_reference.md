# API Reference

## Quick Reference
- **What this file contains:** complete specification of the HTTP API, ZMQ messages, and RPC usage, with request/response examples.
- **See also:** [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md), [06_database_schema.md](06_database_schema.md), [07_deployment_and_ops.md](07_deployment_and_ops.md)

---

## HTTP API (provided by `bal-server`)

### Rate Limiting

All endpoints are rate-limited via `actix-governor` with a token-bucket algorithm. Defaults:

| Endpoint | Rate (req/s) | Burst |
|----------|-------------|-------|
| `POST /{network}/pushtxs` | 1 | 3 |
| `POST /searchtx` | 5 | 10 |
| `GET /{network}/info` | 20 | 30 |
| All others | 50 | 100 |

Rate limits are configurable via `BAL_SERVER_ACTIX_*` environment variables.

### `GET /`
- **Description:** Returns a static identification string (default: "Will Executor Server").
- **Response:** Plain text `200 OK`.

### `GET /version`
- **Description:** Returns the Cargo package version.
- **Response:** `text/plain` (e.g., `0.3.2`).

### `GET /.pub_key.pem`
- **Description:** Returns the static Ed25519 public key PEM file for signature verification of remote stats.
- **Response:** `text/plain` with the PEM file content.
- **File:** `public_key.pem` in the project root (path configurable via `BAL_SERVER_PUB_KEY_PATH`).

### `GET /:network/info`
- **Description:** Returns JSON with the server's configuration for that specific network.
- **Supported Networks:** `bitcoin`, `testnet`, `testnet4`, `signet`, `regtest`.
- **Response (200 OK):**
  ```json
  {
    "address": "bcrt1q...",
    "base_fee": 50000,
    "chain": "regtest",
    "info": "Will Executor Server",
    "version": "0.3.2"
  }
  ```
- In xpub mode, the `address` field contains a freshly derived P2WPKH address unique to the requesting IP.
- **Error:** `404` if the network is not configured or unknown.

### `GET /:network/stats`
- **Description:** Returns statistics for the given network. Guarded by the `expose_stats` configuration flag.
- **Response (200 OK):** A JSON array of `StatsResponse` objects:
  ```json
  [
    {
      "report_date": "2024-07-20T12:00:00Z",
      "chain": "regtest",
      "totals": 42,
      "waiting": 10,
      "sent": 30,
      "failed": 2,
      "waiting_profit": 10000,
      "sent_profit": 30000,
      "missed_profit": 5000,
      "unique_inputs": 15
    }
  ]
  ```
- **Error:** `403` or `400` if stats are not enabled or the network is unknown.

### `POST /:network/pushtxs`
- **Description:** Accepts one or more raw hex Bitcoin transactions (newline-separated). The server deserializes each transaction, validates that the fee is paid to the correct `our_address` for that network, and stores valid transactions in the database.
- **Request Body:** Newline-separated raw hex transactions.
  ```
  02000000000101...hex...\n
  02000000000101...hex...\n
  ```
- **Response (200 OK):** A JSON object with the results for the batch:
  ```json
  {
    "accepted": 2,
    "rejected": 0,
    "details": [...]
  }
  ```
- **Response (400 Bad Request):** If all transactions are invalid, the fee is missing, or the locktime is not acceptable.
- **Response (413 Payload Too Limited):** If the request body exceeds the configured max size (default 1 MiB).
- **Security Note:** Invalid transactions or those not paying the required fees are not inserted into the database.

### `POST /searchtx`
- **Description:** Searches for a transaction by its `txid`. The request body must contain exactly 64 hex characters.
- **Request Body:**
  ```json
  {
    "txid": "abc123def456..."
  }
  ```
- **Response (200 OK):**
  ```json
  {
    "txid": "abc123...",
    "status": 1,
    "tx": "020000000...",
    "our_address": "bcrt1q...",
    "our_fees": 1000,
    "reqid": "192.168.1.1"
  }
  ```
- **Response (400 Bad Request):** If the txid is not exactly 64 hex characters.
- **Response (404 Not Found):** If the transaction is not found in the database.

---

## ZMQ Messages (consumed by `bal-pusher`)

### Topic: `hashblock`
- **Format:** A multipart ZMQ message. The first frame is the topic name (`hashblock`), the second frame is the 32-byte block hash.
- **Trigger:** When a new Bitcoin block is found by the local node.
- **Action:** The pusher fetches `getblockchaininfo` from the RPC, gets the updated `mediantime` and `blocks` height, then queries and pushes pending transactions.
- **Endpoint:** Per-network (e.g., `tcp://127.0.0.1:28332` for mainnet).
- **Timeout:** 5 seconds (`ZMQ_RCVTIMEO`). The pusher logs a warning after ~720 consecutive timeouts (~1 hour).

---

## Bitcoin Core RPC Usage (used by `bal-pusher`)

### `sendrawtransaction`
- **Method:** `sendrawtransaction` (RPC `2`)
- **Parameters:** `hexstring` (the raw hex of the transaction to broadcast).
- **Description:** Broadcasts the transaction to the Bitcoin network. If the transaction is invalid (e.g., `bad-txns-inputs-missingorspent`), the RPC will return an error with a negative code (e.g., `-25`).
- **Error Handling:** The pusher catches these errors, logs them, and updates the database status to `2` (failed) with the error in `push_err`.

### `getblockchaininfo`
- **Method:** `getblockchaininfo` (RPC `1`)
- **Parameters:** None.
- **Description:** Returns the current blockchain state, including `mediantime` (median timestamp of the last 11 blocks) and `blocks` (best block height). Used to evaluate `nLockTime` of pending transactions.

### `getblock`
- **Method:** `getblock` (RPC `1`)
- **Parameters:** `blockhash`, `verbosity` (set to `1` for JSON with timestamp).
- **Description:** Fetches block details. Used as an alternative for median time calculation.

### RPC Authentication
Authentication is done via `bitcoincore-rpc` using either:
- **`UserPass`**: `BAL_PUSHER_{NETWORK}_RPC_USER` and `BAL_PUSHER_{NETWORK}_RPC_PASSWORD`.
- **`CookieFile`**: `$HOME/.bitcoin/{dir_path}.cookie` or custom path via `BAL_PUSHER_{NETWORK}_COOKIE_FILE`.

The client tries username/password auth first, then falls back to cookie file auth.

---

## Ed25519 Stats Signing

The pusher signs the statistics payload before sending it to the `welist` server:
1. Collects statistics from the database.
2. Serializes the stats as JSON.
3. Signs the JSON payload with the Ed25519 private key (`privkey.pem`).
4. Sends the payload with the base64-encoded signature in the `X-Signature` header.
5. The `welist` server can verify the signature using the public key served at `GET /.pub_key.pem`.
