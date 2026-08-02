#!/usr/bin/env python3
"""Generate a visible placeholder for every screenshot listed in screenshots.yml
that does not yet exist in docs/img/.

Real screenshots are never overwritten: drop the real PNG in docs/img/ with the
manifest filename and this script will simply skip it.

Usage:
    python3 tools/make_placeholders.py            # fill in what's missing
    python3 tools/make_placeholders.py --list     # show what is still missing
    python3 tools/make_placeholders.py --force    # redraw ALL placeholders
                                                  # (never touches real screenshots
                                                  #  unless they are placeholders)
"""
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    sys.exit("Missing dependency: pip install pyyaml")
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "screenshots.yml"
IMG_DIR = ROOT / "docs" / "img"
W, H = 1280, 720
BG = (238, 241, 244)      # --bal-bg  #eef1f4
FG = (91, 102, 114)       # --bal-muted #5b6672
ACCENT = (45, 108, 223)   # --bal-accent #2d6cdf
# Backgrounds ever used by this script — lets --force recognise its own output
# (including older revisions) without ever touching a real screenshot.
KNOWN_PLACEHOLDER_BGS = {BG, (241, 243, 245)}


def draw_placeholder(path: Path, name: str, shows: str) -> None:
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    d.rectangle([8, 8, W - 8, H - 8], outline=ACCENT, width=4)
    d.text((48, H // 2 - 60), "SCREENSHOT NEEDED", fill=ACCENT)
    d.text((48, H // 2 - 20), f"file: docs/img/{name}", fill=FG)
    d.text((48, H // 2 + 10), f"shows: {shows}", fill=FG)
    d.text((48, H - 60), "Replace this file with the real screenshot (same filename).", fill=FG)
    img.save(path)


def is_placeholder(path: Path) -> bool:
    """A placeholder is exactly W x H with our flat background colour."""
    try:
        with Image.open(path) as im:
            return im.size == (W, H) and im.convert("RGB").getpixel((2, 2)) in KNOWN_PLACEHOLDER_BGS
    except Exception:
        return False


def main() -> None:
    manifest = yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))
    IMG_DIR.mkdir(parents=True, exist_ok=True)
    missing, created = [], 0
    for item in manifest["screenshots"]:
        target = IMG_DIR / item["file"]
        if target.exists() and not ("--force" in sys.argv and is_placeholder(target)):
            continue
        missing.append(item)
        if "--list" not in sys.argv:
            draw_placeholder(target, item["file"], item.get("shows", ""))
            created += 1
    if "--list" in sys.argv:
        if not missing:
            print("All screenshots present.")
        else:
            print(f"{len(missing)} screenshot(s) still missing:")
            for m in missing:
                print(f"  - {m['file']:38s} ({m.get('page','')}) — {m.get('shows','')}")
    else:
        print(f"{created} placeholder(s) generated, {len(manifest['screenshots']) - created} real image(s) already in place.")


if __name__ == "__main__":
    main()
