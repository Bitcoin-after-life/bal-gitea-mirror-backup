# Security & Privacy

## Where trust actually sits

BAL does not ask you to trust a Will-Executor. It asks you not to depend on any single one:

- Servers hold **no keys** and cannot spend, alter, or accelerate anything.
- You select **as many as possible**; only one has to survive to the delivery date.
- Whether a given operator is a person, a company, or a consortium is not observable from outside — and by design it does not need to be. Protection comes from **redundancy and competition**, not from certifying operators.

## What a Will-Executor sees

The servers you select receive pre-signed transactions that reflect the state of your wallet. That is real information about a living person, refreshed over time as your balance changes.

This is precisely why the choice of *which* servers receive it stays with you, and why privacy is a competitive dimension between Will-Executors — not just price.

Stored transactions are not publicly accessible. You can verify at any time that yours is still held, via the [Server column](user-guide/will-tab.md#the-server-column).

## UTXO consolidation

Sending the entire contents of a wallet in one transaction damages the privacy of your UTXOs: it links everything together, publicly and permanently.

A useful habit, especially when there is a single heir: leave a small remainder to another address. For example 99.7% to the heir and 0.3% elsewhere — an address of your own, or one picked at random from a block explorer. It makes the transaction considerably harder to interpret.

## The heirs list export

The [exported heirs list](user-guide/wallet-backup.md#export-the-heirs-list) states who inherits and how much. Anyone who reads it learns both. Store it accordingly.

The same applies, more strongly, to an [offline backup transaction](user-guide/backup-transaction.md) handed directly to an heir: it reveals the exact date and amount, years in advance. A notary or trusted third party is often the more prudent route.

## The same seed on multiple devices

Strongly discouraged in general, and specifically damaging here: spending a single satoshi from another device changes the wallet's UTXO structure and **invalidates the inheritance transaction**. The Will-Executors will broadcast something the nodes reject.

## Your wallet file is your will

The inheritance data lives in the Electrum **wallet file**, not in the seed. Restoring from seed recovers the funds and loses the will. Back up the file, keep the password — see [Backing up wallet and will](user-guide/wallet-backup.md).

## Long time horizons

Any very long time-lock carries a theoretical risk tied to how the Bitcoin network evolves over decades. This is a further argument for **renewing periodically** rather than signing a thirty-year will once and forgetting it.

## Before real funds

Test on **testnet** or with a small-amount wallet first. Review the generated transactions before broadcasting. The plugin builds real transactions with real consequences, and it never signs without asking you.
