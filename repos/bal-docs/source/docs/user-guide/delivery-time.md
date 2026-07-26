# Delivery time

The **delivery time** (technically the transaction's `nLockTime`) is the date on which the Will-Executors broadcast your inheritance to the Bitcoin network.

## How to express it

| Form | Example | Meaning |
|---|---|---|
| **Date** | 1 December 2027 | A precise calendar date, pickable from the system calendar |
| **Raw / relative** | `1y` | One year from today |
| | `1d` | Tomorrow |

## Timing tolerance

The plugin converts your date into an estimated block height, so execution is not accurate to the minute:

- With **relative** values, expect a tolerance of a few hours.
- Bitcoin nodes accept a time-locked transaction based on the **median time past** of the last 11 blocks, so in practice the inheritance is executed on average about **one hour after** the time you set.

For an inheritance measured in years, this is irrelevant. It only matters if you are running a short test.

!!! info "Why nobody can execute it early"
    A transaction with a future `nLockTime` is **rejected by the Bitcoin mempool** until that time is reached. This is a consensus rule, not a courtesy: even a malicious Will-Executor cannot make the inheritance happen sooner.

## No expiry, no upper limit in practice

Absolute time-locks are not subject to the ~15 month ceiling that affects *relative* time-locks (BIP 68). You can set a delivery date decades ahead.

!!! tip "Even so, renew every 2–3 years"
    A rolling deadline — "always five years from today", refreshed periodically — is the recommended way to use BAL:

    - the network grows, so renewing lets you include **newer Will-Executors** and drop ones that have shut down;
    - it keeps you far from the delivery date at all times;
    - it picks up future protocol improvements.

    See [Keeping the will up to date](updating.md).

## Changing the date later

Bringing the date **forward** and pushing it **back** are not symmetric operations — the difference is explained in [Keeping the will up to date](updating.md#postponing-vs-anticipating).
