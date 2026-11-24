"""Paint the gel cannon's skin from scratch, Quake-style (texture.rgb.zlib).

python3 tools/paint_gel_texture.py [assets/gel] [--seed 1]
    [--palette gfx/palette.lmp] [--preview atlas.png] [--dry-run]

Like tools/paint_toy_texture.py this ignores the baked Meshy colours and
paints the metal from the model's shape (baked top light, bevels on outer
edges, seams in inner ones, cavity shading), but with a material per
connected part of the mesh (PARTS) and hand-placed details in model space:

- steel barrel and drum, warm iron receiver, bronze canister caps, muzzle lip
  and fittings, ribbed rubber hoses, walnut stock and grip,
- panel lines, rivet rows, bolts around the canister caps, vent slots,
- canisters of gel (orange at +Y, blue at -Y) with currents, bubbles, a
  highlight along the top and an iron strap, and a gel sight glass in each
  rear hub, facing the player.

Metal texels are exact palette colours and never pass the engine's glow test
(src/addons/toy_model.rs gel_glow); gel texels all do, and are drawn unlit in
exactly the colour painted here, so GEL's colours are final.
Positions are model units (X forward, Z up); one texel is about 0.13 units.
"""
import argparse
import os
from pathlib import Path
import sys
import zlib

import numpy as np

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from clean_toy_texture import (ROOT, fill_gaps, glow_texels, load_mdl,  # noqa: E402
                               palette_from_pak, rasterise, srgb_to_oklab)
from paint_toy_texture import cells, parts, seg_dist, sharp_edges, speckle  # noqa: E402
from toy_wear import convexity, fbm, scratches, smoothstep  # noqa: E402

# material: palette ramp, lightness (OKLab L), mottle, wear on outer edges
MATERIALS = {
    'steel': (0, 0.27, 0.022, 0.12),
    'dark': (0, 0.20, 0.016, 0.09),
    'iron': (10, 0.27, 0.025, 0.11),
    'bronze': (1, 0.27, 0.025, 0.11),
    'rubber': (0, 0.15, 0.010, 0.04),
    'wood': (7, 0.33, 0.015, 0.07),
}
# connected parts (ids from paint_toy_texture.parts; the mesh is fixed)
PARTS = {
    0: 'wood', 1: 'wood', 2: 'steel', 3: 'iron', 4: 'steel', 5: 'iron', 6: 'steel',
    7: 'bronze', 8: 'bronze', 9: 'dark', 10: 'bronze', 11: 'bronze', 12: 'bronze', 13: 'bronze',
    14: 'bronze', 15: 'bronze', 16: 'bronze', 17: 'bronze', 18: 'bronze', 19: 'bronze',
    20: 'bronze', 21: 'steel', 22: 'bronze', 23: 'steel', 24: 'rubber', 25: 'rubber',
    26: 'iron', 27: 'bronze', 28: 'dark',
}
CAN = {14: -1.0, 15: 1.0}  # canister part: side (sign of Y)
CAN_Y, CAN_Z = 3.66, 5.12  # canister axis
HUB_Y, HUB_Z = 3.72, 5.26  # rear hub axis
# gel colours, dark to light; +1 orange (Propulsion), -1 blue (Repulsion)
GEL = {
    1.0: np.array([[116, 24, 0], [136, 32, 0], [156, 42, 0], [176, 52, 0], [194, 68, 2], [216, 100, 12]], float),
    -1.0: np.array([[2, 8, 98], [3, 14, 120], [5, 22, 142], [8, 34, 166], [18, 52, 190], [50, 90, 212]], float),
}


def line(d, width=0.1, light=0.16):
    """A panel line: dark within `width` of the plane, with a lit lip beside it."""
    return -0.11 * (np.abs(d) < width) + 0.07 * ((d >= width) & (d < width + light))


def dots(u, v, r=0.2):
    """Rivets at the origin of (u, v): a lit head with a dark rim below it."""
    head = u * u + v * v < r * r
    rim = u * u + (v + 0.1) ** 2 < (r + 0.12) ** 2
    return 0.13 * head - 0.10 * (rim & ~head)


def every(x, pitch, lo, hi):
    """Signed distance to the nearest multiple of pitch from lo, inf outside lo..hi."""
    d = (x - lo + pitch / 2) % pitch - pitch / 2
    return np.where((x > lo - pitch / 2) & (x < hi + pitch / 2), d, 99.0)


def details(p, n, part, mat):
    """Lightness offsets of the hand-placed details; may re-label `mat`."""
    x, y, z = p.T
    L = np.zeros(len(p))
    side, top = np.abs(n[:, 1]) > 0.6, n[:, 2] > 0.5
    end = np.abs(n[:, 0]) > 0.7

    # receiver: panel breaks, a rivet row under the rail
    s = (part == 3) & ~end
    L[s] += line(x[s] + 8.6) + line(x[s] + 1.1) + line(x[s] - 3.2)
    s = (part == 3) & side
    L[s] += dots(every(x[s], 1.45, -7.8, 2.4), z[s] - 2.9)
    window = (part == 3) & (np.abs(n[:, 1]) > 0.8) & (np.abs(x + 4.5) < 2.0) & (np.abs(z - 1.35) < 0.75)
    mat[window] = 'rubber'
    L[window] -= 0.03
    # top plate: cross ribs and screws at its edges
    s = (part == 6) & top
    L[s] += line(every(x[s], 1.6, -7.2, 2.0))
    L[s] += dots(every(x[s], 1.6, -6.4, 1.2), np.abs(y[s]) - 0.95, 0.17)
    L[s] += 0.05 * (scratches(p[s], 0.45, 0.13, 23) > 0.5)
    s = (part == 9) & ~end
    L[s] += line(every(x[s], 1.2, -7.2, -1.6), 0.12)

    # barrel: reinforcing bands in bronze, vent slots on the sides
    b = part == 23
    band = b & ((np.abs(x - 6.3) < 0.35) | (np.abs(x - 15.4) < 0.4)) & (z > -0.5)
    mat[band] = 'bronze'
    L[b] += (line(x[b] - 5.85) + line(x[b] - 6.75) + line(x[b] - 14.9))
    s = b & ~end & (z > -0.3) & (z < 4.4)
    slot = (np.abs(every(x[s], 1.7, 8.2, 13.4)) < 0.5) & (np.abs(z[s] - 2.5) < 0.2)
    lip = (np.abs(every(x[s], 1.7, 8.2, 13.4)) < 0.62) & (np.abs(z[s] - 2.5) < 0.36) & ~slot
    L[s] += -0.2 * slot + 0.06 * lip
    # muzzle: heat-darkened towards a bronze lip
    m = part == 28
    L[m] += line(x[m] - 17.4) - 0.05 * smoothstep(17.5, 19.2, x[m])
    lip = m & (x > 19.35)
    mat[lip] = 'bronze'
    bore = m & (np.hypot(y, z - 1.9) < 1.2)
    mat[bore] = 'rubber'
    L[bore] -= 0.1

    # under-barrel rail: ribs; drum: flutes round the side and a spoked face
    s = part == 26
    L[s] += 0.06 * np.where(np.abs(every(x[s], 1.0, 7.0, 15.0)) < 0.28, -1.2, 0.35)
    d = part == 21
    ang = np.arctan2(z + 3.9, x - 1.3)
    s = d & ~side
    L[s] += line(np.abs(y[s]) - 1.9) + line(np.abs(y[s]) - 0.5)
    s = d & side
    r = np.hypot(x - 1.3, z + 3.9)
    L[s] += line(r[s] - 1.0) + line(r[s] - 2.1)
    L[s] += dots((every(ang[s], np.pi / 4, -np.pi, np.pi)) * 1.6, r[s] - 1.55)

    # hoses: rubber ribs along their run, bronze collars at the ends
    for k in (24, 25):
        s = part == k
        run = (x - 3.7) * 0.6 + (5.2 - z) * 0.55 + (4.1 - np.abs(y)) * 0.6
        L[s] += 0.05 * np.where(run[s] % 0.5 < 0.25, 1, -1)
        mat[s & ((run < 0.45) | (z < 3.3))] = 'bronze'

    # walnut: grain along the stock, or down the grip, with darker rings
    grip = (part == 4) & (x < -4.6) & (z < -1.75)
    mat[grip] = 'wood'
    for s, along in (((part == 0) | (part == 1), np.array([0.12, 1.3, 1.3])), (grip, np.array([1.5, 1.3, 0.25]))):
        g = fbm(p[s] * along, 0.55, 3, 41)
        L[s] += 0.06 * g - 0.04 * (np.abs(fbm(p[s] * along, 1.0, 2, 47)) < 0.06)
    butt = (part == 0) & (x < -19.35)
    mat[butt] = 'rubber'
    L[part == 0] += line(x[part == 0] + 19.3)
    s = part == 1
    for c in (-17.5, -11.0):
        mat[s & (np.abs(x - c) < 0.45)] = 'steel'
        L[s] += line(np.abs(x[s] - c) - 0.5)
    s = grip & side
    L[s] += 0.05 * np.where(((x[s] + z[s]) % 0.9 < 0.3) | ((x[s] - z[s]) % 0.9 < 0.3), -1.0, 0.3) * (
        (z[s] < -2.8) & (z[s] > -6.0))

    # canister caps: dark recess and a bolt circle on the flat ends, a groove round the rim
    for k, sign in CAN.items():
        s = part == k
        dy, dz = y - sign * CAN_Y, z - CAN_Z
        r, a = np.hypot(dy, dz), np.arctan2(dz, dy)
        face = s & end
        mat[face & (r < 1.5)] = 'steel'
        L[face & (r < 1.5)] -= 0.10
        f = s & end & (r >= 1.5)
        L[f] += 0.05
        bolt = every(a, np.pi / 4, -np.pi, np.pi)
        f = face & (r < 1.5)
        L[f] += dots(bolt[f] * 1.15, r[f] - 1.15, 0.17)
        rim = s & ~end & (r > 1.75)
        mid = np.where(x < 0, -4.7, 3.45)
        L[rim] += line(np.abs(x[rim] - mid[rim]) - 0.28, 0.08, 0.12)
    return L


def gel(p, sign, hub):
    """Gel colour of canister texels."""
    x, dy, dz = p[:, 0], p[:, 1] - sign * CAN_Y, p[:, 2] - CAN_Z
    if hub:
        c = 4.1 - 2.4 * np.hypot(p[:, 1] - sign * HUB_Y, p[:, 2] - HUB_Z) + 0.4 * cells(p, 0.26, 3)
    else:
        ang = np.degrees(np.arctan2(dz, dy * sign))  # 90 = top, 0 = outward
        # currents drift along the tube; bent by a second noise so they curl
        q = p * np.array([0.5, 1.0, 1.0]) + 0.5 * fbm(p, 1.6, 2, 61)[:, None]
        c = 2.1 + 2.3 * fbm(q, 1.3, 3, 71) + 0.9 * dz / 1.6
        c += 1.6 * (np.abs(ang - 112) < 7) * (cells(p * np.array([0.35, 0, 0]), 0.3, 5) > -0.55)
        c -= 1.3 * smoothstep(0.55, 0.0, np.minimum(x + 4.16, 2.72 - x))
        b = cells(p * np.array([1.0, 1.0, 1.0]), 0.27, 83)
        c += 1.8 * (b > 0.95) - 0.5 * (b < -0.93)
    return GEL[sign][np.clip(np.round(c), 0, 5).astype(int)]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, nargs='?', default=Path('assets/gel'))
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
    assert ids.max() + 1 == len(PARTS), 'assets/gel/model.mdl.zlib changed: redo PARTS'

    cy, cx = np.nonzero(covered)
    k = tri_id[cy, cx]
    p, n, part = xyz[cy, cx], fn[k], ids[k]

    # gel: the glass between the caps (less an iron strap) and the rear hubs
    out_col = np.zeros((len(p), 3), np.uint8)
    is_gel = np.zeros(len(p), bool)
    strap = np.zeros(len(p), bool)
    for c, sign in CAN.items():
        r = np.hypot(p[:, 1] - sign * CAN_Y, p[:, 2] - CAN_Z)
        glass = (part == c) & (np.abs(n[:, 0]) < 0.5) & (p[:, 0] > -4.2) & (p[:, 0] < 2.95) & (r < 1.8)
        strap |= glass & (np.abs(p[:, 0] + 0.75) < 0.3)
        hub_r = np.hypot(p[:, 1] - sign * HUB_Y, p[:, 2] - HUB_Z)
        hub = (part == {14: 11, 15: 13}[c]) & (p[:, 0] < -5.2) & (hub_r < 0.4)
        for sel, is_hub in ((glass & ~strap, False), (hub, True)):
            out_col[sel] = gel(p[sel], sign, is_hub)
            is_gel |= sel

    mat = np.array([PARTS[i] for i in range(len(PARTS))], dtype=object)[part]
    mat[strap] = 'dark'
    L = details(p, n, part, mat)
    L[strap] += 0.06 + line(np.abs(p[strap, 0] + 0.75) - 0.24, 0.06, 0.0)

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
                dst[sel] = dist[:, m].min(1) < 0.2
    # worn edges are broken up; soft streaks of grime run down the sides
    bevel *= 0.35 + 0.65 * (fbm(p, 1.1, 2, a.seed + 29) > -0.15)
    grime = smoothstep(0.15, 0.5, fbm(p * np.array([1.4, 1.4, 0.3]), 1.6, 3, a.seed + 9))
    ramp, base, mottle, wear = (np.array([MATERIALS[m][i] for m in mat]) for i in range(4))
    L += (base + 0.05 * n[:, 2] + wear * bevel - 0.09 * seam - 0.06 * cavity - 0.03 * grime
          + mottle * fbm(p, 2.0, 3, a.seed + 11) + mottle * speckle(p, 0.26, 0.25, a.seed + 3))
    pick = np.abs(rows_l[ramp] - L[:, None]).argmin(1)
    metal = rows[ramp, pick].copy()
    while (bad := glow_texels(metal[None])[0] & (pick > 0)).any():
        pick[bad] -= 1
        metal = rows[ramp, pick].copy()
    out_col[~is_gel] = metal[~is_gel]
    assert (glow_texels(out_col[None])[0] == is_gel).all(), 'gel must glow, metal must not'

    out = np.zeros((h, w, 3), np.uint8)
    out[cy, cx] = out_col
    out = fill_gaps(out, covered)
    print(f'{covered.sum()} covered texels, {is_gel.sum()} gel, materials '
          + ', '.join(f'{m}:{(mat[~is_gel] == m).sum()}' for m in MATERIALS))
    if a.preview:
        from PIL import Image
        Image.fromarray(out).save(a.preview)
    if not a.dry_run:
        (d / 'texture.rgb.zlib').write_bytes(zlib.compress(out.tobytes(), 9))


if __name__ == '__main__':
    main()
