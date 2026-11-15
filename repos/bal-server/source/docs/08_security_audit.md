# Security Audit

## Quick Reference
- **What this file contains:** threat model, vulnerability assessment, hardening recommendations, and a security checklist.
- **See also:** [AGENTS.md](AGENTS.md), [07_deployment_and_ops.md](07_deployment_and_ops.md), [06_database_schema.md](06_database_schema.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [04_modules_detail.md](04_modules_detail.md)

---

## Threat Model

### Assets
1. **`bal.db` (SQLite database):** Contains all transaction details, user IP addresses, and stats data. Single unencrypted file on disk. WAL mode enabled for concurrent access safety.
2. **Private Keys (`privkey.pem`):** Used to sign statistics payloads for the `welist` server. Located in `.gitignore`.
3. **Bitcoin Node (`bitcoind`) Access:** The `bal-pusher` has RPC access. Compromise allows arbitrary transaction broadcasting.
4. **Server Availability (`bal-server`):** Public-facing HTTP endpoint. DoS attacks threaten service availability.
5. **WeTime ping keys:** `SSL_KEY_PATH` signs stats/pings. Onion delegation adds a dedicated, rotatable online key; the onion identity key stays offline and is used only to issue delegation certificates.

### Attackers
- **Remote Anonymous Users:** Can interact with the API via public HTTP. No credentials required.
- **Network Man-in-the-Middle (MITM):** TLS termination is via Nginx reverse proxy. The `bal-server` itself is plain HTTP.
- **Local/Insider Threats:** If the server is compromised, the attacker can access `bal.db`, private keys, and env files.

---

## Vulnerability Assessment

### 1. SQL Injection (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/db.rs`, `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** SQL queries previously used `format!()` for string interpolation. All queries now use parameterized statements (`?` with `bind()`).
**Mitigation Applied:** All SQL queries rewritten with prepared statements. `execute_insert` uses parameterized batch inserts. `check_duplicate_txids` uses parameterized `IN` clauses. `echo_stats` handler uses prepared statements for chain filtering.
**Regression Tests:** `tests/sql_injection_tests.rs` (3 tests).

### 2. Panic on Untrusted Input (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** All `unwrap()`/`expect()` calls on critical paths have been replaced with safe error handling.
**Mitigation Applied:**
- `bal-server`: `from_utf8` returns 400, `sqlite::open` uses `open_db()` with validation, all handlers return proper HTTP status codes.
- `bal-pusher`: RPC failures log errors + sleep + retry (no panic), ZMQ `recv` uses `set_rcvtimeo(5000)`, malformed ZMQ frames are handled with `get()`/`first()` and `from_utf8_lossy` (no indexing or UTF-8 `expect`), stats signing returns an error that is logged and skipped rather than `expect`-panicking.
**Regression Tests:** `tests/panic_regression_tests.rs` (2 tests).

### 3. Secret Leakage (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `make_release.sh`, `.gitignore`
**Description:** `make_release.sh` now loads `TOKEN` from `.env` (`.env.example` provided). Private keys (`.pem`, `.key`) are in `.gitignore`. `generate_keys.sh` sets `chmod 600` on generated keys.
**Mitigation Applied:** Secrets removed from scripts and repository. `.gitignore` protects `.env`, `*.pem`, `*.key` files.
**Regression Tests:** `tests/secret_leakage_tests.rs` (3 tests).

### 4. Denial of Service (DoS) (FIXED)
**Severity:** HIGH | **Status:** Fixed
**Location:** `src/bin/bal-server.rs` (HTTP), `src/bin/bal-pusher.rs` (ZMQ)
**Description:** All DoS vectors mitigated via actix-web migration.
**Mitigation Applied:**
- Body size limit: `PayloadConfig::default().limit(max_body_size)` via `BAL_SERVER_ACTIX_MAX_BODY_SIZE` (default 1 MiB).
- Rate limiting: `actix-governor` with a token bucket per client IP, with an **independent bucket for each endpoint class** (push txs, searchtx, info/stats, default) so that polling a cheap endpoint cannot exhaust the push-txs budget. Uses `RealIpKeyExtractor` to extract the real client IP from proxy headers.
- Connection limits: `workers(4)` and `max_connections(100)` via `BAL_SERVER_ACTIX_WORKERS`/`MAX_CONNECTIONS`.
- Request timeout: `middleware::from_fn` wraps every handler in `tokio::time::timeout`, returning `408 Request Timeout` when `BAL_SERVER_ACTIX_TIMEOUT_SECS` (default 5) elapses, covering slow request bodies.
- ZMQ timeout: `set_rcvtimeo(5000)` prevents infinite blocking.
- RPC retry: sleep + retry on connection failure instead of panic.
- ZMQ message handling is panic-free: multipart frames are accessed with `get()`/`first()` and topics are decoded with `from_utf8_lossy`, so malformed frames cannot abort the pusher.
- Stats-report signing failures are logged and skipped instead of panicking the pusher when the key is unreadable.

### 5. SSRF / Network Abuse via `reqwest` (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/bin/bal-pusher.rs`, `src/validation.rs`
**Description:** URL validation prevents redirecting requests to internal/private IPs.
**Mitigation Applied:** `is_valid_welist_url()` in `src/validation.rs` blocks localhost, loopback, RFC1918, link-local, multicast, unspecified, IPv6 unique-local addresses, and **IPv4-mapped IPv6 addresses** (`::ffff:a.b.c.d`) that route to loopback/private/link-local/unspecified IPv4 targets. HTTPS-only scheme enforced. Note: hostnames are not DNS-resolved before validation, so a domain that resolves to a private address is not caught by this check — restrict `WELIST_SERVER_URL` to trusted values.
**Regression Tests:** `tests/ssrf_tests.rs` (integration) + 11 unit tests in `src/validation.rs`.

### 6. Insecure Database Access (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/db.rs`, `src/bin/bal-server.rs`, `src/bin/bal-pusher.rs`
**Description:** Database path validation and WAL mode for concurrent access.
**Mitigation Applied:**
- `open_db()` rejects `..` traversal, forbidden system directories (`/etc`, `/proc`, `/sys`, `/dev`, `/usr`, `/bin`, `/sbin`, `/lib`, `/opt`), symlinks, and non-regular files.
- WAL mode (`PRAGMA journal_mode=WAL`) with retry logic (up to 5 attempts).
- `busy_timeout=5000` for concurrent access.
- `bal-server` uses `Arc<Mutex<Connection>>` for thread-safe shared access.
**Regression Tests:** `tests/db_path_validation.rs` (5 tests).

### 7. ZMQ Authentication and Encryption (OPEN)
**Severity:** MEDIUM | **Status:** Open
**Location:** `src/bin/bal-pusher.rs`
**Description:** ZMQ connection is plaintext TCP. No authentication (ZAP), no encryption (ZMQ_CURVE). If the ZMQ port is exposed, any attacker can subscribe to topics.
**Mitigation (Operational):**
- Bind ZMQ to `127.0.0.1` only.
- Firewall blocks external access to ZMQ ports.
- If public ZMQ is required, use ZMQ_CURVE with public-key cryptography.

### 8. Missing HTTPS / Insecure Server Communication (FIXED)
**Severity:** HIGH | **Status:** Fixed (Infrastructure)
**Location:** Nginx configuration, `contrib/nginx/bal-server.conf`
**Description:** TLS termination via Nginx reverse proxy. The `bal-server` intentionally does not implement TLS.
**Mitigation Applied:**
- Dedicated Nginx template with `listen 443 ssl http2`, Let's Encrypt paths.
- Security headers: `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`.
- `client_max_body_size` matching `BAL_SERVER_ACTIX_MAX_BODY_SIZE`.
- HTTP 80 redirect to HTTPS.
- Deployment checklist ensures no accidental plain HTTP exposure.

### 9. Missing Input Validation (FIXED)
**Severity:** MEDIUM | **Status:** Fixed
**Location:** `src/bin/bal-server.rs`
**Description:** Network, txid, and content validation.
**Mitigation Applied:**
- Network validation: `NETWORKS.contains(&param.as_str())` before processing. Unknown networks return 404.
- Txid validation: `echo_search` requires exactly 64 ASCII hex characters. Non-hex or wrong length returns 400.
- XPub address caching: `get_all_addresses_by_xpub` loads all addresses once per batch (O(1) lookup), eliminating N+1 queries.
- Content-Length: Handled by actix-web `PayloadConfig` size limit.
**Regression Tests:** `tests/input_validation_tests.rs` (4 tests).

### 10. Information Leakage (MITIGATED)
**Severity:** LOW | **Status:** Mitigated
**Location:** Application logs
**Description:** Raw file logging (`valid_txs`/`invalid_txs`) has been removed. `info!`/`warn!` macros may still log txid and details in application logs.
**Mitigation (Operational):** Set production `RUST_LOG` to `warn` or higher. Log files restricted with `chmod 600`.

### 11. WeTime Metadata Transport (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `src/bin/bal-pusher.rs` (removed `fetch_executor_metadata`), `src/db` (`tbl_executor_metadata`), `src/bin/bal-server.rs`
**Description:** The v2 pusher previously fetched `info`, `base_fee` and `version` from `{BAL_SERVER_URL}/{chain}/info` and signed them. In that configuration `BAL_SERVER_URL` is the public hostname, so an HTTP MITM could choose the signed metadata. The response body was also unbounded, and xpub mode derived a deposit address for the pusher IP.
**Mitigation Applied:** `bal-server` writes the metadata to `tbl_executor_metadata` at startup and the pusher reads it from the shared database. No HTTP request is made. Metadata is best effort: if the table or row is missing, the pusher signs `null` values instead of blocking. When `pubkey_pem` is present, the pusher verifies that it matches the key derived from `SSL_KEY_PATH`; a malformed PEM or mismatch skips the ping. The server caches the PEM at startup and serves the same value at `/.pub_key.pem`.
**Residual risk:** The database is trusted. An attacker who can write it can alter metadata, but can already alter transaction state. The key check detects configuration drift, not a compromised server.
**v2 and v3 metadata signing:** both schemes sign `info`, `base_fee` and `version` in a canonical JSON payload. v2 verifies the signature with the key served at `/.pub_key.pem`, so its authenticity depends on TLS and the service fetching that endpoint. v3 uses DNSSEC or the onion identity key and remains the stronger option.

### 12. Onion Ping Delegation (DESIGN / ACCEPTED RISK)
**Severity:** LOW | **Status:** Accepted
**Location:** `src/bin/bal-pusher.rs` (`validate_onion_delegation`, `delegation_for_url`); certificate verification on the WeTime `/ping` endpoint
**Description:** Onion pings may carry a `delegation` certificate that binds a dedicated Ed25519 ping key to the onion host until `valid_until`. The certificate is signed by the onion identity key embedded in the `.onion` address, so the identity key can stay offline.
**Compromise impact:** An attacker who obtains the online ping key and its certificate can sign pings and metadata for that onion host until the key is revoked or the certificate expires. It cannot take over the onion service identity, sign for another host, or forge a certificate without the identity key.
**Revocation:** `POST /ping-keys/revoke` accepts a signed `WELIST-PING-REVOCATION-V1` statement. Onion revocations are signed by the identity key (which cannot itself be revoked); DNS TXT revocations are self-signed by the key being revoked, the only authority available for a TXT record. Entries are permanent and key-scoped, checked before the ping signature on both paths, and fail closed on database errors.
**Accepted risks:**
- **Rotation alone does not revoke:** a leaked key must be explicitly revoked. If the signing authority is lost (onion identity key, or the DNS private key), the key remains usable until `valid_until` or the DNS cache TTL passes. Use short lifetimes (30–90 days).
- **Self-signed DNS revocation:** anyone holding the DNS key, including an attacker who stole it, can sign a revocation for that key. `bal-welist` additionally requires the key to be bound to the host's current DNSSEC TXT record or cache and the host to be registered with a positive balance, so a revocation cannot be forged for an unrelated host.
- **Replay:** the signed payload has no nonce. v2 and v3 pings are covered by `bal-welist` replay protection: an exact duplicate `(executor, chain, height, blockhash)` is acknowledged without side effects, a higher height is accepted, and a lower or conflicting height requires an RPC-verified canonical reorg within `WELIST_MAX_REORG_DEPTH` (default 100; `0` disables rewinds). Other lower/conflicting heights return `409`.
**Mitigation Applied:** The offline identity key issues certificates; `SSL_KEY_PATH` holds only the delegated key. The pusher verifies the certificate signature against the identity key embedded in the onion address. A present but invalid, expired or mismatched certificate is rejected without downgrade, `delegation` is rejected on non-onion hosts, and revoked keys are rejected for both authentication paths.

### 13. Local v3 DNS TXT Consistency Check (ADVISORY)
**Severity:** LOW | **Status:** Advisory
**Location:** `src/bin/bal-pusher.rs` (`TxtCheckCache`, `lookup_txt_key`)
**Description:** Before the first v3 WeTime ping after startup, the pusher queries `_welist-key.<host>` through the system resolver without local DNSSEC validation and compares the key with `SSL_KEY_PATH`. This is a configuration consistency check, not an authentication boundary; the service performs DNSSEC validation.
**Behaviour:** NXDOMAIN, empty answers, malformed records, or a mismatching key trigger one retry after 30 seconds; two definitive mismatches skip WeTime pings until restart. Timeout or SERVFAIL is transient and does not block. The result is cached per signing key for the process lifetime.
**Residual risk:** A spoofed DNS response can cause a local denial of service by producing two definitive mismatches; it cannot authenticate a key to the service. The service remains authoritative.
**Mitigation Applied:** Two lookups 30 seconds apart, transient errors do not block, and no DNSSEC state is trusted locally.

### 14. `bal-stats.rs.dontcompile` (REMOVED)
**Severity:** LOW | **Status:** Removed
**Description:** The broken HTML report generator file no longer exists in the source tree.

### 15. Canonical v3 URL and Delegation Lifetime (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `src/url_policy.rs`, `src/bin/bal-pusher.rs`
**Description:** The v3 signature covers the exact `url` string, but the pusher previously only trimmed whitespace and trailing slashes. Case, explicit default ports, paths, userinfo, raw Unicode, and IP literals could produce multiple signed representations of the same executor. Delegation certificates also had no upper lifetime bound.
**Mitigation Applied:** A shared `canonical_executor_url` policy requires an ASCII origin with a lowercase host, no trailing dot, no userinfo/path/query/fragment, no IP literal, no `localhost`, and no explicit default port; non-default ports are preserved. The pusher canonicalizes `BAL_SERVER_URL` before signing and skips v3 pings with a clear error when the value is not canonical. `BAL_PUSHER_MAX_DELEGATION_DAYS` (default 366; `0` disables) caps `valid_until`.
**Residual risk:** Punycode `xn--` labels remain visually confusable with other domains; operators are responsible for publishing the intended A-label. The service authenticates the host key, not the visual appearance.

### 16. Revocation Left a Poisoned Replay Height (FIXED)
**Severity:** HIGH (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`revoke_ping_key`, `evaluate_replay`), `src/ping_auth.rs` (`clear_ping_heights`)
**Description:** `evaluate_replay()` accepts any height above the stored one. A compromised v3 key could publish a valid ping with a height near `MAX_PING_HEIGHT` and then be rotated. Revocation removed the key but not `executor_ping_height`, so the replacement key's real ping was far below the stored height, exceeded `WELIST_MAX_REORG_DEPTH`, and was rejected as stale permanently.
**Mitigation Applied:** A newly stored revocation deletes every `executor_ping_height` row for the host when the host has reached lock level `3`. The revocation insert and the delete run in one transaction. Duplicate submissions are acknowledged without resetting replay state, so replaying a published revocation certificate cannot make old pings valid again. Revocations stored by older versions are healed by the startup backfill (`backfill_revoked_host_state`), which clears the same state once per process start.
**Residual risk:** A database failure during the transaction returns `"ko"` and leaves both writes unapplied; the operator can retry.

### 17. `/ping` Throttle Could Block Revocation (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`revoke_ping_key`)
**Description:** `/ping` and `/ping-keys/revoke` shared one per-host DNS refresh cooldown. An attacker could keep a host's cooldown active with invalid v3 pings, causing the legitimate owner's revocation request to return `503` before it performed the TXT lookup needed to authorize it.
**Mitigation Applied:** Revocation uses its own per-host cooldown. Invalid ping traffic can still throttle further invalid pings, but it cannot starve the authenticated recovery path.

### 18. Signed Host Fields Were Not Normalized (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/ping_auth.rs` (`OnionDelegation::canonical_bytes`, `PingKeyRevocation::canonical_bytes`)
**Description:** The canonical delegation/revocation payloads used the raw `onion`/`host` and `key` strings, while `bal-pusher` and the helper scripts normalize the host and trim the key before signing. A certificate with an uppercase onion or a trailing dot was rejected by the service even though every producer considered it valid.
**Mitigation Applied:** Both canonical payload builders trim `key` and apply the same trim/strip-trailing-dot/lowercase normalization used for host comparison. This removes a producer/verifier mismatch without weakening any trust check.

### 19. Host Squatting and Zero-Balance Attribution (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`ping`, `insert_data`), `src/willexecutor.rs` (`migrate_executor_host`)
**Description:** registration deduplicated on `(url, chain, contact)` and v3 lookup accepted any row with the same host, ordered by `(balance > 0) DESC, id ASC`. A second registration for the same host on a different port/path could receive attribution for pings authenticated by the owner's key, and zero-balance rows could still authenticate pings.
**Mitigation Applied:** registration checks and the partial unique `idx_willexecutors_active_host_chain_contact` index enforce `(host, chain, contact)` with `COALESCE(contact, '')` for rows with `balance > 0`. Zero-balance re-registrations are inert. All ping lookups require `balance > 0` and use `MIN(id)`. If legacy duplicate funded rows block index creation, the error is logged so the operator can merge them.
**Residual risk:** historical zero-balance duplicates remain in the table but cannot authenticate pings; duplicate funded rows cannot create the unique index until merged. Because the active identity includes `contact`, two funded contacts may legitimately share the same host and chain. A host-authenticated v3 ping is attributed to the lowest `id` for `(host, chain)`, so the lowest-id funded row receives the attribution. `migrate_executor_host` logs each such host at startup (`Multiple funded executors share host=...`) so operators can review unexpected rows.

### 20. `/ping` Had No General Per-IP Rate Limit (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`ping`, `RateLimiter`)
**Description:** only `/ping-keys/revoke` had a per-IP limiter. An attacker could flood `/ping` with cheap requests, consuming CPU and triggering one DNS refresh per host per cooldown window.
**Mitigation Applied:** `/ping` uses a fixed 10-minute window limited to 20 requests per client IP (`WELIST_PING_RATE_PER_10_MINUTES`, `0` disables). The check runs before chain parsing or key lookup and returns `429`, so invalid requests are counted too.

### 21. v3 DNS Lookup Accepted Non-Public Hostnames (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/url_policy.rs` (`is_public_dns_name`), `src/bin/webservice.rs` (`revoke_ping_key`)
**Description:** a funded registrant could choose an internal or split-horizon hostname, causing the service to query its configured resolver for `_welist-key.<host>`. The service never connects to the resolved address for v3, but the lookup could load the resolver or reveal internal-name existence through timing.
**Mitigation Applied:** `canonical_executor_url` now requires a public DNS name (at least two labels, no private/special-use suffix) for every v3 host except `.onion`. The DNS revocation endpoint applies the same rule. The shared `url_policy` also makes the pusher reject such `BAL_SERVER_URL` values at startup.

### 22. Cached v3 DNS Key Was Not Authoritative (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`verify_v3_signature`, `revoke_ping_key`), `src/ping_auth.rs` (`clear_cached_dns_keys`)
**Description:** whenever a cached key failed to verify a signature, the service performed another DNS lookup. Every invalid v3 ping could therefore drive resolver traffic, and a rotated key could be discovered without an explicit cache-invalidating event.
**Mitigation Applied:** a non-expired, non-revoked cached key is authoritative; a mismatch is rejected without a DNS refresh. DNS is queried only when no valid cached key remains. A newly stored revocation deletes the host's cached DNS keys in the same transaction, so the replacement key is discovered on the next ping. Rotation without revocation waits for the cached-key TTL.

### 23. v1/v2 Pusher Signed the Unnormalized URL (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-server` `src/bin/bal-pusher.rs` (`send_stats_report`, `legacy_ping_message`)
**Description:** the service normalizes the request URL before rebuilding the v1/v2 signed message, but the pusher signed `BAL_SERVER_URL` verbatim. A trailing slash or surrounding whitespace made every v1/v2 ping fail with `"ko"`, which looked like an authentication failure rather than a configuration problem.
**Mitigation Applied:** the pusher normalizes the URL once per report and uses the same value in the signed message and the request body. Regression test: `legacy_ping_message_normalizes_the_configured_url`.

### 24. Re-registration Left the Stored Executor URL Stale (FIXED)
**Severity:** MEDIUM (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`insert_data`)
**Description:** `POST /data` reused an existing row by `(host, chain, contact)` without updating `willexecutors.url`. Re-registering the same host with a different scheme, port or path returned the new URL to the client while the row kept the old one, so v1/v2 pings failed the exact `(url, chain)` lookup and `update_onion_url` could not find the row.
**Mitigation Applied:** a reused row's `url` is updated to the normalized request URL before address and onion handling run. A unique-constraint collision returns `409` instead of a silent success. Regression test: `test_post_data_host_change_updates_stored_url`.

### 25. `/.pub_key.pem` Error Body Could Be Cached as the Key (FIXED)
**Severity:** LOW (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`get_public_key_pem`)
**Description:** the service read `res.text()` without checking the HTTP status. If `bal-server` started before its key file existed, it returned `500` with body `"error"`, which was stored in `willexecutors.pub_key`. A non-empty cached key is never re-fetched, so v1/v2 pings stayed broken until an operator cleared the column.
**Mitigation Applied:** non-2xx responses and empty bodies are treated as a failed fetch and are not cached.

### 26. Revocation Duplicate Contract and Legacy Cleanup (FIXED)
**Severity:** LOW (was) | **Status:** Fixed
**Location:** `bal-welist` `src/bin/webservice.rs` (`revoke_ping_key`, `backfill_revoked_host_state`)
**Description:** the public-name policy ran before the idempotent duplicate check, so a revocation recorded by an older version for an internal host returned `403` instead of `"ok"`. Cleanup of cached keys and replay heights only ran when a new row was inserted, so revocations stored by older versions kept poisoned state.
**Mitigation Applied:** the duplicate check runs before the public-name policy and still returns `"ok"`. It deliberately does not clear replay state, because a published revocation certificate can be replayed and must not be able to reset the stored height. A startup backfill (`backfill_revoked_host_state`) clears cached keys and, for hosts at lock level 3, replay heights for every stored revocation.

### Residual v3 Risks (ACCEPTED / DOCUMENTED)
**Severity:** LOW | **Status:** Accepted with monitoring
- **Punycode confusables:** `xn--` A-labels remain visually confusable with other domains; operators must publish the intended A-label.
- **Rotation without revocation:** a rotated key is not discovered until the cached key expires. Operators must revoke the old key or wait for the TTL; this is documented in the DNS publishing guide.

---

## Hardening Recommendations

### System-Level
1. Run as non-root user with systemd hardening (`ProtectSystem=full`, `NoNewPrivileges`, `PrivateDevices`, `MemoryDenyWriteExecute`).
2. Use firewall to block all inbound ports except HTTPS (443) and SSH (22).
3. Use VPN or Tor for `welist` connections if on public network.
4. Run in a container or chroot for isolation.
5. Enable SELinux or AppArmor profiles for the binaries.
6. Use read-only filesystem for the server binary.

### Application-Level
1. **Rate Limiting:** Implemented via `actix-governor` with token-bucket per client IP and a **separate bucket per endpoint class**. `RealIpKeyExtractor` identifies clients behind reverse proxy using `X-Real-IP` / `X-Forwarded-For` headers. Trusted proxy IP configurable via `BAL_SERVER_TRUSTED_PROXY`.
2. **Input Validation:** Network enum check, txid hex validation, body size limits.
3. **Request Timeout:** `tokio::time::timeout` middleware returns `408` when a handler (including body read) exceeds `BAL_SERVER_ACTIX_TIMEOUT_SECS`.
4. **HTTPS:** Via Nginx reverse proxy with Let's Encrypt.
5. **WAL Mode:** Enabled with retry logic for concurrent access.
6. **ZMQ Timeout:** 5-second receive timeout prevents infinite blocking.
7. **Transaction Size Limits:** Configurable via rate limiting and body size.
8. **ZMQ Retry:** Reconnect logic with timeout-based detection.
9. **Fee Limits:** Per-network `fixed_fee` configuration.
10. **Network Limits:** Only known networks accepted (bitcoin, testnet, testnet4, signet, regtest).
11. **Locktime Reasonableness:** Locktime compared against blockchain height and median time with threshold-based distinction.

---

## Regression Test Suite

| Test File | Tests | Coverage |
|-----------|-------|----------|
| `tests/sql_injection_tests.rs` | 3 | Parameterized queries, injection prevention |
| `tests/panic_regression_tests.rs` | 2 | Mutex poisoning recovery, NULL value handling |
| `tests/ssrf_tests.rs` | 4+ | URL validation, internal IP blocking |
| `tests/secret_leakage_tests.rs` | 3 | .gitignore, no tracked secrets, no hardcoded tokens |
| `tests/input_validation_tests.rs` | 4 | Address caching, network validation, txid hex validation |
| `tests/db_path_validation.rs` | 5 | Path traversal, forbidden dirs, symlinks, WAL pragma, valid paths |
| `src/validation.rs` (inline) | 11 | SSRF URL validation unit tests |
