"""Paint the akimbo pistol's skin from scratch, Quake-style (texture.rgb.zlib).

python3 tools/paint_akimbo_texture.py [assets/akimbo] [--seed 1]
    [--palette gfx/palette.lmp] [--preview atlas.png] [--dry-run]

Like tools/paint_gel_texture.py: a material per connected part of the mesh
(told apart by its bounding box, see `materials`), shading from the model's
shape (top light, bevels, seams, cavities) and hand-placed details in model
space: serrations, panel lines, a walnut grip with leather strips, a hazard
band on the recoil weight. All texels are exact palette colours. Positions are
model units (X forward, Z up).
"""
import argparse
import os
from pathlib import Path
import sys
import zlib

import numpy as np

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from clean_toy_texture import (ROOT, fill_gaps, load_mdl, palette_from_pak,  # noqa: E402
                               rasterise, srgb_to_oklab)
from paint_gel_texture import dots, every, line  # noqa: E402
from paint_toy_texture import parts, seg_dist, sharp_edges, speckle  # noqa: E402
from toy_wear import convexity, fbm, scratches, smoothstep  # noqa: E402

# material: palette ramp, lightness (OKLab L), mottle, wear on outer edges
MATERIALS = {
    'steel': (0, 0.265, 0.014, 0.15),
    'dark': (0, 0.19, 0.012, 0.10),
    'iron': (10, 0.27, 0.025, 0.13),
    'bronze': (1, 0.31, 0.022, 0.12),
    'rust': (6, 0.24, 0.030, 0.10),
    'wood': (7, 0.35, 0.015, 0.06),
    'leather': (1, 0.23, 0.030, 0.08),
    'yellow': (1, 0.55, 0.020, 0.05),
    'black': (0, 0.10, 0.010, 0.03),
}


def materials(pos, tris, ids):
    """Material of each connected part."""
    out = []
    for i in range(ids.max() + 1):
        q = pos[tris[ids == i]].reshape(-1, 3)
        (x0, y0, z0), (x1, y1, z1) = q.min(0), q.max(0)
        n = (ids == i).sum()
        if z0 > 5.7 or (n > 1000):
            m = 'steel'  # slide, fins, front sight
        elif n > 800:
            m = 'bronze'  # frame
        elif x0 > 8.0:
            m = 'dark' if n > 12 else 'bronze'  # muzzle brake and its bars
        elif x0 > 1.0 and z1 < 3.0 and n > 50:
            m = 'bronze'  # recoil weight
        elif z1 < -1.5 and abs(y1 - y0) > 4.0:
            m = 'leather'
        elif z0 < -5.0 and n > 60 and abs(y0 + y1) > 2.0:
            m = 'wood'
        elif z0 < -5.0:
            m = 'dark'  # magazine base, grip collar
        elif x0 < -10.0 or (x0 < -3.0 and x1 < -2.0 and y1 - y0 < 0.8 and z0 < 0):
            m = 'rust'  # hammer, trigger
        elif x1 < -8.6 and z0 > 3.9:
            m = 'bronze'  # firing pin cap
        elif n > 50 and x1 < -7.0 and z0 > 3.0:
            m = 'steel'  # slide's back plate
        else:
            m = 'iron'  # bolts, barrel, pins
        out.append(m)
    return out


def details(p, n, big, mat):
    """Lightness offsets of the hand-placed details; may re-label `mat`.
    `big`: 'slide', 'frame', 'weight', 'brake' or '' per texel."""
    x, y, z = p.T
    L = np.zeros(len(p))
    side, top = np.abs(n[:, 1]) > 0.6, n[:, 2] > 0.5
    end = np.abs(n[:, 0]) > 0.7

    s = (big == 'slide') & side
    L[s] += line(z[s] - 3.35, 0.07, 0.12)  # rail line
    L[s] += line(np.abs(x[s] - 6.9) - 0.05, 0.08, 0.14)
    s = (big == 'slide') & (n[:, 2] > 0.2) & ~end
    L[s] += 0.02 + 0.04 * (scratches(p[s], 0.45, 0.1, 23) > 0.5)
    s = (big == 'slide') & top
    L[s] += line(np.abs(y[s]) - 0.7, 0.06, 0.1)
    s = (big == 'slide') & (z < 3.2) & (x > 5.0)
    L[s] -= 0.06  # dust cover in shadow
    # back plate: a bronze ring of screws round the firing pin's cap
    s = (mat == 'steel') & (n[:, 0] < -0.7) & (x < -8.0)
    r = np.hypot(y[s], z[s] - 4.5)
    L[s] += line(r - 1.0, 0.07, 0.12)

    s = (big == 'frame') & side
    L[s] += line(x[s] + 0.55, 0.07, 0.12) * (z[s] > 1.7)
    L[s] += line(z[s] - 1.75, 0.06, 0.1) * (x[s] > -3.2)
    s = (big == 'frame') & (z < -0.2)
    mat[s & (np.abs(y) < 2.2) & (x < -3.6)] = 'dark'  # grip's steel
    mat[s & (z > -4.5) & (z < -0.9) & (x < -3.6) & (side | (n[:, 0] > 0.3))] = 'leather'
    mat[(big == 'frame') & (x < -8.9)] = 'dark'  # beavertail
    # ridges down the grip's back
    s = (big == 'frame') & (x < -6.0) & (n[:, 0] < -0.5)
    L[s] += 0.05 * np.where(z[s] % 0.5 < 0.25, -1.0, 0.6)

    # recoil weight: hazard chevrons on a band at its nose
    band = (big == 'weight') & (x > 5.0) & (x < 6.55) & ~end
    stripe = ((x + np.where(side, z, np.abs(y))) % 0.9) < 0.45
    mat[band & ~stripe] = 'yellow'
    mat[band & stripe] = 'black'
    s = (big == 'weight') & ~end
    L[s] += line(np.abs(x[s] - 5.0), 0.06, 0.1)
    s = (big == 'weight') & side & (x < 4.9)
    L[s] += line(z[s] - 2.5, 0.05, 0.08)

    # muzzle brake: bands, soot towards a black bore
    s = big == 'brake'
    L[s] += line(np.abs(x[s] - 8.55), 0.06, 0.1) * (~end[s])
    soot = s & (x > 10.2)
    L[soot] -= 0.05 * smoothstep(10.2, 11.0, x[soot])
    bore = s & (np.hypot(y, z - 4.54) < 1.0)
    mat[bore] = 'black'

    # walnut: grain down the grip; leather strips wound across it
    g = mat == 'wood'
    f = fbm(p[g] * np.array([1.5, 1.3, 0.28]), 0.55, 3, 41)
    L[g] += 0.06 * f - 0.05 * (np.abs(fbm(p[g] * np.array([1.5, 1.3, 0.28]), 1.0, 2, 47)) < 0.07)
    s = g & side
    wood = side & g
    diamond = wood & (z > -1.4) & (z < 1.5)
    L[diamond] += 0.05 * np.where(((x[diamond] + z[diamond]) % 0.6 < 0.2) | ((x[diamond] - z[diamond]) % 0.6 < 0.2), -1.0, 0.3)
    screw = wood & (np.hypot(x + 5.1 - 0.18 * (z - 1.8), z - 1.85) < 0.4)
    screw |= wood & (np.hypot(x + 7.1, z + 4.1) < 0.4)
    mat[screw] = 'steel'
    L[screw] -= 0.16 * (np.abs((x + z)[screw] % 10 - 0.2) < 0.1)
    g = mat == 'leather'
    w = (z + 0.35 * np.sin(x * 1.1 + 2 * (y > 0)) * 0.8 + 0.25 * x) % 0.62
    L[g] += np.where(w[g] < 0.12, -0.12, 0.05 * (w[g] - 0.12) / 0.5)
    L[g] += 0.03 * fbm(p[g], 0.5, 2, 13)
    return L


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, nargs='?', default=Path('assets/akimbo'))
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--palette', type=Path)
    ap.add_argument('--preview', type=Path, help='write the atlas as PNG (needs Pillow)')
    ap.add_argument('--dry-run', action='store_true', help='do not rewrite the texture')
    a = ap.parse_args()

    d = a.dir if a.dir.is_absolute() else ROOT / a.dir
    pos, st, tris, mask = load_mdl(zlib.decompress((d / 'model.mdl.zlib').read_bytes()))
    h, w = mask.shape
    pal_bytes = a.palette.read_bytes() if a.palette else palette_from_pak(
        os.environ.get('QUAKE_BASEDIR', ROOT / 'vkQuake directory with maps'))
    rows = np.frombuffer(pal_bytes, np.uint8)[:224 * 3].reshape(14, 16, 3)
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
    ids = parts(pos, tris)
    mats = materials(pos, tris, ids)
    count = np.bincount(ids)
    name = {int(np.argmax(count)): 'slide'}
    for i in range(len(count)):
        q = pos[tris[ids == i]].reshape(-1, 3)
        if i in name:
            continue
        if count[i] > 800:
            name[i] = 'frame'
        elif q[:, 0].min() > 8.0 and count[i] > 60:
            name[i] = 'brake'
        elif q[:, 0].min() > 1.0 and q[:, 2].max() < 3.0 and count[i] > 50:
            name[i] = 'weight'

    cy, cx = np.nonzero(covered)
    k = tri_id[cy, cx]
    p, n, part = xyz[cy, cx], fn[k], ids[k]
    mat = np.array(mats, dtype=object)[part]
    big = np.array([name.get(i, '') for i in range(len(count))], dtype=object)[part]
    L = details(p, n, big, mat)

    conv = convexity(p, n, 1.0)
    cavity = smoothstep(0.3, 1.5, -conv / (conv.std() + 1e-9))
    edges = sharp_edges(pos, tris, fn, 35)
    bevel, seam = np.zeros(len(p)), np.zeros(len(p))
    for t in np.unique(k):
        ea, eb, convex = edges[t]
        if len(ea) == 0:
            continue
        sel = np.flatnonzero(k == t)
        dist = seg_dist(p[sel], pos[ea], pos[eb])
        for m, dst in ((convex, bevel), (~convex, seam)):
            if m.any():
                dst[sel] = dist[:, m].min(1) < 0.1
    bevel *= 0.35 + 0.65 * (fbm(p, 1.1, 2, a.seed + 29) > -0.15)
    grime = smoothstep(0.15, 0.5, fbm(p * np.array([1.4, 1.4, 0.3]), 1.6, 3, a.seed + 9))
    rust = (np.isin(mat, ['steel', 'iron', 'dark'])
            & (fbm(p, 1.3, 3, a.seed + 51) + 0.35 * cavity - 0.4 * bevel > 0.52))
    mat[rust] = 'rust'
    ramp, base, mottle, wear = (np.array([MATERIALS[m][i] for m in mat]) for i in range(4))
    L += (base + 0.06 * n[:, 2] + wear * bevel - 0.09 * seam - 0.06 * cavity - 0.03 * grime
          + mottle * fbm(p, 2.0, 3, a.seed + 11) + mottle * speckle(p, 0.2, 0.25, a.seed + 3))
    pick = np.abs(rows_l[ramp] - L[:, None]).argmin(1)
    out = np.zeros((h, w, 3), np.uint8)
    out[cy, cx] = rows[ramp, pick]
    out = fill_gaps(out, covered)
    print(f'{covered.sum()} covered texels, ' + ', '.join(f'{m}:{(mat == m).sum()}' for m in MATERIALS))
    if a.preview:
        from PIL import Image
        Image.fromarray(out).save(a.preview)
    if not a.dry_run:
        (d / 'texture.rgb.zlib').write_bytes(zlib.compress(out.tobytes(), 9))


if __name__ == '__main__':
    main()
