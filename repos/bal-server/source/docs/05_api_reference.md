# API Reference

## Quick Reference
- **What this file contains:** complete specification of the HTTP API, ZMQ messages, and RPC usage, with request/response examples.
- **See also:** [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md), [06_database_schema.md](06_database_schema.md), [07_deployment_and_ops.md](07_deployment_and_ops.md)

---

## HTTP API (provided by `bal-server`)

### `GET /`
- **Description:** Returns a static identification string (e.g., "Will Executor Server").
- **Response:** Plain text `200 OK`.

### `GET /version`
- **Description:** Returns the Cargo package version (`bal_server` version).
- **Response:** `text/plain` (e.g., `0.2.3`).

### `GET /.pub_key.pem`
- **Description:** Returns the static Ed25519 public key PEM file for signature verification of remote stats.
- **Response:** `text/plain` with the PEM file content.
- **File:** `public_key.pem` in the project root.

### `GET /:network/info`
- **Description:** Returns JSON with the server's configuration for that specific network.
- **Supported Networks:** `bitcoin`, `testnet`, `testnet4`, `signet`, `regtest`.
- **Response (200 OK):**
  ```json
  {
    "network": "regtest",
    "our_address": "bcrt...",
    "fee": 1000,
    "chain": "regtest",
    "version": "0.2.3"
  }
  ```
- **Error:** `404` if the network is not configured.

### `GET /:network/stats`
- **Description:** Returns statistics for the given network. This endpoint is guarded by the `expose_stats` configuration flag.
- **Response (200 OK):**
  ```json
  {
    "report_date": 1712345678,
    "chain": "regtest",
    "total": 42,
    "waiting": 10,
    "sent": 30,
    "failed": 2,
    "waiting_profit": 10000,
    "sent_profit": 30000,
    "missed_profit": 5000,
    "unique_input": 15
  }
  ```
- **Error:** `403` or `400` if stats are not enabled or the network is unknown.

### `POST /:network/pushtxs`
- **Description:** Accepts one or more raw hex Bitcoin transactions. The server deserializes the transaction, validates that the fee is paid to the correct `our_address` for that network, and stores the transaction in the database. It also stores all inputs and outputs.
- **Request Body:**
  - `Content-Type: application/json` (or plain text, depending on the client).
  - The payload format is typically an array of raw hex strings or a single hex string.
  ```json
  [
    "02000000000101...hex..."
  ]
  ```
- **Response (200 OK):** A JSON array with the result for each transaction.
  ```json
  [
    {
      "txid": "abc123...",
      "wtxid": "def456...",
      "status": 0,
      "locktime": 2100,
      "our_fees": 1000,
      "our_address": "bcrt1q..."
    }
  ]
  ```
- **Response (400 Bad Request):** If the transaction is invalid, the fee is missing, or the locktime is not acceptable.
- **Response (500 Internal Server):** `Database error`, `Invalid hex`, `Invalid transaction` (may contain a panic trace if an internal `unwrap` is hit).
- **Security Note:** If a transaction is not valid or does not pay the required fees, it is not inserted into the database.

### `POST /searchtx`
- **Description:** Searches for a transaction by its `txid`. Returns the transaction details, status, raw hex, and fees.
- **Request Body:**
  ```json
  {
    "txid": "abc123..."
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
    "locktime": 2100,
    "timestamp": 1712345678
  }
  ```
- **Response (404):** If the transaction is not found in the database.
- **Response (400):** If the request body is invalid.

---

## ZMQ Messages (consumed by `bal-pusher`)

### Topic: `hashblock` (Consumed by `bal-pusher`)
- **Format:** A multipart ZMQ message. The first frame is the topic name (`hashblock`), the second frame is the 32-byte block hash.
- **Trigger:** When a new Bitcoin block is found by the local node.
- **Action:** The pusher fetches `getblockchaininfo` from the RPC, gets the updated `mediantime`, then queries and pushes pending transactions.
- **Endpoint:** `tcp://127.0.0.1:28332` (or network-specific ports).

---

## Bitcoin Core RPC Usage (used by `bal-pusher`)

### `sendrawtransaction` (Both pushers)
- **Method:** `sendrawtransaction` (RPC `2`)
- **Parameters:** `hexstring` (the raw hex of the transaction to broadcast).
- **Description:** Broadcasts the transaction to the Bitcoin network. If the transaction is invalid (e.g., `bad-txns-inputs-missingorspent`), the RPC will return an error with a negative code (e.g., `-25`).
- **Error Handling:** The pusher catches these errors, logs them, and updates the database status to `2` (failed).

### `getblockchaininfo` (Only `bal-pusher`)
- **Method:** `getblockchaininfo` (RPC `1`)
- **Parameters:** None.
- **Description:** Returns the current blockchain state, including the `mediantime` (the median timestamp of the last 11 blocks). This is used to evaluate the `nLockTime` of pending transactions.


### `getblock` (Only used by `bal-pusher` for median time)
- **Method:** `getblock` (RPC `1`)
- **Parameters:** `blockhash`, `verbosity` (set to `1` for JSON with timestamp).
- **Description:** Fetches the details of a block. It is used as an alternative to `getblockchaininfo` to get the block's `time` if `getblockchaininfo` fails or is insufficient.

---
