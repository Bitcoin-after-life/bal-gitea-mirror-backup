# DRAFT — Design choices and comparison with other approaches

> **STATUS: NOT PUBLISHED. NOT VALIDATED BY THE DEVELOPERS.**
>
> This page contains technical positioning material that must be reviewed before it
> becomes an official project statement. It is kept here, out of the published site,
> so it is ready when that review happens.
>
> Source: internal drafts (July 2026), corrected version of 24/07/2026.
> Tone rule: factual, no disparagement — these are design trade-offs, not mistakes.

## Three different approaches

| | Liana (Wizardsardine) | Timelock Recovery (Electrum plugin) | BAL |
|---|---|---|---|
| Mechanism | **Script-based** wallet (Miniscript): the timelock lives in the script locking the UTXOs | **Pre-signed transactions**: an Alert + Recovery pair | **Pre-signed transactions** with absolute `nLockTime` |
| Timelock type | Relative (CSV / BIP 68), per UTXO | Relative (nSequence / BIP 68) on the *second* transaction | Absolute (`nLockTime`) |
| What the BIP 68 cap limits | Maximum inactivity before the recovery key becomes usable | Only the **cancellation window** (2–388 days), not the inheritance horizon | — (does not use relative timelocks) |
| Distribution | None: on-chain | User/heir keeps the plan; monitoring services exist | Decentralized Will-Executor network |

## Liana: where the ~15 month limit comes from

Liana uses **relative** timelocks (CSV/BIP 68) inside the script: the countdown starts
when each individual UTXO is received. The field is 2 bytes, so it cannot exceed 65,535 —
a **Bitcoin consensus limit**, not a wallet limitation. In blocks: 65,535 blocks ≈ **455
theoretical days (~15 months)**. In the time variant (512-second units) the cap is
equivalent to **~388 days**.

Documented consequence, acknowledged by its own developers: the user must periodically
"rotate" coins (send them to a new address of their own) to reset the timer — paying fees
each time — otherwise the recovery path opens. The project itself notes this fits poorly
with cold wallets that are never touched, and that a non-technical heir may struggle,
since not all UTXOs unlock at the same block height.

**The DCA example (valid for Liana, NOT for Timelock Recovery):** someone buying monthly
for a year accumulates twelve UTXOs with twelve distinct countdowns. If they die after the
last purchase, the heirs do not find a single inheritance but **unlocks staggered over more
than a year**, with several recovery transactions to execute at different moments — unless
the user had manually consolidated the UTXOs (which is the same periodic rotation already
required).

## Timelock Recovery: NOT like Liana — closer to BAL

Timelock Recovery is **not** a script wallet and does **not** cap the inheritance horizon
at ~1 year. It uses a pair of pre-signed transactions:

1. **Alert Transaction** — consolidates funds to a new address of the user's own wallet
   (plus small anchor outputs for CPFP). The heir broadcasts it when needed, and its
   appearance on-chain acts as an *alert* to the user.
2. **Recovery Transaction** — spends the Alert's output to the heirs' wallets, with a
   **relative nSequence** imposing a waiting period.

The **388** figure belongs here: it is the maximum **cancellation window** (whole days,
2 to 388), not the duration of the inheritance. Beyond 388 days you overflow BIP 68's
bits. The logic is the opposite of Liana's: that window gives a still-living user time to
cancel everything (by spending the funds) if the plan is triggered by mistake or under
duress.

**Positioning note:** the author of Timelock Recovery has filed **BIP 128** (draft,
assigned 5 February 2026) to standardise the JSON format of such plans, with a reference
implementation in the plugin officially included in Electrum's repository. This is a
competitor on a standardisation trajectory — to be treated with respect and precision.

## BIP 128's critique of BAL's approach — have the answer ready

The Motivation section of BIP 128 explicitly criticises the scheme BAL uses: the single
pre-signed transaction with a future `nLockTime`. The argument: in the "all is well"
scenario (user alive, seed not lost), as the date approaches the user **must** access the
wallet and spend a UTXO to revoke the transaction, otherwise it becomes valid and moves
the funds **with no cancellation period**. There is no window for second thoughts: on the
date, the transaction is valid and the Will-Executors broadcast it.

This is the most serious technical objection BAL will face from a competent audience. It
should be met head-on, not avoided. Elements of a reply (**to be validated by the devs**):

- The **2–3 year renewal policy** with a rolling deadline is precisely the safeguard: someone
  who renews never gets close to the date.
- The user always retains control: spending a single UTXO invalidates the inheritance.
  Revocation is always in their hands and requires nobody's permission.
- The plugin allows setting a reminder in the system calendar.
- Declared trade-off: BAL chooses **simplicity for the heir** (one date, one transaction,
  nothing to execute) at the price of requiring **renewal discipline from the owner**.
  Timelock Recovery chooses the opposite: no looming deadline for the owner, but the heir
  must execute two transactions in sequence and wait out the cancellation period.
- Possible future improvement (issue for [DEV]): an alert / cancellation-window mechanism
  in BAL as well.

## BAL's real advantage, stated precisely

Not "the others are limited to one year" — that is imprecise and easy to attack. The
correct positioning rests on three points:

1. **Nothing required of the heirs.** With Timelock Recovery someone must broadcast the
   Alert, wait days or months, then broadcast the Recovery — knowing what they are doing
   and having kept the plan. With BAL the Will-Executor network broadcasts on its own: the
   heir may not even know in advance.
2. **No consensus-forced rotation** (vs Liana), and no problem of UTXOs unlocking at
   different times: a single date for the whole wallet.
3. **Distributed redundancy.** The plan does not depend on a file someone must keep and
   find: one Will-Executor out of N still operating is enough. (The USB file remains as an
   extra backup.)

## Technical points not to get wrong in public

- The 65,535 limit is a **Bitcoin consensus** limit (BIP 68) and applies only to **relative**
  timelocks. Absolute `nLockTime` has no such cap: in practice it can set dates decades away
  (with 32-bit Unix timestamps the practical limit is the year 2106).
- Do **not** say "Timelock Recovery only allows one year of inheritance" — it is **false**,
  and anyone familiar with the project would dismantle it instantly. The 388 is the maximum
  cancellation window.
- Do **not** say "Liana is limited to 388 days" without qualification: in the block variant
  it is 65,535 blocks (~455 theoretical days, ~15 months); 388 days is the time variant.
- Remember that transactions with a future `nLockTime` are **not accepted by the mempool**
  until the date is reached — this is why someone (the Will-Executors) must broadcast them
  at the right moment.
- Balance note: any very long timelock carries a theoretical risk tied to the future
  evolution of the Bitcoin network — a further argument for periodic renewal.

## Editorial rules carried over from the source drafts

- Never use, in any public version: "investment", "return", "yield", "pension",
  "passive income", or any projection of future sat or fee prices.
- Always keep the risk disclaimer when publishing Will-Executor recruitment material.
- Correct formula for Electrum: "listed in Electrum's third-party plugin directory" —
  **never** "approved by Electrum".

## Open actions

1. Have the devs validate the technical FAQ answers (what a Will-Executor can actually see,
   CPFP) and above all the reply to the BIP 128 critique — the most exposed point of BAL's design.
2. Evaluate as a [DEV] issue a possible alert / cancellation mechanism in BAL.
3. Turn the Will-Executor recruitment article into a blog post (the remaining `[to complete]`
   fields need dev input: current fee range, minimum system requirements).
