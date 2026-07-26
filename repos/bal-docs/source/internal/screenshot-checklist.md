# Screenshots — how to produce and replace them

## The short version

Every image in the manual is listed in **`screenshots.yml`** at the project root.
To replace one, save your screenshot as `docs/img/<filename>` using **exactly** the
filename from the manifest. No Markdown edit is needed — the pages already point there.

```bash
python3 tools/make_placeholders.py --list   # what's still missing
python3 tools/make_placeholders.py          # fill gaps with placeholders
```

Placeholders never overwrite a real image.

## Capture rules — keep these identical everywhere

- **Electrum window only**, not the whole desktop.
  Windows: ++win+shift+s++ · macOS: ++cmd+shift+4++ then ++space++ · Linux: your screenshot tool's window mode.
- **~1280×800**, system zoom at 100%.
- **One theme** for the entire manual — never mix light and dark shots.
- **PNG** format.

## Data hygiene — this is the important part

Screenshots of an inheritance plugin leak real information if you are careless.

- Use a **dedicated test wallet**, ideally on testnet.
- **Fake heir names** (Jonny, Andrea — as in the original manual).
- **Round, obviously fake amounts.**
- **No real Bitcoin address** that belongs to you or anyone you know.
- Check the window title bar, the status bar, and any visible file paths — they often
  contain a username or a real wallet filename.

## Annotations

If you add arrows or highlight boxes, keep one consistent style across the whole manual:
same colour, same stroke width, same arrowhead. Inconsistent annotation looks worse than none.

## Adding a new image

1. Add an entry to `screenshots.yml` (`file`, `page`, `shows`).
2. Reference it in the page:

    ```markdown
    ![Alt text](../img/your-file.png){ .screenshot }
    *Caption line.*
    ```

3. Run `python3 tools/make_placeholders.py` so the build stays green until the real shot exists.
