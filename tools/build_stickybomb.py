"""Build the stickybomb launcher: mesh (model.mdl.zlib) and skin (texture.rgb.zlib).

python3 tools/build_stickybomb.py [assets/stickybomb] [--seed 1]
    [--palette gfx/palette.lmp] [--preview atlas.png]

No source model: everything is computed here. An open carousel holds six
studded bombs (the colours of the fired bomb, src/addons/sticky_model.rs)
between two star plates; the top one lies in line with a short, wide barrel
that ends in a toothed crown. A skeleton frame (top strap, yoke, receiver)
leaves the bombs in sight from behind. On the receiver's left sits the
detonator box, its sloped panel facing the player: gauge, lamp, toggles, an
aerial and a cable. Walnut grip, fore-end and butt.

The skin is painted per texel from the model-space position, the face normal
and the part, so atlas seams cannot show: ray-cast ambient occlusion, worn
outer edges, rivets, panel lines, hazard paint on the barrel, grain in the
wood. Texels are exact non-fullbright palette colours.

Units are Quake units: X forward (muzzle), Y left, Z up, the barrel axis at
y = z = 0 and the carousel's middle at x = 0.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import zlib

import numpy as np

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from build_boomerang import dots, every, write_mdl  # noqa: E402
from clean_toy_texture import ROOT, fill_gaps, palette_from_pak, rasterise, srgb_to_oklab  # noqa: E402
from paint_toy_texture import seg_dist, sharp_edges, speckle  # noqa: E402
from toy_wear import fbm, scratches, smoothstep  # noqa: E402

SIZE = 512
HUB_Z = -5.0      # carousel axis below the barrel axis
RING = 5.0        # bomb centres from the carousel axis
BOMB = 2.1        # bomb radius
CUT = 1.75        # the plates' cut-outs around each bomb
PLATES = (-2.9, 2.9)
# palette ramp, lightness (OKLab L), mottle, wear on outer edges
MATERIALS = {
    'steel': (0, 0.285, 0.020, 0.12), 'dark': (0, 0.225, 0.016, 0.10), 'iron': (10, 0.275, 0.022, 0.12),
    'bronze': (1, 0.31, 0.022, 0.12), 'wood': (6, 0.27, 0.015, 0.06), 'rubber': (0, 0.16, 0.010, 0.05),
    'red': (4, 0.215, 0.026, 0.0), 'yellow': (12, 0.42, 0.020, 0.0), 'rust': (6, 0.26, 0.02, 0.0),
    'lamp': (4, 0.33, 0.0, 0.0), 'face': (10, 0.66, 0.01, 0.0), 'cream': (10, 0.55, 0.02, 0.0), 'gun': (0, 0.25, 0.018, 0.10),
}
NAMES = list(MATERIALS)
X, Y, Z = np.eye(3)
BOMBS = [(HUB_Z + RING * np.cos(a), RING * np.sin(a)) for a in np.radians(np.arange(6) * 60.0)]  # (z, y)


# --- mesh ------------------------------------------------------------------
def unit(v):
    v = np.asarray(v, float)
    return v / np.maximum(np.linalg.norm(v, axis=-1, keepdims=True), 1e-9)


class Mesh:
    """Patches of (part, positions, normals, uv in units, triangles, density)."""

    def __init__(self):
        self.patches = []
        self.parts = []

    def add(self, part, pos, nrm, uv, tris, density=1.0):
        if part not in self.parts:
            self.parts.append(part)
        tris = np.array(tris)
        tp = pos[tris]
        fn = np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0])
        flip = (fn * nrm[tris].sum(1)).sum(1) < 0
        tris[flip] = tris[flip][:, [0, 2, 1]]
        tris = tris[np.linalg.norm(fn, axis=1) > 1e-5]
        self.patches.append((self.parts.index(part), pos, nrm, uv - uv.min(0), tris, density))

    def grid(self, part, pos, nrm, density=1.0):
        """pos, nrm [m, n, 3]: rows along the profile, columns around."""
        m, n = pos.shape[:2]
        du = np.linalg.norm(np.diff(pos, axis=0), axis=2).mean(1)
        dv = np.linalg.norm(np.diff(pos, axis=1), axis=2).max(0)
        uv = np.stack(np.meshgrid(np.r_[0, np.cumsum(dv)], np.r_[0, np.cumsum(du)]), -1)
        q = (np.arange(m - 1)[:, None] * n + np.arange(n - 1)).ravel()
        tris = np.concatenate([np.stack([q, q + 1, q + n + 1], 1), np.stack([q, q + n + 1, q + n], 1)])
        self.add(part, pos.reshape(-1, 3), nrm.reshape(-1, 3), uv.reshape(-1, 2), tris, density)

    def flat(self, part, pos, normal, density=1.0):
        """A flat convex polygon."""
        pos, normal = np.asarray(pos, float), unit(normal)
        u = unit(pos[1] - pos[0])
        uv = np.stack([pos @ u, pos @ np.cross(normal, u)], 1)
        # a thin face lies along the atlas rows
        if np.ptp(uv[:, 1]) > np.ptp(uv[:, 0]):
            uv = uv[:, ::-1]
        tris = [(0, i, i + 1) for i in range(1, len(pos) - 1)]
        self.add(part, pos, np.broadcast_to(normal, pos.shape), uv, tris, density)

    def lathe(self, part, profile, org=(0, 0, 0), axis=X, seg=16, start=0.0, sweep=360.0, density=1.0, sharp=38):
        """Revolve (along, radius) points round `axis`; outside is to the left
        of the direction of travel. Runs split where the profile turns sharply."""
        prof, axis = np.asarray(profile, float), unit(axis)
        e1 = unit(np.cross(axis, Y if abs(axis @ Y) < 0.9 else Z))
        e2 = np.cross(axis, e1)
        ang = np.radians(np.linspace(start, start + sweep, seg + 1))
        rad = np.outer(np.cos(ang), e1) + np.outer(np.sin(ang), e2)
        d = unit(np.diff(prof, axis=0))
        out = np.stack([-d[:, 1], d[:, 0]], 1)
        cos = np.cos(np.radians(sharp))
        cuts = [0] + [i for i in range(1, len(d)) if d[i] @ d[i - 1] < cos] + [len(d)]
        for a, b in zip(cuts, cuts[1:]):
            p = prof[a:b + 1]
            nn = unit(np.stack([out[max(a, min(i - 1, b - 1))] + out[min(b - 1, max(i, a))] for i in range(a, b + 1)]))
            pos = np.asarray(org) + p[:, 0, None, None] * axis + p[:, 1, None, None] * rad
            nrm = nn[:, 0, None, None] * axis + nn[:, 1, None, None] * rad
            if b == a + 1 and abs(d[a, 0]) < 0.35 and p.min() >= 0 and p[:, 1].min() < 1e-6:
                # a closed end: unwrap it flat
                k = int(p[0, 1] < p[1, 1])
                self.flat(part, pos[k][:-1] if sweep == 360 else pos[k], nrm[k].mean(0), density)
            else:
                self.grid(part, pos, nrm, density)

    def prism(self, part, poly, axis, lo, hi, ch=0.15, density=1.0, caps=(True, True)):
        """Extrude a convex polygon (the two other axes, in x, y, z order)
        from lo to hi along an axis, with chamfered ends."""
        ai = 'xyz'.index(axis)
        oi = [i for i in range(3) if i != ai]
        poly = np.asarray(poly, float)
        c = poly.mean(0)
        e = np.roll(poly, -1, 0) - poly
        en = unit(np.stack([e[:, 1], -e[:, 0]], 1))
        en *= np.sign(((poly - c) * en).sum(1))[:, None]
        inner = []
        for i in range(len(poly)):  # offset the two edges at corner i inwards
            n0, n1 = en[i - 1], en[i]
            inner.append(np.linalg.solve(np.array([n0, n1]), [n0 @ poly[i] - ch, n1 @ poly[i] - ch]))
        inner = np.array(inner)

        def at(p2, h):
            q = np.zeros((len(p2), 3))
            q[:, oi], q[:, ai] = p2, h
            return q
        rings = [at(inner, lo), at(poly, lo + ch), at(poly, hi - ch), at(inner, hi)]
        ax = np.eye(3)[ai]
        for i in range(len(poly)):
            j = (i + 1) % len(poly)
            side = np.zeros(3)
            side[oi] = en[i]
            for k, n in ((0, side - ax), (1, side), (2, side + ax)):
                self.flat(part, [rings[k][i], rings[k][j], rings[k + 1][j], rings[k + 1][i]], n, density)
        for cap, r, n in ((caps[0], rings[0], -ax), (caps[1], rings[3], ax)):
            if cap:
                self.flat(part, r, n, density)

    def hexa(self, part, bottom, top, density=1.0):
        """A convex block from two quads listed the same way round."""
        b, t = np.asarray(bottom, float), np.asarray(top, float)
        mid = (b.mean(0) + t.mean(0)) / 2
        for f in ([*b], [*t], *([b[i], b[(i + 1) % 4], t[(i + 1) % 4], t[i]] for i in range(4))):
            f = np.array(f)
            n = np.cross(f[1] - f[0], f[2] - f[0])
            self.flat(part, f, n * np.sign(n @ (f.mean(0) - mid)), density)

    def tube(self, part, path, r, seg=6, density=1.0):
        path = np.asarray(path, float)
        t = unit(np.gradient(path, axis=0))
        ang = np.linspace(0, 2 * np.pi, seg + 1)
        u = unit(np.cross(t, unit(np.array([0.31, 0.2, 0.93]))))
        w = np.cross(t, u)
        rad = np.cos(ang)[:, None] * u[:, None] + np.sin(ang)[:, None] * w[:, None]
        self.grid(part, path[:, None] + r * rad, rad, density)

    def pack(self):
        """Shelf-pack every patch; the largest texel scale that fits."""
        ext = np.array([uv.max(0) * d for *_, uv, _, d in self.patches])
        turn = ext[:, 1] > ext[:, 0]
        ext[turn] = ext[turn][:, ::-1]
        order = np.lexsort((-ext[:, 0], -np.round(ext[:, 1], 2)))
        for scale in np.arange(14, 4, -0.05):
            x = y = 2
            shelf, spots = 0, {}
            for i in order:
                w, h = (max(1, int(np.ceil(e * scale))) for e in ext[i])
                if x + w + 2 > SIZE:
                    x, y, shelf = 2, y + shelf + 3, 0
                shelf = max(shelf, h)
                if x + w + 2 > SIZE or y + h + 2 > SIZE:
                    break
                spots[i] = (x, y, w, h)
                x += w + 3
            else:
                return scale, spots, turn
        raise ValueError('atlas too small')

    def build(self):
        scale, spots, turn = self.pack()
        pos, nrm, st, tris, part, base = [], [], [], [], [], 0
        for i, (pt, p, n, uv, t, _) in enumerate(self.patches):
            x, y, w, h = spots[i]
            uv = uv[:, ::-1] if turn[i] else uv
            pos.append(p)
            nrm.append(n)
            st.append(np.round([x, y] + uv / np.maximum(uv.max(0), 1e-6) * [w, h]))
            tris.append(t + base)
            part += [pt] * len(t)
            base += len(p)
        return (scale, *(np.concatenate(a) for a in (pos, nrm, st, tris)), np.array(part))


def octagon(hw, z0, z1, c):
    """Cross-section (y, z): half width, bottom, top, corner cut."""
    return [(-hw + c, z0), (hw - c, z0), (hw, z0 + c), (hw, z1 - c), (hw - c, z1), (-hw + c, z1), (-hw, z1 - c), (-hw, z0 + c)]


def star(mesh, part, x, thick, hub):
    """A carousel plate: teeth between the bombs, round cut-outs at them."""
    half = np.arcsin(CUT / RING)
    da = np.r_[np.linspace(-half, half, 6), half, np.radians(30) * 1.15 - half * 0.15]
    rr = RING * np.cos(da[:6]) - np.sqrt(np.maximum(CUT ** 2 - (RING * np.sin(da[:6])) ** 2, 0))
    r = np.r_[rr, RING + 1.15, RING + 1.4]
    ang, rad = [], []
    for k in range(6):
        ang += list(np.radians(60 * k) + np.r_[da[:7], np.radians(30)])
        rad += list(r)
        ang += [np.radians(60 * k + 60) - half]
        rad += [RING + 1.15]
    ang, rad = np.array(ang + [2 * np.pi - half]), np.array(rad + [rr[0]])
    dirs = np.stack([np.zeros_like(ang), np.sin(ang), np.cos(ang)], 1)
    centre = np.array([x, 0, HUB_Z])
    out = np.stack([np.zeros_like(ang), np.sin(ang), np.cos(ang)], 1)
    for side in (-1, 1):
        rim = centre + dirs * rad[:, None] + side * X * thick / 2
        inner = centre + dirs * hub + side * X * thick / 2
        # unwrapped flat, so the face keeps one texel scale
        pos = np.concatenate([inner, rim])
        n = len(ang)
        q = np.arange(n - 1)
        tris = np.concatenate([np.stack([q, q + 1, q + n + 1], 1), np.stack([q, q + n + 1, q + n], 1)])
        mesh.add(part, pos, np.broadcast_to(side * X, pos.shape), pos[:, 1:], tris)
    # the edge, split at its corners so that it shades in flat runs
    for a in range(len(ang) - 1):
        d = centre + dirs[a:a + 2] * rad[a:a + 2, None]
        e = d[1] - d[0]
        if np.linalg.norm(e) < 1e-4:
            continue
        nn = unit(np.cross(X, e))
        if nn @ out[a] < 0:
            nn = -nn
        mesh.flat(part, [d[0] - X * thick / 2, d[0] + X * thick / 2, d[1] + X * thick / 2, d[1] - X * thick / 2], nn, 0.8)


def bomb(mesh, part, z, y, k):
    c = np.array([0.0, y, z])
    outv = unit([0, y, z - HUB_Z])
    lat = np.radians(np.linspace(-90, 90, 7))
    mesh.lathe(part, np.stack([BOMB * np.sin(lat), BOMB * np.cos(lat)], 1), c, outv, 10, sharp=80, density=0.9)
    tan = np.cross(outv, X)
    for d in (outv, unit(outv - 1.5 * X), unit(outv + 1.5 * X), unit(outv + 1.2 * tan), unit(outv - 1.2 * tan)):
        mesh.lathe('stud', [(BOMB - 0.15, 0.48), (BOMB + 0.3, 0.3), (BOMB + 0.32, 0)], c, d, 5, density=0.9)


def build_mesh():
    m = Mesh()
    # barrel: breech ring, tapered tube, two bands, crown; then the bore
    m.lathe('barrel', [(5.6, 2.2), (5.6, 3.35), (7.2, 3.35), (7.5, 2.75), (11.0, 2.75), (11.2, 3.1), (12.4, 3.1), (12.6, 2.75),
                       (16.2, 2.7), (16.5, 3.15), (18.0, 3.45), (18.3, 3.35), (18.3, 2.35), (14.0, 2.2), (14.0, 0)], seg=16)
    for k in range(6):  # crown teeth, bent outwards
        a = np.radians(60 * k + 30 + np.array([-15, 15]))
        ring = lambda x, r: [np.array([x, r * np.sin(t), r * np.cos(t)]) for t in a]  # noqa: E731,B023
        i0, o0, i1, o1 = ring(18.0, 2.45), ring(18.0, 3.4), ring(19.9, 2.9), ring(19.9, 3.45)
        m.hexa('tooth', [i0[0], i0[1], o0[1], o0[0]], [i1[0], i1[1], o1[1], o1[0]])
    # yoke: joins barrel, strap and carousel axle
    m.prism('yoke', [(-2.2, -6.7), (2.2, -6.7), (3.2, -3.5), (3.5, 0.5), (2.4, 3.1), (1.4, 3.5), (-1.4, 3.5), (-2.4, 3.1),
                     (-3.5, 0.5), (-3.2, -3.5)], 'x', 3.9, 5.9, 0.3)
    # top strap with a raised, slotted rib and sights
    m.prism('strap', octagon(1.0, 2.35, 3.35, 0.3), 'x', -5.0, 4.4, 0.2)
    m.prism('rib', octagon(0.62, 3.3, 3.95, 0.16), 'x', -4.4, 5.6, 0.15)
    m.prism('sight', [(-4.5, 3.8), (-3.5, 3.8), (-3.7, 4.95), (-4.4, 4.95)], 'y', 0.28, 0.95, 0.1)
    m.prism('sight', [(-4.5, 3.8), (-3.5, 3.8), (-3.7, 4.95), (-4.4, 4.95)], 'y', -0.95, -0.28, 0.1)
    m.prism('sight', [(16.6, 3.0), (17.7, 3.0), (17.5, 4.5), (17.1, 4.5)], 'y', -0.22, 0.22, 0.08)
    # carousel: plates, hub and axle, rods, bombs
    for x in PLATES:
        star(m, 'plate', x, 0.7, 0.9)
    m.lathe('hub', [(-3.9, 0), (-3.9, 1.0), (-3.6, 1.35), (-3.3, 1.35), (-3.3, 1.1), (4.2, 1.1)], (0, 0, HUB_Z), seg=10)
    for k in range(6):
        a = np.radians(60 * k + 30)
        m.lathe('rod', [(-3.5, 0), (-3.5, 0.42), (3.5, 0.42), (3.5, 0)], (0, (RING + 0.75) * np.sin(a), HUB_Z + (RING + 0.75) * np.cos(a)), seg=6)
    for k, (z, y) in enumerate(BOMBS):
        bomb(m, f'bomb{k}', z, y, k)
    # receiver, breech shield behind the top bomb, trigger housing
    m.prism('receiver', octagon(2.1, -3.6, 2.6, 0.7), 'x', -12.5, -4.4, 0.35)
    m.lathe('shield', [(-4.6, 0), (-4.6, 2.75), (-4.2, 3.0), (-3.7, 3.0), (-3.55, 2.8), (-3.55, 0)], seg=16)
    m.prism('cover', octagon(1.4, 2.4, 3.3, 0.4), 'x', -11.6, -5.4, 0.25)
    m.lathe('hubcap', [(-5.2, 0), (-5.2, 1.5), (-4.2, 1.9), (-3.4, 1.9)], (0, 0, HUB_Z), seg=12)
    m.prism('receiver', octagon(1.4, -6.6, -3.2, 0.4), 'x', -6.2, -4.0, 0.2)
    # charging handle and ejector port on the right
    m.lathe('bolt', [(0, 0.5), (1.5, 0.45), (1.6, 0.8), (2.3, 0.9), (2.6, 0.7), (2.6, 0)], (-9.6, -1.9, 0.6), unit([0, -1, 0.25]), 8)
    m.prism('plateR', [(-10.8, -1.6), (-5.6, -1.6), (-5.6, 1.9), (-10.8, 1.9)], 'y', -2.4, -1.95, 0.15)
    # detonator box on the left: a sloped panel faces the player
    m.prism('det', [(-10.9, 0.2), (-5.4, 0.2), (-5.0, 0.8), (-5.0, 3.9), (-5.5, 4.4), (-8.0, 4.4), (-10.9, 1.4)], 'y', 1.95, 4.85, 0.22)
    pn = unit([-3.0, 0, 2.9])
    pu = np.array([2.9, 0, 3.0]) / np.hypot(2.9, 3.0)
    mid = np.array([-9.45, 3.4, 2.9])
    m.lathe('gauge', [(-0.2, 1.05), (0.28, 1.05), (0.36, 0.88), (0.22, 0.8), (0.2, 0)], mid + 0.42 * pu + 0.28 * Y, pn, 12, density=1.9)
    m.lathe('lampcup', [(-0.2, 0.5), (0.22, 0.5), (0.22, 0.34)], mid + 1.05 * pu - 0.98 * Y, pn, 8)
    m.lathe('lamp', [(0.0, 0.36), (0.3, 0.32), (0.5, 0.2), (0.56, 0)], mid + 1.05 * pu - 0.98 * Y, pn, 8, sharp=80)
    for dy in (-1.0, -0.45):
        m.lathe('toggle', [(-0.1, 0.24), (0.1, 0.24), (0.15, 0.1), (0.75, 0.13), (0.8, 0)],
                mid - 0.5 * pu + dy * Y, unit(pn + 0.5 * pu * (1 if dy < -0.9 else -1)), 6)
    m.lathe('aerial', [(0, 0.5), (0.5, 0.5), (0.7, 0.26), (1.7, 0.24), (1.75, 0.1), (5.6, 0.07), (5.65, 0.22), (6.0, 0.2), (6.05, 0)],
            (-6.2, 4.2, 4.2), unit([0.1, 0.22, 1]), 6)
    t = np.linspace(0, 1, 9)[:, None]
    b = [np.array(q) for q in ((-5.2, 3.5, 2.2), (-2.2, 4.2, 2.8), (-1.5, 3.2, 5.0), (1.2, 0.95, 3.2), )]
    m.tube('cable', (1 - t) ** 3 * b[0] + 3 * (1 - t) ** 2 * t * b[1] + 3 * (1 - t) * t ** 2 * b[2] + t ** 3 * b[3], 0.32)
    m.lathe('ferrule', [(-0.4, 0.55), (0.7, 0.55), (0.8, 0.4), (0.8, 0)], (-5.2, 3.5, 2.2), unit([1, 0.25, 0.2]), 8)
    # fore-end on the axle tube, grip, trigger guard, stock
    m.lathe('tube', [(4.2, 1.1), (14.6, 1.1), (14.8, 0.9), (14.8, 0)], (0, 0, HUB_Z), seg=10)
    m.lathe('fore', [(6.6, 1.1), (6.6, 1.8), (7.0, 2.0), (9.8, 1.8), (12.8, 2.05), (13.3, 1.8), (13.3, 1.1)], (0, 0, HUB_Z), seg=10, sharp=30)
    m.prism('hanger', [(13.4, -5.6), (14.3, -5.6), (14.3, -2.0), (13.4, -2.0)], 'y', -0.8, 0.8, 0.12, 0.7)
    m.prism('grip', [(-9.4, -3.4), (-11.9, -3.4), (-14.2, -9.6), (-11.6, -10.2)], 'y', -1.15, 1.15, 0.4, 0.7)
    guard = [(-6.4, 0, -3.5), (-6.5, 0, -5.6), (-7.6, 0, -6.6), (-9.4, 0, -6.5), (-10.4, 0, -5.4)]
    m.tube('guard', guard, 0.32, 6, 0.6)
    m.prism('stock', octagon(1.5, -2.6, 1.6, 0.5), 'x', -15.5, -12.2, 0.2, 0.8)
    m.prism('butt', [(-15.0, 1.5), (-15.0, -2.6), (-21.5, -6.2), (-23.0, -5.8), (-23.0, 1.3), (-22.0, 1.7)], 'y', -1.5, 1.5, 0.5, 0.6)
    return m


# --- paint -----------------------------------------------------------------
def occlusion(p, n, rays=64, reach=5.0, seed=0):
    """Share of the hemisphere blocked by other surface within `reach`."""
    vox = 0.3
    lo = p.min(0) - 1
    shape = np.ceil((p.max(0) - lo) / vox).astype(int) + 4
    occ = np.zeros(shape, bool)
    occ[tuple(((p - lo) / vox).astype(int).T)] = True
    rng = np.random.default_rng(seed)
    dirs = unit(rng.normal(size=(rays, 3)))
    hit = np.zeros(len(p))
    total = np.zeros(len(p))
    for d in dirs:
        c = n @ d
        up = np.flatnonzero(c > 0.15)
        open_ = np.ones(len(up), bool)
        for s in np.arange(0.5, reach, vox * 0.9):
            q = ((p[up] + n[up] * 0.65 + d * s - lo) / vox).astype(int)
            ok = ((q >= 0) & (q < shape)).all(1)
            blocked = np.zeros(len(up), bool)
            blocked[ok] = occ[tuple(q[ok].T)]
            first = open_ & blocked
            hit[up[first]] += c[up[first]] * (1 - s / reach)
            open_ &= ~blocked
        total[up] += c[up]
    return hit / np.maximum(total, 1e-9)


def line(d, width=0.09, light=0.14):
    """A panel line: dark within `width`, a lit lip beside it."""
    return -0.12 * (np.abs(d) < width) + 0.07 * ((d >= width) & (d < width + light))


def paint(p, n, part, names, seed):
    """Material names, lightness offsets and the metal's chippable mask."""
    x, y, z = p.T
    L = np.zeros(len(p))
    mat = np.full(len(p), 'steel', dtype=object)
    is_ = {nm: part == i for i, nm in enumerate(names)}
    blotch = fbm(p, 1.6, 3, seed + 5)
    fine = fbm(p, 0.6, 2, seed + 6)
    side, top, end = np.abs(n[:, 1]) > 0.6, n[:, 2] > 0.5, np.abs(n[:, 0]) > 0.7
    base = {'yoke': 'iron', 'strap': 'dark', 'rib': 'steel', 'sight': 'dark', 'plate': 'bronze', 'hub': 'steel',
            'rod': 'steel', 'receiver': 'gun', 'shield': 'steel', 'cover': 'dark', 'hubcap': 'bronze', 'bolt': 'steel',
            'plateR': 'dark', 'det': 'dark', 'gauge': 'bronze', 'lampcup': 'steel', 'lamp': 'lamp', 'toggle': 'steel',
            'aerial': 'steel', 'cable': 'rubber', 'ferrule': 'bronze', 'tube': 'steel', 'fore': 'wood', 'hanger': 'iron',
            'grip': 'wood', 'guard': 'dark', 'stock': 'gun', 'butt': 'wood', 'barrel': 'steel', 'tooth': 'dark', 'stud': 'steel'}
    for nm, s in is_.items():
        mat[s] = base.get(nm, 'red')
    turn = np.arctan2(y, z)
    r = np.hypot(y, z)

    # barrel: bolts on the breech ring, hazard paint between the bands
    b = is_['barrel']
    out = b & (r > 2.5) & ~end
    L[b] += 0.02
    breech = out & (x > 5.5) & (x < 7.3)
    mat[breech] = 'iron'
    L[breech] += dots(x[breech] - 6.4, every(turn[breech], np.pi / 6) * 3.35, 0.22)
    for c, half, wide in ((11.8, 0.62, 0.3), (17.2, 0.9, 0.3)):
        s = out & (np.abs(x - c) < half + 0.05)
        mat[s] = 'bronze' if c < 12 else 'dark'
        L[s] += dots(x[s] - c, every(turn[s], np.pi / 6) * 3.1, 0.16) if c < 12 else 0
    hz = out & (x > 12.95) & (x < 15.8)
    stripe = ((turn * 2.75 * 1.0 + x * 0.9) % (2 * np.pi * 2.75 / 8)) < (2 * np.pi * 2.75 / 16)
    worn = blotch + 0.45 * fine > 0.30
    mat[hz & stripe & ~worn] = 'yellow'
    mat[hz & ~stripe & ~worn] = 'rubber'
    L[b] += line(x[b] - 12.9, 0.07) + line(x[b] - 15.85, 0.07) + line(x[b] - 9.2)
    lines = out & (x > 7.6) & (x < 10.9)  # cooling flutes
    L[lines] += np.where(np.abs(every(turn[lines], np.pi / 8)) * 2.75 < 0.16, -0.10, 0) * (np.abs(x[lines] - 9.2) > 0.35)
    heat = smoothstep(15.5, 18.3, x) * (b | is_['tooth'])
    L -= 0.05 * heat
    bore = b & (r < 2.5)
    mat[bore] = 'rubber'
    L[bore] -= 0.08 * smoothstep(18.2, 15.5, x[bore])
    lip = is_['tooth'] & (x > 19.5)
    L[lip] += 0.10

    # yoke: bolts round the barrel, a forge line
    s = is_['yoke'] & ~end
    L[s] += line(x[s] - 4.9)
    s = is_['yoke'] & end
    for by, bz in ((2.6, -2.2), (-2.6, -2.2), (2.7, 1.6), (-2.7, 1.6), (1.6, -5.9), (-1.6, -5.9)):
        L[s] += dots(y[s] - by, z[s] - bz, 0.24)

    # strap: screws at its edges; rib: cross slots
    s = is_['strap'] & top
    L[s] += dots(every(x[s] + 0.4, 1.9), np.abs(y[s]) - 0.72, 0.15)
    s = is_['strap'] & side
    L[s] += line(z[s] - 2.75, 0.06)
    s = is_['rib'] & top
    L[s] += np.where(np.abs(every(x[s], 0.7)) < 0.17, -0.14, 0.03)
    s = is_['sight'] & (x < -3.0) & (n[:, 0] < -0.5) & (np.hypot(np.abs(y) - 0.62, z - 4.5) < 0.17)
    mat[s] = 'cream'

    # carousel plates: worn bronze, a ring line, numbers of teeth marked, rivets
    s = is_['plate'] & end
    ca, cr = np.arctan2(y, z - HUB_Z), np.hypot(y, z - HUB_Z)
    L[s] += line(cr[s] - 1.9, 0.07)
    L[s] += dots(every(ca[s] + np.pi / 6, np.pi / 3) * 4.0, cr[s] - 4.0, 0.22)
    edge = s & (np.abs(np.hypot(every(ca, np.pi / 3) * cr, cr * np.cos(every(ca, np.pi / 3)) - RING) - CUT - 0.14) < 0.1)
    L[edge] += 0.07
    s = is_['plate'] & (blotch > 0.28)
    L[s] -= 0.06
    s = is_['hubcap']
    L[s & end] += line(cr[s & end] - 0.95, 0.08)
    L[s] += dots(every(ca[s], np.pi / 3) * 1.2, cr[s] - 1.2, 0.2) * end[s]

    # bombs: red halves, a dark steel belt with lamps
    for k, (bz, by) in enumerate(BOMBS):
        s = is_[f'bomb{k}']
        d = unit(p[s] - [0, by, bz])
        outv = unit([0, by, bz - HUB_Z])
        lat = d @ outv
        lon = np.arctan2(d @ np.cross(outv, X), d @ X)
        q = fbm(p[s] + 7.0 * k, 0.9, 3, seed + 20)
        mm, dl = np.full(s.sum(), 'red', dtype=object), 0.05 * q
        belt = np.abs(lat) < 0.17
        mm[belt] = 'dark'
        dl = dl + np.where(belt, 0.03, 0) - 0.10 * (np.abs(np.abs(lat) - 0.20) < 0.035)
        lamp = belt & (np.abs(every(lon, np.pi / 4)) < 0.16) & (np.abs(lat) < 0.10)
        mm[lamp] = 'lamp'
        dl[lamp] = 0.02
        pole = (np.abs(lat) > 0.55) & (np.abs(np.abs(lat) - 0.58) < 0.03)
        dl = dl - 0.07 * pole
        chip = (q > 0.42) & ~belt
        mm[chip] = 'rust'
        dl = dl + 0.07 * lat - 0.08 * (np.abs(every(lon, np.pi / 2)) < 0.06) * (np.abs(lat) > 0.22) * (np.abs(lat) < 0.55)
        mat[s], L[s] = mm, dl
    s = is_['stud']
    L[s] += 0.05 * fine[s] + 0.02

    # receiver: panel lines, rivets, a worn red panel on the right plate
    s = is_['receiver'] & ~end & (x < -4)
    L[s] += line(x[s] + 5.4) + line(x[s] + 11.4)
    s = is_['receiver'] & side
    L[s] += dots(every(x[s] + 0.2, 1.5), z[s] + 2.4, 0.18)
    s = is_['receiver'] & (n[:, 0] < -0.7) & (x < -12)
    for by in (-1.35, 1.35):
        for bz in (-2.5, 1.7):
            L[s] += dots(y[s] - by, z[s] - bz, 0.25)
    s = is_['plateR'] & side
    mat[s & ((np.abs(x + 8.2) < 2.1) & (np.abs(z - 0.15) < 1.25)) & ~(blotch + 0.5 * fine > 0.25)] = 'red'
    for bx in (-10.4, -6.0):
        for bz in (-1.25, 1.55):
            L[s] += dots(x[s] - bx, z[s] - bz, 0.2)
    s = is_['shield']
    L[s & end] += line(r[s & end] - 2.2, 0.08)
    # cover: louvres and a worn warning bar across
    s = is_['cover'] & top & (np.abs(y) < 1.2)
    lou = s & (x < -7.2)
    L[lou] += line(every(x[lou], 0.85), 0.14, 0.16) * (np.abs(y[lou]) < 0.95)
    bar = s & (np.abs(x + 6.3) < 0.55) & ~(blotch + 0.5 * fine > 0.35)
    mat[bar] = np.where((x[bar] * 1.0 + y[bar]) % 0.9 < 0.45, 'yellow', 'rubber')

    # detonator: seams, corner screws, a painted panel; gauge and lamp
    d = is_['det']
    pn = unit([-3.0, 0, 2.9])
    panel = d & (n @ pn > 0.9)
    u = (p - [-9.45, 3.4, 2.9]) @ unit([2.9, 0, 3.0])
    v = y - 3.4
    inset = panel & (np.abs(u) < 1.75) & (np.abs(v) < 1.15)
    mat[panel] = 'steel'
    mat[inset] = 'dark'
    L[panel & (np.abs(np.maximum(np.abs(u) - 1.75, np.abs(v) - 1.15)) < 0.09)] -= 0.12
    L[panel] += dots(np.abs(u[panel]) - 1.85, np.abs(v[panel]) - 1.2, 0.16)
    mark = inset & (np.abs(u + 1.25) < 0.14) & (np.abs(v) < 0.9) & ((v * 5).astype(int) % 3 != 0)
    mat[mark] = 'cream'
    s = d & side & (y > 4.2)
    L[s] += dots(every(x[s] + 5.7, 4.4), z[s] - 0.85, 0.2)
    s = d & top & ~panel
    L[s] += line(x[s] + 6.8)
    tri = d & (y > 4.75) & (x > -8.3) & (x < -5.8) & (z > 1.4) & (z < 3.6)
    tri &= (z - 1.4) < 2.2 * (1 - np.abs(x + 7.05) / 1.25)
    inner = tri & (z > 1.75) & ((z - 1.4) < 2.2 * (1 - np.abs(x + 7.05) / 1.25) - 0.55)
    mat[tri & ~(blotch > 0.4)] = 'yellow'
    mat[inner] = 'rubber'
    g = is_['gauge']
    gp = p - ([-9.45, 3.4, 2.9] + 0.42 * unit([2.9, 0, 3.0]) + 0.28 * Y)
    gu, gv = gp @ unit([2.9, 0, 3.0]), -gp[:, 1]
    gr, ga = np.hypot(gu, gv), np.degrees(np.arctan2(gu, gv))
    f = g & (gr < 0.8) & (n @ pn > 0.8) & (gp @ pn < 0.3)
    mat[f] = 'face'
    tick = f & (gr > 0.5) & (gr < 0.74) & (np.abs(every(ga, 45)) < 7) & (ga > -20)
    red = f & (gr > 0.5) & (ga > -5) & (ga < 42)
    mat[red] = 'lamp'
    mat[tick] = 'rubber'
    na = np.radians(125)
    along, off = gu * np.sin(na) + gv * np.cos(na), gu * np.cos(na) - gv * np.sin(na)
    mat[f & (along > -0.15) & (along < 0.62) & (np.abs(off) < 0.085)] = 'rubber'
    L[is_['lamp']] += 0.06 * n[is_['lamp']] @ unit([-0.4, 0, 1]) + 0.02
    s = is_['aerial'] & (z > 4.85) & (z < 5.95)
    L[s] += np.where(z[s] % 0.3 < 0.15, 0.07, -0.07)
    tip = is_['aerial'] & (z > 9.5)
    mat[tip] = 'red'
    s = is_['cable']
    L[s] += 0.05 * np.where((x[s] + 0.7 * y[s]) % 0.5 < 0.25, 1, -1)

    # wood: grain along each piece, brass butt plate and screws
    for nm, along in (('fore', [0.14, 1.3, 1.3]), ('grip', [1.3, 1.3, 0.25]), ('butt', [0.14, 1.3, 1.3])):
        s = is_[nm]
        q = p[s] * along
        L[s] += 0.06 * fbm(q, 0.55, 3, 41) - 0.05 * (np.abs(fbm(q, 1.0, 2, 47)) < 0.06)
    s = is_['fore'] & (np.abs(every(x, 0.75)) < 0.1) & (np.abs(x - 10) < 2.4)
    L[s] -= 0.08
    s = is_['grip'] & side
    L[s] += 0.04 * np.where(((x[s] + z[s]) % 0.9 < 0.3) | ((x[s] - z[s]) % 0.9 < 0.3), -1.0, 0.3)
    s = is_['butt'] & (x < -22.4)
    mat[s] = 'rubber'
    s = is_['butt'] & (np.abs(x + 22.4) < 0.15)
    L[s] -= 0.1

    metal = np.isin(mat.astype(str), ('steel', 'gun', 'dark', 'iron', 'bronze'))
    L[metal] += 0.06 * (scratches(p, 0.35, 0.10, seed + 8) > 0.5)[metal]
    rust = metal & (fbm(p, 0.9, 3, seed + 9) > 0.47) & np.isin(mat.astype(str), ('iron', 'steel'))
    mat[rust] = 'rust'
    return mat, L


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, nargs='?', default=Path('assets/stickybomb'))
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--palette', type=Path)
    ap.add_argument('--preview', type=Path, help='write the atlas as PNG (needs Pillow)')
    a = ap.parse_args()
    d = a.dir if a.dir.is_absolute() else ROOT / a.dir

    mesh = build_mesh()
    texels, pos, nrm, st, tris, tri_part = mesh.build()
    tri_id, xyz, bary = rasterise(pos, st, tris, SIZE, SIZE)
    covered = tri_id >= 0
    k, bc, p = tri_id[covered], bary[covered], xyz[covered]
    w = np.column_stack([1 - bc.sum(1), bc])[..., None]
    n = unit((nrm[tris[k]] * w).sum(1))
    mat, L = paint(p, n, tri_part[k], mesh.parts, a.seed)

    # worn outer edges, dark inner ones; occlusion
    tp = pos[tris]
    fn = unit(np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0]))
    edges = sharp_edges(pos, tris, fn, 32)
    bevel, seam = np.zeros(len(p)), np.zeros(len(p))
    for t in np.unique(k):
        ea, eb, convex = edges[t]
        if len(ea):
            sel = np.flatnonzero(k == t)
            dist = seg_dist(p[sel], pos[ea], pos[eb])
            for m, dst in ((convex, bevel), (~convex, seam)):
                if m.any():
                    dst[sel] = dist[:, m].min(1) < 0.14
    bevel *= 0.35 + 0.65 * (fbm(p, 1.1, 2, a.seed + 29) > -0.15)
    ao = occlusion(p, n, seed=a.seed)
    grime = smoothstep(0.15, 0.5, fbm(p * np.array([1.4, 1.4, 0.4]), 1.8, 3, a.seed + 19))
    ramp, base, mottle, wear = (np.array([MATERIALS[m][i] for m in mat]) for i in range(4))
    L += (base + 0.045 * n[:, 2] + wear * bevel * (1 - ao) - 0.07 * seam - 0.20 * ao - 0.025 * grime
          + mottle * fbm(p, 2.2, 3, a.seed + 11) + mottle * speckle(p, 0.22, 0.2, a.seed + 3))

    pal = a.palette.read_bytes() if a.palette else palette_from_pak(
        os.environ.get('QUAKE_BASEDIR', ROOT / 'vkQuake directory with maps'))
    ramps = np.frombuffer(pal, np.uint8)[:224 * 3].reshape(14, 16, 3)
    ramp[(ramp == 1) & (L < 0.24)] = 10  # the ramp's dark end is olive
    pick = np.abs(srgb_to_oklab(ramps.astype(float))[..., 0][ramp] - L[:, None]).argmin(1)
    out = np.zeros((SIZE, SIZE, 3), np.uint8)
    out[covered] = ramps[ramp, pick]
    out = fill_gaps(out, covered)

    d.mkdir(parents=True, exist_ok=True)
    nv, nt, bounds = write_mdl(d / 'model.mdl.zlib', pos, nrm, st, tris, SIZE, b'stickybomb')
    (d / 'texture.rgb.zlib').write_bytes(zlib.compress(out.tobytes(), 9))
    info = dict(build=f'python3 tools/build_stickybomb.py --seed {a.seed}', triangles=nt, vertices=nv,
                texture_size=SIZE, texels_per_unit=round(float(texels), 2), bounds=bounds)
    (d / 'import.json').write_text(json.dumps(info, indent=2) + '\n')
    print(json.dumps(info), f'{covered.mean():.0%} of the atlas')
    if a.preview:
        from PIL import Image
        Image.fromarray(out).save(a.preview)


if __name__ == '__main__':
    main()
