# Database Schema

## Quick Reference
- **What this file contains:** the full SQL schema, data types, indexing, queries, and data lifecycle for the SQLite `bal.db` database.
- **See also:** [04_modules_detail.md](04_modules_detail.md), [05_api_reference.md](05_api_reference.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md)

---

## Database Technology
- **Engine:** `sqlite` (Rust `sqlite` crate, version 0.34.0)
- **File:** `bal.db` (default, configurable via `BAL_SERVER_DB_FILE` / `BAL_PUSHER_DB_FILE`)
- **Connection Management:** Shared `Arc<Mutex<Connection>>` in `bal-server`, single connection in `bal-pusher`.
- **WAL Mode:** Enabled via `PRAGMA journal_mode=WAL` with retry logic (up to 5 attempts) for concurrent access safety.
- **Busy Timeout:** Set to 5000ms via `PRAGMA busy_timeout=5000`.
- **Synchronous Mode:** Set to `NORMAL` via `PRAGMA synchronous=NORMAL`.
- **Path Validation:** The `open_db` function validates the database path before opening, rejecting directory traversal (`..`), forbidden system directories (`/etc`, `/proc`, `/sys`, `/dev`, `/usr`, `/bin`, `/sbin`, `/lib`, `/opt`), and symlinks.

---

## Table Schema

### `tbl_tx` (Transactions)

```sql
CREATE TABLE IF NOT EXISTS tbl_tx (
    txid PRIMARY KEY,                   -- TEXT: The unique transaction ID (hex string)
    date_creation TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    date_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    wtxid,                              -- TEXT: The witness transaction ID
    ntxid,                              -- TEXT: The non-witness transaction ID
    tx,                                 -- TEXT: The full raw serialized transaction (hex)
    locktime INTEGER,                   -- INTEGER: The locktime value (block height or timestamp)
    network,                            -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    network_fees,                       -- TEXT: The total fees paid by the user (satoshi)
    reqid,                              -- TEXT: A request ID or client IP for the submitter
    our_fees,                           -- TEXT: The fees paid to us (the operator) (satoshi)
    our_address,                        -- TEXT: The address the operator fee is paid to
    status INTEGER DEFAULT 0,           -- INTEGER: 0 = waiting, 1 = sent, 2 = failed
    push_err TEXT                       -- TEXT: The error message if the RPC broadcast failed
);
ALTER TABLE tbl_tx ADD COLUMN push_err TEXT;
```
- **Indexes:** The `txid` is the primary key, so it is automatically indexed.
- **Notes:** `date_creation` and `date_update` track when the transaction was inserted and last modified. `locktime` is compared against the blockchain's best block height or `mediantime` (for timestamps above `LOCKTIME_THRESHOLD`). The `status` column is the core of the transaction lifecycle state machine. `push_err` stores the RPC error message when status=2.

### `tbl_inp` (Transaction Inputs)

```sql
CREATE TABLE IF NOT EXISTS tbl_inp (
    id,                                 -- INTEGER: Auto-increment ID
    txid,                               -- TEXT: The transaction ID of the transaction being submitted
    in_txid,                            -- TEXT: The previous transaction ID (output being spent)
    in_vout                             -- INTEGER: The previous output index
);
CREATE UNIQUE INDEX ON tbl_inp(txid, in_txid, in_vout);
```
- **Purpose:** Tracks all inputs of submitted transactions. Enables identification of double-spends.
- **Constraints:** A unique index prevents duplicate entries for the same input in the same transaction.

### `tbl_out` (Transaction Outputs)

```sql
CREATE TABLE IF NOT EXISTS tbl_out (
    id,                                 -- INTEGER: Auto-increment ID
    txid,                               -- TEXT: The transaction ID of the transaction being submitted
    script_pubkey,                      -- TEXT: The hex scriptPubKey of this output
    amount,                             -- TEXT: The amount in this output (satoshi)
    vout                                -- INTEGER: The output index (0-based) in this transaction
);
CREATE UNIQUE INDEX ON tbl_out(txid, script_pubkey, amount, vout);
```
- **Purpose:** Tracks all outputs of submitted transactions. The server searches for the `script_pubkey` matching the `our_address` for the network to verify fee payment.
- **Constraints:** A unique index prevents duplicate entries for the same output in the same transaction.

### `tbl_xpub` (Extended Public Keys)

```sql
CREATE TABLE IF NOT EXISTS tbl_xpub (
    id INTEGER PRIMARY KEY,             -- INTEGER: Auto-increment ID
    network TEXT,                        -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    xpub TEXT,                           -- TEXT: The extended public key (xpub, zpub, ypub, etc.)
    date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    path_idx INTEGER DEFAULT -1          -- INTEGER: The next address index to derive for this xpub
);
CREATE UNIQUE INDEX idx_xpub ON tbl_xpub (network, xpub);
```
- **Purpose:** Stores the master xpub/zpub/ypub keys for each network. When the server receives a transaction in xpub mode, it derives new receiving addresses from these keys.
- **Relationships:** `tbl_xpub` is linked to `tbl_address` via `id`. The `path_idx` tracks which child index is the next unused one for the wallet's derivation path.

### `tbl_address` (Derived Addresses)

```sql
CREATE TABLE IF NOT EXISTS tbl_address (
    address TEXT PRIMARY_KEY,            -- TEXT: The Bech32 P2WPKH address (e.g., 'bcrt1q...')
    path TEXT NOT NULL,                  -- TEXT: The derivation path (e.g., 'm/0/0')
    date_create TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    xpub INTEGER,                        -- INTEGER: The ID of the xpub in `tbl_xpub`
    remote_address TEXT                  -- TEXT: IP or client identifier that requested this address
);
```
- **Purpose:** Stores all generated addresses. In xpub mode, addresses are derived on-demand per requesting IP.
- **Relationships:** `xpub` (FK) links to `tbl_xpub.id`. `remote_address` is used for rate-limiting and preventing address reuse per IP.

### `tbl_stats` (Per-Network Statistics)

```sql
CREATE TABLE IF NOT EXISTS tbl_stats (
    report_date TEXT,                    -- TEXT: The ISO timestamp of the report
    chain TEXT,                          -- TEXT: The network name (e.g., 'regtest', 'bitcoin')
    totals INTEGER,                      -- INTEGER: Total number of transactions submitted
    waiting INTEGER,                     -- INTEGER: Transactions currently waiting (status=0)
    sent INTEGER,                        -- INTEGER: Transactions successfully sent (status=1)
    failed INTEGER,                      -- INTEGER: Transactions that failed to broadcast (status=2)
    waiting_profit INTEGER,              -- INTEGER: Total fees for waiting transactions (satoshi)
    sent_profit INTEGER,                 -- INTEGER: Total fees for sent transactions (satoshi)
    missed_profit INTEGER,               -- INTEGER: Total fees for failed/expired transactions (satoshi)
    unique_inputs INTEGER                -- INTEGER: The number of unique inputs (for deduplication analysis)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_stats_chain ON tbl_stats(chain);
```
- **Purpose:** Stores aggregate statistics for each network. The pusher calculates and upserts stats (`ON CONFLICT(chain) DO UPDATE`). The server reads from it for the `stats` endpoint if `expose_stats` is enabled.
- **Relationships:** `chain` has a unique index. Data is updated by `bal-pusher` via `calculate_stats`.

---

## Data Query Strategy

### Key Queries

- **Get Pending Transactions (by status and locktime):**
  ```sql
  SELECT * FROM tbl_tx
  WHERE network = :network
    AND status = :status
    AND (locktime < :bestblock_height
         OR locktime > :locktime_threshold AND locktime < :bestblock_time);
  ```
  Used by the `bal-pusher` to find transactions ready to broadcast. The `locktime_threshold` constant distinguishes block heights from timestamps.

- **Insert Transaction (batched):**
  ```sql
  INSERT INTO tbl_tx (txid, wtxid, ntxid, tx, locktime, network, network_fees, reqid, our_fees, our_address)
  UNION ALL SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
  UNION ALL SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
  -- ... more rows
  ```
  Used by `bal-server` for efficient bulk inserts.

- **Update Status:**
  ```sql
  UPDATE tbl_tx SET status = ? WHERE txid = ?;
  ```
  Used by the pusher after broadcast. Status `1` = sent, `2` = failed.
  ```sql
  UPDATE tbl_tx SET status = 2, push_err = ? WHERE txid = ?;
  ```
  For failed broadcasts, the error message is stored.

- **Check Duplicate Txids:**
  ```sql
  SELECT txid FROM tbl_tx WHERE txid IN (?, ?, ?, ...);
  ```
  Used by `bal-server` to batch-check duplicates before inserting. Chunks in groups of 500 for SQLite parameter limit safety.

- **Search Transaction:**
  ```sql
  SELECT * FROM tbl_tx WHERE txid = ?;
  ```
  Used by the `searchtx` endpoint.

- **Get All Addresses by XPub:**
  ```sql
  SELECT a.address
  FROM tbl_address a
  JOIN tbl_xpub x ON a.xpub = x.id
  WHERE x.xpub = ?;
  ```
  Used to load all known addresses for an xpub in a single query, enabling O(1) fee validation in the push handler.

- **Get Last Used Address by IP:**
  ```sql
  SELECT address FROM tbl_address
  WHERE remote_address = ? AND xpub = ?
  ORDER BY date_create DESC LIMIT 1;
  ```
  Used to check if an IP already has a generated address (address reuse prevention).

- **Get Next Address Index:**
  ```sql
  UPDATE tbl_xpub SET path_idx = path_idx + 1
  WHERE network = ? AND xpub = ?
  RETURNING id, path_idx;
  ```
  Atomically increments the derivation index and returns the new value.

---

## Data Lifecycle
- **Creation:** Transactions are created when a user submits raw hex tx via `POST /{network}/pushtxs`. Addresses are derived on-demand in xpub mode.
- **Waiting:** Transactions are in `status=0` and are queried by the pusher every new block.
- **Broadcast:** Transactions are pushed via `sendrawtransaction`. If successful, status becomes `1`. If the RPC returns an error, status becomes `2` and the error is stored in `push_err`.
- **Statistics:** The pusher aggregates stats from the database and upserts into `tbl_stats` with `ON CONFLICT(chain) DO UPDATE`.
- **Retention:** There is no explicit cleanup mechanism for old records. For production, a periodic vacuum or purge of old `status=1` transactions may be required.
- **Backup:** The database is a single SQLite file. It can be copied directly using `cp` or `rsync`. WAL mode ensures consistency during copies.
