# WeList web pages

Static pages for the Bitcoin After Life Will-Executor list, served by the
`bal-welist` webservice from its `WELIST_STATIC_DIR` (the `www` directory). There
is no build step: the files are served as they are, and pushing to `main`
republishes them through the server hook.

| File | Page |
|------|------|
| `index.html` | Leaderboard: the ordered list of executors |
| `weinfo.html` | Server detail page (`/weinfo/{chain}/{id}`) |
| `terms.html` | Terms and conditions |
| `w3.css`, `bal.js`, `qrcode.min.js`, `purify.min.js` | Shared assets |

## Header statistics

`index.html` reads one `/api/info?chain=…` request per chain, which already
carries the chain header, the ranking settings and the ordered executor list.
The hero shows four values:

| Stat | Source | Notes |
|------|--------|-------|
| **At tip** | count of executors whose `status` is `OK` | Not "online": see below |
| **Servers** | length of the returned list | The page's page size, not the total |
| **Block** | `height` | Tip height, `—` when the chain has no synced block |
| **Median time** | `median_time` | Rendered in the visitor's own locale and timezone |

The block and median are refreshed every 60 seconds, skipped while the tab is in
the background. Median time is the median-time-past of the last block (the median
of the last 11 blocks), so it normally reads about an hour behind the current
time; the element's tooltip says so.

## Executor status

`status` is derived by the server at read time and answers one question: **is the
executor's last accepted ping at the chain tip?** Reachability plays no part — an
executor that answers HTTP but stopped pinging is not healthy.

```text
no ping height (signed or verified WeTime ping) -> KO
no synced tip for the chain                     -> KO
tip older than WELIST_TIP_MAX_AGE_SECS          -> KO   (updater down)
ping height >= tip - WELIST_PING_MAX_LAG_BLOCKS -> OK
otherwise                                       -> KO
```

- `OK` / `KO` are the only two values.
- The ping height is the signed ping height for v2/v3/onion executors, and the
  last verified WeTime ping height for legacy v1 executors, which record no
  replay state.
- `WELIST_PING_MAX_LAG_BLOCKS` (default `6`) allows for the delay between a new
  block and the next ping; `WELIST_TIP_MAX_AGE_SECS` (default `900`) is how long
  a tip may go unrefreshed before it counts as unknown. When the updater stops,
  every executor reads `KO` instead of the list freezing green.

### Server detail page

`weinfo.html` shows the same status as a badge, with the reason:

| Badge | Meaning |
|-------|---------|
| **At chain tip** | `OK` |
| **Behind by N blocks** | `KO`, with the distance from the tip |
| **Updater not reporting** | `KO` because the tip itself is stale |

The badge reads `ping_height`, `tip_height`, `tip_fresh` and `blocks_behind`
from `/weinfodata/{chain}/{id}`, alongside the listing dates, balance, donations,
WeTime activity and fees.

## Related documentation

The server-side behaviour, the API and the business rules live in the
`bal-welist` repository:

- `knowledge/webservice.md` — endpoints and the derived status
- `docs/api-reference.md` — every HTTP endpoint
- `docs/payment-and-ranking.md` — deposits, balances, donations and list order
