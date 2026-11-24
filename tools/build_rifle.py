"""Build the assault rifle's models and skins from the imported Meshy mesh.

python3 tools/build_rifle.py SRC.mdl.zlib [assets/rifle] [--seed 1]
    [--palette gfx/palette.lmp] [--preview atlas.png]

SRC is tools/import_toy_model.py's output with Meshy's own, even UV layout (the
commands are in assets/rifle/import.json). Writes

- model.mdl.zlib (everything but the magazine) and mag.mdl.zlib (the
  magazine, so that it can be swapped while reloading), sharing the atlas
  texture.rgb.zlib,
- sight.mdl.zlib with sight.rgb.zlib: a tube red dot sight built here, so the
  tube is open and its axis and aperture are known exactly (src/addons/
  rifle_model.rs LENS_*, checked by its test),

all in one space: Quake units, X forward (muzzle), Y left, Z up. Meshy's
colours are ignored: the skin is painted per texel from the model's shape
(material per connected part, ray-cast occlusion, worn edges, panel lines) in
exact non-fullbright palette colours, like tools/build_stickybomb.py.
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
import build_stickybomb as sb  # noqa: E402
from build_boomerang import write_mdl  # noqa: E402
from clean_toy_texture import ROOT, fill_gaps, load_mdl, palette_from_pak, rasterise, srgb_to_oklab  # noqa: E402
from paint_toy_texture import cells, parts, seg_dist, sharp_edges  # noqa: E402
from toy_wear import fbm, scratches, smoothstep  # noqa: E402

S = 1.3                      # units per unit of a 40-unit long rifle
# sight: tube axis height, middle, inner radius, length
LENS = np.array([0.4, 0.0, 11.3])
APERTURE, TUBE = 1.75, 3.4
RAIL = 6.2 * S               # rail top
# palette ramp (row of 16), lightness (OKLab L), grain, wear on outer edges
MATERIALS = {
    'steel': (0, 0.285, 0.012, 0.09), 'dark': (0, 0.190, 0.010, 0.085), 'gun': (0, 0.240, 0.010, 0.09),
    'poly': (10, 0.250, 0.018, 0.05), 'rubber': (0, 0.135, 0.010, 0.03), 'brass': (6, 0.34, 0.02, 0.06),
    'bore': (0, 0.16, 0.0, 0.0), 'paint': (4, 0.24, 0.015, 0.0),
}


def normals(data, nv):
    """Vertex normals of a one-pose MDL (its last nv × 4 bytes)."""
    text = (ROOT / 'crates/qcore/src/anorms.rs').read_text()
    table = np.array([[float(f) for f in m] for m in re.findall(
        r'\[\s*(-?\d+\.\d+),\s*(-?\d+\.\d+),\s*(-?\d+\.\d+)\s*\]', text)])
    return table[np.frombuffer(data[-4 * nv:], np.uint8).reshape(nv, 4)[:, 3]]


def line(d, width=0.08, light=0.13):
    """A panel line: dark within `width` of the plane, with a lit lip beside it."""
    return -0.09 * (np.abs(d) < width) + 0.06 * ((d >= width) & (d < width + light))


def every(x, pitch, lo, hi):
    """Signed distance to the nearest multiple of pitch from lo, far outside lo..hi."""
    d = (x - lo + pitch / 2) % pitch - pitch / 2
    return np.where((x > lo - pitch / 2) & (x < hi + pitch / 2), d, 99.0)


# --- sight -----------------------------------------------------------------
def build_sight():
    m = sb.Mesh()
    c, r, h = LENS, APERTURE, TUBE / 2
    # tube: rear flare, body, front flare; then the inside (dark)
    m.lathe('tube', [(-h, r), (-h, r + 0.3), (-h + 0.12, r + 0.36), (-h + 0.55, r + 0.36), (-h + 0.7, r + 0.22),
                     (h - 0.7, r + 0.22), (h - 0.55, r + 0.36), (h - 0.12, r + 0.36), (h, r + 0.3), (h, r)], c, seg=28)
    m.lathe('inside', [(h, r), (-h, r)], c, seg=28)
    # turrets: windage on the right, elevation on top, battery cap on the left
    cap = [(0, 0.62), (0.5, 0.62), (0.58, 0.5), (0.95, 0.5), (1.03, 0.42), (1.03, 0)]
    m.lathe('turret', [(a, q) for a, q in cap], c + [0.2, 0, r + 0.15], (0, 0, 1), 10)
    m.lathe('turret', [(a, q) for a, q in cap], c + [0.2, -(r + 0.15), 0], (0, -1, 0), 10)
    m.lathe('battery', [(0, 0.85), (0.6, 0.85), (0.7, 0.72), (0.7, 0)], c + [-0.1, r + 0.1, -0.15], (0, 1, 0), 12)
    # mount: saddle under the tube, post, rail clamp with a thumb nut
    z = RAIL - c[2]
    m.prism('mount', sb.octagon(1.0, c[2] - r - 0.75, c[2] - r - 0.04, 0.2), 'x', c[0] - 1.15, c[0] + 1.15, 0.12)
    m.prism('mount', sb.octagon(0.75, RAIL + 0.3, c[2] - r - 0.6, 0.1), 'x', c[0] - 0.95, c[0] + 0.95, 0.1)
    m.prism('clamp', [(-1.45, RAIL - 0.75), (-1.05, RAIL - 0.95), (1.05, RAIL - 0.95), (1.45, RAIL - 0.75), (1.45, RAIL + 0.15),
                      (1.1, RAIL + 0.45), (-1.1, RAIL + 0.45), (-1.45, RAIL + 0.15)], 'x', c[0] - 1.35, c[0] + 1.35, 0.14)
    for dx in (-0.75, 0.75):
        m.lathe('nut', [(0, 0.34), (0.35, 0.34), (0.42, 0.24), (0.42, 0)], (c[0] + dx, -1.4, RAIL - 0.25), (0, -1, 0), 6)
    # a crisp rail over the receiver, where the player looks along it
    m.prism('rail', [(-0.95, RAIL - 0.55), (0.95, RAIL - 0.55), (0.95, RAIL - 0.12), (0.8, RAIL + 0.04), (-0.8, RAIL + 0.04),
                     (-0.95, RAIL - 0.12)], 'x', -7.2, 2.5, 0.06, 0.8)
    for x in np.arange(-6.9, 2.4, 0.62):
        if abs(x - c[0]) > 1.6:
            m.prism('tooth', [(-1.0, RAIL - 0.3), (1.0, RAIL - 0.3), (1.0, RAIL + 0.1), (0.75, RAIL + 0.3), (-0.75, RAIL + 0.3),
                              (-1.0, RAIL + 0.1)], 'x', x - 0.18, x + 0.18, 0.05, 0.8)
    assert z < 0
    return m


def paint_sight(p, n, part, names):
    of = np.array(names, dtype=object)[part]
    mat = np.full(len(p), 'gun', dtype=object)
    L = np.zeros(len(p))
    x, y, z = (p - LENS).T
    for k, v in {'inside': 'bore', 'turret': 'dark', 'battery': 'dark', 'mount': 'steel', 'clamp': 'steel', 'nut': 'dark',
                 'tooth': 'gun', 'rail': 'dark'}.items():
        mat[of == k] = v
    r = np.hypot(y, z)
    s = (of == 'tube') & (np.abs(n[:, 0]) > 0.7)
    L[s] += 0.04
    s = (of == 'tube') & (np.abs(n[:, 0]) < 0.4)
    L[s] += line(np.abs(x[s]) - 0.55, 0.05, 0.08)
    # knurled turret and battery caps
    for c, axis, k in ((np.array([0.2, 0, APERTURE]), 2, 'turret'), (np.array([0.2, -APERTURE, 0]), 1, 'turret'),
                       (np.array([-0.1, APERTURE, -0.15]), 1, 'battery')):
        q = np.delete(p - LENS - c, axis, 1)
        near = (of == k) & (np.hypot(q[:, 0], q[:, 1]) < 1.0) & (np.abs(n[:, axis]) < 0.5) & (
            (p - LENS - c)[:, axis] * np.sign(c[axis] + 1e-9) > 0)
        ang = np.arctan2(q[:, 1], q[:, 0])
        L[near] += 0.05 * np.where((ang[near] / (2 * np.pi) * 18) % 1 < 0.4, -1, 0.6)
        face = (of == k) & (np.abs(n[:, axis]) > 0.8)
        slot = face & (np.abs(q[:, 0]) < 0.09) & (np.hypot(q[:, 0], q[:, 1]) < 0.36)
        L[slot] -= 0.1
    # an index mark on the rear rim
    s = (of == 'tube') & (n[:, 0] < -0.7) & (z > 0) & (np.abs(y) < 0.12) & (r > APERTURE + 0.12)
    mat[s] = 'paint'
    L[s] += 0.06
    return mat, L


# --- body ------------------------------------------------------------------
def paint_body(p, n, part, seed):
    """Materials and lightness offsets of the rifle (part ids of the source)."""
    x, y, z = (p / S).T
    mat = np.full(len(p), 'gun', dtype=object)
    L = np.zeros(len(p))
    side, top, end = np.abs(n[:, 1]) > 0.6, n[:, 2] > 0.5, np.abs(n[:, 0]) > 0.7
    # stock, foregrip: polymer; buffer tube: steel
    mat[(part == 0)] = 'poly'
    mat[(part == 0) & (x < -19.4)] = 'rubber'
    L[part == 0] += line(x[part == 0] + 19.35)
    s = (part == 0) & side
    L[s] += line(z[s] - 3.35, 0.06) * (x[s] > -18.8) + line(x[s] + 11.4, 0.06)
    s = (part == 0) & top
    L[s] += 0.035 * np.where(every(x[s], 0.5, -18.5, -14.0) < 0.12, 1, 0) * (np.abs(y[s]) < 0.7)
    mat[part == 1] = 'steel'
    mat[part == 22] = 'poly'
    s = part == 22
    L[s] += 0.05 * np.where((z[s] % 0.42) < 0.16, -1.1, 0.35) * (z[s] < 0.9) * (np.abs(n[s, 2]) < 0.6)
    # lower receiver and grip
    grip = (part == 2) & (x < -3.9) & (z < 0.55) & (z + 0.75 * (x + 3.9) < 0.3)
    low = (part == 2) & ~grip
    mat[low] = 'gun'
    mat[grip] = 'poly'
    s = grip & side
    L[s] += 0.045 * np.where(((x[s] + z[s]) % 0.62 < 0.2) | ((x[s] - z[s]) % 0.62 < 0.2), -1.0, 0.35) * (z[s] < -0.4) * (z[s] > -3.2)
    s = low & side
    L[s] += line(z[s] - 2.45, 0.07)                       # upper/lower split
    L[s] += line(x[s] - 2.05, 0.06) * (z[s] > 2.3)        # barrel nut
    L[s] += line(z[s] - 4.55, 0.06) * (x[s] < 1.9) * (x[s] > -5.6)
    s = low & side & (z < 2.3)
    L[s] += line(x[s] + 1.95, 0.06) * (z[s] > -0.2)
    # raised rib border round the magazine well
    well = low & side & (x > -1.9) & (x < 2.0) & (z < 2.1) & (z > 0.1)
    L[well] += 0.03
    # ejection port on the right with a shiny bolt carrier
    port = low & (n[:, 1] < -0.45) & (x > -2.35) & (x < 0.15) & (z > 3.0) & (z < 3.95)
    rim = low & (n[:, 1] < -0.3) & (x > -2.6) & (x < 0.4) & (z > 2.75) & (z < 4.2) & ~port
    mat[port] = 'steel'
    L[port] += 0.07 + 0.05 * scratches(p[port], 0.6, 0.15, seed + 5)
    L[port & (x < -1.6)] -= 0.13
    L[rim] -= 0.05
    # selector and pins
    for cx, cz, r in ((-5.05, 2.15, 0.17), (-2.95, 2.2, 0.17), (1.55, 2.15, 0.2), (-3.6, 1.35, 0.13), (-4.7, 1.35, 0.13)):
        d = np.hypot(x - cx, z - cz)
        s = low & side
        L[s] += 0.10 * (d[s] < r) - 0.09 * ((d[s] >= r) & (d[s] < r + 0.11))
    for k in (4, 5, 6, 7, 9, 10, 11, 12, 13):
        mat[part == k] = 'steel'
    for k in (3, 8, 14, 16, 17):
        mat[part == k] = 'dark'
    # handguard and its rails: dark, slots and teeth painted crisp
    s = part == 18
    mat[s] = 'dark'
    rail_side = s & (np.abs(y) > 1.18) & side & (np.abs(z - 4.35) < 0.55)
    L[rail_side] += 0.045 * np.where(np.abs(every(x[rail_side], 0.72, 2.6, 12.2)) < 0.2, -1.3, 0.5)
    slot = s & ~end & ~top & (np.abs(y) > 0.75) & (np.abs(np.abs(every(x, 3.3, 4.2, 11.0)) - 0) < 1.15) & (
        (np.abs(z - 5.4) < 0.16) | (np.abs(z - 3.25) < 0.16))
    mat[slot] = 'bore'
    railtop = ((part == 18) | (part == 2)) & (z > 5.75) & ~end
    mat[railtop] = 'gun'
    L[railtop] += 0.04 * np.where(np.abs(every(x[railtop], 0.48, 2.4, 12.6)) < 0.13, -1.5, 0.35) - 0.02
    # barrel: heat-darkened at the flash hider
    s = part == 19
    mat[s] = 'steel'
    L[s] -= 0.06 * smoothstep(16.2, 19.5, x[s])
    for cx in (14.2, 16.05, 16.9):
        L[s] += line(x[s] - cx, 0.06)
    bore = s & (x > 19.3) & (np.hypot(y + 0.07, z - 4.3) < 0.34)
    mat[bore] = 'bore'
    mat[(part == 20) | (part == 21)] = 'steel'
    # magazine: ribs along it, a dark base plate
    s = part == 15
    mat[s] = 'dark'
    L[s] += 0.015
    run = x - 0.12 * (z + 2.8) + 0.02 * (z + 2.8) ** 2
    ss = s & side
    L[ss] += 0.05 * (np.where(np.abs(every(run[ss], 0.95, -1.6, 2.4)) < 0.11, -1.4, 0.2))
    L[s] += line(z[s] + 5.5 + 0.2 * x[s], 0.07)
    return mat, L


def shade(p, n, k, pos, tris, mat, L, ramps, seed, ao_gain=0.2):
    """Palette colours from materials, offsets, edges and occlusion."""
    tp = pos[tris]
    fn = sb.unit(np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0]))
    edges = sharp_edges(pos, tris, fn, 32)
    bevel, seam = np.zeros(len(p)), np.zeros(len(p))
    for t in np.unique(k):
        ea, eb, convex = edges[t]
        if len(ea):
            sel = np.flatnonzero(k == t)
            dist = seg_dist(p[sel], pos[ea], pos[eb])
            for m, dst in ((convex, bevel), (~convex, seam)):
                if m.any():
                    dst[sel] = dist[:, m].min(1) < 0.13
    bevel *= 0.35 + 0.65 * (fbm(p, 1.1, 2, seed + 29) > -0.1)
    ao = sb.occlusion(p, n, seed=seed)
    ramp, base, grain, wear = (np.array([MATERIALS[m][i] for m in mat]) for i in range(4))
    # grain of single texels dithers between the few dark palette steps, so
    # that slow shading never breaks into blotches
    L = L + (base + 0.05 * n[:, 2] + wear * bevel * (1 - ao) - 0.06 * seam - ao_gain * ao
             + grain * cells(p, 0.14, seed + 3) + 0.004 * fbm(p, 2.2, 3, seed + 11))
    L[mat == 'bore'] = MATERIALS['bore'][1]
    lum = srgb_to_oklab(ramps.astype(float))[..., 0]
    return ramps[ramp, np.abs(lum[ramp] - L[:, None]).argmin(1)]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('src', type=Path)
    ap.add_argument('dir', type=Path, nargs='?', default=Path('assets/rifle'))
    ap.add_argument('--seed', type=int, default=1)
    ap.add_argument('--palette', type=Path)
    ap.add_argument('--preview', type=Path, help='write both atlases as PNG (needs Pillow)')
    a = ap.parse_args()
    d = a.dir if a.dir.is_absolute() else ROOT / a.dir
    d.mkdir(parents=True, exist_ok=True)
    pal = a.palette.read_bytes() if a.palette else palette_from_pak(
        os.environ.get('QUAKE_BASEDIR', ROOT / 'vkQuake directory with maps'))
    ramps = np.frombuffer(pal, np.uint8)[:224 * 3].reshape(14, 16, 3)

    data = zlib.decompress(a.src.read_bytes())
    pos, st, tris, mask = load_mdl(data)
    nrm = normals(data, len(pos))
    size = mask.shape[0]
    ids = parts(pos, tris)
    assert ids.max() + 1 == 23, 'the source changed: redo paint_body'
    pos = pos * S
    tris = tris[:, [0, 2, 1]]
    tp = pos[tris]
    fn = np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0])
    if (fn * nrm[tris].sum(1)).sum() < 0:
        tris = tris[:, [0, 2, 1]]
    tri_id, xyz, bary = rasterise(pos, st, tris, size, size)
    cov = tri_id >= 0
    k, bc, p = tri_id[cov], bary[cov], xyz[cov]
    w = np.column_stack([1 - bc.sum(1), bc])[..., None]
    n = sb.unit((nrm[tris[k]] * w).sum(1))
    mat, L = paint_body(p, n, ids[k], a.seed)
    out = np.zeros((size, size, 3), np.uint8)
    out[cov] = shade(p, n, k, pos, tris, mat, L, ramps, a.seed, 0.11)
    out = fill_gaps(out, cov)
    (d / 'texture.rgb.zlib').write_bytes(zlib.compress(out.tobytes(), 9))
    info = {}
    for name, sel in (('model', ids != 15), ('mag', ids == 15)):
        used, inv = np.unique(tris[sel], return_inverse=True)
        info[name] = write_mdl(d / f'{name}.mdl.zlib', pos[used], nrm[used], st[used], inv.reshape(-1, 3), size, b'rifle')

    sb.SIZE = 256
    mesh = build_sight()
    texels, pos, nrm, st, tris, part = mesh.build()
    tri_id, xyz, bary = rasterise(pos, st, tris, 256, 256)
    cov = tri_id >= 0
    k, bc, p = tri_id[cov], bary[cov], xyz[cov]
    w = np.column_stack([1 - bc.sum(1), bc])[..., None]
    n = sb.unit((nrm[tris[k]] * w).sum(1))
    mat, L = paint_sight(p, n, part[k], mesh.parts)
    s_out = np.zeros((256, 256, 3), np.uint8)
    s_out[cov] = shade(p, n, k, pos, tris, mat, L, ramps, a.seed)
    s_out = fill_gaps(s_out, cov)
    (d / 'sight.rgb.zlib').write_bytes(zlib.compress(s_out.tobytes(), 9))
    info['sight'] = write_mdl(d / 'sight.mdl.zlib', pos, nrm, st, tris, 256, b'rifle')
    meta = {
        'source': 'Meshy meshy-t2.5 + meshy-n1, image-to-3d task 01a20ef1-cd19-75c4-b93f-4fe57e001aba',
        'build': [
            'blender -b --factory-startup --python tools/import_toy_model.py -- ~/Assets/Meshy/rifle/model.glb '
            '--out TMP --length 40 --anchor origin --flip --back 20 --gain 1.0 --keep-uv',
            f'python3 tools/build_rifle.py TMP/model.mdl.zlib assets/rifle --seed {a.seed}',
        ],
        'models': {m: dict(vertices=v, triangles=t, bounds=b) for m, (v, t, b) in info.items()},
        'lens': dict(centre=LENS.tolist(), aperture=APERTURE, tube=TUBE),
    }
    (d / 'import.json').write_text(json.dumps(meta, indent=2) + '\n')
    print(json.dumps(meta['models']), f'{texels:.1f} sight texels per unit')
    if a.preview:
        from PIL import Image
        Image.fromarray(np.hstack([out, np.pad(s_out, ((0, size - 256), (0, 0), (0, 0)))])).save(a.preview)


if __name__ == '__main__':
    main()
