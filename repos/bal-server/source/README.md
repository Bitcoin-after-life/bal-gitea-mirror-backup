# bal-server

## Installation

```bash
git clone https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server.git
cd bal-server
openssl genpkey -algorithm ED25519 -out private_key.pem
openssl pkey -in private_key.pem -pubout -out public_key.pem
cargo build --release
sudo cp target/release/bal-server target/release/bal-pusher /usr/local/bin
```

## Docker

### Quick Start (release download)

Download the latest pre-built release — no Rust toolchain needed:

```bash
docker build -f Dockerfile.release -t bal-server .
```

### Build from source

```bash
docker build -t bal-server .
```

### Run

```bash
docker run -d \
  --name bal-server \
  --network host \
  --tmpfs /tmp:rw,noexec,nosuid \
  -v /path/to/data:/var/bal:rw \
  -v /path/to/.bitcoin/regtest/.cookie:/var/bal/.bitcoin/regtest/.cookie:ro \
  -e BAL_SERVER_REGTEST_ADDRESS="your_xpub_or_address" \
  -e BAL_SERVER_REGTEST_FIXED_FEE=50000 \
  -e BAL_SERVER_INFO="BAL server" \
  -e BAL_PUSHER_NETWORK=regtest \
  -e BAL_PUSHER_REGTEST_ZMQ_HASHBLOCK=tcp://127.0.0.1:28332 \
  -e BAL_PUSHER_REGTEST_COOKIE_FILE=/var/bal/.bitcoin/regtest/.cookie \
  bal-server
```

### Pin a specific version

```bash
docker build -f Dockerfile.release --build-arg BAL_VERSION=v0.3.2 -t bal-server:0.3.2 .
```

### Docker environment variables

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_PUSHER_NETWORK` | Network to run pusher on (`bitcoin`, `testnet`, `testnet4`, `signet`, `regtest`). | `bitcoin` |
| `BAL_PUSHER_REGTEST_ZMQ_HASHBLOCK` | ZMQ endpoint for regtest blocks. | `tcp://127.0.0.1:21332` |
| `BAL_PUSHER_REGTEST_COOKIE_FILE` | Absolute path to Bitcoin Core cookie file inside the container. | - |

> **Note:** The container runs as a non-root `bal` user (uid 1000) with `tini` as PID 1.
> The `/var/bal` volume stores the database. Mount Bitcoin Core's cookie file as read-only.
> When using `--network host`, ensure only `127.0.0.1` is used for internal services.
> `Dockerfile.release` fetches the latest release from the Gitea server and verifies its SHA-256 checksum.

## Configuration (bal-server)

The `bal-server` application can be configured using environment variables.

### General

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_SERVER_DB_FILE` | Path to the SQLite3 database file. | `bal.db` |
| `BAL_SERVER_BIND_ADDRESS` | Address to listen on. **Never bind to `0.0.0.0` in production without a reverse proxy.** | `127.0.0.1` |
| `BAL_SERVER_BIND_PORT` | Port to listen on. | `9137` |
| `BAL_SERVER_INFO` | Server info string returned by the `/` endpoint. | - |
| `BAL_SERVER_PUB_KEY_PATH` | Ed25519 public key for signature verification. | `public_key.pem` |
| `BAL_SERVER_URL` | Public URL of this server (used for stats reporting). | - |
| `SSL_KEY_PATH` | Ed25519 private key for signing stats reports. | `private_key.pem` |
| `RUST_LOG` | Log level (`error`, `warn`, `info`, `debug`, `trace`). | `info` |

### Per-network addresses and fees

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_SERVER_BITCOIN_ADDRESS` | xpub or address for mainnet. | - |
| `BAL_SERVER_BITCOIN_FIXED_FEE` | Fixed fee (satoshis) for mainnet. | `50000` |
| `BAL_SERVER_REGTEST_ADDRESS` | xpub or address for regtest. | - |
| `BAL_SERVER_REGTEST_FIXED_FEE` | Fixed fee (satoshis) for regtest. | `50000` |
| `BAL_SERVER_SIGNET_ADDRESS` | xpub or address for signet. | - |
| `BAL_SERVER_SIGNET_FIXED_FEE` | Fixed fee (satoshis) for signet. | `50000` |
| `BAL_SERVER_TESTNET_ADDRESS` | xpub or address for testnet. | - |
| `BAL_SERVER_TESTNET_FIXED_FEE` | Fixed fee (satoshis) for testnet. | `50000` |
| `BAL_SERVER_TESTNET4_ADDRESS` | xpub or address for testnet4. | - |
| `BAL_SERVER_TESTNET4_FIXED_FEE` | Fixed fee (satoshis) for testnet4. | `50000` |

### DoS protection (Actix Web)

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_SERVER_ACTIX_MAX_BODY_SIZE` | Maximum request body size in bytes. | `1048576` (1 MB) |
| `BAL_SERVER_ACTIX_TIMEOUT_SECS` | Request timeout in seconds. | `5` |
| `BAL_SERVER_ACTIX_PUSHTXS_PER_SEC` | Rate limit: push txs requests per second. | `1` |
| `BAL_SERVER_ACTIX_PUSHTXS_BURST` | Rate limit: push txs burst size. | `3` |
| `BAL_SERVER_ACTIX_SEARCHTX_PER_SEC` | Rate limit: search tx requests per second. | `5` |
| `BAL_SERVER_ACTIX_SEARCHTX_BURST` | Rate limit: search tx burst size. | `10` |
| `BAL_SERVER_ACTIX_INFO_PER_SEC` | Rate limit: info requests per second. | `20` |
| `BAL_SERVER_ACTIX_INFO_BURST` | Rate limit: info burst size. | `30` |
| `BAL_SERVER_ACTIX_DEFAULT_PER_SEC` | Rate limit: default requests per second. | `50` |
| `BAL_SERVER_ACTIX_DEFAULT_BURST` | Rate limit: default burst size. | `100` |
| `BAL_SERVER_ACTIX_WORKERS` | Number of Actix worker threads. | `4` |
| `BAL_SERVER_ACTIX_MAX_CONNECTIONS` | Maximum concurrent connections. | `100` |

---

# bal-pusher

`bal-pusher` monitors Bitcoin blocks via ZMQ and pushes time-locked transactions from the database to the Bitcoin network when their **locktime** exceeds the **median time past** (MTP).

## Prerequisites

- **Bitcoin Core** with ZMQ support enabled. Add to `bitcoin.conf`:
  ```
  zmqpubhashblock=tcp://127.0.0.1:28332
  ```
- **Rust and Cargo**: [Rust Installation](https://www.rust-lang.org/tools/install)
- **Libraries**: `libssl-dev`, `libzmq5-dev`, `libsqlite3-dev`

## Running

```bash
bal-pusher [bitcoin|testnet|testnet4|signet|regtest]
```

If no network is specified, defaults to `bitcoin`.

## Configuration (bal-pusher)

### General

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_PUSHER_DB_FILE` | Path to the SQLite3 database file. | `bal.db` |
| `BAL_PUSHER_SEND_STATS` | Send stats to welist server. | `false` |
| `BAL_SERVER_URL` | URL of bal-server (for stats reporting). | - |
| `SSL_KEY_PATH` | Ed25519 private key for signing stats reports. | `private_key.pem` |
| `WELIST_SERVER_URL` | Welist server URL. | `https://welist.bitcoin-after.life` |

### Per-network configuration

Each network (`bitcoin`, `regtest`, `testnet`, `testnet4`, `signet`) supports the following variables.
Replace `{NETWORK}` with the uppercase network name (e.g., `REGTEST`, `BITCOIN`).

| Variable | Description | Default |
| --- | --- | --- |
| `BAL_PUSHER_{NETWORK}_ZMQ_HASHBLOCK` | ZMQ endpoint for block notifications. | `tcp://127.0.0.1:28332` (mainnet) |
| `BAL_PUSHER_{NETWORK}_COOKIE_FILE` | Absolute path to Bitcoin Core cookie file. | `$HOME/.bitcoin/{dir}/.cookie` |
| `BAL_PUSHER_{NETWORK}_RPC_USER` | Bitcoin Core RPC username (alternative to cookie auth). | - |
| `BAL_PUSHER_{NETWORK}_RPC_PASSWORD` | Bitcoin Core RPC password. | - |
| `BAL_PUSHER_{NETWORK}_HOST` | Bitcoin Core RPC host. | `http://127.0.0.1` |
| `BAL_PUSHER_{NETWORK}_PORT` | Bitcoin Core RPC port. | `8332` (mainnet) |
| `BAL_PUSHER_{NETWORK}_DIR_PATH` | Bitcoin Core data directory subfolder. | `` (mainnet) |

Default ZMQ ports per network:

| Network | ZMQ Port | RPC Port |
| --- | --- | --- |
| `bitcoin` | 28332 | 8332 |
| `regtest` | 21332 | 18443 |
| `testnet` | 23332 | 18332 |
| `testnet4` | 22332 | 48332 |
| `signet` | 24332 | 38332 |
