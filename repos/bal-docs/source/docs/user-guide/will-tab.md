# :material-format-list-checks: The WILL tab

The **WILL** tab is the technical view of your inheritance: one row per inheritance transaction, per Will-Executor.

![WILL tab](../img/will-tab.png){ .screenshot }
*Each row is a signed inheritance transaction held by one Will-Executor.*

## Columns

| Column | Meaning |
|---|---|
| **Locktime** | The delivery date of the inheritance |
| **Txid** | Identifier of the Bitcoin transaction |
| **Will-Executor** | Which server holds this copy |
| **Status** | How far the transaction has progressed (see below) |
| **Server** | Whether that server actually has it (see below) |

The two rightmost columns answer different questions. **Status** describes the life of the
transaction itself; **Server** describes only whether one particular Will-Executor is holding
its copy.

## The Status column

Status is **cumulative**: it keeps the whole history, joining each stage with a dot. A freshly
signed will that has not been sent yet reads `New.Signed`, while one stored and verified on a
server reads `New.Signed.Pushed.Checked`.

The **row's background colour** shows the latest stage reached, so you can read the state of
every will at a glance without parsing the text. Each stage name below links to the point in
[How the protocol works](../protocol/how-it-works.md#the-lifecycle) where that step happens.

<div class="status-table" markdown>

| # | Stage | Meaning | Colour |
|---|---|---|---|
| 1 | [**New**](../protocol/how-it-works.md#stage-prepare) | Created, not yet signed | <span class="sw" style="--c:#FFFFFF"></span> White `#FFFFFF` |
| 2 | [**Signed**](../protocol/how-it-works.md#stage-sign) | Signed in the wallet | <span class="sw" style="--c:#2BC8ED"></span> Azure `#2BC8ED` |
| 3 | [**Pushed**](../protocol/how-it-works.md#stage-distribute) | Sent to the Will-Executor | <span class="sw" style="--c:#73F3C8"></span> Azure-green `#73F3C8` |
| 4 | [**Checked**](../protocol/how-it-works.md#stage-distribute) | Verified on the server | <span class="sw" style="--c:#8AFA6C"></span> Bright green `#8AFA6C` |
| 5 | [**Confirmed**](../protocol/how-it-works.md#stage-execute) | Confirmed on-chain | <span class="sw" style="--c:#BFBFBF"></span> Grey `#BFBFBF` |
| 6 | [**Pending**](../protocol/how-it-works.md#stage-execute) | Awaiting confirmation | <span class="sw" style="--c:#FFCE30"></span> Yellow `#FFCE30` |
| 7 | [**Failed**](../protocol/how-it-works.md#stage-distribute) | Server could not be reached | <span class="sw" style="--c:#E83845"></span> Red `#E83845` |
| 8 | [**Invalidated**](../protocol/how-it-works.md#what-invalidates-a-will) | An input UTXO was spent | <span class="sw" style="--c:#F87838"></span> Orange `#F87838` |
| 9 | [**Replaced**](../protocol/how-it-works.md#what-invalidates-a-will) | Superseded by a newer will | <span class="sw" style="--c:#FF97E9"></span> Violet `#FF97E9` |

</div>

!!! tip "The three colours you want to see"
    In normal operation a will sits at **Checked** (bright green) until the delivery date, then
    moves to **Pending** (yellow) and finally **Confirmed** (grey). **Failed** (red) means a server
    is unreachable; **Invalidated** (orange) means the will no longer matches your wallet and must
    be rebuilt — see [Keeping the will up to date](updating.md).

## The Server column

The Server column tells you, independently of the row colour, whether each inheritance transaction is really stored on its Will-Executor. It is a plain textual label — no colour coding.

![Server column](../img/will-server-column.png){ .screenshot }
*The Server column reports storage state per Will-Executor.*

| Label | Meaning |
|---|---|
| **Confirmed on server** | The Will-Executor confirmed it has stored the transaction |
| **Sent (not checked)** | Pushed to the Will-Executor, not re-checked since |
| **Send failed / Not on server** | The push failed, or the server no longer has it |
| **Signed (not sent)** | Signed locally, not sent to any Will-Executor |
| **Not sent** | Not signed or sent yet |

!!! tip "Check your wills periodically"
    Right-click a transaction and use the check action to re-query the servers. A row that drifts to *Not on server* is a signal that this Will-Executor is no longer reliable — consider [renewing your will](updating.md) with a different selection.

## Actions

![Right-click menu on a will transaction](../img/will-context-menu.png){ .screenshot }
*Right-click a row to act on it.*

| Action | What it does |
|---|---|
| **Prepare** | Builds the inheritance transaction and adds it to the list |
| **Sign** | Signs it — Electrum asks for your password or hardware wallet confirmation |
| **Broadcast / push** | Sends the signed transaction to the selected Will-Executors |
| **Check** | Asks each server whether it still holds its copy, and updates the Server column |
| **Display / Details** | Opens the Will-Details window |

!!! note "The plugin finishes the job on close"
    When you close Electrum, the plugin completes any of prepare / sign / broadcast that is still pending — asking you to confirm the signature. It never signs on your behalf.

## Will details

![Will-Details window](../img/will-details.png){ .screenshot }
*Everything about one inheritance transaction in one window.*

The details window shows the locktime (delivery date), the creation time, the miner fee, the current status, the list of heirs, the Will-Executor address, and the Will-Executor's fee.

---

!!! question "Still have a question?"
    The [FAQ](../faq.md) answers the most common ones — costs, hardware wallets,
    privacy, and what happens if a Will-Executor disappears.
