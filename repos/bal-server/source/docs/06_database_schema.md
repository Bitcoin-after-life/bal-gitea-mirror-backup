# Database Schema

## Quick Reference
- **What this file contains:** the full SQL schema, data types, indexing, queries, and data lifecycle for the SQLite `bal.db` database.
- **See also:** [04_modules_detail.md](04_modules_detail.md), [05_api_reference.md](05_api_reference.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md)

---

## Database Technology
- **Engine:** `sqlite` (Rust `sqlite` crate, version 0.34.0)
- **File:** `bal.db` (default, configured in environment)
- **Connection Pooling:** The Rust `sqlite` crate handles connections but does not use a thread pool.
- **Transactions:** The `execute_insert` function attempts to use atomic transactions for batched inserts, but this is not guaranteed for all operations.

---

## Table Schema

### `tbl_tx` (Transactions)

```sql
CREATE TABLE tbl_tx (
    txid PRIMARY KEY,                -- TEXT: The unique transaction ID (hex string)
    wtxid,                           -- TEXT: The witness transaction ID
    ntxid,                           -- TEXT: The non-witness transaction ID
    tx,                              -- TEXT: The full raw serialized transaction (hex)
    locktime INTEGER,                 -- INTEGER: The locktime value (block height or timestamp)
    network,                         -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    network_fees,                    -- TEXT: The total fees paid by the user (satoshi)
    reqid,                           -- TEXT: A request ID or client IP for the submitter
    our_fees,                        -- TEXT: The fees paid to us (the operator) (satoshi)
    our_address,                     -- TEXT: The address the operator fee is paid to
    status INTEGER DEFAULT 0,         -- INTEGER: 0 = waiting, 1 = sent, 2 = failed
    push_err TEXT                     -- TEXT: The error message if the RPC broadcast failed
);
```
- **Indexes:** The `txid` is the primary key, so it is automatically indexed.
- **Notes:** `locktime` is stored as an integer. It is compared against the `mediantime` or `block_height` from the blockchain to evaluate when a transaction is ready to send. The `status` column is the core of the transaction lifecycle state machine. `our_fees` and `our_address` are used to validate the transaction and ensure the correct fee is included before accepting it.

### `tbl_inp` (Transaction Inputs)

```sql
CREATE TABLE tbl_inp (
    id,                              -- INTEGER: Auto-increment ID
    txid,                            -- TEXT: The transaction ID of the transaction being submitted
    in_txid,                         -- TEXT: The previous transaction ID (output being spent)
    in_vout                          -- INTEGER: The previous output index
);
CREATE UNIQUE INDEX ON tbl_inp(txid, in_txid, in_vout);
```
- **Purpose:** Tracks all inputs of the submitted transactions. This allows the database to identify double-spends and ensure the inputs are valid and available.
- **Constraints:** A unique index prevents duplicate entries for the same input in the same transaction.
- **Relationships:** `in_txid` and `in_vout` refer to outputs from previous transactions in the Bitcoin blockchain. The `txid` column refers to the transaction being submitted (the one in `tbl_tx`).

### `tbl_out` (Transaction Outputs)

```sql
CREATE TABLE tbl_out (
    id,                              -- INTEGER: Auto-increment ID
    txid,                            -- TEXT: The transaction ID of the transaction being submitted
    script_pubkey,                   -- TEXT: The hex scriptPubKey of this output
    amount,                          -- TEXT: The amount in this output (satoshi)
    vout                             -- INTEGER: The output index (0-based) in this transaction
);
CREATE UNIQUE INDEX ON tbl_out(txid, script_pubkey, amount, vout);
```
- **Purpose:** Tracks all outputs of the submitted transactions. The server searches for the `script_pubkey` matching the `our_address` for the network to determine if the correct fee is included.
- **Constraints:** A unique index prevents duplicate entries for the same output in the same transaction.
- **Relationships:** `txid` refers to the transaction being submitted. The `script_pubkey` is matched against the known addresses for each network to verify the fee payment.

### `tbl_xpub` (Extended Public Keys)

```sql
CREATE TABLE tbl_xpub (
    id INTEGER PRIMARY KEY,          -- INTEGER: Auto-increment ID
    network TEXT,                    -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    xpub TEXT,                       -- TEXT: The extended public key (xpub or zpub)
    date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- TEXT: The date the xpub was added
    path_idx INTEGER DEFAULT -1      -- INTEGER: The next address index to derive for this xpub
);
CREATE UNIQUE INDEX idx_xpub ON tbl_xpub (network, xpub);
```
- **Purpose:** Stores the master xpub/zpub keys for each network. When the server receives a transaction, it uses these to derive new receiving addresses (if applicable) or to verify the `our_address`.
- **Relationships:** `tbl_xpub` is linked to `tbl_address` via `xpub` (the ID). The `path_idx` tracks which child index is the next unused one for the wallet's derivation path.

### `tbl_address` (Derived Addresses)

```sql
CREATE TABLE tbl_address (
    address TEXT PRIMARY KEY,        -- TEXT: The Bech32 P2WPKH address (e.g., 'bcrt1q...')
    path TEXT NOT NULL,              -- TEXT: The derivation path used to create this address (e.g., 'm/84'/1'/0'/0/0')
    date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- TEXT: The date the address was generated
    xpub INTEGER,                    -- INTEGER: The ID of the xpub in `tbl_xpub` that owns this address
    remote_address TEXT                -- TEXT: IP or client identifier that requested this address (if applicable)
);
```
- **Purpose:** Stores all generated addresses. The `our_address` for each network is derived from a specific xpub path. The server can also generate new addresses on demand for clients.
- **Relationships:** `xpub` (FK) links to `tbl_xpub.id`. The `address` is the primary key because it is unique by design. `remote_address` is used for rate-limiting or identifying address ownership in logs.

### `tbl_stats` (Per-Network Statistics)

```sql
CREATE TABLE tbl_stats (
    report_date INTEGER,             -- INTEGER: The Unix timestamp of the report
    chain TEXT PRIMARY KEY,           -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    totals INTEGER,                  -- INTEGER: Total number of transactions submitted
    waiting INTEGER,                  -- INTEGER: Transactions currently waiting (status=0)
    sent INTEGER,                     -- INTEGER: Transactions successfully sent (status=1)
    failed INTEGER,                    -- INTEGER: Transactions that failed to broadcast (status=2)
    waiting_profit INTEGER,            -- INTEGER: Total fees for waiting transactions (satoshi)
    sent_profit INTEGER,               -- INTEGER: Total fees for sent transactions (satoshi)
    missed_profit INTEGER,             -- INTEGER: Total fees for transactions that expired or failed (satoshi)
    unique_inputs INTEGER               -- INTEGER: The number of unique inputs (for deduplication analysis)
);
```
- **Purpose:** Stores aggregate statistics for each network. The pusher sends this data to a remote `welist` server. The server also reads from it for the `stats` endpoint if `expose_stats` is enabled.
- **Relationships:** `chain` is the primary key. The data is updated by the `bal-pusher` binary.

---

## Data Query Strategy

### Key Queries (from `db.rs` and `bal-pusher.rs`)

- **Get Pending Transactions (by status and locktime):**
  ```sql
SELECT
    txid, tx, wtxid, ntxid, locktime, status,
    our_address, our_fees, network_fees
FROM
    tbl_tx
WHERE
    network = ?
  AND status = 0
  AND locktime < ?;
  ```
  Used by the `bal-pusher` daemon to find transactions that are ready to broadcast. The `?` placeholders are bound at runtime. `locktime` is compared with the `mediantime` or block height from the ZMQ `new block` event.

- **Insert Transaction:**
  ```sql
INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, network_fees, reqid, our_fees, our_address)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
  ```
  Used by the `bal-server` when accepting a new valid transaction.

- **Update Status:**
  ```sql
UPDATE tbl_tx SET status = ? WHERE txid = ?;
  ```
  Used by the pusher after a successful or failed broadcast attempt. The status is set to `1` (sent) and `2` (failed).
  **WARNING:** The `bal-pusher` also uses a raw `WHERE txid IN ('...')` format for batch updates. These string formats have **SQL injection risk** because the `txid` strings are concatenated into the raw SQL string without proper parameterization.

- **Search Transaction:**
  ```sql
SELECT * FROM tbl_tx WHERE txid = ?;
  ```
  Used by the `searchtx` endpoint.
- **Get Address for Rate Limiting:**
  ```sql
SELECT a.address, x.xpub
FROM tbl_address a
JOIN tbl_xpub x ON a.xpub = x.id
WHERE a.remote_address = ?;
  ```
  Used to check if an IP or client address already has a generated address. This is part of the address reuse logic to prevent users from requesting too many addresses or using the same IP to bypass fees.

---

## Data Lifecycle
- **Creation:** Transactions are created when a user submits a raw hex tx via the `POST /pushtxs` endpoint. The address is inserted when an xpub is configured.
- **Waiting:** Transactions are in `status=0` and are queried by the pusher every new block.
- **Broadcast:** Transactions are pushed to the network via `sendrawtransaction`. If successful, the status becomes `1`. If the RPC returns an error (e.g., `-25`), the status becomes `2` and the error string is stored in `push_err`.
- **Retention:** There is no explicit cleanup mechanism for old records. The `valid_txs` and `invalid_txs` files contain logs of past transaction pushes, but the database itself may grow indefinitely. For a production system, a periodic vacuum or purge of old `status=1` transactions might be required.
- **Backup:** The database is a single SQLite file (`bal.db`). It can be copied directly using `cp` or `rsync` (see `scripts/download_bal_db.sh`). There is no WAL mode or online backup mechanism implemented.
