#!/usr/bin/env python3
"""Generates the three tray icons (tray-active.ico, tray-paused.ico, tray-attention.ico).

Run from the repository root:  python3 crates/ks-platform-win/assets/gen_tray_icons.py
Original artwork drawn by this script (no third-party art, standard library only): a keycap
(rounded square) in a single light colour with a dark outline, so it reads on light and
dark taskbars. Paused: two dark vertical bars on the keycap. Attention (error or hotkey
conflict): a dark exclamation mark. Sizes 16, 20, 24, 32, 48 (small icon at 100-300 %
scaling), 32-bit BGRA DIBs with an AND mask, as `CreateIconFromResourceEx` expects.
The output is deterministic: re-running it gives byte-identical files.
"""
import pathlib
import struct

OUT = pathlib.Path(__file__).resolve().parent
SIZES = (16, 20, 24, 32, 48)
FILL = (0xF2, 0xF2, 0xF2)  # light grey glyph
EDGE = (0x20, 0x20, 0x20)  # dark outline


def coverage(n, inside, samples=4):
    """Anti-aliased coverage grid (rows top-down) of `inside(x, y)` over the unit square."""
    grid = []
    for py in range(n):
        row = []
        for px in range(n):
            hit = 0
            for sy in range(samples):
                for sx in range(samples):
                    x = (px + (sx + 0.5) / samples) / n
                    y = (py + (sy + 0.5) / samples) / n
                    hit += inside(x, y)
            row.append(hit / (samples * samples))
        grid.append(row)
    return grid


def rounded(x0, y0, x1, y1, r):
    def inside(x, y):
        if not (x0 <= x <= x1 and y0 <= y <= y1):
            return False
        cx = min(max(x, x0 + r), x1 - r)
        cy = min(max(y, y0 + r), y1 - r)
        return (x - cx) ** 2 + (y - cy) ** 2 <= r * r

    return inside


def rect(x0, y0, x1, y1):
    return lambda x, y: x0 <= x <= x1 and y0 <= y <= y1


def glyph(kind, n):
    line = max(1.0, n / 16) / n  # outline width: one pixel at 16 px
    outer = rounded(0.03, 0.03, 0.97, 0.97, 0.2)
    inner = rounded(0.03 + line, 0.03 + line, 0.97 - line, 0.97 - line, 0.2 - line)
    if kind == "paused":
        marks = [rect(0.28, 0.25, 0.44, 0.75), rect(0.56, 0.25, 0.72, 0.75)]
    elif kind == "attention":
        marks = [rect(0.42, 0.18, 0.58, 0.60), rect(0.42, 0.68, 0.58, 0.82)]
    else:
        marks = []

    def edge(x, y):
        # The outline and the marks share the dark colour.
        return (outer(x, y) and not inner(x, y)) or any(m(x, y) for m in marks)

    def fill(x, y):
        return inner(x, y) and not any(m(x, y) for m in marks)

    return coverage(n, edge), coverage(n, fill)


def image(kind, n):
    edge, fill = glyph(kind, n)
    pixels = bytearray()
    mask_rows = []
    stride = (n + 31) // 32 * 4
    for py in reversed(range(n)):  # DIBs are stored bottom-up
        mask_bits = 0
        for px in range(n):
            e, f = edge[py][px], fill[py][px]
            a = min(1.0, e + f)
            if a <= 0:
                b = g = r = 0
            else:
                r, g, b = (round((EDGE[i] * e + FILL[i] * f) / (e + f)) for i in range(3))
            pixels += bytes((b, g, r, round(a * 255)))
            mask_bits = (mask_bits << 1) | (1 if a == 0 else 0)
        nbytes = (n + 7) // 8
        mask_bits <<= nbytes * 8 - n
        mask_rows.append(mask_bits.to_bytes(nbytes, "big").ljust(stride, b"\0"))
    header = struct.pack("<IiiHHIIiiII", 40, n, 2 * n, 1, 32, 0, 0, 0, 0, 0, 0)
    return header + bytes(pixels) + b"".join(mask_rows)


def ico(kind):
    images = [image(kind, n) for n in SIZES]
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for n, data in zip(SIZES, images):
        out += struct.pack("<BBBBHHII", n % 256, n % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return out + b"".join(images)


for kind in ("active", "paused", "attention"):
    (OUT / f"tray-{kind}.ico").write_bytes(ico(kind))
    print(f"tray-{kind}.ico")
