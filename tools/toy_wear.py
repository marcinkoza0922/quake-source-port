"""3D-space helpers for tools/clean_toy_texture.py: voxel blur, value noise
and procedural wear (chipped edges, cavity grime, rust patches and drips,
scratches) placed by the model's shape rather than its atlas layout."""
import numpy as np
from scipy import ndimage


def blur3d(pts, values, sigma):
    """Gaussian blur of per-texel values over 3D space (normalised convolution
    on a voxel grid of sigma/2), sampled back at the texels."""
    vox = sigma / 2
    g = (pts - pts.min(0)) / vox + 3
    shape = tuple(np.ceil(g.max(0)).astype(int) + 4)
    values = values.reshape(len(pts), -1)
    i0 = np.floor(g).astype(int)
    f = g - i0
    out = np.empty_like(values, dtype=np.float64)
    weight = np.zeros(shape, np.float32)
    grids = [np.zeros(shape, np.float32) for _ in range(values.shape[1])]
    for corner in range(8):
        o = np.array([(corner >> k) & 1 for k in range(3)])
        wgt = np.prod(np.where(o, f, 1 - f), axis=1)
        idx = tuple((i0 + o).T)
        np.add.at(weight, idx, wgt)
        for grid, v in zip(grids, values.T):
            np.add.at(grid, idx, wgt * v)
    weight = ndimage.gaussian_filter(weight, 2.0, truncate=2.5)
    wsample = ndimage.map_coordinates(weight, g.T, order=1)
    for k, grid in enumerate(grids):
        grid = ndimage.gaussian_filter(grid, 2.0, truncate=2.5)
        out[:, k] = ndimage.map_coordinates(grid, g.T, order=1) / np.maximum(wsample, 1e-9)
    return out


def _hash(ix, iy, iz, seed):
    h = (ix * 73856093) ^ (iy * 19349663) ^ (iz * 83492791) ^ (seed * 2654435761)
    h = (h ^ (h >> 13)) * 1274126177
    h = h ^ (h >> 16)
    return (h & 0xFFFF) / 32767.5 - 1.0


def noise(p, seed=0):
    """Smooth value noise in [-1, 1] at points p (lattice spacing 1)."""
    i0 = np.floor(p).astype(np.int64)
    f = p - i0
    f = f * f * (3 - 2 * f)
    out = np.zeros(len(p))
    for corner in range(8):
        o = np.array([(corner >> k) & 1 for k in range(3)])
        wgt = np.prod(np.where(o, f, 1 - f), axis=1)
        c = i0 + o
        out += wgt * _hash(c[:, 0], c[:, 1], c[:, 2], seed)
    return out


def fbm(p, scale, octaves=4, seed=0):
    """Fractal value noise, features about `scale` units across, in ~[-1, 1]."""
    total, amp, norm = np.zeros(len(p)), 1.0, 0.0
    for o in range(octaves):
        total += amp * noise(p / scale * 2 ** o, seed + 101 * o)
        norm += amp
        amp *= 0.5
    return total / norm


def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1)
    return t * t * (3 - 2 * t)


def convexity(p, n, radius):
    """Positive on outside edges and corners, negative in creases and
    cavities, ~0 on flat surfaces: how far a texel stands out from its 3D
    neighbourhood along its face normal, in units of radius."""
    mean = blur3d(p, p, radius)
    return ((p - mean) * n).sum(1) / radius


def rotation(rng):
    q = rng.normal(size=4)
    w, x, y, z = q / np.linalg.norm(q)
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def scratches(p, density, texel, seed):
    """Straight scratch lines about a texel wide (thinner ones would break
    into dots once quantised) in a few random directions, gathered in
    patches; 0..1."""
    rng = np.random.default_rng(seed)
    out = np.zeros(len(p))
    for k in range(4):
        q = p @ rotation(rng).T
        # long along q.x, very thin across: ridges of stretched noise
        across = 0.045 / texel
        ridge = 1 - np.abs(noise(q * np.array([across / 25, across, across]), seed + 7 + k))
        lines = smoothstep(0.93, 0.985, ridge)
        patch = smoothstep(0.1, 0.45, fbm(p, 5.0, 2, seed + 31 + k) + density - 0.5)
        out = np.maximum(out, lines * patch)
    return out


def top(x, fraction, soft=0.25):
    """0..1 mask of the highest `fraction` of x, with a soft edge of
    `soft` standard deviations."""
    if fraction <= 0:
        return np.zeros(len(x))
    t = np.percentile(x, 100 * (1 - fraction))
    w = soft * x.std() + 1e-9
    return smoothstep(t - w, t + w, x)


def wear(p, n, L, texel, a):
    """Procedural wear masks and the adjusted lightness/chroma.

    Returns (L, edge, rust): edge and rust are 0..1 masks that the caller
    turns into bare-metal and rust ramps; scratches, grime and grain only
    change lightness, so they stay within the texel's material ramp. Coverages are fractions of the
    surface, so the same settings suit small and large models."""
    fine = fbm(p, 0.7, 3, a.seed)
    conv = convexity(p, n, a.edge_radius)
    cs = conv.std() + 1e-9
    # chipped paint on outer edges, broken up so it is not a clean bevel; only
    # where the surface really turns (rounded bodies have no edge to chip)
    sharp = smoothstep(0.8, 1.5, conv / cs)
    edge = top(conv / cs + 0.5 * fine + 0.35 * fbm(p, 3.0, 2, a.seed + 5), a.edges) * sharp
    cavity = top(-conv / cs + 0.5 * fine, 0.2, soft=0.6)
    # rust: blotches that favour creases, stretched downward (+Z is up) as drips
    drip = fbm(p * np.array([1.0, 1.0, 0.3]), a.rust_scale, 4, a.seed + 9)
    blot = fbm(p, a.rust_scale, 4, a.seed + 13)
    rust = top(0.5 * blot + 0.4 * drip + 0.35 * cavity + 0.3 * fine, a.rust, soft=0.12)
    scr = scratches(p, a.scratches, texel, a.seed + 17) if a.scratches > 0 else np.zeros(len(p))
    grain = fbm(p * np.array([0.25, 1.0, 1.0]), 0.5, 2, a.seed + 23)

    L = L - a.grime * cavity + a.wear * edge + 0.06 * (scr > 0.5) * (1 - rust) + a.grain * grain
    # rust is dark whatever it grows on (light steel would make it orange)
    rust_l = np.minimum(L, a.rust_light) - 0.04 * (1 + fine)
    L = L + rust * (rust_l - L)
    return L, edge * (1 - rust), rust
