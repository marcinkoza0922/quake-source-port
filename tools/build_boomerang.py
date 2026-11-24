"""Build the boomerang: mesh (boomerang.mdl.zlib) and skin (texture.rgb.zlib).

python3 tools/build_boomerang.py [assets/boomerang] [--seed 1]
    [--palette gfx/palette.lmp] [--preview atlas.png] [--obj mesh.obj]

No source model: everything is computed here. A bent-wood blade with an airfoil
section follows a centreline that turns sharply at the elbow and bows back a
little along the arms. On top of it sit shells that follow the same surface:
steel caps with a chevron collar and a ground edge, bronze straps and a boss at
the elbow, and a leather wrap on one arm (the handle).

The skin is painted per texel from the model-space position and the surface
coordinates (s along the arm, v across it), so atlas seams cannot show: ply
lines and grain in the wood, a worn red band and burnt chevrons on the arms,
rivets, filed steel edges, stitched leather. Texels are exact non-fullbright
palette colours (nearest palette-ramp entry in OKLab lightness).

Units are Quake units: 28 long in X, thinnest in Z, V opening towards -Y, the
origin at the blade's centre of area so that it spins about its balance point.
"""
import argparse
import json
import os
from pathlib import Path
import re
import struct
import sys
import zlib

import numpy as np

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from clean_toy_texture import ROOT, fill_gaps, palette_from_pak, rasterise, srgb_to_oklab  # noqa: E402
from paint_toy_texture import speckle  # noqa: E402
from toy_wear import fbm, scratches, smoothstep  # noqa: E402

SIZE, LENGTH, ARM = 256, 28.0, 16.4
CAP = 12.3               # |s| where the caps' collars start
WRAP = (-10.6, -4.9)     # leather, along the handle arm
STRAP = 3.1              # |s| of the bronze straps
BOSS = 1.75              # radius of the elbow boss
# palette ramp, lightness (OKLab L), mottle; DARK: ramp -> (lightness below
# which texels move to another ramp, that ramp): some ramps' dark ends are olive
MATERIALS = {
    'wood': (7, 0.355, 0.018), 'paint': (4, 0.20, 0.015), 'steel': (0, 0.30, 0.020),
    'bronze': (1, 0.36, 0.020), 'leather': (10, 0.225, 0.014), 'rust': (6, 0.26, 0.02),
}
DARK = {7: (0.335, 10), 1: (0.26, 10)}
NAMES = list(MATERIALS)

# --- surface ---------------------------------------------------------------
_S = np.linspace(-ARM, ARM, 4001)
_bend = lambda c, w: np.exp(-(((_S - c) / w) ** 2)) / (w * np.sqrt(np.pi))  # noqa: E731
_TH = np.cumsum(-np.radians(86) * _bend(0, 2.9) + np.radians(9) * (_bend(-9.5, 4) + _bend(9.5, 4)))
_TH = _TH * (_S[1] - _S[0])
_TH -= np.interp(0, _S, _TH)
_C = np.cumsum(np.stack([np.cos(_TH), np.sin(_TH)], 1), 0) * (_S[1] - _S[0])
_C -= _C[2000]


def half_widths(s):
    """Distance from the centreline to the outer and inner edge."""
    t = np.abs(s)
    w = 1.95 + 0.65 * np.exp(-((t / 3.4) ** 2)) + 0.42 * smoothstep(8.0, 13.5, t)
    d = np.maximum(ARM - t, 0)

    def tip(r):
        return np.sqrt(np.maximum(1 - (1 - np.minimum(d / r, 1)) ** 2, 0))
    return w * tip(5.2), w * tip(1.4)


def surface(s, v, side, grow=0.0, lift=0.0):
    """Point at s along the arm, v in -1 (outer edge)..1 (inner edge).
    `side` 1 top, -1 underside; shells add `grow` sideways and `lift` in Z."""
    s, v = np.broadcast_arrays(np.asarray(s, float), np.asarray(v, float))
    c = np.stack([np.interp(s, _S, _C[:, k]) for k in (0, 1)], -1)
    th = np.interp(s, _S, _TH)
    outer, inner = half_widths(s)
    off = np.where(v < 0, -v * (outer + grow), -v * (inner + grow))
    t = np.abs(s)
    thick = (1.0 + 0.28 * np.exp(-((t / 4) ** 2))) * (0.35 + 0.65 * smoothstep(0, 4.5, ARM - t + 0.4))
    # airfoil: crown towards the outer (leading) edge, flatter underneath
    u = np.where(v < -0.25, (v + 0.25) / 0.75, (v + 0.25) / 1.25)
    dome = 1 - np.abs(u) ** 2.3
    edge = 0.16
    z = side * (edge + lift + (dome if side > 0 else 0.6 * dome) * (thick - edge))
    return np.stack([c[..., 0] - np.sin(th) * off, c[..., 1] + np.cos(th) * off, z], -1)


# --- mesh ------------------------------------------------------------------
class Mesh:
    """Grid patches: positions, normals, (s, v) and a part id, one atlas
    rectangle each."""

    def __init__(self):
        self.patches = []

    def grid(self, part, pos, out, s, v, cuts=()):
        """`pos` [m, n, 3]; `out` the rough outward direction [m, n, 3].
        `cuts`: rows to split the atlas rectangle at."""
        m, n = pos.shape[:2]
        du, dv = np.gradient(pos, axis=0), np.gradient(pos, axis=1)
        nrm = np.cross(du, dv)
        ln = np.linalg.norm(nrm, axis=2, keepdims=True)
        nrm = np.where(ln > 1e-6, nrm / np.maximum(ln, 1e-9), out)
        nrm *= np.where((nrm * out).sum(2, keepdims=True) < 0, -1, 1)
        s, v = (np.broadcast_to(np.asarray(a, float), (m, n)) for a in (s, v))
        edges = [0, *cuts, m - 1]
        for a, b in zip(edges, edges[1:]):
            self.patches.append((part, pos[a:b + 1], nrm[a:b + 1], s[a:b + 1], v[a:b + 1]))

    def pack(self):
        """Shelf-pack every patch (rows along x); returns the texel scale."""
        dims = []
        for _, p, *_ in self.patches:
            w = np.linalg.norm(np.diff(p, axis=0), axis=2).sum(0).max()
            h = np.linalg.norm(np.diff(p, axis=1), axis=2).sum(1).max()
            dims.append((w, h))
        order = sorted(range(len(dims)), key=lambda i: -dims[i][1])
        for scale in np.arange(16, 4, -0.05):
            x = y = shelf = 2
            spots = {}
            for i in order:
                w, h = (max(2, int(np.ceil(d * scale))) for d in dims[i])
                if x + w + 2 > SIZE:
                    x, y, shelf = 2, y + shelf + 3, 0
                shelf = max(shelf, h)
                if x + w + 2 > SIZE or y + h + 2 > SIZE:
                    break
                spots[i] = (x, y, w, h)
                x += w + 3
            if len(spots) == len(dims):
                return scale, spots
        raise ValueError('atlas too small')

    def build(self):
        scale, spots = self.pack()
        pos, nrm, st, sv, tris, part = [], [], [], [], [], []
        base = 0
        for i, (pt, p, nn, s, v) in enumerate(self.patches):
            m, n = p.shape[:2]
            x, y, w, h = spots[i]
            run = np.linalg.norm(np.diff(p, axis=0), axis=2).mean(1)
            col = np.linalg.norm(np.diff(p, axis=1), axis=2).mean(0)
            fx = np.r_[0, np.cumsum(run)] / run.sum()
            fy = np.r_[0, np.cumsum(col)] / col.sum()
            tx, ty = np.meshgrid(np.round(x + fx * w), np.round(y + fy * h), indexing='ij')
            pos.append(p.reshape(-1, 3))
            nrm.append(nn.reshape(-1, 3))
            st.append(np.stack([tx, ty], 2).reshape(-1, 2))
            sv.append(np.stack([s, v], 2).reshape(-1, 2))
            for a in range(m - 1):
                for b in range(n - 1):
                    q = base + np.array([a * n + b, a * n + b + 1, (a + 1) * n + b + 1, (a + 1) * n + b])
                    for t in ((0, 1, 2), (0, 2, 3)):
                        tris.append(q[list(t)])
                        part.append(pt)
            base += m * n
        pos, nrm, st, sv = (np.concatenate(a) for a in (pos, nrm, st, sv))
        # handle on the +X arm, so the blade reaches left from a right hand
        pos[:, 0] *= -1
        nrm[:, 0] *= -1
        tris, part = np.array(tris), np.array(part)
        # counter-clockwise seen from outside
        tp = pos[tris]
        fn = np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0])
        flip = (fn * nrm[tris].sum(1)).sum(1) < 0
        tris[flip] = tris[flip][:, [0, 2, 1]]
        keep = np.linalg.norm(fn, axis=1) > 1e-5
        return scale, pos, nrm, st, sv, tris[keep], part[keep]


WOOD, CAPS, BAND, LEATHER, DISC = range(5)
V = np.array([-1, -0.86, -0.6, -0.25, 0.15, 0.55, 0.84, 1.0])


def shell(mesh, part, s, grow, lift, skirt, cuts=(), vs=V):
    """Both faces and the rim of a piece over the surface. `s` [m, n] (n =
    len(vs)); `grow` per row, `lift` per row or vertex; `skirt`: rows to close down to the wood."""
    m = len(s)
    v = np.broadcast_to(vs, s.shape)
    g = np.broadcast_to(np.asarray(grow, float), (m,))[:, None]
    li = np.asarray(lift, float)
    li = li if li.ndim == 2 else np.broadcast_to(li, (m,))[:, None]
    up = np.zeros(s.shape + (3,))
    up[..., 2] = 1
    faces = {}
    for side in (1, -1):
        faces[side] = surface(s, v, side, g, li)
        mesh.grid(part, faces[side], side * up, s, v, cuts)
        for row in skirt:
            low = surface(s[row], vs, side, 0.0, -0.16)
            out = surface(s[row] + (-0.3 if row == 0 else 0.3), vs, side) - low
            mesh.grid(part, np.stack([faces[side][row], low]), np.stack([out, out]), s[row], vs * 0.5)
    for col in (0, -1):
        mid = (faces[1][:, col] + faces[-1][:, col]) / 2
        out = mid - (faces[1][:, 3] + faces[-1][:, 3]) / 2
        out /= np.maximum(np.linalg.norm(out, axis=1, keepdims=True), 1e-6)
        pos = np.stack([faces[1][:, col], faces[-1][:, col]], 1)
        mesh.grid(part, pos, np.stack([out, out], 1), s[:, [col, col]], vs[col], cuts)


def build_mesh():
    mesh = Mesh()
    rows = lambda a, b, n: np.broadcast_to(np.linspace(a, b, n)[:, None], (n, len(V)))  # noqa: E731
    # wood, ending just inside the caps' collars
    s = np.r_[np.linspace(-CAP - 1.6, -4.5, 8), np.linspace(-3.6, 3.6, 13), np.linspace(4.5, CAP + 1.6, 8)]
    shell(mesh, WOOD, np.broadcast_to(s[:, None], (len(s), len(V))), 0, 0, (), cuts=(14,))
    # caps: the collar is a chevron, the far end closes to the tip
    for sign in (-1, 1):
        t = np.array([0, 0.22, 0.45, 0.65, 0.8, 0.9, 0.96, 0.99, 1.0])[:, None]
        start = CAP + 1.1 * (1 - np.abs(V + 0.1) / 1.1) ** 1.0
        sr = start + (ARM - 0.001 - start) * t
        taper = np.r_[1, 1, 1, 1, 0.9, 0.8, 0.6, 0.3, 0.0]
        # thin at the rim: a cutting edge
        lift = np.outer(taper, 0.15 - 0.22 * (np.abs(V) > 0.95))
        shell(mesh, CAPS, sign * sr, 0.2 * taper, lift, (0,))
        # bronze straps either side of the elbow boss
        shell(mesh, BAND, rows(sign * STRAP - 0.5, sign * STRAP + 0.5, 3), 0.13, 0.14, (0, -1))
    # leather: a ridge per turn, slanted across the arm
    turns, per = 6, 2
    n = turns * per + 1
    s = rows(*WRAP, n) + 0.28 * V[None]
    ridge = 0.12 + 0.11 * (np.arange(n) % per > 0)
    shell(mesh, LEATHER, s, ridge, ridge, (0, -1))
    # elbow boss, both sides
    ang = np.linspace(0, 2 * np.pi, 15)
    centre = surface(0.0, 0.0, 1)[:2]
    for side in (1, -1):
        base = abs(surface(0.0, -0.25, side)[2])
        prof = [(1.0, -0.45), (1.0, 0.02), (0.92, 0.12), (0.66, 0.14), (0.58, 0.00), (0.46, 0.02), (0.34, 0.20), (0.0, 0.30)]
        r, z = np.array(prof).T
        pos = np.stack([centre[0] + BOSS * np.outer(r, np.cos(ang)), centre[1] + BOSS * np.outer(r, np.sin(ang)),
                        side * (base + z)[:, None] * np.ones_like(ang)], -1)
        out = np.stack([np.outer(r, np.cos(ang)) * 0.4, np.outer(r, np.sin(ang)) * 0.4, np.full((len(r), len(ang)), side)], -1)
        mesh.grid(DISC, pos, out, np.broadcast_to(ang, pos.shape[:2]), np.broadcast_to(r[:, None], pos.shape[:2]))
    return mesh.build()


# --- paint -----------------------------------------------------------------
def every(x, pitch):
    """Signed distance to the nearest multiple of pitch."""
    return (x + pitch / 2) % pitch - pitch / 2


def dots(u, v, r):
    """Rivets: a lit head with a dark rim."""
    d = np.hypot(u, v)
    return 0.16 * (d < r) - 0.14 * ((d >= r) & (d < r + 0.14))


def paint(p, n, part, s, v, seed):
    """Material index and OKLab lightness per texel."""
    t, x, y, z = np.abs(s), *p.T
    mat = np.full(len(p), NAMES.index('wood'))
    L = np.zeros(len(p))
    across = v * 2.0  # roughly units from the centreline
    blotch = fbm(p, 1.3, 3, seed + 5)
    rim = np.abs(n[:, 2]) < 0.4

    # wood: ply contours where the airfoil cuts the layers, grain along the arm
    w = part == WOOD
    ply = every(z + 0.10 * fbm(np.stack([s * 0.25, across, z * 0], 1), 1.0, 2, seed), 0.34)
    grain = fbm(np.stack([s * 0.16, across * 1.9, z * 1.2], 1), 0.55, 3, seed + 1)
    streak = np.abs(fbm(np.stack([s * 0.10, across * 1.2, z], 1), 0.8, 2, seed + 2)) < 0.07
    L[w] += (-0.07 * (np.abs(ply) < 0.05) + 0.05 * grain - 0.07 * streak)[w]
    L[w] -= (0.06 * np.exp(-((t - CAP - 0.6) / 0.6) ** 2))[w]  # grime against the steel
    edge = w & (np.abs(v) > 0.9) & ~rim
    L[edge] += 0.09 * (blotch[edge] > -0.2)
    L[w & rim] += 0.02
    # painted band below each cap, worn through, between burnt lines
    for sign, c in ((1, 10.6), (-1, 11.65)):
        half = 0.95 if sign > 0 else 0.45
        d = s * sign - c
        on = w & (np.abs(d) < half)
        worn = (blotch + 0.5 * grain + 0.7 * (np.abs(v) - 0.6).clip(0)) > 0.42
        mat[on & ~worn] = NAMES.index('paint')
        L[w] += (-0.13 * (np.abs(np.abs(d) - half - 0.1) < 0.09))[w]
    # burnt chevrons on the blade arm, pointing at the tip
    ch = (s - 0.75 * (1 - np.abs(v)) * 1.2)
    on = w & (s > 4.4) & (s < 9.3) & (np.abs(v) < 0.80) & (ch > 4.4) & (ch < 8.1)
    cut = np.abs(every(ch - 4.9, 1.2))
    L[on] += (-0.17 * (cut < 0.18) + 0.06 * ((cut >= 0.18) & (cut < 0.32)))[on]

    # steel: forged flats, a filed bright edge, rivets along the collar
    c = part == CAPS
    mat[c] = NAMES.index('steel')
    collar = t - (CAP + 1.1 * (1 - np.abs(v + 0.1) / 1.1))
    d_edge = np.abs(v)
    bevel = c & (((d_edge > 0.72) & (t < ARM - 1.0)) | (t > ARM - 1.1 + 0.4 * blotch))
    L[c] += (-0.05 * (blotch > 0.15) + 0.03 * fbm(p, 0.5, 2, seed + 7))[c]
    L[c] += (0.08 * (scratches(p, 0.35, 0.12, seed + 8) > 0.5))[c]
    L[c] += (0.10 * (np.abs(collar - 0.12) < 0.12) - 0.10 * (np.abs(collar - 0.36) < 0.10))[c]
    L[bevel] += 0.12 + 0.05 * fbm(p * np.array([2.5, 2.5, 0.5]), 0.5, 2, seed + 13)[bevel]
    line = c & (np.abs(d_edge - 0.70) < 0.035) & (t < ARM - 1.0)
    L[line] -= 0.14
    for vr in (-0.5, -0.02, 0.42):
        rv = CAP + 1.1 * (1 - abs(vr + 0.1) / 1.1) + 0.85
        L[c & ~rim] += dots(t - rv, (v - vr) * 2.2, 0.21)[c & ~rim]
    rust = c & (fbm(p, 0.7, 3, seed + 9) > 0.45) & (collar > 0.4) & (collar < 1.8) & ~bevel
    mat[rust] = NAMES.index('rust')

    # bronze straps with a rivet each side; the boss: rings and a sun of notches
    b = part == BAND
    mat[b] = NAMES.index('bronze')
    d = t - STRAP
    L[b] += (0.10 * (np.abs(np.abs(d) - 0.42) < 0.09) - 0.10 * (np.abs(np.abs(d) - 0.27) < 0.07))[b]
    for vr in (-0.62, 0.0, 0.62):
        L[b & ~rim] += dots(d, (v - vr) * 2.4, 0.17)[b & ~rim]
    b = part == DISC
    mat[b] = NAMES.index('bronze')
    notch = np.abs(every(s, np.pi / 6)) * v * BOSS
    L[b] += (0.10 * (v > 0.9) + 0.02)[b]
    ring = b & (v > 0.60) & (v < 0.9)
    L[ring] += np.where(notch[ring] < 0.13, -0.20, 0.03)
    L[b & (v > 0.44) & (v < 0.60)] -= 0.15
    hub = b & (v <= 0.44)
    mat[hub] = NAMES.index('steel')
    L[hub] += 0.02 + 0.12 * smoothstep(0.3, 0.05, v[hub])
    verd = (part == BAND) | (part == DISC)
    L[verd] += (-0.07 * (blotch > 0.2))[verd]

    # leather: dark between turns, a stitch line, scuffed ridges
    e = part == LEATHER
    mat[e] = NAMES.index('leather')
    run = (s - 0.28 * v - WRAP[0]) / (WRAP[1] - WRAP[0]) * 6
    d = np.abs(every(run, 1.0))  # 0 in a valley, 0.5 on a ridge
    L[e] += (-0.12 * (d < 0.09) + 0.09 * smoothstep(0.2, 0.48, d) + 0.05 * blotch)[e]
    L[e & (np.abs(v - 0.3) < 0.07) & (d > 0.25)] += 0.10

    base, mottle = (np.array([MATERIALS[m][k] for m in NAMES])[mat] for k in (1, 2))
    L += base + 0.035 * n[:, 2] + mottle * fbm(p, 2.4, 3, seed + 11) + 0.6 * mottle * speckle(p, 0.2, 0.2, seed + 3)
    return mat, L


def write_mdl(path, pos, nrm, st, tris, size=SIZE, name=b'boomerang'):
    text = (ROOT / 'crates/qcore/src/anorms.rs').read_text()
    table = np.array([[float(f) for f in m] for m in re.findall(
        r'\[\s*(-?\d+\.\d+),\s*(-?\d+\.\d+),\s*(-?\d+\.\d+)\s*\]', text)])
    lo, hi = pos.min(0), pos.max(0)
    scale = (hi - lo) / 255
    packed = np.round((pos - lo) / scale).astype(np.uint8)
    normal = (nrm @ table.T).argmax(1).astype(np.uint8)
    rows, inv = np.unique(np.column_stack([packed, normal, st.astype(int)]).astype(int), axis=0, return_inverse=True)
    tris = inv.ravel()[tris]
    q = rows[tris][..., :3].astype(float)
    area = np.linalg.norm(np.cross(q[:, 1] - q[:, 0], q[:, 2] - q[:, 0]), axis=1)
    tris = tris[area > 0]
    verts = rows[:, :4].astype(np.uint8)
    data = b'IDPO' + struct.pack('<i3f3ff3f8if', 6, *scale, *lo, np.linalg.norm(pos, axis=1).max(), 0, 0, 0,
                                 1, size, size, len(verts), len(tris), 1, 0, 0, 1.0)
    data += struct.pack('<i', 0) + bytes(size * size)
    data += b''.join(struct.pack('<3i', 0, *map(int, r[4:])) for r in rows)
    # native aliases use clockwise winding
    data += b''.join(struct.pack('<4i', 1, t[0], t[2], t[1]) for t in tris.tolist())
    data += struct.pack('<i', 0) + bytes(4) + bytes((255, 255, 255, 0)) + name.ljust(16, b'\0') + verts.tobytes()
    path.write_bytes(zlib.compress(data, 9))
    return len(verts), len(tris), [lo.round(3).tolist(), hi.round(3).tolist()]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, nargs='?', default=Path('assets/boomerang'))
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--palette', type=Path)
    ap.add_argument('--preview', type=Path, help='write the atlas as PNG (needs Pillow)')
    ap.add_argument('--obj', type=Path, help='write the mesh as OBJ with UVs')
    a = ap.parse_args()
    d = a.dir if a.dir.is_absolute() else ROOT / a.dir

    texels, pos, nrm, st, sv, tris, part = build_mesh()
    tri_id, p, bary = rasterise(pos, st, tris, SIZE, SIZE)
    covered = tri_id >= 0
    k, bc = tri_id[covered], bary[covered]
    w = np.column_stack([1 - bc.sum(1), bc])[..., None]
    interp = lambda a: (a[tris[k]] * w).sum(1)  # noqa: E731
    n, (s, v) = interp(nrm), interp(sv).T
    n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)
    mat, L = paint(p[covered], n, part[k], s, v, a.seed)

    pal = a.palette.read_bytes() if a.palette else palette_from_pak(
        os.environ.get('QUAKE_BASEDIR', ROOT / 'vkQuake directory with maps'))
    ramps = np.frombuffer(pal, np.uint8)[:224 * 3].reshape(14, 16, 3)
    ramp = np.array([MATERIALS[m][0] for m in NAMES])[mat]
    for light, (below, dark) in DARK.items():
        ramp[(ramp == light) & (L < below)] = dark
    pick = np.abs(srgb_to_oklab(ramps.astype(float))[..., 0][ramp] - L[:, None]).argmin(1)
    out = np.zeros((SIZE, SIZE, 3), np.uint8)
    out[covered] = ramps[ramp, pick]
    out = fill_gaps(out, covered)

    # balance point of the blade to the origin, then scale to LENGTH in X
    tp = pos[tris]
    area = np.linalg.norm(np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0]), axis=1)
    pos = pos - [*(tp.mean(1) * area[:, None]).sum(0)[:2] / area.sum(), 0]
    pos *= LENGTH / np.ptp(pos[:, 0])
    d.mkdir(parents=True, exist_ok=True)
    nv, nt, bounds = write_mdl(d / 'boomerang.mdl.zlib', pos, nrm, st, tris)
    (d / 'texture.rgb.zlib').write_bytes(zlib.compress(out.tobytes(), 9))
    info = dict(build=f'python3 tools/build_boomerang.py --seed {a.seed}', triangles=nt, vertices=nv,
                texture_size=SIZE, texels_per_unit=round(float(texels), 2), bounds=bounds, length=LENGTH)
    (d / 'import.json').write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info))
    if a.preview:
        from PIL import Image
        Image.fromarray(out).save(a.preview)
    if a.obj:
        with a.obj.open('w') as f:
            f.writelines('v %f %f %f\n' % tuple(q) for q in pos)
            f.writelines('vt %f %f\n' % ((q[0] + .5) / SIZE, 1 - (q[1] + .5) / SIZE) for q in st)
            f.writelines('vn %f %f %f\n' % tuple(q) for q in nrm)
            f.writelines('f %s\n' % ' '.join(f'{i}/{i}/{i}' for i in t + 1) for t in tris)


if __name__ == '__main__':
    main()
