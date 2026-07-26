# Mobile readability fixes — `index.html`

Base: commit `3dcd822` (`bitcoinafterlife/bal-welist-website`, `main`).

**Scope:** header, filter bar and server list only. CSS-only — the JavaScript is
byte-for-byte unchanged. Desktop rendering is unaffected: every change sits
inside a `@media (max-width: ...)` block, except one line on `body` (item 7).

The "Add your Will Executor" panel, the invoice and the QR code were left
untouched on purpose, since payments are expected to be made from a desktop
browser.

---

## 1. Chain selector overflowed the viewport

**Before:** `.seg` was an `inline-flex` row of four buttons (Bitcoin, Testnet,
Testnet4, Regtest) with no wrapping — roughly 340px wide. On a 360–390px phone,
minus page padding, it pushed past the right edge and made the whole page scroll
horizontally.

**After:** below 600px the selector becomes a full-width 2×2 grid. Inner
dividers are recalculated with `:nth-child` so the borders stay correct in the
new arrangement.

## 2. Touch targets too small

Chain buttons were ~33px tall. Raised to `min-height: 44px` (the standard
minimum for a finger), applied to the chain buttons, the Tor toggle row and the
JSON link.

## 3. Nested scrolling in the server list

**Before:** `.mobile-list` carried `max-height: 350px; overflow: auto`, so on a
phone the list was a small window scrolling inside a page that also scrolls —
the two gestures fight each other.

**After:** the height cap is removed below 860px; the list flows with the page.
The desktop table (`.table-card`) keeps its 350px cap unchanged.

## 4. Long URLs broke out of the cards

Server URLs contain no spaces, so the browser cannot break them and they
overflowed the card. Added `overflow-wrap: anywhere`, plus `min-width: 0` on the
flex parent — without it a flex child defaults to `min-width: auto` and refuses
to shrink below its content width.

The JS truncation at 40 characters is unchanged.

## 5. Uneven card grid

The five fields (Score, Wins, Fee, Version, Height) sat in two fixed columns,
leaving Height alone on a half-empty row. Switched to
`repeat(auto-fit, minmax(84px, 1fr))`, which fits as many columns as the width
allows.

## 6. iOS auto-zoom on the search field

iOS Safari zooms the entire page when a focused input has a font size below
16px, and the user then has to pinch back out. The filter field is raised to
16px below 600px and made full width.

## 7. Text inflation in landscape

Added `-webkit-text-size-adjust: 100%` to `body`. iOS enlarges font sizes when a
phone is rotated to landscape, which distorted the header and the cards. This is
the only change outside a media query; it has no effect on desktop browsers.

## 8. Tighter spacing below 480px

New breakpoint reducing padding and type size where space is scarce:

| Element | Before | After |
| --- | --- | --- |
| `.wrap` side padding | 20px | 14px (+12px usable width per row) |
| `header.hero` padding | 48px 0 28px | 28px 0 20px |
| `.brand h1` | 24px | 20px |
| Logo mark | 46px | 40px |
| `.stat .n` | 26px | 22px |
| `.intro-card` padding | 28px 32px | 20px 18px |
| `.info-block` padding | 22px 24px | 18px 16px |
| `.srv` padding | 16px | 14px |
| Footer | space-between, wrapping | single column |

---

## Verification performed

- CSS braces balanced (148 open / 148 close).
- All 32 element IDs referenced by the JavaScript still present in the markup.
- Tag balance checked for `div`, `section`, `header`, `footer`, `main`, `table`,
  `style`, `script`.
- Media queries present in order: 860px, 600px, 480px, `prefers-reduced-motion`.

## Not yet verified

Rendering was not tested on a physical device. Worth a quick check on one iPhone
and one Android phone before merging, in particular the 2×2 chain selector at
320px width (iPhone SE, the narrowest screen still in common use).
