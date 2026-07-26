<div class="bal-hero">
  <img class="logo-light" src="assets/logo-mark-black.svg" alt="Bitcoin After Life">
  <img class="logo-dark"  src="assets/logo-mark-white.svg" alt="Bitcoin After Life">
  <span class="name">Bitcoin After Life</span>
  <span class="tagline">BAL Protocol — decentralized Bitcoin inheritance</span>
</div>

# Bitcoin After Life

**Bitcoin After Life (BAL)** is an open-source plugin for [Electrum](https://electrum.org) that makes **decentralized Bitcoin inheritance** possible: no notary, no lawyer, no custodian.

You set your heirs and a delivery date. The plugin builds a time-locked transaction, Electrum signs it with your key, and a network of independent **Will-Executor** servers holds it and broadcasts it to the Bitcoin network on that date.

!!! info "This manual covers"
    Plugin **v{{ bal.plugin_version }}** on **Electrum {{ bal.electrum_latest }}**. Both the plugin and the protocol are under active development — check the [releases page](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-electrum-plugin/releases) for the version you are running.

## Core ideas in one minute

- **Your keys never leave your device.** The plugin never signs on your behalf: it asks, you decide — password or hardware wallet.
- **The transaction is pre-signed and time-locked.** Bitcoin nodes reject it until the delivery date arrives, so nobody can execute it early.
- **Will-Executors cannot steal or alter anything.** They only store the signed transaction and broadcast it at the right time.
- **Redundancy is the guarantee.** The plugin creates one transaction per selected Will-Executor. Only one of them has to still be alive on the delivery date.
- **The incentive is on-chain.** Each transaction carries a fee for its Will-Executor, paid only when that server successfully broadcasts and the transaction confirms.

## Where to start

<div class="grid cards" markdown>

- **New here?** → [Installation](getting-started/installation.md), then [create your first will](getting-started/quick-start.md).
- **Want the details?** → [Heirs and shares](user-guide/heirs.md), [Delivery time](user-guide/delivery-time.md), [The WILL tab](user-guide/will-tab.md).
- **Curious about the servers?** → [What Will-Executors are](will-executors/what-they-are.md).
- **Want to run one?** → [Running a Will-Executor](will-executors/running-a-server.md).

</div>

!!! warning "Test before you trust it with real funds"
    BAL builds **real** Bitcoin inheritance transactions with time-locks. Try it on **testnet**, or with a small-amount wallet, before setting up an inheritance that matters. You can always review the generated transactions before broadcasting.

## Project links

| Resource | Link |
|---|---|
| Website | [bitcoin-after.life](https://bitcoin-after.life) |
| Plugin source & releases | [bal-electrum-plugin](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-electrum-plugin) |
| Will-Executor server source | [bal-server](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server) |
| Public Will-Executor directory | [WeList](https://welist.bitcoin-after.life/) |
| All repositories | [Gitea](https://bitcoin-after.life/gitea/bitcoinafterlife) |
| Contact | [info@bitcoin-after.life](mailto:info@bitcoin-after.life) |
