# Deployment and Operations

## Quick Reference
- **What this file contains:** environment variables, systemd service files, deployment scripts, nginx/Tor configuration, and installation procedures.
- **See also:** [01_project_overview.md](01_project_overview.md), [03_architecture_and_data_flow.md](03_architecture_and_data_flow.md), [05_api_reference.md](05_api_reference.md), [08_security_audit.md](08_security_audit.md)

---

## Environment Variables

### `bal-server` (`bal-server.env`)
The `bal-server.env` file is a production environment file that sets the configuration for the `bal-server` binary. The `bal-server.sh` script sources it before executing `cargo run --bin=bal-server`.

```env
RUST_LOG=info
BAL_DB_FILE=/var/bal/bal.db
BAL_BIND_ADDRESS=0.0.0.0:3031
BAL_EXPOSE_STATS=true
BAL_REGTEST_XPUB=tpub... (example for regtest testing)
BAL_PUB_KEY_PATH=public_key.pem
```
- `RUST_LOG`: Log level (e.g., `info`, `debug`, `error`). The `env_logger` crate uses this.
- `BAL_DB_FILE`: Path to the `sqlite` database file. If not specified, it defaults to `bal.db` in the working directory.
- `BAL_BIND_ADDRESS`: The TCP address and port to listen on. For example, `0.0.0.0:3031` means it will listen on any interface, port `3031`. For local development, you may want `127.0.0.1:3031`.
- `BAL_EXPOSE_STATS`: Boolean flag (`true` or `false`) to enable the `GET /:network/stats` endpoint. Set to `false` if you do not want to expose statistics to the public internet.
- `BAL_NETWORK_XPUB`: The `XPUB` or `ZPUB` for each network. For example, `BAL_REGTEST_XPUB`, `BAL_BITCOIN_XPUB`, etc. These are used to derive the receiving and fee collection addresses.
- `BAL_PUB_KEY_PATH`: The file path to the `public_key.pem` file that is served via the `GET /.pub_key.pem` endpoint. This is used for signature verification by the `welist` server or other clients.

### `bal-pusher` (`bal-pusher.env`)
The `bal-pusher.env` file is used for the `bal-pusher` binary. It contains sensitive information and is sourced by the `bal-pusher.sh` script.

```env
ZMQ_ENDPOINT=tcp://127.0.0.1:21332
BAL_SERVER_URL=http://127.0.0.1:3031
BAL_PUSHER_RPC_URL=http://127.0.0.1:18443
BAL_PUSHER_RPC_COOKIE_PATH=/home/bal/.bitcoin/.cookie
BAL_SSL_KEY_PATH=private_key.pem
SEND_STATS=true
WELIST_URL=https://welist.example.com/api/stats
```
- `ZMQ_ENDPOINT`: The ZMQ endpoint for the `hashblock` or `rawblock` topic. For `regtest`, use `tcp://127.0.0.1:21332`. For mainnet, use `tcp://127.0.0.1:28332`.
- `BAL_SERVER_URL`: The URL of the `bal-server` that the pusher can use to query statistics or for other internal communication.
- `BAL_PUSHER_RPC_URL`: The URL for the Bitcoin Core JSON-RPC endpoint. For `regtest`, the default is `http://127.0.0.1:18443`.
- `BAL_PUSHER_RPC_COOKIE_PATH`: The path to the `.cookie` file for RPC authentication. If not set, the pusher must use `user_pass` authentication. The cookie file is created by `bitcoind` when it starts with `rpccookieauth`.
- `BAL_SSL_KEY_PATH`: The path to the Ed25519 private key (`private_key.pem`) used to sign the statistics payload before sending it to the `welist` server. This is a critical secret.
- `SEND_STATS`: A boolean flag to enable the reporting of statistics to the remote `welist` server.
- `WELIST_URL`: The URL to which the statistics are sent. If `SEND_STATS` is `true`, this URL must be reachable. If the server is unreachable, the pusher will log an error but might not crash (see `08_security_audit.md` for DoS analysis).

---

## System Services

### `bal-server.service` (Systemd Unit)
This file is the systemd unit for the `bal-server` binary. It runs the server as a dedicated `bal` user with hardening options.

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
- **User:** The service runs as a dedicated, non-privileged user (`bal` user) to ensure the server doesn't run as root.
- **Hardening:** `ProtectSystem=full` prevents writing to most of the filesystem. `NoNewPrivileges=true` prevents privilege escalation. `MemoryDenyWriteExecute=true` prevents executable memory allocations (W^X). `PrivateDevices=true` limits the exposure to the physical hardware.
- **Security:** The `bal-server` does not need root access, and the database should be in a directory owned by the `bal` user.

### `bitcoind.service` (Systemd Unit for Mainnet)
The `bitcoind.service` file is the systemd unit to run the Bitcoin Core daemon. It must be configured with the appropriate ZMQ and RPC flags. For example, `bitcoind` must be started with `zmqpubhashblock=tcp://127.0.0.1:28332` to send `new block` notifications to the pusher.

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
- **Note:** The full `bitcoind` configuration is in `bitcoin.conf` (or the `contrib/download_and_install_bitcoincore.sh` script). The script sets `zmqpubhashblock` (not `zmqpubrawblock`) for the pusher's new-block notifications. The `zmqpubhashblock` and `zmqpubrawtx` ports must be bound to `127.0.0.1` (never `0.0.0.0`) and match the pusher's `ZMQ_ENDPOINT`.

### `tbitcoind.service` (Systemd Unit for Testnet)
This is the same as `bitcoind.service` but for the `testnet` network. It uses a different data directory (`~/.bitcoin/testnet/` by default) and a different ZMQ port (e.g., `tcp://127.0.0.1:23332`).

---

## Bash Scripts

### `bal-server.sh` (Development Server Startup)
This script sources the `bal-server.env` file and then runs the development server with Cargo for easy development and reloading.

```bash
export $(grep -v '^#' bal-server.env | xargs)
RUST_LOG=info cargo run --bin=bal-server 2>&1
```
- It is intended for development use only. It is not suitable for production because it compiles and runs in a single step, which is slow and insecure.

### `bal-pusher.sh` (Development Pusher Startup)
This script sources the `bal-pusher.env` and runs the pusher in development mode. It also accepts the `network` name as an argument (e.g., `sh bal-pusher.sh regtest`).

```bash
export $(grep -v '^#' bal-pusher.env | xargs)
RUST_LOG=info cargo run --bin=bal-pusher $1
```

### `sendtx.sh` (One-liner Transaction Sender)
This script is a one-liner helper that sends a raw transaction to a local node using a sequence of `bitcoin-cli` calls. It is not part of the main system but is used for testing purposes.

```bash
bitcoin-cli -regtest gettransaction ... | bitcoin-cli -regtest sendrawtransaction ... | bitcoin-cli -regtest sendtoaddress ...
```
- It is a helper script that wraps `bitcoin-cli` to send a pre-created transaction, get the raw bytes, and send them to a new address. It is only useful for manual testing and integration checks.

### `make_release.sh` (Release Builder)
This script builds a release binary, creates a Git tag, and uploads the release to a Git server (Gitea). It also hardcodes a Gitea API token (`TOKEN="5cfa8c33e337ebaadb355c0ffa2d053d521ee43b"`), which is a major security risk.

```bash
# WARNING: This script contains a hardcoded secret token. Do not use it as-is for production.
```
- **Release Assets:** It generates a `.tar.gz` archive with the binaries, a `.sha256` checksum file, and both a `.sig` GPG detached binary signature and a `.asc` ASCII-armored version.
- **Signature:** The release tarball is signed with the GPG key `Svātantrya <svatantrya@bitcoin-after.life>`. The script verifies that `gpg`, `sha256sum`, and `jq` are installed before proceeding.
- **Verification:** The release body includes instructions for verifying the checksum and signature (binary or ASCII-armored):
  ```bash
  sha256sum -c <release>.tar.gz.sha256
  gpg --verify <release>.tar.gz.sig <release>.tar.gz
  gpg --verify <release>.tar.gz.asc <release>.tar.gz
  ```
- **Security:** It also builds and uploads the binaries. The binaries should be built and signed on a separate, clean build machine, not on the production server.

### `download_bal_db.sh` (Database Pull Script)
This script uses `scp` to pull the production `bal.db` from a remote server (`debian@bitcoin-after.life`). It requires passwordless or key-based SSH access to the remote server.

```bash
scp debian@bitcoin-after.life:/var/bal/bal.db ./bal.db
```
- **Security:** It requires the remote server to be accessible. The remote server's IP address is hardcoded. This is a maintenance script, not part of the core system.

---

## Nginx and SSL Configuration

The `bal-server` is a plain HTTP server. To expose it to the internet, a production environment should put a reverse proxy like `Nginx` in front of it. The `nginx` configuration (from `contrib/download_and_install_bal.sh`) is used to terminate TLS and provide SSL certificates. Nginx also handles rate limiting, request filtering, and static file serving for `public_key.pem`.

### Example Nginx Configuration (from `contrib`)

```nginx
server {
    listen 80;
    server_name bal.example.com;
    return 301 https://$server_name$request_uri;
}
server {
    listen 443 ssl http2;
    server_name bal.example.com;
    ssl_certificate /etc/letsencrypt/live/bal.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/bal.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3031;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    # Rate limiting can be added here
}
```
- **Certbot:** The `contrib` script installs `certbot` and automatically generates the certificate. This configuration is used to ensure the `bal-server` is served over HTTPS with valid TLS.
- **Rate limiting:** It is recommended to add `limit_req` or `limit_conn` to the Nginx configuration to prevent the server from being overwhelmed by too many concurrent requests (e.g., `pushtxs` spam, or DoS attacks). The `bal-server` has no built-in rate limiting on the HTTP level.

---

## Production Deployment Checklist

Before exposing `bal` to the internet, verify the following steps. The `bal-server` is a plain HTTP application and must **never** be bound directly to a public IP or `0.0.0.0`.

### 1. `bal-server` Bind Address
- [ ] `bal-server.env` (or `.env`) sets `BAL_SERVER_BIND_ADDRESS=127.0.0.1` (not `0.0.0.0`).
- [ ] `BAL_SERVER_BIND_PORT` is the port used by Nginx `proxy_pass` (default `9137`).
- [ ] Firewall blocks inbound connections to `BAL_SERVER_BIND_PORT` from external interfaces (e.g., `iptables -A INPUT -p tcp --dport 9137 -s 127.0.0.1 -j ACCEPT` and `DROP` for others).

### 2. Reverse Proxy (Nginx + TLS)
- [ ] Nginx is installed (`contrib/download_and_install_bal.sh` handles this).
- [ ] The template `contrib/nginx/bal-server.conf` is copied to `/etc/nginx/sites-available/` and symlinked to `sites-enabled` (the `contrib/download_and_install_bal.sh` script does this automatically).
- [ ] The file has a real domain name replacing `BAL_DOMAIN`.
- [ ] `listen 443 ssl http2;` is active.
- [ ] `certbot --nginx` has obtained a valid certificate (the script runs `certbot --nginx` which avoids the port 80 conflict of `--standalone`). For manual installs, use `sudo certbot --nginx -d $domain`.
- [ ] `proxy_pass` points to `http://127.0.0.1:9137` (or whatever `BAL_SERVER_BIND_PORT` is).
- [ ] `client_max_body_size` in Nginx matches `BAL_SERVER_ACTIX_MAX_BODY_SIZE` (default `1m`).
- [ ] HTTP port 80 redirects to HTTPS (`return 301 https://...`).
- [ ] Nginx `limit_req` zone is configured if desired (backup to `actix-governor`).

### 3. Database and Secrets
- [ ] Database file is owned by the `bal` user (`chown bal:bal /var/bal/bal.db`).
- [ ] Database file permissions are `600` (`chmod 600 /var/bal/bal.db`).
- [ ] `.env` file is in `.gitignore` and not committed.
- [ ] `private_key.pem` and `privkey.pem` are not in the repository (use `git ls-files` to verify).
- [ ] `public_key.pem` is readable by Nginx if served directly (otherwise let the actix endpoint handle it).

### 4. Pusher and ZMQ
- [ ] ZMQ endpoints are configured for `127.0.0.1` only (e.g., `tcp://127.0.0.1:28332`).
- [ ] `BAL_PUSHER_SEND_STATS` is set to `false` unless the `welist` endpoint is actually needed.
- [ ] If stats are enabled, `WELIST_SERVER_URL` is a valid external HTTPS domain (not IP, not local).
- [ ] Firewall blocks inbound TCP port `28332` (or your custom `bitcoin`, `regtest`, etc. ZMQ ports) from external interfaces.

### 5. Logging and Monitoring
- [ ] `RUST_LOG` is set to `info` or `warn` in production (not `debug` or `trace`).
- [ ] Log files are rotated (e.g., via `logrotate`) and stored only under `/var/log/bal/` or systemd journal.
- [ ] Log files are not in the same directory as the database or the private key.

---

## Tor and Privacy

The `contrib/install_tor.sh` script installs Tor for use as an onion-routed proxy. It can be used to:
1. Allow the `bal-server` to be reachable via a `.onion` address for privacy and censorship resistance.
2. Allow the `bal-pusher` to connect to the Bitcoin RPC or the `welist` server through Tor to hide its origin IP.
3. Allow the server to run behind NAT without exposing the real IP to the public internet.

The script uses `ControlPort 9051` and enables `CookieAuthentication`. If `SEND_STATS` is true, the `welist` URL can be configured to be a `.onion` address to hide the origin. For example, the `bal-pusher` could use `reqwest` with SOCKS5 proxy settings to connect to the `welist` server via Tor.
- `reqwest` feature `socks` (enabled in `Cargo.toml`) supports proxy settings.
- For a production privacy setup, it is recommended to run the server and the pusher behind a Tor or VPN proxy.
- **Security:** The Tor service itself (`tor.service`) should be hardened and run as a separate user. The `ControlPort` `9051` should be bound to `127.0.0.1` and should not be exposed to the public without authentication.

---
