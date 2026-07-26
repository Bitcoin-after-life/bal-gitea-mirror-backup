# Internal notes — NOT published

Everything in this folder is versioned in Gitea but **excluded from the built site**
(`mkdocs build` only reads `docs/`). Use it for material that is not ready — or not
intended — for public publication.

| File | Purpose |
|---|---|
| `design-choices-DRAFT.md` | Competitor comparison and the BIP 128 critique. **Not validated by the devs.** |
| `source-notes.md` | Where each part of the manual came from, and what still needs verification. |
| `screenshot-checklist.md` | How to produce and replace the screenshots. |

To publish something from here: move the file into `docs/`, add it to the `nav` in
`mkdocs.yml`, and remove the draft warnings.
