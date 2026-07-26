# Deployment and Operations

## Quick Reference
- **What this file contains:** environment variables, systemd service files, deployment scripts, nginx/Tor configuration, Docker support, and installation procedures.
- **See also:** [01_project_overview.md](01_project_overview.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [05_api_reference.md](05_api_reference.md), [08_security_audit.md](08_security_audit.md)

---

## Environment Variables

### `bal-server` (all prefixed `BAL_SERVER_`)

#### Core Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `BAL_SERVER_DB_FILE` | `"bal.db"` | Path to the SQLite database file |
| `BAL_SERVER_BIND_ADDRESS` | `"127.0.0.1"` | TCP address to bind to (**never use `0.0.0.0` in production**) |
| `BAL_SERVER_BIND_PORT` | `9137` | TCP port to listen on |
| `BAL_SERVER_EXPOSE_STATS` | `false` | Enable/disable the `GET /:network/stats` endpoint |
| `BAL_SERVER_PUB_KEY_PATH` | `"public_key.pem"` | Path to the Ed25519 public key PEM file |
| `BAL_SERVER_INFO` | `"Will Executor Server"` | String returned by `GET /` |

#### Per-Network Settings

For each network (`regtest`, `testnet`, `testnet4`, `signet`, `bitcoin`):

| Variable | Default | Description |
|----------|---------|-------------|
| `BAL_SERVER_{NETWORK}_ADDRESS` | (empty) | The xpub/zpub/ypub or fixed address for fee collection |
| `BAL_SERVER_{NETWORK}_FIXED_FEE` | `50000` | Minimum fee in satoshis required for transaction acceptance |

Example: `BAL_SERVER_REGTEST_ADDRESS=tpub...`, `BAL_SERVER_BITCOIN_FIXED_FEE=50000`.

#### Actix-Web Tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `BAL_SERVER_ACTIX_MAX_BODY_SIZE` | `1048576` (1 MiB) | Maximum HTTP request body size |
| `BAL_SERVER_ACTIX_TIMEOUT_SECS` | `5` | Request timeout in seconds |
| `BAL_SERVER_ACTIX_WORKERS` | `4` | Number of actix-web worker threads |
| `BAL_SERVER_ACTIX_MAX_CONNECTIONS` | `100` | Maximum concurrent connections |
| `BAL_SERVER_ACTIX_PUSHTXS_PER_SEC` | `1` | Rate limit: pushtxs requests per second |
| `BAL_SERVER_ACTIX_PUSHTXS_BURST` | `3` | Rate limit: pushtxs burst size |
| `BAL_SERVER_ACTIX_SEARCHTX_PER_SEC` | `5` | Rate limit: searchtx requests per second |
| `BAL_SERVER_ACTIX_SEARCHTX_BURST` | `10` | Rate limit: searchtx burst size |
| `BAL_SERVER_ACTIX_INFO_PER_SEC` | `20` | Rate limit: info requests per second |
| `BAL_SERVER_ACTIX_INFO_BURST` | `30` | Rate limit: info burst size |
| `BAL_SERVER_ACTIX_DEFAULT_PER_SEC` | `50` | Rate limit: default requests per second |
| `BAL_SERVER_ACTIX_DEFAULT_BURST` | `100` | Rate limit: default burst size |

### `bal-pusher` (prefixed `BAL_PUSHER_`)

#### Core Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `BAL_PUSHER_DB_FILE` | `"bal.db"` | Path to the SQLite database file |
| `BAL_PUSHER_BITCOIN_DIR` | `""` | Bitcoin data directory (for cookie file path resolution) |
| `BAL_PUSHER_SEND_STATS` | `false` | Enable/disable remote stats reporting |
| `BAL_SERVER_URL` | `"http://localhost/"` | URL of the bal-server for internal communication |
| `SSL_KEY_PATH` | `"privkey.pem"` | Path to Ed25519 private key for signing stats |
| `BAL_PUSHER_PREFER_IPV6` | `false` | Pin HTTP connection to first IPv6 address (for broken IPv4 routes) |
| `WELIST_SERVER_URL` | `"https://welist.bitcoin-after.life"` | URL to POST signed stats to (validated against SSRF) |
| `WELIST_SKIP_URL_VALIDATION` | `false` | Bypass SSRF URL validation (for testing only) |

#### Per-Network Settings

For each network (`regtest`, `testnet`, `testnet4`, `signet`, `bitcoin`):

| Variable | Default (regtest) | Description |
|----------|-------------------|-------------|
| `BAL_PUSHER_{NETWORK}_HOST` | `"127.0.0.1"` | Bitcoin Core RPC host |
| `BAL_PUSHER_{NETWORK}_PORT` | `18443` | Bitcoin Core RPC port |
| `BAL_PUSHER_{NETWORK}_DIR_PATH` | `".bitcoin"` | Relative directory under `$HOME` for cookie file |
| `BAL_PUSHER_{NETWORK}_DB_FIELD` | (empty) | Database field name for this network |
| `BAL_PUSHER_{NETWORK}_COOKIE_FILE` | (empty) | Absolute path to cookie file (overrides `DIR_PATH`) |
| `BAL_PUSHER_{NETWORK}_RPC_USER` | (empty) | RPC username (if using user/pass auth) |
| `BAL_PUSHER_{NETWORK}_RPC_PASSWORD` | (empty) | RPC password (if using user/pass auth) |
| `BAL_PUSHER_{NETWORK}_ZMQ_HASHBLOCK` | `"tcp://127.0.0.1:21332"` | ZMQ hashblock endpoint |

Default ports per network:

| Network | RPC Port | ZMQ Port |
|---------|----------|----------|
| bitcoin | 8332 | 28332 |
| regtest | 18443 | 21332 |
| testnet | 18332 | 23332 |
| testnet4 | 48332 | 24332 |
| signet | 18332 | 22332 |

---

## Docker

The project provides two Dockerfiles:

### `Dockerfile.release` — Download pre-built release (recommended for production)

Downloads the latest release from the Gitea server. No Rust toolchain needed. Fast builds.

```bash
# Latest release
docker build -f Dockerfile.release -t bal-server .

# Specific version
docker build -f Dockerfile.release --build-arg BAL_VERSION=v0.3.2 -t bal-server:0.3.2 .
```

- Fetches `.tar.gz` from `https://bitcoin-after.life/gitea/api/v1/repos/bitcoinafterlife/bal-server/releases/latest`.
- Verifies SHA-256 checksum if available.
- Single-stage image (`debian:bookworm-slim`), minimal size.
- `BAL_VERSION` build arg: set to a tag (e.g., `v0.3.2`) to pin a specific release.

### `Dockerfile` — Build from source

Multi-stage build with the Rust toolchain. Use for development or custom builds.

- **Builder stage:** `rust:1.95-bookworm` with full build. Each binary is compiled with only its required features (`--no-default-features --features server` / `--features pusher`).
- **Runtime stage:** `debian:bookworm-slim` with minimal runtime.
- **User:** Non-root `bal` user (uid 1000).
- **PID 1:** `tini` for proper signal handling.
- **Healthcheck:** `curl -f http://localhost:9137/ || exit 1`.

### Run (both Dockerfiles)

```bash
docker run -d \
  --name bal-server \
  -v /var/bal:/var/bal \
  --env-file bal-server.env \
  -p 127.0.0.1:9137:9137 \
  bal-server
```

---

## System Services

### `bal-server.service` (Systemd Unit)

```ini
[Unit]
Description=Bal Server
After=network.target
[Service]
User=bal
Group=bal
ExecStart=/usr/local/bin/bal-server
Restart=always
RestartSec=5
WorkingDirectory=/var/bal
EnvironmentFile=/var/bal/bal-server.env
ProtectSystem=full
NoNewPrivileges=true
PrivateDevices=true
MemoryDenyWriteExecute=true

[Install]
WantedBy=multi-user
```
- Runs as a dedicated non-privileged `bal` user.
- Hardened with `ProtectSystem=full`, `NoNewPrivileges=true`, `PrivateDevices=true`, `MemoryDenyWriteExecute=true`.

### `bitcoind.service` (Bitcoin Core Daemon)

```ini
[Unit]
Description=Bitcoin Core Daemon
After=network.target
[Service]
User=bitcoin
Group=bitcoin
ExecStart=/usr/local/bin/bitcoind ... -zmqpubhashblock=tcp://127.0.0.1:28332 ...
Restart=on-failure
RestartSec=30
[Install]
WantedBy=multi-user
```
- Must be started with `zmqpubhashblock` (not `zmqpubrawblock`).
- ZMQ ports must be bound to `127.0.0.1` only.

### `tbitcoind.service` (Testnet Bitcoind)
Same as `bitcoind.service` but for testnet with a different data directory and ZMQ port (e.g., `tcp://127.0.0.1:23332`).

---

## Bash Scripts

### `bal-server.sh` (Development Server Startup)
Sources `bal-server.env` and runs the development server:
```bash
export $(grep -v '^#' bal-server.env | xargs)
RUST_LOG=info cargo run --bin=bal-server 2>&1
```

### `bal-pusher.sh` (Development Pusher Startup)
Sources `bal-pusher.env` and runs the pusher with a network argument:
```bash
export $(grep -v '^#' bal-pusher.env | xargs)
RUST_LOG=info cargo run --bin=bal-pusher $1
```

### `sendtx.sh` (Test Transaction Sender)
A helper script that wraps `bitcoin-cli` for manual testing.

### `make_release.sh` (Release Builder)
Builds release binaries, creates Git tags, and uploads to Gitea. Signs the release tarball with GPG. Release assets include `.tar.gz`, `.sha256`, `.sig`, and `.asc` files. Token is loaded from `.env` (not hardcoded).

### `download_bal_db.sh` (Database Pull Script)
Uses `scp` to pull the production `bal.db` from a remote server.

---

## Nginx and SSL Configuration

The `bal-server` is a plain HTTP server. A reverse proxy (Nginx) with TLS termination is required for production.

### Template: `contrib/nginx/bal-server.conf`

```nginx
server {
    listen 80;
    server_name BAL_DOMAIN;
    return 301 https://$server_name$request_uri;
}
server {
    listen 443 ssl http2;
    server_name BAL_DOMAIN;
    ssl_certificate /etc/letsencrypt/live/BAL_DOMAIN/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/BAL_DOMAIN/privkey.pem;

    client_max_body_size 1m;
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header Referrer-Policy no-referrer;

    location / {
        proxy_pass http://127.0.0.1:9137;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    # Uncomment for rate limiting:
    # limit_req zone=pal limit=10 nodelay;
}
```

Key points:
- `client_max_body_size` must match `BAL_SERVER_ACTIX_MAX_BODY_SIZE`.
- `certbot --nginx` obtains the certificate automatically.
- Security headers: `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`.

---

## Production Deployment Checklist

### 1. `bal-server` Bind Address
- [ ] `BAL_SERVER_BIND_ADDRESS=127.0.0.1` (never `0.0.0.0`).
- [ ] `BAL_SERVER_BIND_PORT` matches Nginx `proxy_pass` (default `9137`).
- [ ] Firewall blocks inbound connections to `BAL_SERVER_BIND_PORT` from external interfaces.

### 2. Reverse Proxy (Nginx + TLS)
- [ ] Nginx installed (`contrib/download_and_install_bal.sh` handles this).
- [ ] `contrib/nginx/bal-server.conf` template copied to `/etc/nginx/sites-available/`.
- [ ] Real domain name replacing `BAL_DOMAIN`.
- [ ] `listen 443 ssl http2` active.
- [ ] `certbot --nginx` has obtained a valid certificate.
- [ ] `proxy_pass` points to `http://127.0.0.1:9137`.
- [ ] `client_max_body_size` matches `BAL_SERVER_ACTIX_MAX_BODY_SIZE`.
- [ ] HTTP port 80 redirects to HTTPS.
- [ ] Security headers configured.

### 3. Database and Secrets
- [ ] Database file owned by `bal` user (`chown bal:bal /var/bal/bal.db`).
- [ ] Database file permissions `600` (`chmod 600 /var/bal/bal.db`).
- [ ] `.env` files in `.gitignore` and not committed.
- [ ] `private_key.pem` / `privkey.pem` not in the repository.
- [ ] `public_key.pem` readable by Nginx if served directly.

### 4. Pusher and ZMQ
- [ ] ZMQ endpoints configured for `127.0.0.1` only.
- [ ] `BAL_PUSHER_SEND_STATS=false` unless `welist` endpoint is needed.
- [ ] If stats enabled, `WELIST_SERVER_URL` is a valid external HTTPS domain.
- [ ] Firewall blocks inbound ZMQ ports from external interfaces.

### 5. Logging and Monitoring
- [ ] `RUST_LOG=info` or `warn` in production (not `debug`/`trace`).
- [ ] Log files rotated and stored under `/var/log/bal/` or systemd journal.
- [ ] Log files not in the same directory as the database or private key.

---

## Tor and Privacy

The `contrib/install_tor.sh` script installs Tor for onion-routed proxy use:
1. `bal-server` can be reachable via a `.onion` address.
2. `bal-pusher` can connect to Bitcoin RPC or `welist` through Tor.
3. The server can run behind NAT without exposing the real IP.

The script uses `ControlPort 9051` with `CookieAuthentication`. The `bal-pusher` supports SOCKS5 proxy via the `reqwest` `socks` feature for `.onion` connectivity.
