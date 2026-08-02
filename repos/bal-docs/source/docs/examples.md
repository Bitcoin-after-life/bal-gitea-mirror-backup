# :material-script-text: An example of a proper succession plan with Bitcoin

A concrete, start-to-finish walkthrough of how someone actually sets up a Bitcoin
inheritance with BAL — and keeps it current for years without spending anything.

## Step by step

<div class="steps" markdown>

<div class="step" markdown>
**:material-shield-check: Start from Electrum.** You use [Electrum](https://electrum.org),
a state-of-the-art Bitcoin wallet with one of the longest security track records in the
ecosystem. BAL builds on it rather than reinventing the wallet.
</div>

<div class="step" markdown>
**:material-usb-flash-drive: Add a hardware wallet (optional).** If you want, pair Electrum
with a hardware device to raise the security of your keys even further. BAL works the same
either way — Electrum still does all the signing.
</div>

<div class="step" markdown>
**:material-account-heart: You have funds to protect.** You hold bitcoin in Electrum that,
in case of an accident, death, or a lost seed, should reach your children — or simply a
backup wallet of your own.
</div>

<div class="step" markdown>
**:material-puzzle: Install the BAL plugin.** Add Bitcoin After Life to Electrum. See
[Installation](getting-started/installation.md).
</div>

<div class="step" markdown>
**:material-auto-fix: Set up the inheritance with the guided wizard.** BAL walks you through
the whole thing step by step — see [Create your first will](getting-started/quick-start.md).
</div>

<div class="step" markdown>
**:material-account-group: Add your heirs and their shares.** Enter how many heirs receive
the funds and the share for each. *To avoid future disputes, equal shares for everyone are
usually the wisest choice.*
</div>

<div class="step" markdown>
**:material-calendar-clock: Choose the delivery date.** For example 5 or 10 years from now.
</div>

<div class="step" markdown>
**:material-file-sign: The plugin builds the post-dated transaction.** It includes the fee
for the miners and for the Will-Executors — the servers that hold the will until the chosen
date and, on that day, broadcast it to the Bitcoin nodes that will accept the transaction.

!!! success "Setting up and updating a will costs nothing"
    You pay no fee to create or update an inheritance. The fees exist only in the transaction
    itself and are paid **only if and when** the inheritance is actually executed.
</div>

<div class="step" markdown>
**:material-refresh: Renew it once a year or two.** Mark it in your calendar. When you renew
— adding an heir, changing shares — the plugin rewrites the inheritance transaction and
re-sends it to the Will-Executors **dated one day earlier**, so you avoid an on-chain
transaction and any mining fee. Renewing regularly brings two real advantages:

- **More redundancy** — each renewal can pick up additional Will-Executors that will hold
your will until the delivery date.
- **Future-proofing** — a fresh time-locked transaction can incorporate future Bitcoin
protocol upgrades, such as quantum-resistant security, as they arrive.
</div>

</div>

## Using your wallet normally

Setting up an inheritance does not freeze your wallet — you keep using Electrum as usual.

If you **receive** or **spend** funds after the will is set up, the next time you close
Electrum the BAL plugin checks your balance (by inspecting your UTXOs) and warns you if the
inheritance needs updating.

Here too the plugin **anticipates the date by one day**. So a will set to execute on
**30 July 2031** is reissued for **29 July 2031** — with no need to write anything on-chain
and no miner fee. This works because an *earlier* time-locked transaction automatically
invalidates the later one it replaces.

!!! info "Why anticipating is free, and postponing is not"
    Moving the date **earlier** (anticipating) is free, because the new transaction simply
    supersedes the old one. Moving the date **later** (postponing) is the one case that needs
    a real on-chain transaction, and therefore a small miner fee. For the full mechanics see
    [Keeping the will up to date](user-guide/updating.md#postponing-vs-anticipating).

---

!!! question "Still have a question?"
    The [FAQ](faq.md) answers the most common ones — costs, hardware wallets,
    privacy, and what happens if a Will-Executor disappears.
