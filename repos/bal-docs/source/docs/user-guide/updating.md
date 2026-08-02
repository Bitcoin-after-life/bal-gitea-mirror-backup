# :material-refresh: Keeping the will up to date

An inheritance transaction spends specific UTXOs. If those UTXOs change, the transaction the Will-Executors are holding becomes invalid — so the will has to be rebuilt.

The plugin handles this for you: **every time you close Electrum it checks your balance and UTXO set**, and if something changed it asks you to refresh the will. You confirm the signature; it never signs on your behalf.

This is what makes it practical to use an inheritance-ready wallet for everyday transactions.

## What triggers a refresh

- You receive funds.
- You spend funds.
- You add, remove, or edit an heir.
- You change the delivery date.

## Postponing vs anticipating

The two directions are not symmetric, and the difference has a real cost:

=== "Anticipating (earlier date)"

    Bringing the delivery date **forward** is cheap. The plugin signs a new inheritance with an earlier locktime and sends it to the Will-Executors. Because it becomes valid sooner, it supersedes the previously distributed transaction — **no on-chain fee**.

    This is also how balance and heir changes are handled: the will is anticipated by one day and redistributed, without invalidating the old one on-chain.

=== "Postponing (later date)"

    Pushing the delivery date **back** requires invalidating the committed coins **on-chain first**. Otherwise a Will-Executor still holding the old, earlier-locktime transaction could broadcast it and execute the inheritance too early.

    That invalidation is a real Bitcoin transaction: you sign it and it costs a **miner fee**.

!!! tip
    If you expect to adjust the date often, plan for it: a rolling deadline that you re-anticipate is cheaper than repeatedly postponing.

## Watch-only copies on other devices

If you monitor the same wallet from another device using its public key (Zpub/Xpub), funds arriving there are **not** added to the inheritance automatically. Open the wallet in Electrum with the plugin so it can pick up the new UTXOs and re-send the updated will to the Will-Executors.

!!! danger "The same seed on multiple devices invalidates the inheritance"
    Using the same seed on more than one device is strongly discouraged in general. With BAL it has a specific consequence: spending even a single satoshi from the other device changes the wallet's UTXO structure, and the nodes will **discard the inheritance transaction** when the Will-Executors try to broadcast it.

## How often to renew

Renewing is not strictly required, but it is recommended roughly **every 2–3 years**. A practical approach is a rolling deadline — "always five years from today" — refreshed at regular intervals.

Renewing serves two purposes: it lets you include Will-Executors that joined the network in the meantime (and drop the ones that shut down), and it picks up future protocol changes.
