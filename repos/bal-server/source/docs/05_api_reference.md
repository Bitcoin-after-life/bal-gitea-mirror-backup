# API Reference

## Quick Reference
- **What this file contains:** complete specification of the HTTP API, ZMQ messages, and RPC usage, with request/response examples.
- **See also:** [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md), [06_database_schema.md](06_database_schema.md), [07_deployment_and_ops.md](07_deployment_and_ops.md)

---

## HTTP API (provided by `bal-server`)

### Rate Limiting

All endpoints are rate-limited via `actix-governor` with a token-bucket algorithm. The rate limit key is the **real client IP address**, extracted from `X-Real-IP` / `X-Forwarded-For` headers when the request comes from a trusted proxy (default `127.0.0.1`, configurable via `BAL_SERVER_TRUSTED_PROXY`). Direct connections (non-proxy) use the TCP peer IP.

Default: 1 req/s with burst of 3 (configurable via `BAL_SERVER_ACTIX_PUSHTXS_PER_SEC` / `BAL_SERVER_ACTIX_PUSHTXS_BURST`).

When behind Nginx, ensure `proxy_set_header X-Real-IP $remote_addr` is set so the server can identify individual clients.

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

## Ed25519 WeTime Ping Signing

The pusher signs the report before sending it to the `welist` server. The
scheme is selected with `WELIST_PING_SIG_VERSION`:

### Version 3 (canonical JSON, DNSSEC/onion key discovery)

1. Reads `info`, `base_fee` and `version` from `tbl_executor_metadata`, keyed
   by the canonical chain name. `bal-server` writes that row at startup. If the
   table or row is unavailable, the fields are signed as `null`.
2. When `pubkey_pem` is present, verifies that `SSL_KEY_PATH` matches it. A
   malformed PEM or mismatch skips the ping; a missing row or key does not.
3. For DNS hosts, performs one TXT consistency check before the first ping:
   queries `_welist-key.<host>` through the system resolver (no local DNSSEC
   validation), compares it with `SSL_KEY_PATH`, retries once after 30 seconds
   on a mismatch, and skips WeTime pings until restart after two mismatches.
   NXDOMAIN/empty/malformed records are mismatches; timeout/SERVFAIL is
   transient and does not block.
4. Canonicalizes `BAL_SERVER_URL` to an origin (`scheme://host[:port]`, ASCII,
   lowercase host, no path/query/userinfo, no explicit default port). Values
   that can be normalized (host case, trailing dot or slash) are rewritten to
   the canonical form; values that cannot (IP literal, path, query, raw Unicode)
   are logged and v3 pings are skipped.
5. Builds a compact JSON object with lexicographically sorted keys, every field
   present, and `null` for absent optional values:
   `base_fee`, `chain`, `height`, `info`, `last_block_hash`, `median_time`,
   `sig_version`, `url`, `version`.
6. Signs `WELIST-PING-V3\n` followed by that JSON with the Ed25519 private key
   (`SSL_KEY_PATH`).
7. POSTs the same fields plus the base64 signature and `"sig_version": 3` to
   `{WELIST_SERVER_URL}/ping`.
8. The `welist` server discovers the public key from a DNSSEC-validated
   `_welist-key.<host>` TXT record (`v=we1; alg=ed25519; key=<base64 SPKI DER>`)
   or from a v3 `.onion` address. No HTTP callback to `/.pub_key.pem` is made
   for v3.

For `.onion` URLs the pusher includes a `delegation` object when
`WELIST_PING_DELEGATION_FILE` is set and `WELIST_PING_SIG_VERSION=3`. The onion
identity key signs that certificate offline; `SSL_KEY_PATH` is then the
dedicated ping key, and the service verifies the certificate before checking
the ping signature. The pusher validates the certificate signature locally
against the identity key embedded in the onion address, in addition to the host,
expiry, key, and structure checks. The signed certificate bytes use a trimmed
`key` and a normalized `onion` (lowercase, trailing dot removed); the service
applies the same normalization before verifying, so equivalent spellings
produce the same signature input.

The pusher validates the certificate at startup and before each ping: `onion`
must match the configured host after normalization (lowercase, trailing dot
removed), `valid_until` must be in the future and at most
`BAL_PUSHER_MAX_DELEGATION_DAYS` (default 366; `0` disables the cap) days ahead,
the key must be a valid Ed25519 SPKI and the signature must be 64 bytes, and
`key` must match `SSL_KEY_PATH`. If the file is missing or malformed, or any
check fails, the pusher logs a warning and sends no `delegation` object. It then
signs with `SSL_KEY_PATH` directly; in the recommended setup that key is the
dedicated ping key, so the service rejects the pings until delegation is
restored. `BAL_PUSHER_DELEGATION_WARN_DAYS` controls an advance expiry warning.

The `url` sent and signed in v2 and v3 is the canonical origin computed from
`BAL_SERVER_URL` by the same `url_policy` module used by `bal-welist`. The host
must be a public DNS name (at least two labels, no private/special-use suffix)
unless it is `.onion`. `chain` is lowercase, matching the service-side
canonicalization. The service rejects any v2 or v3 ping whose `url` is not
already in canonical form.

To stop accepting a ping key, run `contrib/make_ping_key_revocation.py` and
submit the result to `{WELIST_SERVER_URL}/ping-keys/revoke`. Onion revocations
are signed by the onion identity key; DNS TXT revocations are self-signed by the
key being revoked. The signed revocation bytes use a normalized `host`
(lowercase, trailing dot removed) and a trimmed `key`. Revoke before removing
the key from the TXT record. See
[`10_dnssec_key_publishing.md`](10_dnssec_key_publishing.md) for the operator
guide on publishing `_welist-key.<host>`, onion delegation and revocation.

### Version 2 (canonical JSON, `/.pub_key.pem` key discovery)

v2 is identical to v3 except for how the WeTime service discovers the signing
key. Both use the same canonical URL and the same canonical JSON payload; only
the prefix, `sig_version`, and key-discovery mechanism differ.

1. Reads `info`, `base_fee` and `version` from `tbl_executor_metadata`, keyed
   by the canonical chain name. `bal-server` writes that row at startup. If the
   table or row is unavailable, the fields are signed as `null`.
2. When `pubkey_pem` is present, verifies that `SSL_KEY_PATH` matches it. A
   malformed PEM or mismatch skips the ping; a missing row or key does not.
3. Canonicalizes `BAL_SERVER_URL` with the same `url_policy` module used by v3.
   Values that cannot be canonicalized are logged and v2 pings are skipped.
4. Builds a compact JSON object with lexicographically sorted keys, every field
   present, and `null` for absent optional values:
   `base_fee`, `chain`, `height`, `info`, `last_block_hash`, `median_time`,
   `sig_version`, `url`, `version`.
5. Signs `WELIST-PING-V2\n` followed by that JSON with the Ed25519 private key
   (`SSL_KEY_PATH`).
6. POSTs the same fields plus the base64 signature and `"sig_version": 2` to
   `{WELIST_SERVER_URL}/ping`.
7. The WeTime service verifies the signature with the public key served at
   `GET /.pub_key.pem` and applies the signed metadata. v2 does not use DNSSEC,
   onion identity keys or delegation; a `delegation` object is never sent.

v2 is intended for executors that already publish `/.pub_key.pem` and want
authenticated metadata without moving to DNSSEC. A valid v2 ping permanently
disables v1 for that host; a later v3 ping disables both v1 and v2.

### Version 1 (legacy)

1. Trims surrounding whitespace and trailing slashes from `BAL_SERVER_URL`,
   concatenates `url + chain + height + median_time + last_block_hash`, and
   signs that string with the Ed25519 private key.
2. POSTs those five fields plus the base64 signature to
   `{WELIST_SERVER_URL}/ping`. No `info`, `base_fee` or `version` is sent.
3. The `welist` server verifies the signature with the public key served at
   `GET /.pub_key.pem`.
4. The v1 signature authenticates only liveness. The service refreshes
   `last_update` for an accepted ping but does not persist executor metadata.
   Because any metadata added to a v1 ping would be outside the signature, use
   v2 or v3 when `info`, `base_fee` and `version` must be published and
   authenticated.
