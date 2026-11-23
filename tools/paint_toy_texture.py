"""Repaint a toy's metal from scratch as Quake-style pixel-art (texture.rgb.zlib).

python3 tools/paint_toy_texture.py assets/portal-gun [--mdl NAME] [--ramp 10]
    [--base 0.5] [--light 0.08] [--bevel 0.10] [--seam 0.10] [--line 0.3] [--ao 0.05] [--parts 0.03]
    [--mottle 0.035] [--speckle 0.03] [--keep-mask] [--glow-ribs 0.25]
    [--palette gfx/palette.lmp] [--preview out.png] [--dry-run]

tools/clean_toy_texture.py keeps the colours Meshy baked and only tidies
them; when those colours are the problem (blotchy camo, rust everywhere,
parts that don't read), this tool ignores them and paints the surface from
the model's shape alone, the way a Quake skin is drawn:

- one palette ramp (--ramp; id1 rows: 0 grey, 1 khaki, 2 blue-grey,
  6 copper, 10 warm grey-brown) at lightness --base (OKLab L), with a
  fraction --alt of texels, in small clumps, on a second ramp (--alt-ramp)
  so the colour is a dithered mix, like stone,
- a baked key light from above (--light, by the face normal's Z), since
  alias lighting is nearly flat,
- a light line along every outer edge (--bevel) and a dark one in every
  inner edge (--seam), --line units wide, where the faces turn by more than
  --sharp degrees, so plates and chamfers read like hand-drawn pixel art,
- darker creases and cavities (--ao), from the 3D convexity of the surface
  (tools/toy_wear.py),
- each connected part of the mesh a little lighter or darker (--parts), so
  plates and rings read as separate pieces,
- faint mottling (--mottle, --mottle-scale units across) and lone lighter
  or darker texels (--speckle on --speckle-frac of them), both from 3D noise, so nothing depends on the atlas
  layout and seams cannot show.

Output texels are exact palette colours (nearest ramp entry in lightness).
--keep-mask leaves texels whose MDL skin byte is non-zero alone (the portal
gun's owner-colour regions); --glow-ribs then rewrites the brightness of its
energy texels (mask 1-3, whose maximum channel is the owner-colour gain in
src/addons/portal_skin.rs) as bright bands with darker ribs between them, like
glowing tubes behind a grille. Empty atlas space takes the nearest island's
colour.
"""
import argparse
import os
from pathlib import Path
import sys
import zlib

import numpy as np
from scipy import sparse

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from clean_toy_texture import (ROOT, fill_gaps, load_mdl, palette_from_pak,  # noqa: E402
                               rasterise, srgb_to_oklab)
from toy_wear import _hash, convexity, fbm, smoothstep  # noqa: E402


def parts(pos, tris):
    """Connected-component id of each triangle (vertices welded by position)."""
    _, inv = np.unique(np.round(pos, 3), axis=0, return_inverse=True)
    t = inv.ravel()[tris]
    e = np.concatenate([t[:, [0, 1]], t[:, [1, 2]]])
    n = t.max() + 1
    g = sparse.csr_matrix((np.ones(len(e)), (e[:, 0], e[:, 1])), shape=(n, n))
    _, comp = sparse.csgraph.connected_components(g, directed=False)
    return comp[t[:, 0]]


def sharp_edges(pos, tris, fn, angle):
    """For each triangle, the sharp edges of the flat face it lies in:
    (a, b, convex) arrays. An edge is sharp when its two faces turn by more
    than `angle` degrees (open edges count as convex). Edges of the face's
    other triangles that touch this one are included too, so a fan of
    triangles gets an unbroken line along its outline."""
    _, inv = np.unique(np.round(pos, 3), axis=0, return_inverse=True)
    t = inv.ravel()[tris]
    cos = np.cos(np.radians(angle))
    edge_tris = {}
    for k, tri in enumerate(t):
        for c in range(3):
            edge_tris.setdefault(tuple(sorted((tri[c], tri[(c + 1) % 3]))), []).append((k, c))
    own = [[] for _ in tris]  # (a, b, convex) per triangle, original vertex ids
    for users in edge_tris.values():
        for k, c in users:
            others = [j for j, _ in users if j != k]
            if not others:
                convex = True
            else:
                j = others[0]
                if fn[k] @ fn[j] > cos:
                    continue
                opp = pos[tris[j]].mean(0) - pos[tris[k][c]]
                convex = opp @ fn[k] < 0
            own[k].append((tris[k][c], tris[k][(c + 1) % 3], convex))
    by_vert = {}
    for k, tri in enumerate(t):
        for v in tri:
            by_vert.setdefault(v, set()).add(k)
    out = []
    for k, tri in enumerate(t):
        near = set().union(*(by_vert[v] for v in tri))
        e = [x for j in near if fn[k] @ fn[j] > cos for x in own[j]]
        out.append((np.array([x[0] for x in e], int), np.array([x[1] for x in e], int),
                    np.array([x[2] for x in e], bool)))
    return out


def seg_dist(p, a, b):
    """Distance from points p[N,3] to segments a[M,3]-b[M,3]: [N, M]."""
    ab = b - a
    t = ((p[:, None] - a[None]) * ab[None]).sum(2) / np.maximum((ab * ab).sum(1), 1e-12)[None]
    q = a[None] + np.clip(t, 0, 1)[..., None] * ab[None]
    return np.linalg.norm(p[:, None] - q, axis=2)


def cells(p, size, seed):
    """Blocky noise in [-1, 1]: one value per cube of `size` units."""
    c = np.floor(p / size).astype(np.int64)
    return _hash(c[:, 0], c[:, 1], c[:, 2], seed)


def speckle(p, size, frac, seed):
    """-1 or +1 on a `frac` of `size`-unit cubes, 0 elsewhere: lone lighter
    and darker texels on a flat colour, like grit in stone."""
    v = cells(p + 0.37, size, seed)
    return np.where(np.abs(v) > 1 - frac, np.sign(v), 0.0)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, help='asset directory with texture.rgb.zlib')
    ap.add_argument('--mdl', default=None, help='MDL file in DIR (default: the only *.mdl.zlib)')
    ap.add_argument('--ramp', type=int, default=10, help='palette ramp (row of 16) for the metal')
    ap.add_argument('--alt-ramp', type=int, default=0, help='second ramp mixed in as speckle')
    ap.add_argument('--alt', type=float, default=0.0, help='fraction of texels on --alt-ramp')
    ap.add_argument('--base', type=float, default=0.5, help='mean lightness (OKLab L)')
    ap.add_argument('--light', type=float, default=0.08, help='baked top light (L per unit normal Z)')
    ap.add_argument('--bevel', type=float, default=0.10, help='light line on outer edges (L)')
    ap.add_argument('--seam', type=float, default=0.10, help='dark line in inner edges (L)')
    ap.add_argument('--line', type=float, default=0.3, help='bevel/seam line width, units')
    ap.add_argument('--sharp', type=float, default=35, help='faces turning more are edges, degrees')
    ap.add_argument('--ao', type=float, default=0.05, help='darkening in creases and cavities (L)')
    ap.add_argument('--ao-radius', type=float, default=1.0, help='cavity radius, units')
    ap.add_argument('--parts', type=float, default=0.03, help='per-part lightness spread (L)')
    ap.add_argument('--mottle', type=float, default=0.035, help='mottling (L)')
    ap.add_argument('--mottle-scale', type=float, default=2.0, help='mottle size, units')
    ap.add_argument('--speckle', type=float, default=0.05, help='speckle lightness step (L)')
    ap.add_argument('--speckle-frac', type=float, default=0.25, help='fraction of texels speckled')
    ap.add_argument('--speckle-size', type=float, default=0.25, help='speckle size, units')
    ap.add_argument('--keep-mask', action='store_true')
    ap.add_argument('--glow-ribs', type=float, default=0.0,
                    help='energy texel rib depth, 0..1 of the gain (needs --keep-mask)')
    ap.add_argument('--rib-pitch', type=float, default=1.2, help='rib spacing along X, units')
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--palette', type=Path)
    ap.add_argument('--preview', type=Path, help='write before|after PNG (needs Pillow)')
    ap.add_argument('--dry-run', action='store_true', help='do not rewrite the texture')
    a = ap.parse_args()

    d = a.dir if a.dir.is_absolute() else ROOT / a.dir
    mdl_path = d / a.mdl if a.mdl else next(iter(sorted(d.glob('*.mdl.zlib'))))
    pos, st, tris, mask = load_mdl(zlib.decompress(mdl_path.read_bytes()))
    h, w = mask.shape
    tex_path = d / 'texture.rgb.zlib'
    rgb = np.frombuffer(zlib.decompress(tex_path.read_bytes()), np.uint8).reshape(h, w, 3)
    if a.palette:
        pal_bytes = a.palette.read_bytes()
    else:
        pal_bytes = palette_from_pak(os.environ.get('QUAKE_BASEDIR', ROOT / 'vkQuake directory with maps'))
    rows = np.frombuffer(pal_bytes, np.uint8)[:224 * 3].reshape(14, 16, 3)[[a.ramp, a.alt_ramp]]
    rows_l = srgb_to_oklab(rows.astype(float))[..., 0]
    order = np.argsort(rows_l, axis=1)
    rows = np.take_along_axis(rows, order[..., None], 1)
    rows_l = np.take_along_axis(rows_l, order, 1)

    tri_id, xyz, _ = rasterise(pos, st, tris, w, h)
    covered = tri_id >= 0
    tp = pos[tris]
    fn = np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0])
    fn /= np.maximum(np.linalg.norm(fn, axis=1, keepdims=True), 1e-9)
    if (fn * (tp.mean(1) - pos.mean(0))).sum() < 0:  # make face normals point out
        fn = -fn
    part = parts(pos, tris)

    cy, cx = np.nonzero(covered)
    k = tri_id[cy, cx]
    keep = (mask[cy, cx] != 0) if a.keep_mask else np.zeros(len(cy), bool)
    work = ~keep
    p, n = xyz[cy, cx][work], fn[k[work]]

    conv = convexity(p, n, a.ao_radius)
    cavity = smoothstep(0.3, 1.5, -conv / (conv.std() + 1e-9))
    # exact edge lines: distance from each texel to its face's sharp edges
    edges = sharp_edges(pos, tris, fn, a.sharp)
    bevel = np.zeros(len(p))
    seam = np.zeros(len(p))
    kw = k[work]
    for t in np.unique(kw):
        ea, eb, convex = edges[t]
        if len(ea) == 0:
            continue
        sel = np.flatnonzero(kw == t)
        dist = seg_dist(p[sel], pos[ea], pos[eb])
        for mask_, out_ in ((convex, bevel), (~convex, seam)):
            if mask_.any():
                out_[sel] = dist[:, mask_].min(1) < a.line
    part_l = _hash(np.arange(part.max() + 1), 7, 3, a.seed)[part[kw]]
    L = (a.base + a.light * n[:, 2] + a.bevel * bevel - a.seam * seam - a.ao * cavity + a.parts * part_l
         + a.mottle * fbm(p, a.mottle_scale, 3, a.seed + 11)
         + a.speckle * speckle(p, a.speckle_size, a.speckle_frac, a.seed + 3))
    row = (cells(p + 0.11, a.speckle_size, a.seed + 19) < 2 * a.alt - 1).astype(int)
    pick = np.abs(rows_l[row] - L[:, None]).argmin(1)

    out = rgb.copy()
    out[cy[work], cx[work]] = rows[row, pick]
    if a.keep_mask and a.glow_ribs > 0:
        energy = keep & (mask[cy, cx] <= 3)
        q = xyz[cy, cx][energy]
        # ribs run around the chamber (bands along X), a texel-ish wide
        phase = (q[:, 0] / a.rib_pitch) % 1.0
        rib = smoothstep(0.0, 0.25, np.abs(phase - 0.5) - 0.2)
        gain = 255 * (1 - a.glow_ribs * rib) * (0.9 + 0.1 * cells(q, 0.4, a.seed + 5))
        src = rgb[cy[energy], cx[energy]].astype(float)
        hue = src / np.maximum(src.max(1, keepdims=True), 1)
        out[cy[energy], cx[energy]] = np.clip(np.round(hue * gain[:, None]), 0, 255)
    out = fill_gaps(out, covered)

    used = np.bincount(pick, minlength=16)
    print(f'{tex_path.relative_to(ROOT) if tex_path.is_relative_to(ROOT) else tex_path}: '
          f'{covered.sum()} covered texels, {keep.sum()} kept, {part.max() + 1} parts, {row.mean():.0%} on '
          f'ramp {a.alt_ramp}, entries used ' + ', '.join(f'{i}:{c}' for i, c in enumerate(used) if c))
    if a.preview:
        from PIL import Image
        Image.fromarray(np.concatenate([rgb, out], 1)).save(a.preview)
    if not a.dry_run:
        tex_path.write_bytes(zlib.compress(out.tobytes(), 9))


if __name__ == '__main__':
    main()
