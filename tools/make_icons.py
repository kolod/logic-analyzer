#!/usr/bin/env python3
"""Draws the application icons (spec 014, user story 6).

The artwork is original to this project (GPL-3.0). `crates/app/assets/icon/app.svg` and
`session.svg` are drawn by hand; this script draws the same shapes with Pillow and writes the
files the application and the installers use:

* `app-16.png`, `app-32.png`, `app-48.png`, `app-64.png`, `app-128.png`, `app-256.png`,
* `app.ico` (16, 32, 48 and 256 pixels, for the Windows executable),
* `app-64.rgba`: 64 x 64 x 4 bytes of raw pixels, the window icon the application embeds,
* `session-<size>.png` and `session.ico` for the icon of a session file.

Usage:  python tools/make_icons.py        (needs `pip install pillow`)

It is not part of the build: the outputs are committed. A change of the shapes here must be
made in the two SVG files too (the coordinates are the same, in a box of 256 units).
"""

import struct
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent.parent / "crates" / "app" / "assets" / "icon"
SCALE = 4  # drawn at four times the size and reduced, for smooth edges

BACKGROUND = (31, 58, 95, 255)  # #1f3a5f
CYAN = (79, 209, 197, 255)  # #4fd1c5
AMBER = (246, 173, 85, 255)  # #f6ad55
PAGE = (244, 246, 250, 255)  # #f4f6fa
FOLD = (205, 214, 228, 255)  # #cdd6e4
CLEAR = (0, 0, 0, 0)

# The two traces of the application icon, in the box of 256 units; the stroke is 14 wide.
APP_TRACES = [
    (CYAN, [(44, 104), (84, 104), (84, 64), (128, 64), (128, 104), (172, 104), (172, 64), (212, 64)]),
    (AMBER, [(44, 192), (68, 192), (68, 152), (112, 152), (112, 192), (156, 192), (156, 152), (188, 152), (188, 192), (212, 192)]),
]
# The same waveform on the page of the session icon.
PAGE_TRACES = [
    (CYAN, [(76, 120), (104, 120), (104, 96), (136, 96), (136, 120), (160, 120), (160, 96), (188, 96)]),
    (AMBER, [(76, 200), (92, 200), (92, 168), (124, 168), (124, 200), (152, 200), (152, 168), (188, 168)]),
]
STROKE = 14


def trace(draw, colour, points, width):
    """A trace of axis-aligned segments with square caps and mitred corners: each segment is a
    rectangle extended by half the width, which is what the SVG stroke gives."""
    half = width / 2
    for (x0, y0), (x1, y1) in zip(points, points[1:]):
        draw.rectangle(
            [min(x0, x1) - half, min(y0, y1) - half, max(x0, x1) + half, max(y0, y1) + half],
            fill=colour,
        )


def app_icon(size):
    big = size * SCALE
    unit = big / 256
    image = Image.new("RGBA", (big, big), CLEAR)
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle([16 * unit, 16 * unit, 240 * unit, 240 * unit], radius=44 * unit, fill=BACKGROUND)
    for colour, points in APP_TRACES:
        trace(draw, colour, [(x * unit, y * unit) for x, y in points], STROKE * unit)
    return image.resize((size, size), Image.LANCZOS)


def session_icon(size):
    big = size * SCALE
    unit = big / 256
    image = Image.new("RGBA", (big, big), CLEAR)
    draw = ImageDraw.Draw(image)

    def scaled(points):
        return [(x * unit, y * unit) for x, y in points]

    page = [(52, 16), (164, 16), (212, 64), (212, 240), (52, 240)]
    draw.polygon(scaled(page), fill=PAGE, outline=BACKGROUND, width=int(8 * unit))
    draw.polygon(scaled([(164, 16), (164, 64), (212, 64)]), fill=FOLD, outline=BACKGROUND, width=int(8 * unit))
    for colour, points in PAGE_TRACES:
        trace(draw, colour, scaled(points), 12 * unit)
    return image.resize((size, size), Image.LANCZOS)


def write_ico(path, images):
    """An ICO with PNG-compressed images (what Windows has read since Vista)."""
    entries = []
    blobs = []
    for image in images:
        from io import BytesIO

        buffer = BytesIO()
        image.save(buffer, format="PNG")
        blobs.append(buffer.getvalue())
    offset = 6 + 16 * len(images)
    header = struct.pack("<HHH", 0, 1, len(images))
    for image, blob in zip(images, blobs):
        width, height = image.size
        entries.append(
            struct.pack(
                "<BBBBHHII",
                0 if width >= 256 else width,
                0 if height >= 256 else height,
                0,
                0,
                1,
                32,
                len(blob),
                offset,
            )
        )
        offset += len(blob)
    path.write_bytes(header + b"".join(entries) + b"".join(blobs))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for size in (16, 32, 48, 64, 128, 256):
        app_icon(size).save(OUT / f"app-{size}.png")
        session_icon(size).save(OUT / f"session-{size}.png")
    write_ico(OUT / "app.ico", [app_icon(s) for s in (16, 32, 48, 256)])
    write_ico(OUT / "session.ico", [session_icon(s) for s in (16, 32, 48, 256)])
    (OUT / "app-64.rgba").write_bytes(app_icon(64).tobytes())
    print(f"icons written to {OUT}")


if __name__ == "__main__":
    main()
