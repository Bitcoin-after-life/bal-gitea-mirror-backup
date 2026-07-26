# Using BAL as a wallet recovery net

Inheritance is not the only use for a time-locked transaction. The same mechanism gives you a **decentralized safety net against losing your seed**.

## The idea

Set up an inheritance whose beneficiary is a **second wallet you control** — for example a mobile wallet on your phone.

If you lose access to the original wallet and therefore stop opening Electrum, the Will-Executors broadcast the transaction on the delivery date and the funds arrive in your backup wallet. If you keep access, you simply keep postponing the date and nothing ever happens.

## What you need

- A **second wallet already set up**, whose receiving address you know and whose keys you can actually access.
- The BAL plugin configured on the wallet you want to protect, with that address as the beneficiary.
- A delivery date you are comfortable with, and the discipline to keep it refreshed — see [Keeping the will up to date](updating.md).

!!! warning "The backup wallet must be genuinely independent"
    A recovery destination whose keys live on the same device, or are derived from the same seed, protects you from nothing. Use a different device and a different seed, stored separately.

!!! tip "It combines with inheritance"
    Nothing stops you from listing both a recovery address of your own and your heirs. Remember that the plugin always distributes 100% of the wallet — see [Heirs and shares](heirs.md#the-wallet-is-always-emptied-to-100).
