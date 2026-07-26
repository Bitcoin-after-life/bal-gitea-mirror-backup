# What Will-Executors are

A Will-Executor is a server with one narrow job: **it stores pre-signed inheritance transactions and broadcasts them to the Bitcoin network on the date the user chose.** Nothing else.

## What a Will-Executor cannot do

- **It cannot spend your funds.** It holds no keys.
- **It cannot alter the transaction.** The transaction is already fully signed by you; any modification would invalidate the signature.
- **It cannot execute early.** The transaction carries an absolute `nLockTime`, and Bitcoin nodes reject it until that date is reached.
- **It does not coordinate with other Will-Executors.** Every node is independent. There is no central point to attack or compromise.

What it *does* require is longevity: being still there, still working, years from now.

## Why competition makes it work

When you create your will, the plugin generates **a separate transaction for every Will-Executor you selected**. Each of those transactions carries an extra output paying that specific server its fee.

On the delivery date, every Will-Executor holding a copy can broadcast. Only one transaction can confirm — and the fee goes to whoever broadcast the one that made it into a block. It is a competitive mechanism, conceptually close to mining: **the reward goes to whoever is actually operating**, not to whoever is merely listed in a directory.

For you, the consequence is simple and reassuring: **only one of your selected Will-Executors has to still be alive on the delivery date** for the inheritance to be executed.

!!! tip "Select as many as you can"
    Redundancy is the whole guarantee. Each additional Will-Executor is another independent chance that someone is still there when it matters.

## Who sets the fees

Each Will-Executor freely sets its own commission — there is no price imposed by the protocol. The market sets the price.

When creating your will you can exclude servers you consider too expensive. It is also worth being wary of **suspiciously cheap** ones: a price that cannot cover the cost of running a server for years is a hint that the service may not last.

!!! warning "Don't lower the fee manually"
    Fees are shown both on the [WeList](welist.md) and inside the plugin. Setting a fee below what a server asks will typically get your transaction rejected by that server.

## New servers cannot join old wills

Inheritance transactions are pre-signed and already contain the address of the Will-Executor that will be paid. A signed transaction cannot be modified — so a server that joins the network today receives the wills created **from today onwards**, never the ones already out there.

This is why renewing your will periodically is useful: it brings newer Will-Executors into your selection. See [Keeping the will up to date](../user-guide/updating.md).

## What they can see

The Will-Executors you select receive pre-signed transactions that reflect the state of your wallet. That is why the choice of which servers receive them stays entirely in your hands, and why privacy — alongside reliability — is the real ground of competition between Will-Executors.

Transactions stored on the servers are **not publicly accessible**. From the plugin you can verify at any time that yours is still there: see the [Server column](../user-guide/will-tab.md#the-server-column).
