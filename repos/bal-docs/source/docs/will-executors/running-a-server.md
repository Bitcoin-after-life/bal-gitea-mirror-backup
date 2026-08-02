# :material-console: Running a Will-Executor

This page covers setting up your own Will-Executor node. If you are wondering *why* you would, start with [What Will-Executors are](what-they-are.md).

!!! warning "Read this first"
    Running a Will-Executor is a **long-horizon commitment**, not a quick return. You will hold pre-signed transactions belonging to living people, and you are compensated only when you successfully broadcast an inheritance that confirms on-chain — potentially years from now. Costs are certain; revenue is not.

    Nothing on this page is an offer, a solicitation, or a promise of any economic result. It describes a technical role.

## What the role actually requires

- **A server that stays up.** A dedicated machine, a VPS, or a home node — at least one Will-Executor on the network runs on Umbrel OS. Modern releases run comfortably on a small VPS.
- **Reliable connectivity.** Tor-only operation is possible: at least one active Will-Executor is reachable exclusively that way.
- **A Bitcoin full node** with ZMQ enabled (see below).
- **Real maintenance.** Updates, backups, monitoring. A server that fails to broadcast when it matters earns nothing — and, more importantly, fails the user who relied on the network.
- **Discretion.** You will be handling data that reflects the wallets of living people. Privacy and reliability, not just price, are what Will-Executors compete on.

!!! tip "A Will-Executor doesn't have to be one person"
    From the protocol's point of view a Will-Executor is a service plus a Bitcoin address. Several operators can pool resources, present themselves as a single Will-Executor, and use a shared **multisig** address for the fee, splitting proceeds however they agree. A meetup, an association, or a group of friends can run one together.

    Two things to keep in mind: for a user's redundancy, a consortium still counts as **one** Will-Executor; and the internal split is an agreement outside the protocol, which BAL neither arbitrates nor guarantees.

See the [architecture diagram](../protocol/how-it-works.md#architecture-at-a-glance) for how
`bal-server` and `bal-pusher` fit into the full flow from plugin to Bitcoin node.

## The two components

| Component | Role |
|---|---|
| **`bal-server`** | Receives and stores the pre-signed inheritance transactions sent by plugins |
| **`bal-pusher`** | Watches for new Bitcoin blocks via ZMQ and broadcasts stored transactions once their locktime passes the median time past (MTP) |

Both live in the [bal-server repository](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server) (Rust, [releases](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server/releases)).

## Prerequisites

1. **Bitcoin Core compiled with ZMQ support**, with this line in `bitcoin.conf`:

    ```text
    zmqpubhashblock=tcp://127.0.0.1:28332
    ```

2. **Rust and Cargo** — see the [official installation instructions](https://www.rust-lang.org/tools/install).

## Installing `bal-server`

```bash
git clone https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server.git
cd bal-server

# Ed25519 keypair identifying this Will-Executor
openssl genpkey -algorithm ED25519 -out private_key.pem
openssl pkey -in private_key.pem -pubout -out public_key.pem

cargo build --release
sudo cp target/release/bal-server /usr/local/bin
bal-server
```

!!! danger "Back up `private_key.pem`"
    It is the identity of your Will-Executor. Store it securely and restrict its permissions (`chmod 600`).

### Configuration

`bal-server` is configured through environment variables:

| Variable | Description | Default |
|---|---|---|
| `BAL_SERVER_CONFIG_FILE` | Configuration file path — created if missing | `$HOME/.config/bal-server/default-config.toml` |
| `BAL_SERVER_DB_FILE` | SQLite3 database file — created if missing | `bal.db` |
| `BAL_SERVER_BIND_ADDRESS` | Public listening address | `127.0.0.1` |
| `BAL_SERVER_BIND_PORT` | Listening port | `9137` |
| `BAL_SERVER_PUB_KEY_PATH` | Will-Executor Ed25519 public key | `public_key.pem` |
| `BAL_SERVER_BITCOIN_ADDRESS` | Address that receives fees on **mainnet** | — |
| `BAL_SERVER_BITCOIN_FIXED_FEE` | Fixed fee on mainnet | `50000` |
| `BAL_SERVER_TESTNET_ADDRESS` / `..._FIXED_FEE` | Same, for testnet | — / `50000` |
| `BAL_SERVER_SIGNET_ADDRESS` / `..._FIXED_FEE` | Same, for signet | — / `50000` |
| `BAL_SERVER_REGTEST_ADDRESS` / `..._FIXED_FEE` | Same, for regtest | — / `50000` |

A sample environment file (`bal-server.env`) and a systemd unit (`bal-server.service`) ship with the repository.

!!! warning "Bind address and exposure"
    The default `127.0.0.1` is local-only. To be reachable by users you must expose the service deliberately — typically behind a reverse proxy with TLS, or as a Tor hidden service. Do not expose it carelessly.

## Installing `bal-pusher`

`bal-pusher` reads the same database and does the broadcasting.

```bash
bal-pusher [bitcoin|testnet|regtest]
```

### Configuration

| Variable | Description | Default |
|---|---|---|
| `BAL_PUSHER_CONFIG_FILE` | Configuration file path — created if missing | `$HOME/.config/bal-pusher/default-config.toml` |
| `BAL_PUSHER_DB_FILE` | SQLite3 database file | `bal.db` |
| `BAL_PUSHER_ZMQ_LISTENER` | ZMQ endpoint for block updates | `tcp://127.0.0.1:28332` |
| `BAL_PUSHER_BITCOIN_HOST` | Bitcoin RPC host | `http://127.0.0.1` |
| `BAL_PUSHER_BITCOIN_PORT` | Bitcoin RPC port | `8332` |
| `BAL_PUSHER_BITCOIN_COOKIE_FILE` | Bitcoin RPC cookie file | `$HOME/.bitcoin/.cookie` |
| `BAL_PUSHER_BITCOIN_RPC_USER` | Bitcoin RPC username | — |
| `BAL_PUSHER_BITCOIN_RPC_PASSWORD` | Bitcoin RPC password | — |
| `BAL_PUSHER_SEND_STATS` | Report timing statistics to the WeList | `false` |
| `WELIST_SERVER_URL` | WeList server URL | `https://welist.bitcoin-after.life` |
| `BAL_SERVER_URL` | URL of your Will-Executor | — |
| `SSL_KEY_PATH` | Ed25519 private key (PEM) | `private_key.pem` |

!!! note "`bal-pusher` must point at the same database as `bal-server`"
    `BAL_PUSHER_DB_FILE` and `BAL_SERVER_DB_FILE` have to resolve to the same SQLite file, otherwise the pusher will have nothing to broadcast.

## RPC interface

The request/response interface used by the plugin is documented in [`RPC.md`](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server/src/branch/main/RPC.md) in the repository.

## Going live

1. **Test on testnet or regtest first.** The WeList lets users filter by network, so a testnet server is a legitimate way to validate the whole chain of events.
2. **Verify reachability** from outside your own network, at the exact URL you intend to publish.
3. **Set up monitoring and restart policies** (the provided systemd unit is a starting point).
4. **Plan your backups** — the SQLite database holds the wills you have accepted; the Ed25519 key is your identity.
5. **List on the [WeList](welist.md)** if you want automatic discovery by plugin users — {{ bal.welist_fee_sats }} sats per year. Listing is optional; users can also add your server manually.

!!! tip "CPFP for congested conditions"
    Inheritance transactions are built with a prudent miner fee, but a Will-Executor can use **CPFP (Child Pays For Parent)** to speed up confirmation if the network is congested at delivery time. Being the first to get a transaction confirmed is what earns the fee.

## Getting in touch

For help setting up a Will-Executor, or to collaborate on the project: **[info@bitcoin-after.life](mailto:info@bitcoin-after.life)**

---

!!! question "Still have a question?"
    The [FAQ](../faq.md) answers the most common ones — costs, hardware wallets,
    privacy, and what happens if a Will-Executor disappears.
