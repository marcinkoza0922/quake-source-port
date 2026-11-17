"""Clean a baked toy texture (texture.rgb.zlib) so it quantises without splotches.

python3 tools/clean_toy_texture.py assets/washer [--mdl model.mdl.zlib] [--from OLD.mdl.zlib]
    [--chroma 5] [--luma 3] [--detail 0.5] [--gain 1] [--keep-mask] [--keep-glow]
    [--ramps 0,10] [--ramp-map 7:5,...] [--contrast 1.0] [--palette gfx/palette.lmp]
    [--preview out.png] [--dry-run]

Meshy atlases baked by tools/import_toy_model.py are noisy: colour specks,
per-triangle islands that each got a slightly different shade, black gaps
that bleed into mipmaps, and plain nearest-colour quantisation that throws
neighbouring texels into unrelated palette rows (brown -> pink/green/purple).
This tool works from the shipped MDL + RGB atlas (no Meshy source needed):

1. Rasterises every triangle into the atlas, so each texel knows its 3D
   position (with --from, the old texture is first resampled into this
   layout through the matching triangle).
2. Smooths colour across the model's surface in 3D (seams and tiny islands
   blend with their 3D neighbours, not their atlas neighbours): chroma with a
   wide radius (--chroma, texels), luminance with --luma, keeping --detail of
   the fine luminance on top.
3. Gives each texel one Quake palette ramp (row of 16) chosen from the
   smoothed chroma, with a 3D majority vote so materials have clean edges,
   then the ramp entry nearest in lightness. Output texels are exact palette
   colours, so the engine's nearest-colour load is lossless on id1.
4. Optionally adds wear made from the model's shape and 3D noise (see
   tools/toy_wear.py): chipped paint showing bare metal on outer edges
   (--wear, --edge-ramp), grime in creases (--grime), rust patches that
   favour creases and run down as drips (--rust, --rust-scale, --rust-ramp),
   scratches (--scratches) and brushed grain (--grain). Seams cannot show
   because nothing depends on the atlas layout.
5. Fills the empty atlas space with the nearest island colour (no dark seams
   or mip bleeding).

--keep-mask leaves texels whose MDL skin byte is non-zero untouched (the
portal gun's owner-colour regions). --keep-glow leaves large connected
saturated blue/orange areas untouched (the gel canisters, which become the
true-colour glow layer) and cleans the isolated flecks. --ramp-map remaps a
chosen ramp (0-13) to another; --ramps limits the choice (id1 rows: 0 grey,
1 khaki, 2 blue-grey, 6 copper/orange, 7 flesh, 10 warm grey-brown, 13 blue),
e.g. --ramp-map 7:6 sends flesh to copper.
The palette defaults to gfx/palette.lmp from $QUAKE_BASEDIR/id1/pak0.pak.
"""
import argparse
import os
from pathlib import Path
import struct
import sys
import zlib

import numpy as np
from scipy import ndimage, sparse
from scipy.spatial import cKDTree

# tools/* are Cargo workspace members: a tools/__pycache__ would break cargo
sys.dont_write_bytecode = True
from toy_wear import blur3d, wear  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


def load_mdl(data):
    """(positions[N,3], st[N,2], tris[T,3], mask[H,W]) of a one-skin, one-pose MDL."""
    assert data[:4] == b'IDPO'
    h = struct.unpack_from('<i3f3ff3f8if', data, 4)
    scale, origin = np.array(h[1:4]), np.array(h[4:7])
    nskins, w, hgt, nverts, ntris, nframes = h[11:17]
    assert nskins == 1 and nframes >= 1
    off = 84
    assert struct.unpack_from('<i', data, off)[0] == 0
    mask = np.frombuffer(data, np.uint8, w * hgt, off + 4).reshape(hgt, w)
    off += 4 + w * hgt
    st = np.frombuffer(data, '<i4', nverts * 3, off).reshape(nverts, 3)[:, 1:].astype(float)
    off += nverts * 12
    tris = np.frombuffer(data, '<i4', ntris * 4, off).reshape(ntris, 4)[:, 1:]
    off += ntris * 16
    assert struct.unpack_from('<i', data, off)[0] == 0
    off += 4 + 8 + 16
    packed = np.frombuffer(data, np.uint8, nverts * 4, off).reshape(nverts, 4)[:, :3]
    return origin + packed * scale, st, tris, mask


def rasterise(pos, st, tris, w, h):
    """Per texel: triangle index (-1 = empty), 3D position and barycentric
    (u, v) of the texel centre."""
    tri_id = np.full((h, w), -1, np.int32)
    xyz = np.zeros((h, w, 3))
    bary = np.zeros((h, w, 2))
    for k, (a, b, c) in enumerate(tris):
        p = st[[a, b, c]]
        x0, y0 = np.floor(p.min(0)).astype(int)
        x1, y1 = np.ceil(p.max(0)).astype(int)
        x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, w - 1), min(y1, h - 1)
        if x1 < x0 or y1 < y0:
            continue
        ys, xs = np.mgrid[y0:y1 + 1, x0:x1 + 1]
        d = p[1] - p[0], p[2] - p[0]
        den = d[0][0] * d[1][1] - d[1][0] * d[0][1]
        if abs(den) < 1e-9:
            continue
        rx, ry = xs - p[0][0], ys - p[0][1]
        u = (rx * d[1][1] - d[1][0] * ry) / den
        v = (d[0][0] * ry - rx * d[0][1]) / den
        inside = (u >= -1e-6) & (v >= -1e-6) & (u + v <= 1 + 1e-6)
        yy, xx = ys[inside], xs[inside]
        uu, vv = u[inside][:, None], v[inside][:, None]
        tri_id[yy, xx] = k
        bary[yy, xx] = np.hstack([uu, vv])
        xyz[yy, xx] = pos[a] + uu * (pos[b] - pos[a]) + vv * (pos[c] - pos[a])
    return tri_id, xyz, bary


def fill_gaps(img, covered):
    """Every uncovered texel takes the colour of the nearest covered one."""
    _, (iy, ix) = ndimage.distance_transform_edt(~covered, return_indices=True)
    return img[iy, ix]


def srgb_to_oklab(rgb):
    c = rgb / 255.0
    c = np.where(c > 0.04045, ((c + 0.055) / 1.055) ** 2.4, c / 12.92)
    m1 = np.array([[0.4122214708, 0.5363325363, 0.0514459929],
                   [0.2119034982, 0.6806995451, 0.1073969566],
                   [0.0883024619, 0.2817188376, 0.6299787005]])
    m2 = np.array([[0.2104542553, 0.7936177850, -0.0040720468],
                   [1.9779984951, -2.4285922050, 0.4505937099],
                   [0.0259040371, 0.7827717662, -0.8086757660]])
    return np.cbrt(c @ m1.T) @ m2.T


def palette_from_pak(base):
    pak = Path(base) / 'id1' / 'pak0.pak'
    data = pak.read_bytes()
    _, diroff, dirlen = struct.unpack_from('<4sii', data)
    for i in range(dirlen // 64):
        name, fo, fl = struct.unpack_from('<56sii', data, diroff + i * 64)
        if name.rstrip(b'\0') == b'gfx/palette.lmp':
            return data[fo:fo + fl]
    raise FileNotFoundError('gfx/palette.lmp not in ' + str(pak))


def blur2d(img, covered, sigma):
    """Gaussian blur within the covered atlas texels (normalised convolution)."""
    m = covered.astype(float)
    num = ndimage.gaussian_filter(img * m, sigma)
    den = ndimage.gaussian_filter(m, sigma)
    return num / np.maximum(den, 1e-9)


def glow_texels(rgb):
    """Same test as src/addons/toy_model.rs gel_glow."""
    r, g, b = (rgb[..., k].astype(int) for k in range(3))
    blue = (b > 90) & (b > r + 50) & (b > g + 10)
    orange = (r > 110) & (r > b + 70) & (r > g + 25) & (g > b)
    return blue | orange


def big_components(pts, sel, radius, min_count):
    """Mask of the `sel` points whose 3D connected component (links < radius)
    has at least min_count texels."""
    idx = np.flatnonzero(sel)
    out = np.zeros(len(pts), bool)
    if len(idx) == 0:
        return out
    tree = cKDTree(pts[idx])
    pairs = tree.query_pairs(radius, output_type='ndarray')
    g = sparse.csr_matrix((np.ones(len(pairs)), (pairs[:, 0], pairs[:, 1])), shape=(len(idx),) * 2)
    _, comp = sparse.csgraph.connected_components(g, directed=False)
    counts = np.bincount(comp)
    out[idx[counts[comp] >= min_count]] = True
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('dir', type=Path, help='asset directory with texture.rgb.zlib')
    ap.add_argument('--mdl', default=None, help='MDL file in DIR (default: the only *.mdl.zlib)')
    ap.add_argument('--from', dest='old', type=Path,
                    help='MDL the current texture was laid out for (tools/relayout_toy_uv.py '
                         'input): resample the texture into the layout of --mdl first')
    ap.add_argument('--chroma', type=float, default=5.0, help='chroma blur sigma, texels')
    ap.add_argument('--luma', type=float, default=3.0, help='luminance blur sigma, texels')
    ap.add_argument('--detail', type=float, default=0.5, help='fine luminance kept, 0..1')
    ap.add_argument('--gain', type=float, default=1.0, help='brightness multiplier before cleaning')
    ap.add_argument('--contrast', type=float, default=1.0, help='luminance contrast about the mean')
    ap.add_argument('--keep-mask', action='store_true')
    ap.add_argument('--keep-glow', action='store_true')
    ap.add_argument('--ramps', default='', help='allowed ramps (0-13), comma separated; default all')
    ap.add_argument('--ramp-map', default='', help='FROM:TO ramp remaps, comma separated')
    wg = ap.add_argument_group('wear (procedural, placed in 3D by the model shape; 0 = off)')
    wg.add_argument('--wear', type=float, default=0.0, help='chipped-edge lightening (OKLab L, ~0.1)')
    wg.add_argument('--edges', type=float, default=0.12, help='fraction of the surface on chipped edges')
    wg.add_argument('--edge-radius', type=float, default=0.35, help='edge detection radius, units')
    wg.add_argument('--edge-ramp', type=int, default=0, help='ramp of bare metal on chipped edges (-1 keep)')
    wg.add_argument('--grime', type=float, default=0.0, help='crease/cavity darkening (OKLab L)')
    wg.add_argument('--rust', type=float, default=0.0, help='fraction of the surface rusted')
    wg.add_argument('--rust-scale', type=float, default=2.5, help='rust patch size, units')
    wg.add_argument('--rust-light', type=float, default=0.38, help='rust lightness cap (OKLab L)')
    wg.add_argument('--rust-ramp', type=int, default=6, help='ramp for rust')
    wg.add_argument('--scratches', type=float, default=0.0, help='scratch density, 0..1')
    wg.add_argument('--grain', type=float, default=0.0, help='fine brushed grain (OKLab L)')
    wg.add_argument('--seed', type=int, default=1)
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
    pal = np.frombuffer(pal_bytes, np.uint8)[:224 * 3].reshape(14, 16, 3)

    tri_id, xyz, bary = rasterise(pos, st, tris, w, h)
    covered = tri_id >= 0
    if a.old:
        _, ost, otris, _ = load_mdl(zlib.decompress(a.old.read_bytes()))
        assert len(otris) == len(tris), 'the old and new MDL must share their triangles'
        old_id, _, _ = rasterise(pos, ost, otris, w, h)
        src = fill_gaps(rgb, old_id >= 0).astype(float)
        cy, cx = np.nonzero(covered)
        k = tri_id[cy, cx]
        u, v = bary[cy, cx, 0:1], bary[cy, cx, 1:2]
        t0, t1, t2 = (ost[otris[k, c]] for c in range(3))
        at = t0 + u * (t1 - t0) + v * (t2 - t0)
        rgb = np.zeros((h, w, 3), np.uint8)
        for c in range(3):
            rgb[cy, cx, c] = np.clip(np.round(ndimage.map_coordinates(
                src[..., c], [at[:, 1], at[:, 0]], order=1, mode='nearest')), 0, 255)
    tp = pos[tris]
    fn = np.cross(tp[:, 1] - tp[:, 0], tp[:, 2] - tp[:, 0])
    area3d = 0.5 * np.linalg.norm(fn, axis=1)
    fn /= np.maximum(2 * area3d[:, None], 1e-9)
    if (fn * (tp.mean(1) - pos.mean(0))).sum() < 0:  # make face normals point out
        fn = -fn
    texel = np.sqrt(area3d.sum() / covered.sum())  # world units per texel

    cy, cx = np.nonzero(covered)
    pts = xyz[cy, cx]
    col = rgb[cy, cx].astype(float)
    keep = np.zeros(len(pts), bool)
    if a.keep_mask:
        keep |= mask[cy, cx] != 0
    if a.keep_glow:
        keep |= big_components(pts, glow_texels(rgb)[cy, cx], 2.5 * texel, 400)
    work = ~keep
    p = pts[work]
    lab = srgb_to_oklab(np.clip(col[work] * a.gain, 0, 255))

    ab = blur3d(p, lab[:, 1:], a.chroma * texel)
    lo = blur3d(p, lab[:, 0], a.luma * texel)[:, 0]
    wmask = np.zeros((h, w), bool)
    wmask[cy[work], cx[work]] = True
    lgrid = np.zeros((h, w))
    lgrid[cy[work], cx[work]] = lab[:, 0]
    fine = blur2d(lgrid, wmask, 0.7)[cy[work], cx[work]]
    L = lo + a.detail * (fine - lo)
    L = L.mean() + a.contrast * (L - L.mean())
    L, edge, rust = wear(p, fn[tri_id[cy[work], cx[work]]], L, texel, a)

    # ramps in OKLab, each sorted by lightness
    rl = srgb_to_oklab(pal.reshape(-1, 3).astype(float)).reshape(14, 16, 3)
    order = np.argsort(rl[..., 0], axis=1)
    rl = np.take_along_axis(rl, order[..., None], 1)
    rgb_sorted = np.take_along_axis(pal, order[..., None], 1)
    score = np.empty((len(L), 14))
    for r in range(14):
        ra = np.interp(L, rl[r, :, 0], rl[r, :, 1])
        rb = np.interp(L, rl[r, :, 0], rl[r, :, 2])
        out = np.clip(L, rl[r, 0, 0], rl[r, -1, 0]) - L
        score[:, r] = (ab[:, 0] - ra) ** 2 + (ab[:, 1] - rb) ** 2 + 0.5 * out ** 2
    if a.ramps:
        allowed = [int(r) for r in a.ramps.split(',')]
        score[:, [r for r in range(14) if r not in allowed]] = np.inf
    label = score.argmin(1)
    votes = blur3d(p, np.eye(14)[label], 1.5 * a.chroma * texel)
    label = votes.argmax(1)
    for pair in filter(None, a.ramp_map.split(',')):
        src, dst = map(int, pair.split(':'))
        label[label == src] = dst
    if a.edge_ramp >= 0:
        label[edge > 0.5] = a.edge_ramp
    label[rust > 0.5] = a.rust_ramp
    ramp_l = rl[label, :, 0]
    pick = np.abs(ramp_l - L[:, None]).argmin(1)
    new = rgb_sorted[label, pick]
    if a.keep_glow:
        # the engine derives the glow layer from the texture: cleaned texels
        # (copper rust in particular) must not pass its test and light up
        while (bad := glow_texels(new[None])[0] & (pick > 0)).any():
            pick[bad] -= 1
            new = rgb_sorted[label, pick]

    out = rgb.copy()
    out[cy[work], cx[work]] = new
    out = fill_gaps(out, covered)

    used = np.bincount(label, minlength=14)
    print(f'{tex_path.relative_to(ROOT) if tex_path.is_relative_to(ROOT) else tex_path}: '
          f'{covered.sum()} covered texels ({texel:.3f} u each), {keep.sum()} kept, ramps used '
          + ', '.join(f'{r}:{c}' for r, c in enumerate(used) if c))
    if a.preview:
        from PIL import Image
        Image.fromarray(np.concatenate([rgb, out], 1)).save(a.preview)
    if not a.dry_run:
        tex_path.write_bytes(zlib.compress(out.tobytes(), 9))


if __name__ == '__main__':
    main()
