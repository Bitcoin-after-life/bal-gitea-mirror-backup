# WeTime v3 ping keys: DNSSEC TXT and onion delegation

This document explains how to publish the Ed25519 public key used by the
`bal-pusher` v3 WeTime ping, how to delegate onion ping signing to a dedicated,
rotatable key, and how to verify the configuration.

In v3 the WeTime service discovers the executor public key from:

- a DNSSEC-validated TXT record at `_welist-key.<host>`, or
- the Ed25519 identity key embedded in a v3 `.onion` address, or a dedicated
  ping key delegated by that identity key.

Sections 1–10 below cover the DNS TXT record; onion delegation is documented in
[Onion URLs](#onion-urls). Onion authentication needs no DNS record and should
use delegation so the onion identity key stays offline. For local development
no record is needed either (see [Local development](#local-development)).

> **v2 also exists.** `WELIST_PING_SIG_VERSION=2` uses the same canonical JSON
> payload and canonical URL as v3, but verifies the signature with the key served
> at `https://<host>/.pub_key.pem` instead of DNSSEC or the onion identity key.
> It needs no TXT record and no DNSSEC. The DNSSEC TXT and onion delegation guide
> below applies to v3 only. A valid v2 ping disables v1 for that host; a valid v3
> ping disables both v1 and v2.

All values below use `we.example.com` as a placeholder for the hostname in
`BAL_SERVER_URL`. Replace it with your real executor hostname.

---

## 1. Choose the hostname

Use the public hostname that clients use to reach your executor:

```text
BAL_SERVER_URL=https://we.example.com
```

The DNS key is published at:

```text
_welist-key.we.example.com
```

Rules:

- Use a DNS hostname, not an IP address. The service rejects v3 pings whose URL
  host is an IP address with `v3 DNS lookup requires a hostname`.
- Use the host only: no scheme, port, path, or trailing slash.
- The record must exist at exactly `_welist-key.<host>`. CNAME answers are
  rejected by the service.

---

## 2. Generate or confirm the Ed25519 key pair

The pusher signs with the private key at `SSL_KEY_PATH`. The TXT record must
contain the matching public key.

> **The pusher and the bal-server use two separate settings.** `SSL_KEY_PATH`
> is the private key used for signing; `BAL_SERVER_PUB_KEY_PATH` is the public
> key served at `/.pub_key.pem` and used by the legacy v1 scheme (and by the
> check in section 7). Configure both with the same key pair.

If you already have a key pair for the executor, reuse it. Do not generate a
new pair for `SSL_KEY_PATH` alone: the legacy v1 signature is verified against
`/.pub_key.pem`, so a split pair breaks v1 authentication and makes the
section 7 check fail.

> **Onion hosts:** do not point `SSL_KEY_PATH` at the onion identity key. Use a
> separate ping key and issue a delegation certificate (see
> [Onion URLs](#onion-urls)). The identity key is only needed offline, once, to
> sign that certificate. Set `BAL_SERVER_PUB_KEY_PATH` to the **delegated ping
> public key**, not the onion identity public key; the server caches it at
> startup and serves it at `/.pub_key.pem`, which keeps the v1 path and the
> pusher consistency check aligned.

If you do not have a key pair yet:

```bash
openssl genpkey -algorithm ED25519 -out private_key.pem
openssl pkey -in private_key.pem -pubout -out public_key.pem
chmod 600 private_key.pem
```

Alternatively, `contrib/make_welist_txt.sh <host>` generates
`private_key.pem` and `public_key.pem` in the current directory when the key
argument is omitted, then prints the TXT record. The private key is created at
mode `0600`. If `./private_key.pem` already exists, the script reuses it instead
of overwriting it. Move both files to a protected location before starting the
services.

Then point both settings at the new pair and restart `bal-server` and
`bal-pusher`:

```bash
export SSL_KEY_PATH="/absolute/path/to/private_key.pem"
export BAL_SERVER_PUB_KEY_PATH="/absolute/path/to/public_key.pem"
```

Keep `private_key.pem` private and outside version control. The TXT record only
contains the public key.

---

## 3. Build the TXT value

Derive the base64 SubjectPublicKeyInfo (SPKI) DER from the private key:

```bash
KEY=$(openssl pkey -in private_key.pem -pubout -outform DER | base64 -w0)
echo "v=we1; alg=ed25519; key=$KEY"
```

On platforms where `base64 -w0` is unavailable (for example macOS):

```bash
KEY=$(openssl pkey -in private_key.pem -pubout -outform DER | base64 | tr -d '\n')
echo "v=we1; alg=ed25519; key=$KEY"
```

`contrib/make_welist_txt.sh <host> [key]` prints the record name, value, and a
BIND zone line directly. `key` may be a private/public PEM file or an existing
base64 SPKI DER value; omit it to generate a new key pair first.

The value has this form:

```text
v=we1; alg=ed25519; key=MCowBQYDK2VwAyEA...
```

Notes:

- `key` is the base64 of the **full SPKI DER**, not a hash and not the PEM text.
  It is the same base64 body that appears in `public_key.pem`.
- Use standard base64 (`+`, `/`, `=` padding), not base64url.
- Ed25519 SPKI DER is 44 bytes, so the base64 value is about 60 characters and
  the whole TXT string is about 90 characters. It fits in a single TXT string.
- If a DNS provider splits the value into multiple quoted strings, that is
  acceptable: the resolver concatenates TXT chunks before parsing.

---

## 4. Enable DNSSEC

The service validates DNSSEC and **fails closed**. An unsigned or bogus answer
is rejected; there is no unauthenticated fallback.

Enabling DNSSEC normally requires two steps:

1. Enable DNSSEC signing for the zone at the DNS provider.
2. Publish the provider's **DS record** at the registrar or parent zone.

The whole chain from the root to the zone that contains
`_welist-key.we.example.com` must be signed.

### Verify DNSSEC

```bash
dig +short DS example.com
dig +dnssec DNSKEY example.com
dig +dnssec TXT _welist-key.we.example.com
```

Look for `RRSIG` records in the answers. A validating resolver also sets the
`ad` flag:

```text
;; flags: qr rd ra ad; QUERY: 1, ANSWER: 2, ...
```

If `dig` does not show `ad`, your local resolver may not validate. Use a
validating tool instead:

```bash
delv TXT _welist-key.we.example.com
```

or:

```bash
kdig +dnssec TXT _welist-key.we.example.com
```

### Resolver used by the WeTime service

The service performs its own validation with a built-in root trust anchor. The
upstream resolver only needs to return DNSSEC records.

If the service host uses a stub resolver that strips DNSSEC data, point it at a
DNSSEC-capable resolver:

```bash
export WELIST_DNS_NAMESERVERS=1.1.1.1,9.9.9.9
```

`WELIST_DNS_NAMESERVERS` replaces the system resolver configuration; each entry
is an `IP` or `IP:PORT` (default port 53).

You can also point at a custom trust anchor file:

```bash
export WELIST_DNSSEC_TRUST_ANCHOR=/path/to/root.key
```

### Local development

DNSSEC cannot authenticate `localhost` or an IP address. For local development
use the legacy scheme and let the service verify the published public key:

- Keep `WELIST_PING_SIG_VERSION` unset (or `1`), so the pusher signs with
  `SSL_KEY_PATH` and the service verifies against `GET /.pub_key.pem`.
- For loopback or private addresses, start the service with
  `WELIST_ALLOW_LOCALHOST_PING=true`. Legacy v1 pings are accepted by default;
  set `WELIST_ALLOW_LEGACY_PING=false` only if the service must disable v1 (v2
  and v3 remain available).

No TXT record and no DNSSEC setup are required in this case.

---

## 5. Publish the TXT record

| Field | Value |
|-------|-------|
| Name | `_welist-key.we.example.com` |
| Type | `TXT` |
| Value | `v=we1; alg=ed25519; key=<base64 SPKI DER>` |
| TTL | `300` to `3600` seconds |

The WeTime service caches DNS keys for the TXT TTL, clamped to 60–86400
seconds. A non-expired, non-revoked cached key is authoritative; a signature
mismatch does not trigger a refresh. Rotate by revoking the old key (which clears
the cache) or by waiting for the cached-key TTL. A TTL of `300`–`3600` keeps
rotation reasonably fast.

### BIND / generic zone file

```zone
_welist-key.we.example.com. 3600 IN TXT "v=we1; alg=ed25519; key=MCowBQYDK2VwAyEA..."
```

Reload or re-sign the zone after adding the record. If you use inline signing,
the record is signed automatically. If you sign offline, re-run your signing
step and publish the new signed zone.

### Cloudflare

1. Enable DNSSEC in the Cloudflare dashboard.
2. Add the DS record shown by Cloudflare at your registrar.
3. Create the TXT record:

- Type: `TXT`
- Name: `_welist-key.we` (Cloudflare appends the zone name)
- Content: `v=we1; alg=ed25519; key=MCowBQYDK2VwAyEA...`
- TTL: `Auto` or `1h`

Cloudflare proxying applies to HTTP records, not TXT. The TXT record is served
directly, so DNSSEC validation works normally.

### Route 53

1. Enable DNSSEC signing for the hosted zone.
2. Add the resulting DS record at the registrar or parent zone.
3. Create the record:

- Record name: `_welist-key.we.example.com`
- Record type: `TXT`
- Value: `"v=we1; alg=ed25519; key=MCowBQYDK2VwAyEA..."`
- TTL: `3600`

Route 53 may split long TXT values into quoted chunks. That is supported.

### Important

- Do not create a CNAME for `_welist-key`. The service rejects CNAME answers.
- Do not put the private key, a hash, or PEM headers in the TXT value.
- If you need to host the record in another zone, use a proper DNSSEC-signed
  delegation (NS + DS), not a CNAME.

---

## 6. Configure the pusher

Set these variables in the pusher environment (for local development, the
`bal-pusher.sh` launcher requires `-ok-dev` or `BAL_DEV=true`):

```bash
export SSL_KEY_PATH="/absolute/path/to/private_key.pem"
export BAL_SERVER_URL="https://we.example.com"
export WELIST_PING_SIG_VERSION=3
```

- `SSL_KEY_PATH` must point to the private key matching the published TXT key.
- `BAL_SERVER_URL` must be a canonical origin for v2 and v3: ASCII
  `scheme://host[:port]`, lowercase host without a trailing dot, no
  path/query/fragment/userinfo, and no explicit default port. Raw Unicode must
  use the `xn--` form. The pusher canonicalizes it with the same `url_policy`
  module as `bal-welist`; values it cannot canonicalize (IP literal, path, query,
  raw Unicode) are logged and v2/v3 pings are skipped.
- `WELIST_PING_SIG_VERSION=3` activates the canonical JSON signature with
  DNSSEC/onion key discovery. Leaving it unset (or `1`) keeps the legacy v1
  scheme, which the service accepts by default (set
  `WELIST_ALLOW_LEGACY_PING=false` to disable v1; v2 and v3 remain available).
  `WELIST_PING_SIG_VERSION=2` uses the same canonical JSON payload with
  `/.pub_key.pem` key discovery and needs no TXT record.

For onion hosts, also configure the delegation certificate (see
[Onion URLs](#onion-urls)):

```bash
export SSL_KEY_PATH="/absolute/path/to/ping_private_key.pem"
export WELIST_PING_DELEGATION_FILE="/absolute/path/to/onion-delegation.json"
export BAL_SERVER_URL="http://<address>.onion"
export WELIST_PING_SIG_VERSION=3
```

- `SSL_KEY_PATH` is the **dedicated ping key**, not the onion identity key.
- `WELIST_PING_DELEGATION_FILE` is optional. When set, its JSON certificate is
  sent only with onion pings. The pusher validates it at startup: `onion` must
  match `BAL_SERVER_URL`, `valid_until` must be in the future and at most
  `BAL_PUSHER_MAX_DELEGATION_DAYS` (default 366) days ahead, and `key` must
  match `SSL_KEY_PATH`. If the file is missing or malformed, or any check
  fails, it logs a warning and disables delegation, then signs with
  `SSL_KEY_PATH` directly. That fallback works only when `SSL_KEY_PATH` is the
  onion identity key; otherwise the service rejects the pings until the
  certificate or key is corrected.

Restart the pusher after changing the key or environment.

If a ping key leaks, issue and submit a revocation before rotating; see
[Revocation and lifetime](#revocation-and-lifetime).

With v2 and v3, the pusher reads `info`, `base_fee`, and `version` from
`tbl_executor_metadata`, which `bal-server` writes from its effective startup
configuration. It never fetches `/{chain}/info` over HTTP. Metadata is signed
as `null` when it fails validation:

- `info` longer than 4096 bytes, or empty;
- `version` longer than 64 bytes, or empty;
- `base_fee` negative.

The same metadata is signed for `.onion` URLs. Metadata is best effort: if the
table or row is missing, the pusher signs `null` values instead of blocking.
When `tbl_executor_metadata.pubkey_pem` is present, it must match
`SSL_KEY_PATH`; a malformed PEM or mismatch skips the ping. Because the server
caches `BAL_SERVER_PUB_KEY_PATH` at startup and serves the same value at
`/.pub_key.pem`, rotating the key requires a server restart.

For v3 DNS hosts, the pusher performs one TXT consistency check before the first
WeTime ping after startup. It queries `_welist-key.<host>` through the system
resolver (without local DNSSEC validation), compares the key with
`SSL_KEY_PATH`, and on a mismatch retries once after 30 seconds. Two definitive
mismatches skip WeTime pings until restart; NXDOMAIN, empty answers and malformed
records count as mismatches. Timeout/SERVFAIL is transient and does not block.
The service remains the authoritative DNSSEC verifier.

---

## 7. Verify the published key

Query the record and extract the `key=` field (this also joins TXT chunks and
drops the quotes the resolver adds):

```bash
dig +short TXT _welist-key.we.example.com | tr -d '" ' | sed 's/^.*key=//'
```

Compare it with the local public key:

```bash
openssl pkey -in private_key.pem -pubout -outform DER | base64 -w0
```

The two commands must print the same base64 string.

Optionally check that the legacy `/.pub_key.pem` endpoint serves the same key:

```bash
curl -s https://we.example.com/.pub_key.pem \
  | openssl pkey -pubin -outform DER \
  | base64 -w0
```

This third check is only meaningful when `BAL_SERVER_PUB_KEY_PATH` and
`SSL_KEY_PATH` reference the same key pair, which is the recommended setup. The
endpoint serves whatever `BAL_SERVER_PUB_KEY_PATH` points at; if the two
settings use different pairs, compare the TXT record with `SSL_KEY_PATH` only.

---

## 8. Test a v3 ping

Wait for the next block, or restart the pusher. With trace logging enabled you
should see:

```text
canonical message to be sent: WELIST-PING-V3\n{"base_fee":...
Report to welist(...) status=200 body="ok"
```

The WeTime service should log `PING ADDED` and refresh the executor's
`last_update`. After a valid v3 (or v2) ping, signed `info`, `base_fee`, and
`version` values are persisted.

A response of `"ko"` means the signature was rejected. Check the service logs
for the specific `PING REJECTED` reason.

### Onion delegation

For an onion pusher with delegation configured:

1. At startup, the pusher must not log `disabling onion delegation`.
2. After the next block, look for `Report to welist(...) status=200 body="ok"`
   and `PING ADDED` in the service log.
3. Negative check: send a crafted v3 ping with a tampered certificate (change
   `key` or `valid_until` after signing) directly to the WeTime service. It must
   answer `"ko"` and log `PING REJECTED invalid onion delegation`. Do not run
   the pusher with a tampered certificate: it disables delegation locally
   instead of sending it.
4. Revocation check (destructive; use a test executor): issue a revocation with
   `contrib/make_ping_key_revocation.py --certificate ...`, submit it with
   `curl`, then send the delegated ping again. The service must log
   `PING KEY REVOKED` on submission, `PING REJECTED revoked onion delegation`
   on the ping, and answer `"ko"`. Generate a new ping key and certificate
   afterwards: a revoked key stays revoked.

`bal-welist/scripts/test_v3_ping.py` is a local end-to-end reference: it
generates a throwaway onion identity key, issues a delegation certificate, and
checks the accepted and rejected paths on a throwaway database.

---

## 9. Key rotation

Multiple TXT records are supported and any one of them can verify a signature.
The service treats a non-expired, non-revoked cached key as authoritative: a
signature mismatch does **not** trigger a DNS refresh. To rotate:

1. Publish the new public key as an additional TXT record.
2. Switch `SSL_KEY_PATH` to the new private key and restart the pusher.
3. Either revoke the old key with `contrib/make_ping_key_revocation.py` (the
   service clears the host's cache and the next ping discovers the new key), or
   wait at least the old cached-key TTL, after which the cache is empty and the
   next ping performs a fresh lookup.
4. Remove the old TXT record after the new key is in use.

The service stores a key only after it verifies a signature, so an invalid DNS
answer cannot poison the cache. A refreshed TXT answer replaces the cached v3
keys for that host, but a refresh only happens when no valid cached key remains.

---

## 10. Troubleshooting

| Service log | Cause | Fix |
|-------------|-------|-----|
| `no TXT records` | Wrong record name or not propagated | Query `_welist-key.<host>` exactly |
| `zone is not DNSSEC-signed` | DNSSEC not enabled or DS missing | Enable DNSSEC and publish the DS record |
| `DNSSEC validation failed` | Expired/bogus signatures, clock skew, broken chain | Check with `delv` or a DNSSEC debugger |
| `DNSSEC status is indeterminate` | Upstream resolver strips DNSSEC data | Set `WELIST_DNS_NAMESERVERS` to a capable resolver |
| `dns unavailable` | Resolver unreachable or invalid configuration | Check `WELIST_DNS_NAMESERVERS` and `WELIST_DNSSEC_TRUST_ANCHOR` |
| `CNAME answers are not accepted` | `_welist-key` is an alias | Publish a real TXT record |
| `malformed record` | Wrong version, algorithm, or syntax | Use `v=we1; alg=ed25519; key=...` |
| `invalid key` | Not a base64 Ed25519 SPKI DER | Regenerate with the `openssl` command above |
| `v3 DNS lookup requires a hostname` | `BAL_SERVER_URL` uses an IP address | Use a DNS hostname |
| `invalid v3 signature` | Key mismatch or non-canonical URL | Ensure the TXT key matches `SSL_KEY_PATH` and the URL has no trailing slash; for onion delegation, ensure `SSL_KEY_PATH` is the delegated ping key |
| `invalid v2 signature` | v2 key mismatch or non-canonical URL | Ensure the key served at `/.pub_key.pem` matches `SSL_KEY_PATH`; v2 requires a canonical `BAL_SERVER_URL` |
| `invalid onion delegation` | Certificate signature does not verify against the onion identity key | Re-issue the certificate with the correct onion key; do not edit `key`/`valid_until` after signing |
| `delegation does not match the ping onion host` | `onion` field differs from `BAL_SERVER_URL` after normalization | Re-issue the certificate for the exact host |
| `delegation expired` | `valid_until` is at or before the current time | Re-issue the certificate and restart the pusher |
| `delegation is only supported for onion hosts` | `delegation` sent with a DNS URL | Unset `WELIST_PING_DELEGATION_FILE` for DNS hosts |
| `invalid revocation` | Signature does not verify, the host/key is invalid, or an onion identity key was targeted | Re-issue with the helper; onion revocations must be signed by the identity key and cannot target it |
| `revoked_at is in the future` | Issuer clock is more than 24 h ahead | Use the current time or fix the issuer clock |
| `revoked onion delegation` | The delegated key is on the revocation list | Generate a new ping key and certificate; a revoked key cannot be re-enabled |
| `legacy ping disabled` | v1 ping sent after the host already used v2 or v3 | Set `WELIST_PING_SIG_VERSION=2` or `3` (legacy v1 cannot be re-enabled for that host) |

Pusher warning `disabling onion delegation` (not a service log): the
certificate is missing, expired, bound to another host, structurally malformed,
has a signature that does not verify against the onion address, or does not
match `SSL_KEY_PATH`. In the recommended setup onion pings will then be
rejected; re-issue the certificate before restarting the pusher.

Useful diagnostics:

```bash
dig +dnssec TXT _welist-key.we.example.com
delv TXT _welist-key.we.example.com
dig +trace +dnssec TXT _welist-key.we.example.com
```

---

## Onion URLs

A v3 `.onion` address embeds the service's Ed25519 identity key, so no TXT
record is needed. The recommended design keeps that identity key **offline** and
delegates ping signing to a dedicated, rotatable key:

- `ping_private_key.pem` (`SSL_KEY_PATH`) signs every v3 ping.
- The onion identity key signs a delegation certificate once.
- Each onion ping carries the certificate in a `delegation` object.
- The WeTime service verifies the certificate against the onion address, then
  verifies the ping signature with the delegated key.

Without a `delegation` object the service still accepts pings signed directly by
the onion identity key. That fallback exists for compatibility; it requires the
identity key to be online and should not be used for new deployments.

The ping `url` must itself use the `.onion` host. The `onion_url` database field
is display-only and does not affect authentication.

### Certificate format

The request carries:

```json
{
  "onion": "<address>.onion",
  "key": "<base64 SPKI DER of the delegated Ed25519 key>",
  "valid_until": 1800000000,
  "signature": "<base64 Ed25519 signature by the onion identity key>"
}
```

The identity signature covers exactly:

```text
WELIST-PING-DELEGATION-V1\n{"key":"...","onion":"...","valid_until":1800000000}
```

Rules enforced by the service:

- `onion` is normalized (lowercase, trailing dot removed) and `key` is trimmed
  before the signature is verified. The pusher and the helper script sign the
  same normalized payload, so uppercase, trailing-dot, and whitespace-padded
  spellings produce the same signed bytes; issuing certificates in the
  lowercase form remains recommended, and the helper does that.
- `valid_until` is a Unix timestamp in seconds and must be strictly in the
  future. A certificate expiring exactly now is already rejected.
- `key` must be a valid Ed25519 SubjectPublicKeyInfo DER in standard base64.
- The identity signature must verify against the key embedded in the onion
  address.
- A present but invalid, expired, or mismatched certificate is rejected; the
  service does not fall back to the identity key.
- `delegation` is rejected on non-onion hosts.

### Issue the certificate

Create a dedicated ping key pair:

```bash
openssl genpkey -algorithm ED25519 -out ping_private_key.pem
chmod 600 ping_private_key.pem
```

Use the bundled helper with your onion key. It accepts either a Tor
`hs_ed25519_secret_key` file or a standard PKCS#8 PEM Ed25519 key:

```bash
export ONION="<address>.onion"
export ONION_KEY="/var/lib/tor/hidden_service/hs_ed25519_secret_key"

contrib/make_onion_delegation.py \
  --onion "$ONION" \
  --identity-key "$ONION_KEY" \
  --ping-key /etc/bal/ping_private_key.pem \
  --days 90 \
  --out /etc/bal/onion-delegation.json
```

For Tor-format keys the helper needs Python `cryptography` and `pynacl`:

```bash
pip install cryptography pynacl
```

If the onion identity key is a PKCS#8 PEM file, point `--identity-key` at it and
`pynacl` is not needed. `--days` accepts 1–3650 (default 365); prefer 30–90
days as explained in [Revocation and lifetime](#revocation-and-lifetime).

Run this step on a machine that holds the identity key (ideally offline), then
copy only `onion-delegation.json` and `ping_private_key.pem` to the executor.
The certificate contains public data (`0644` is fine); only
`ping_private_key.pem` must stay `0600` and never leave the executor.

> **Tor offline master keys:** when the onion service runs with
> `OfflineMasterKey 1`, the key file on the Tor host is a blinded signing key,
> not the v3 identity key, and the helper fails with `onion secret key does not
> match the onion address`. Issue the certificate on a machine that holds the
> original `hs_ed25519_secret_key` (keep an offline copy from when the service
> was created). The blinded signing key cannot issue a delegation certificate.

### Configure the pusher

```bash
export SSL_KEY_PATH="/etc/bal/ping_private_key.pem"
export WELIST_PING_DELEGATION_FILE="/etc/bal/onion-delegation.json"
export BAL_SERVER_URL="http://<address>.onion"
export WELIST_PING_SIG_VERSION=3
```

Delegation is only sent with v3 pings. With `WELIST_PING_SIG_VERSION=1` or `2`
(or unset) the certificate is loaded and validated but never included in the
request.

The pusher sends `delegation` only when `BAL_SERVER_URL` is an `.onion` host. It
validates the certificate at startup and again before each ping: the host must
match after normalization (lowercase, trailing dot removed), the key must be a
valid Ed25519 SPKI, the signature must be 64 bytes, `valid_until` must be in the
future and at most `BAL_PUSHER_MAX_DELEGATION_DAYS` (default 366; `0` disables
the cap) days ahead, the key must match `SSL_KEY_PATH`, and the Ed25519
signature must verify against the identity key embedded in the onion address. If
the file is missing or malformed, or validation fails, delegation is disabled
and the pusher signs with `SSL_KEY_PATH` directly.
`BAL_PUSHER_DELEGATION_WARN_DAYS` (default 7) controls an advance expiry
warning; `0` disables the warning only.

> **Warning — disabled delegation means onion pings fail.** In this setup
> `SSL_KEY_PATH` is the dedicated ping key, so when delegation is disabled the
> service verifies the ping against the onion identity key and rejects every
> request (`invalid v3 signature`). The executor then stops earning WeTime
> points. Watch for the pusher warning `disabling onion delegation` and
> re-issue before `valid_until`.

### Deployment order

1. Upgrade the WeTime service (`bal-welist`) to a version that supports v3
   onion delegation.
2. Then set `WELIST_PING_DELEGATION_FILE` on the pusher and restart it.

A pusher that sends delegation to an older service receives `"ko"`: the old
service ignores the `delegation` field and verifies the ping with the onion
identity key. When rolling back the service, unset
`WELIST_PING_DELEGATION_FILE` first.

For local development, the `WELIST_ALLOW_LOCALHOST_PING` and legacy v1 notes
above still apply; delegation itself needs no network access. The bundled
development launchers require `-ok-dev` or `BAL_DEV=true` before they run.
`bal-pusher.sh` defaults to v1. v2 and v3 require a public canonical
`BAL_SERVER_URL`, so change the URL inside the launcher before overriding the
version:

```bash
BAL_DEV=true WELIST_PING_SIG_VERSION=2 ./bal-pusher.sh
BAL_DEV=true WELIST_PING_SIG_VERSION=3 ./bal-pusher.sh
```

### Rotation

1. Generate a new `ping_private_key.pem`.
2. Re-run `contrib/make_onion_delegation.py` with the new key and a new
   `valid_until`.
3. Replace `onion-delegation.json` and `SSL_KEY_PATH` on the executor.
4. Update `BAL_SERVER_PUB_KEY_PATH` to the new delegated public key and restart
   both `bal-server` and the pusher. The server caches the PEM at startup and
   writes it to the database, so it must be restarted before the consistency
   check can see the new key.

The onion address does not change, and the identity key stays offline. There is
no DNS cache to wait for; the service checks the certificate on every ping.
Rotation does **not** revoke the previous certificate; use the revocation helper
below before or after rotating.

### Revocation and lifetime

The WeTime service keeps a permanent, key-scoped revocation list
(`POST /ping-keys/revoke`). A revoked key is rejected for both the onion
delegation path and the DNSSEC TXT path, even while a cached TXT record or a
still-valid certificate lists it.

#### Onion delegation (identity-signed)

The onion identity key signs the revocation, so only the onion owner can revoke
a delegated key. The identity key itself cannot be revoked; changing it changes
the `.onion` address.

```bash
contrib/make_ping_key_revocation.py \
  --onion "$ONION" \
  --identity-key "$ONION_KEY" \
  --certificate /etc/bal/onion-delegation.json \
  --service-url "https://<service>" \
  --out /tmp/onion-revocation.json

curl -sS -X POST -H 'Content-Type: application/json' \
  --data @/tmp/onion-revocation.json "https://<service>/ping-keys/revoke"
```

#### DNSSEC TXT key (self-signed)

For a TXT key the only available authority is the key itself, so the helper
signs the revocation with the key being revoked:

```bash
contrib/make_ping_key_revocation.py \
  --host we.example.com \
  --key /etc/bal/private_key.pem \
  --service-url "https://<service>" \
  --out /tmp/dns-revocation.json

curl -sS -X POST -H 'Content-Type: application/json' \
  --data @/tmp/dns-revocation.json "https://<service>/ping-keys/revoke"
```

Also remove the key from `_welist-key.<host>`: revocation makes the stop
immediate, but the TXT record should stop advertising the key to new resolvers.
If the private key was destroyed and cannot sign the revocation, rotating the
TXT record and waiting for the DNS cache TTL is the only remaining path.

#### Certificate format

```json
{
  "host": "<address>.onion or a DNS hostname",
  "key": "<base64 SPKI DER of the revoked Ed25519 key>",
  "revoked_at": 1800000000,
  "signature": "<base64 signature>"
}
```

The signature covers:

```text
WELIST-PING-REVOCATION-V1\n{"host":"...","key":"...","revoked_at":1800000000}
```

Rules:

- `host` is normalized (lowercase, no trailing dot) and `key` is trimmed before
  the signature is verified; the helper script signs the same normalized
  payload.
- Onion hosts require the onion identity signature; DNS hosts require a
  self-signature from the revoked key.
- `revoked_at` may be at most 24 hours in the future.
- The key id is the hex of the raw public key, so re-encoded base64 cannot
  bypass the list.
- Entries never expire and submissions are idempotent (`"ok"`); at most
  `WELIST_MAX_REVOCATIONS_PER_HOST` distinct keys are kept per host (default
  `2048`; reaching the cap returns `409`). `POST /ping-keys/revoke` is
  rate-limited per IP by `WELIST_REVOKE_RATE_PER_MINUTE` (default 30/min).
- Revoking a key is permanent; a later certificate with the same key is also
  rejected.
- Revoking a v3 key clears the host's saved replay height, so a replacement key
  is not rejected as stale after a compromised key published an extreme height.
  Duplicate revocations do not reset replay state.
- Revoke a DNS key before removing it from `_welist-key`; after the cache TTL
  expires a removed key can no longer be proven to belong to the host.

Prefer short certificate lifetimes (for example 30–90 days) anyway: revocation
requires the onion identity key (onion) or the revoked key (DNS), so a key that
is both leaked and lost without a remaining signer cannot be revoked.

### Security notes

- Delegation limits the blast radius: a compromised online ping key can publish
  pings and metadata for that onion host until it is revoked or the certificate
  expires, but it cannot take over the onion service identity or sign for
  another host.
- A leaked key should be revoked immediately; rotation alone is not a
  revocation.
- The delegated key authorizes the full ping capability for that host, including
  the `info`, `base_fee` and `version` metadata.
- The certificate is public data and can be copied freely; only the ping private
  key is secret.
- The onion identity key should never be copied to the executor.

### Metadata for onion pings

Onion pings now carry the same non-null `info`, `base_fee`, and `version` as
DNS hosts. The pusher reads them from `tbl_executor_metadata`; it no longer
needs to reach its own `.onion` endpoint over HTTP.

With `WELIST_PING_SIG_VERSION=1` the pusher sends no metadata at all. The v1
signature covers only the legacy liveness concatenation (`url`, `chain`,
`height`, `median_time`, `last_block_hash`), and the WeTime service refreshes
`last_update` without persisting `info`, `base_fee`, or `version`. Unsigned
metadata is therefore not published. Use v2 or v3 when executor metadata must
be authenticated and stored.

---

## Minimal checklist

- [ ] Ed25519 key pair generated; `private_key.pem` is `chmod 600`.
- [ ] `SSL_KEY_PATH` and `BAL_SERVER_PUB_KEY_PATH` use the same key pair.
- [ ] TXT value built from `openssl pkey ... -outform DER | base64 -w0`.
- [ ] TXT record `_welist-key.<host>` published, not a CNAME.
- [ ] Zone DNSSEC-signed and DS published at the registrar.
- [ ] `dig +dnssec` / `delv` shows a validated answer.
- [ ] `BAL_SERVER_URL` is the public DNS hostname.
- [ ] `WELIST_PING_SIG_VERSION=3` is set.
- [ ] `contrib/make_ping_key_revocation.py --host ... --key ...` can produce a
      self-signed revocation for the TXT key.
- [ ] `bal-server` started after the upgrade so `tbl_executor_metadata` contains
      the row for this chain.
- [ ] Pusher restarted and reports `body="ok"`.
- [ ] If the key changes, `bal-server` is restarted so the cached
      `/.pub_key.pem` and the database row are updated together.

For onion hosts, replace the TXT/DNSSEC items with:

- [ ] The WeTime service (`bal-welist`) runs a version with onion delegation support.
- [ ] A dedicated `ping_private_key.pem` exists and is `chmod 600`.
- [ ] `onion-delegation.json` was issued offline by the v3 **master** identity
      key (not a blinded signing key).
- [ ] `SSL_KEY_PATH` points at the delegated ping key, not the identity key.
- [ ] `BAL_SERVER_PUB_KEY_PATH` points at the delegated **public** key, not the
      onion identity public key.
- [ ] `WELIST_PING_DELEGATION_FILE` points at the certificate.
- [ ] `WELIST_PING_SIG_VERSION=3` is set.
- [ ] `BAL_PUSHER_DELEGATION_WARN_DAYS` is set (default `7`).
- [ ] `valid_until` is strictly in the future, monitored, and rotation is
      scheduled before it.
- [ ] The identity key is available offline so a leaked ping key can be revoked
      with `contrib/make_ping_key_revocation.py`.
- [ ] The pusher starts without a `disabling onion delegation` warning.
- [ ] Pusher restarted and reports `body="ok"`.
