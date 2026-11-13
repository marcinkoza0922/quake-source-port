"""Retint a baked toy texture (texture.rgb.zlib) towards rusty Quake iron/wood.

python3 tools/retint_toy_texture.py assets/stickybomb/texture.rgb.zlib [--seed 1]
    [--preview out.png]

Classifies each texel as background (near black, kept), wood (warm hue) or
iron (everything else, including stray blue fill from Meshy), then darkens
iron with per-texel rust specks and warms/darkens wood with grain noise.
Rewrites the file in place (top-down RGB8, square). Needs only the stdlib.
"""
import argparse
import colorsys
import random
import zlib

ap = argparse.ArgumentParser()
ap.add_argument('path')
ap.add_argument('--seed', type=int, default=1)
ap.add_argument('--preview')
a = ap.parse_args()
rnd = random.Random(a.seed)
data = bytearray(zlib.decompress(open(a.path, 'rb').read()))
n = int((len(data) // 3) ** 0.5)


def clamp(x):
    return max(0, min(255, int(x)))


for i in range(0, len(data), 3):
    r, g, b = data[i], data[i + 1], data[i + 2]
    if r + g + b < 30:
        continue
    h, l, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
    noise = rnd.uniform(-1, 1)
    if 0.0 <= h < 0.12 and s > 0.25 and b < r:
        # wood: warm dark brown with grain
        out = colorsys.hls_to_rgb(0.055, min(0.42, l * 0.85) * (1 + 0.18 * noise), 0.55)
    else:
        # iron: dark grey-brown, some texels rusty
        ll = min(0.3, l * 0.6) * (1 + 0.1 * noise)
        if rnd.random() < 0.06:
            out = colorsys.hls_to_rgb(0.05, ll * 1.1, 0.35)
        else:
            out = colorsys.hls_to_rgb(0.08, ll, 0.12)
    data[i:i + 3] = bytes(clamp(c * 255) for c in out)

open(a.path, 'wb').write(zlib.compress(bytes(data), 9))
if a.preview:
    import struct
    rows = b''.join(b'\0' + bytes(data[y * n * 3:(y + 1) * n * 3]) for y in range(n))

    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', n, n, 8, 2, 0, 0, 0))
    open(a.preview, 'wb').write(png + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
