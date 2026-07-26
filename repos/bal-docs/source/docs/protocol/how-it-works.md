# How the protocol works

A summary of the mechanism, for readers who want to understand what happens underneath the interface.

## The building block: absolute time-locks

Bitcoin lets a transaction declare an **absolute `nLockTime`**: a point in time before which the transaction is not valid. Nodes reject such a transaction from the mempool until that moment arrives — this is a consensus rule, not a policy choice.

BAL builds on exactly this. Your inheritance is an ordinary Bitcoin transaction, signed by you today, that the network will simply refuse to accept until the date you chose.

Absolute time-locks are not subject to the ceiling that limits *relative* time-locks (BIP 68, capped at 65,535 units). In practice you can set a date decades away.

## Why someone has to broadcast it

Because the mempool rejects it, a time-locked transaction **cannot simply be left with the network** to be picked up later. Someone has to hold it and submit it when the time comes. That is the entire reason Will-Executors exist.

## The lifecycle

1. **Prepare.** The plugin builds a transaction spending the wallet's UTXOs, with outputs to your heirs and one extra output paying the Will-Executor's fee. One transaction per selected Will-Executor.
2. **Sign.** Electrum signs with your key. The plugin never signs on your behalf.
3. **Distribute.** Each signed transaction goes to its Will-Executor, which stores it.
4. **Wait.** You keep using your wallet. Balance changes trigger a refresh; [Check Alive](../user-guide/check-alive.md) offers to postpone the date while you are around.
5. **Execute.** On the delivery date, each Will-Executor holding a copy broadcasts. The first one to get its transaction confirmed collects the fee; the heirs receive the funds.

## Why one transaction per Will-Executor

Each transaction pays a *specific* server. That has three consequences worth understanding:

- **Competition.** Servers race to broadcast, so the reward reaches whoever is actually operating rather than whoever is merely listed somewhere.
- **Independence.** Will-Executors never need to talk to each other. No coordination layer means no central point of failure or attack.
- **No retroactive participation.** A signed transaction cannot be modified, and it already names its payee — so a server joining today can only receive wills created from today on.

## What invalidates a will

The transaction spends specific UTXOs. Spending any of them elsewhere makes it permanently invalid: nodes will reject it as a double-spend attempt.

This cuts both ways. It is why the plugin has to rebuild the will whenever your balance changes — and it is also **your escape hatch**: you can revoke a pending inheritance at any time, without anyone's permission, simply by moving a coin.

## Fees

Two distinct fees, both in bitcoin:

| Fee | Paid to | When |
|---|---|---|
| **Miner fee** | Bitcoin miners | On confirmation, like any transaction. Set generously so the inheritance confirms even under congestion, possibly years from now. |
| **Will-Executor fee** | The server that broadcast successfully | Deducted from the inheritance at execution. |

## Known design trade-off

BAL optimises for **simplicity on the heir's side**: one date, one transaction, nothing for the heir to do or even know about in advance.

The cost of that choice falls on the wallet owner: there is **no cancellation window**. On the delivery date the transaction becomes valid and the Will-Executors broadcast it. If you are alive and want to stop it, you must act *before* that date — by postponing it, or by spending a UTXO to invalidate it.

This is why the recommended practice is a **rolling deadline renewed every 2–3 years**: it keeps you structurally far from the delivery date, so the question never becomes urgent. See [Keeping the will up to date](../user-guide/updating.md).
