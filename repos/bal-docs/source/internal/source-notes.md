# Source notes — where this manual came from, and what to verify

Compiled July 2026. Keep this updated whenever the manual is revised.

## Sources used

| Source | Used for |
|---|---|
| [bitcoin-after.life](https://bitcoin-after.life) (homepage + FAQ) | Current behaviour: wizard, Server column, postpone/anticipate logic, compatibility, wallet-backup use case. **Primary source — most current.** |
| [WeList](https://welist.bitcoin-after.life/) | Listing fee, ranking, networks, renewal window |
| [bal-server](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-server) README + RPC.md | The entire "Running a Will-Executor" page |
| [bal-electrum-plugin](https://bitcoin-after.life/gitea/bitcoinafterlife/bal-electrum-plugin) releases | Version/Electrum compatibility table |
| BAL MANUAL revB (PDF, Jan 2025) | Operational detail retained where still valid: 100% sizing rules, heir fields, wallet-file vs seed, UTXO privacy, export list |
| Internal drafts (July 2026) | Will-Executor FAQ, competition/incentive model, consortium & multisig, renewal cadence |

## Where the old PDF is now WRONG — do not reuse

| PDF revB says | Reality |
|---|---|
| Install by copying files into the `plugins/` folder | Tools → Plugins → **install from file** (zip), Electrum 4.7.2 |
| "BAL 1.0" | Plugin v0.2.6 |
| Manual step-by-step parameter setup | Guided **wizard** since v0.2.0 |
| Check Alive is a base parameter | **Advanced mode** only |
| 9-state colour table for transaction status | **Server column** with 5 textual labels |
| Postponing always requires an on-chain invalidation | Balance/heir changes → will **anticipated by one day and redistributed**, no on-chain fee. On-chain invalidation only for a genuine postponement |
| Will-Executor lists come from BitcoinTalk | **WeList** directory, downloaded by default |
| "prepare / sign / broadcast" run automatically on close | Still true, but the plugin **asks** — it never signs on your behalf |

## To verify with the developers before the next revision

1. **Exact execution tolerance.** The manual states "~1 hour after the set time, due to the
   11-block median". Confirm this still holds.
2. **Relative (Raw) date syntax.** The PDF documents `d` and `y` suffixes. Confirm which
   suffixes the current wizard accepts, and whether Raw input still exists alongside the calendar.
3. **What a Will-Executor can actually see** — the FAQ answer is deliberately cautious.
   A precise formulation would be better.
4. **Default miner fee.** The PDF said 100 sat/vbyte. The current wording says only "a prudent
   default" — confirm the actual value so it can be stated.
5. **Fee range currently charged** by Will-Executors on the network (for the recruitment article).
6. **Minimum system requirements** for a Will-Executor node, stated concretely.
7. **BIP 128 critique** — see `design-choices-DRAFT.md`. The single most exposed point.

## Deliberately left out of the published manual

- Competitor comparison and BIP 128 response → `design-choices-DRAFT.md` (needs dev validation).
- Will-Executor recruitment article → belongs on the blog, not in a manual, and still has
  `[to complete]` fields.
