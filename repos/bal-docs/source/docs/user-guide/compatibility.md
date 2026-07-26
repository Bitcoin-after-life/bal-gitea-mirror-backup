# Compatibility

## Wallet types

| Wallet type | Status |
|---|---|
| Standard Electrum wallet (seed-based, single-signature) | **Supported** |
| Hardware wallets supported by Electrum | **Supported** — see [Hardware wallets](hardware-wallets.md) |
| Multisig wallets | **Not yet supported** — planned for a future release |
| Electrum TrustedCoin (2FA) wallets | **Unknown / unsupported** — not yet determined |

!!! danger "Do not rely on BAL with multisig or 2FA wallets"
    If you use a multisig or TrustedCoin (2FA) wallet, do not use BAL for inheritance until compatibility is confirmed.

## Electrum versions

The plugin follows Electrum closely; each release states the Electrum version it was tested against.

| Plugin release | Tested with |
|---|---|
| v{{ bal.plugin_version }} | Electrum {{ bal.electrum_tested }} (manual verified on {{ bal.electrum_latest }}) |
| v0.2.3 | Electrum 4.7.0 |
| v0.2.0 | Electrum 4.6.2 — introduced the will setup wizard and the remote Will-Executor list |
| v0.1.0 | Electrum 4.5.8 — first release |

Always check the [releases page](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-electrum-plugin/releases) for the current pairing before upgrading Electrum.

## Networks

The protocol and the servers support **mainnet**, **testnet**, **testnet4**, **signet** and **regtest**. The [WeList](../will-executors/welist.md) lets you filter Will-Executors by network — useful when you want to run a full test before committing real funds.

## Unconfirmed incoming transactions

You can set up an inheritance on a wallet with an **incoming transaction still in the mempool**. It does not need to be confirmed — being visible in the mempool is enough. The plugin handles this case automatically.
