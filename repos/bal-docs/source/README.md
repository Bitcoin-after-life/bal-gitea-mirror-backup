# Bitcoin After Life — Manual

Docs-as-code source for the BAL documentation, built with [MkDocs Material](https://squidfunk.github.io/mkdocs-material/).
Published at <https://bitcoin-after.life/docs/>.

## Quick start

**Windows:** double-click `serve.bat`.
**Linux / macOS:** run `./serve.sh`.

Either one installs the dependencies and opens a live preview at
<http://127.0.0.1:8000>, with working search and auto-reload on save.

Manually:

```bash
pip install mkdocs-material mkdocs-macros-plugin pyyaml
python3 tools/make_placeholders.py     # fill missing screenshots with placeholders
mkdocs serve                           # preview at http://127.0.0.1:8000
mkdocs build --strict                  # static site into site/
```

> Opening the built HTML files directly from disk (`file://`) works for reading,
> but **search will not run** — browsers block the index fetch. Use the server.

## Layout

```
docs/                 published pages (Markdown)
docs/img/             screenshots — filenames fixed by screenshots.yml
docs/stylesheets/     screenshot framing CSS
screenshots.yml       manifest of every image the manual expects
tools/                helper scripts
internal/             NOT published — drafts and working notes
.gitea/workflows/     CI: build + deploy on push to main
```

## Versions in one place

Plugin and Electrum versions, and the WeList fee, are **not** hardcoded in the pages.
They live in `mkdocs.yml` under `extra.bal:` and are referenced as `{{ bal.plugin_version }}`.
Change the value once and every page updates on the next build.

| Variable | Meaning |
|---|---|
| `plugin_version` | Latest plugin release |
| `electrum_tested` | Electrum version that release was tested against |
| `electrum_latest` | Latest stable Electrum the manual assumes |
| `welist_fee_sats` / `welist_period_days` | WeList listing terms |
| `last_reviewed` | Bump after a full editorial pass |

A typo in a variable name **fails the build** (`on_error_fail: true`), so a broken
reference can never reach the published site.

## Brand assets

`docs/assets/logo.svg` and `docs/assets/favicon.png` are **placeholders**.
See `internal/brand-assets.md` for the official sources and how to swap them in.

## Updating the documentation

**Text.** Edit the `.md` files in `docs/`, commit, push. Pages have an edit link
pointing back at Gitea, so small fixes can be made from the web editor.

**Screenshots.** Save the new image over `docs/img/<name>.png` using the filename in
`screenshots.yml`. Nothing else to change. See `internal/screenshot-checklist.md`
for capture rules and data-hygiene warnings.

**Adding a page.** Create the `.md` file under `docs/`, then add it to `nav:` in
`mkdocs.yml`. `mkdocs build --strict` fails on broken internal links, so a typo in a
cross-reference is caught before deploy.

## Italian translation

The configuration is prepared but not enabled. To turn it on:

```bash
pip install mkdocs-static-i18n
```

then uncomment the `plugins:` block in `mkdocs.yml` and add `page.it.md` files next to
the English ones. Untranslated pages fall back to English automatically, so you can
translate progressively.

## Deployment

`.gitea/workflows/deploy.yml` builds and publishes to `/var/www/bal-docs/` on every push
to `main`. It requires Gitea Actions enabled and a registered runner labelled
`docs-runner`. Until that is set up, deploy manually:

```bash
mkdocs build
rsync -a --delete site/ /var/www/bal-docs/
```

## Status

Based on the website, the WeList, the Gitea repositories and the BAL MANUAL revB PDF
(July 2026). Open questions and points needing developer validation are tracked in
`internal/source-notes.md`.
