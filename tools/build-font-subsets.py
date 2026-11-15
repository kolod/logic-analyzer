#!/usr/bin/env python3
"""Builds the Japanese font subset the application embeds (spec 006, research R5).

The interface must show Japanese on a machine that has no CJK font (a minimal Debian install).
iced already bundles Fira Sans, which covers Latin, Latin Extended, Greek and Cyrillic, so only
Japanese needs a font of its own: Noto Sans JP (SIL Open Font License 1.1), cut down to

* every character of `resources/i18n/ja/*.ftl`,
* basic Latin, the Japanese punctuation block, hiragana, katakana and the full-width forms,
* the JIS X 0208 level 1 kanji (the 2,965 most common ones),

at the regular weight (400). Anything else is drawn from a system font when there is one.

Usage:  python tools/build-font-subsets.py

Needs `fonttools` (`pip install fonttools`) and network access for the first run. The source font
is pinned to a commit of github.com/google/fonts and verified by checksum, so the output is the
same on every run. Output (committed): crates/app/assets/fonts/NotoSansJP-subset.ttf and
crates/app/assets/fonts/OFL-NotoSansJP.txt.
"""

import hashlib
import io
import sys
import urllib.request
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = Path(__file__).resolve().parent.parent
COMMIT = "cc39c4c2c304bc368c9b7c311ea2bcc6013299fe"
BASE = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/ofl/notosansjp/"
SOURCES = {
    "NotoSansJP[wght].ttf": "c2f3b4d463500a2ddcd3849cded1fceeb9fd6d1c32e6cbecd568453ba50fc68f",
    "OFL.txt": "1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9",
}
CACHE = ROOT / "target" / "font-sources"
OUT = ROOT / "crates" / "app" / "assets" / "fonts"


def fetch(name: str, expected: str) -> bytes:
    """The source file, from the cache or the network, checked against its checksum."""
    CACHE.mkdir(parents=True, exist_ok=True)
    cached = CACHE / name.replace("[", "_").replace("]", "_")
    if cached.exists():
        data = cached.read_bytes()
    else:
        url = BASE + urllib.request.quote(name)
        print(f"downloading {url}")
        with urllib.request.urlopen(url, timeout=120) as response:
            data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != expected:
        sys.exit(f"{name}: checksum {digest} is not the pinned {expected}")
    cached.write_bytes(data)
    return data


def jis_level_1() -> set[int]:
    """The code points of the JIS X 0208 level 1 kanji (rows 16 to 47)."""
    points = set()
    for row in range(16, 48):
        for cell in range(1, 95):
            # JIS X 0208 row/cell to the EUC-JP bytes.
            try:
                char = bytes([0xA0 + row, 0xA0 + cell]).decode("euc_jp")
            except UnicodeDecodeError:
                continue
            points.add(ord(char))
    return points


def characters() -> set[int]:
    points = set(range(0x20, 0x7F))  # basic Latin
    points |= set(range(0x3000, 0x3040))  # Japanese punctuation
    points |= set(range(0x3041, 0x3097))  # hiragana
    points |= set(range(0x30A1, 0x3100))  # katakana
    points |= set(range(0xFF01, 0xFF5F))  # full-width ASCII
    points |= {0x2212, 0x00D7, 0x2026, 0x2013, 0x2014, 0x00B5}  # signs the interface uses
    points |= jis_level_1()
    for path in sorted((ROOT / "resources" / "i18n" / "ja").glob("*.ftl")):
        points |= {ord(c) for c in path.read_text(encoding="utf-8") if ord(c) >= 0x20}
    points.discard(0x7F)
    return points


def main() -> None:
    font_bytes = fetch("NotoSansJP[wght].ttf", SOURCES["NotoSansJP[wght].ttf"])
    licence = fetch("OFL.txt", SOURCES["OFL.txt"])

    font = TTFont(io.BytesIO(font_bytes))
    regular = instancer.instantiateVariableFont(font, {"wght": 400})

    options = subset.Options()
    options.layout_features = ["kern", "palt", "locl"]
    options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14]
    options.notdef_outline = True
    options.glyph_names = False
    options.hinting = False
    options.desubroutinize = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=sorted(characters()))
    subsetter.subset(regular)

    OUT.mkdir(parents=True, exist_ok=True)
    target = OUT / "NotoSansJP-subset.ttf"
    regular.save(target)
    (OUT / "OFL-NotoSansJP.txt").write_bytes(licence)
    size = target.stat().st_size
    print(f"{target.relative_to(ROOT)}: {size:,} bytes, {len(characters())} characters")


if __name__ == "__main__":
    main()
