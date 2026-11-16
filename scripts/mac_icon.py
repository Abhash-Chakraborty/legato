#!/usr/bin/env python3
"""Writes src-tauri/icons/icon.icns, the Mac app icon, from icons/logo.png.

A Mac icon sits inside its canvas: the artwork takes 824 of 1024 pixels and
the rest stays clear, so Legato is the same size as its neighbours in the
Dock. Windows' icon fills its square, which is why the two differ.

Needs nothing but Python: PNGs are read and written with zlib, and the
resampling is a plain area average on premultiplied colour. Run it again
when the logo changes:

    python3 scripts/mac_icon.py
"""
import struct
import sys
import zlib
from pathlib import Path

ICONS = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
CANVAS = 1024
ARTWORK = 824
# icns entries that hold a PNG, by pixel size. 16 and 32 pt come from their
# @2x entries (ic11, ic12), as their own entries use an older packed format.
ENTRIES = [
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic13", 256),
    (b"ic14", 512),
]


def read_png(path):
    """An 8-bit RGBA, non-interlaced PNG as (width, height, rows of bytes)."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos, idat, header = 8, b"", None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length
    width, height, depth, colour, _, _, interlace = header
    assert (depth, colour, interlace) == (8, 6, 0), "expected 8-bit RGBA"
    raw = zlib.decompress(idat)
    stride, bpp = width * 4, 4
    rows, prev = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        line = bytearray(raw[start + 1 : start + 1 + stride])
        if kind == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 255
        elif kind == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif kind == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 255
        elif kind == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return width, height, rows


def write_png(width, height, rows):
    raw = b"".join(b"\x00" + bytes(row) for row in rows)

    def chunk(kind, body):
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def taps(src, dst):
    """For each output pixel, the source pixels under it and how much of
    each it covers."""
    scale = src / dst
    out = []
    for i in range(dst):
        lo, hi = i * scale, (i + 1) * scale
        first, last = int(lo), min(src - 1, int(hi - 1e-9))
        weights = []
        for s in range(first, last + 1):
            cover = min(hi, s + 1) - max(lo, s)
            if cover > 0:
                weights.append((s, cover / scale))
        out.append(weights)
    return out


def premultiply(rows, width):
    out = []
    for row in rows:
        line = [0.0] * (width * 4)
        for x in range(width):
            a = row[x * 4 + 3] / 255.0
            line[x * 4] = row[x * 4] * a
            line[x * 4 + 1] = row[x * 4 + 1] * a
            line[x * 4 + 2] = row[x * 4 + 2] * a
            line[x * 4 + 3] = row[x * 4 + 3]
        out.append(line)
    return out


def unpremultiply(rows, width):
    out = []
    for row in rows:
        line = bytearray(width * 4)
        for x in range(width):
            alpha = row[x * 4 + 3]
            if alpha > 0:
                k = 255.0 / alpha
                for c in range(3):
                    line[x * 4 + c] = max(0, min(255, round(row[x * 4 + c] * k)))
                line[x * 4 + 3] = max(0, min(255, round(alpha)))
        out.append(line)
    return out


def resize(rows, width, height, new_width, new_height):
    """Area average of premultiplied rows, across and then down."""
    across = taps(width, new_width)
    wide = []
    for row in rows:
        line = [0.0] * (new_width * 4)
        for x, weights in enumerate(across):
            r = g = b = a = 0.0
            for s, w in weights:
                o = s * 4
                r += row[o] * w
                g += row[o + 1] * w
                b += row[o + 2] * w
                a += row[o + 3] * w
            o = x * 4
            line[o], line[o + 1], line[o + 2], line[o + 3] = r, g, b, a
        wide.append(line)
    out = []
    for weights in taps(height, new_height):
        line = [0.0] * (new_width * 4)
        for s, w in weights:
            src = wide[s]
            for i in range(new_width * 4):
                line[i] += src[i] * w
        out.append(line)
    return out


def main():
    width, height, rows = read_png(ICONS / "logo.png")
    art = resize(premultiply(rows, width), width, height, ARTWORK, ARTWORK)
    margin = (CANVAS - ARTWORK) // 2
    clear = [0.0] * (CANVAS * 4)
    canvas = [list(clear) for _ in range(margin)]
    for row in art:
        canvas.append([0.0] * (margin * 4) + row + [0.0] * ((CANVAS - ARTWORK - margin) * 4))
    canvas += [list(clear) for _ in range(CANVAS - ARTWORK - margin)]

    pngs = {}
    for size in sorted({size for _, size in ENTRIES}, reverse=True):
        scaled = canvas if size == CANVAS else resize(canvas, CANVAS, CANVAS, size, size)
        pngs[size] = write_png(size, size, unpremultiply(scaled, size))
        print(f"{size} px: {len(pngs[size])} bytes", file=sys.stderr)

    body = b"".join(
        kind + struct.pack(">I", len(pngs[size]) + 8) + pngs[size] for kind, size in ENTRIES
    )
    out = ICONS / "icon.icns"
    out.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)
    print(f"wrote {out} ({len(body) + 8} bytes)", file=sys.stderr)


if __name__ == "__main__":
    main()
