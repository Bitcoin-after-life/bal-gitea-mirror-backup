# Brand assets

The official logo files are installed. This page explains what each one is and when to use it.

## Files in `docs/assets/`

| File | What it is | Used by |
|---|---|---|
| `logo.svg` | Copy of `logo-mark-white.svg` | Site header (Material reads `theme.logo`) |
| `logo-mark-white.svg` | Symbol only, white | Header (on the accent blue), dark scheme |
| `logo-mark-black.svg` | Symbol only, black | Homepage lockup in light scheme |
| `logo-lockup-black.svg` | Symbol **+ "Bitcoin After Life"** wordmark, black | Print, covers, external use |
| `logo-lockup-white.svg` | Same lockup, white | Dark backgrounds, external use |
| `favicon.png` | Symbol in `#2d6cdf`, 256×256 | Browser tab (`theme.favicon`) |

All were derived from the official `Logo nero.svg`: recoloured, and — for the mark-only
versions — cropped to `viewBox="0 0 205.29 212.5"` to remove the wordmark band.

## The Optima dependency — important

The wordmark in the **lockup** files is live `<text>` in **Optima-Bold**, not outlines.
On any machine without Optima installed, the browser substitutes another face and the
wordmark no longer matches the brand.

Two mitigations are already in place:

1. A fallback stack — `'Optima-Bold', Optima, Candara, 'Segoe UI', 'Trebuchet MS', sans-serif`.
2. The three original absolutely-positioned text runs (`Bitcoin` / ` ` / `After Life`) were
   merged into a **single centred `<text>`**. In the original they overlapped badly whenever
   Optima was missing.

**The proper fix**, when someone has Optima and a vector editor: open the lockup, convert
the text to outlines (Illustrator: *Type → Create Outlines*; Inkscape: *Path → Object to Path*),
re-export, and replace the two lockup files. After that the wordmark is font-independent.

Until then, prefer the **mark-only** files wherever the rendering environment is unknown.

## Why the homepage does not use the lockup SVG

The homepage builds its own lockup: the mark, then "Bitcoin After Life" as HTML text in
**Inter**. This mirrors what the website's top bar does, and sidesteps the Optima problem
entirely. See `.bal-hero` in `docs/stylesheets/extra.css`.

## Typeface note

`OPTIMA.TTF` ships in the `bal-website` repo, but Optima is a licensed commercial typeface.
Do **not** self-host it as a webfont in this repository unless the project holds a webfont
licence. The manual uses Inter, matching the landing page's `--display` / `--body` stack.

## Replacing a logo later

Keep the filenames. Drop the new file over the old one and rebuild — `mkdocs.yml`,
`index.md` and the CSS all reference these names, so nothing else changes. If you replace
`logo-mark-white.svg`, copy it over `logo.svg` too (that is the header logo).
