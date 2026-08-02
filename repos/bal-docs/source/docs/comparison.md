# :material-scale-balance: How BAL compares

!!! info "This information may be out of date"
    This page compares other projects (Nunchuk, Liana) whose features and prices change
    over time. The details here were gathered from public sources in **mid-2026** — always
    check each project's own site for the current situation before relying on any figure.

Bitcoin After Life is not the only way to plan Bitcoin inheritance. Nunchuk and Liana are
two well-known, respected projects that solve the same problem differently. This page compares
them across the dimensions that tend to matter over a multi-year time horizon — decentralization,
long-term cost, the wallet each one builds on, distribution, and exposure to a single company
being shut down — and it states plainly where each approach, BAL included, is weaker.

## At a glance

<div class="compact-table" markdown>

| | **Bitcoin After Life** | **Liana** (Wizardsardine) | **Nunchuk** (Honey Badger) |
|---|---|---|---|
| Mechanism | Pre-signed transaction, absolute `nLockTime`, held by Will-Executors | Script wallet (Miniscript), relative timelocks in the script | Collaborative-custody multisig (2-of-4) with a timelocked inheritance key |
| Who holds keys | **Only you.** Will-Executors hold no keys | **Only you** (and any recovery keyholders you choose) | You hold 3 keys; **Nunchuk holds 1 platform key** on its servers |
| Base wallet | Runs inside **Electrum** (in use since 2011) | Own codebase (Miniscript / Bitcoin Core) | Own app |
| Recurring cost | **None** | **None** for the wallet, but timelocks need periodic coin rotation → network fees over time | **Subscription** ~$120–$480/yr |
| When you pay | **Only at execution**, from the inheritance transaction, in bitcoin | Network fees on each rotation | Yearly subscription + network fees |
| Distribution | Desktop plugin (`.zip`) — no app store | Desktop + mobile | Primarily mobile app (App Store / Google Play) + desktop |
| Single-company shutdown risk | **No company to shut down** | Wallet survives; hosted backend optional | Assisted flow depends on the company; on-chain failsafe as backup |
| Heir's effort | None — the network broadcasts automatically | Heir signs a recovery transaction after the timelock | Guided beneficiary flow; on-chain failsafe via timelock |
| Open source | Yes | Yes | Yes (app); platform key is a hosted service |

</div>

## Decentralization

**BAL** is the most decentralized in day-to-day operation: a network of independent
Will-Executors, with no coordination between them and no company in the middle. In fairness,
it does depend on Will-Executors continuing to operate, and it uses the [WeList](will-executors/welist.md)
as a discovery directory — a mild point of centralization you can bypass by adding servers
manually.

**Liana** is strongly decentralized on the keys — full self-custody, no company key — and its
recovery path is enforced by the Bitcoin **script itself**, i.e. by consensus rules rather than
any service. Its convenient hosted backend (Liana Connect) is optional; you can run your own node.

**Nunchuk**'s inheritance plans use *collaborative custody*: the company holds one multisig key
on its servers. Nunchuk 2.0 (November 2025) added an on-chain autonomous failsafe using timelocks
and Miniscript, which reduces that dependency — but the platform key remains part of the assisted
setup.

## No cost that recurs over the years

**BAL** costs nothing until the inheritance is actually executed. There is no subscription and no
forced periodic action: the only Will-Executor fee is taken from the time-locked transaction
itself, in bitcoin, and only if a Will-Executor broadcasts it. Keeping the plan current means
[renewing the will](user-guide/updating.md) yourself — no payment involved.

**Liana**'s wallet is free, but its *relative* timelocks have to be reset periodically by moving
coins to a fresh address (a Bitcoin consensus constraint, not a wallet flaw). That means paying
network fees over time and fits cold storage that is never touched poorly.

**Nunchuk**'s inheritance features live behind a yearly subscription (roughly $120–$480 depending
on the plan). It is a real recurring cost, and the plan lapses if it stops being paid.

## A single, battle-tested wallet: Electrum

**BAL** does not build a new wallet. It runs as a plugin **inside Electrum**, which has been in
use since 2011 and is among the most heavily reviewed and tested Bitcoin wallets in existence.
Electrum performs the signing; BAL never signs on your behalf. Electrum is open source and does
not depend on any single company to keep existing, which matters for a plan meant to work decades
from now.

The honest counterpoint: BAL *is* a plugin, so it depends on the plugin being maintained and on
Electrum's plugin interface staying stable. (Note the correct wording: BAL is **listed in
Electrum's third-party plugin directory** — not "approved by Electrum".)

**Liana** and **Nunchuk** ship their own, newer codebases. Both come from competent teams, but
neither has Electrum's number of years in the field.

## No dependency on an app store

**BAL** is a desktop plugin installed from a `.zip` file. It never passes through the Apple App
Store or Google Play, so it cannot be pulled from a store or blocked by a platform's policy — a
recurring problem for crypto apps in some jurisdictions.

**Nunchuk** is primarily a mobile app distributed through the App Store and Google Play (a desktop
version also exists), which makes store availability a real dependency. **Liana** is distributed
from its own site for desktop, with mobile options, so it is less exposed than a mobile-first app.

## Resistance to a company being shut down

This is where an inheritance plan's time horizon really bites: a plan may need to work in ten or
twenty years, across changes in law.

**BAL** has **no company operating a custody service** — so there is no single entity a government
can shut down to break the plan. Will-Executors are independent, can be located anywhere, and at
least one operates Tor-only. If new anti-Bitcoin rules appear in one country (as has been discussed
in places like Russia), there is no headquarters to close and no license to revoke.

**Liana** is made by a company (Wizardsardine), but the wallet is self-custodial and open source:
if the company disappeared, existing scripts keep working on-chain because the recovery path is
enforced by the script, not the company. Only the optional hosted backend would stop, and you can
switch to your own node.

**Nunchuk** runs a collaborative-custody service with a platform key, which makes the company a
regulatory target — a hostile jurisdiction could force it to relocate, and closure is at least as
likely as relocation. To their credit, Nunchuk engineered the on-chain timelock failsafe precisely
so beneficiaries can still recover **without** the company; but the *assisted* experience does
depend on the company continuing to operate.

## Where the other approaches are stronger

Fairness requires stating this plainly — BAL is not the best choice on every axis:

- **Liana** gives you a cancellation window by design and needs no external network to broadcast;
  the recovery path is enforced by the script itself. If you keep coins consolidated, its model is
  very clean.
- **Nunchuk** offers a guided, supported experience with a beneficiary flow and a company standing
  behind it — valuable for users who want assistance rather than a self-run plan, and who are
  comfortable with one key in collaborative custody.
- **BAL**'s own trade-offs: it is **desktop-only** (Electrum), it has **no built-in cancellation
  window** (so it asks the owner for renewal discipline — see [Keeping the will up to date](user-guide/updating.md)),
  and it relies on Will-Executors continuing to operate (mitigated by redundancy and the
  [offline backup transaction](user-guide/backup-transaction.md)).

There is no single best choice — only the one that matches how much you want to self-manage,
whether you accept a third-party key or an app store or a company in the loop, and whether you
want a plan that costs nothing and depends on nothing but Bitcoin.

---

!!! question "Still have a question?"
    The [FAQ](faq.md) answers the most common ones — costs, hardware wallets,
    privacy, and what happens if a Will-Executor disappears.
