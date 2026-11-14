//! Gel splats merged into one contiguous coating per surface.
//!
//! Splats on the same plane whose discs touch form a cluster. Each cluster
//! gets a square height field: every splat adds a smooth kernel
//! (`(1 - d²/r²)²`), so overlapping splats sum into one coat instead of
//! stacking separate puddles, and the coat's edge is where the summed field
//! crosses a threshold, roughened by world-space noise so it stays organic
//! and lines up across clusters. RGB is the gel colour (whichever kind
//! contributes most there, blended at a blue/orange border), alpha the
//! height the gel shader turns into a raised glossy surface.
//!
//! All clusters are packed into one [`ATLAS`]² texture (shelf packing; a
//! cluster too large is sampled more coarsely), rebuilt when splats change.
use qcore::math::Vec3;

/// Atlas side in texels.
pub const ATLAS: usize = 2048;
/// Finest texel size in world units.
const TEXEL: f32 = 2.0;
/// Largest cluster side in texels before it is sampled more coarsely.
const MAX_SIDE: usize = 512;
/// Summed field where the coat's edge lies.
const EDGE: f32 = 0.3;

pub const COLORS: [[f32; 3]; 2] = [[0.14, 0.5, 1.0], [1.0, 0.47, 0.06]];

#[derive(Clone, Copy, Debug)]
pub struct Disc {
    pub kind: u8,
    pub origin: Vec3,
    pub normal: Vec3,
    pub radius: f32,
}

/// One cluster: a square on its plane and where it sits in the atlas.
#[derive(Clone, Debug)]
pub struct Cluster {
    pub center: Vec3,
    pub normal: Vec3,
    /// Half the square's side in world units.
    pub half: f32,
    /// Atlas square: texel x, y and side.
    pub x: usize,
    pub y: usize,
    pub side: usize,
}

impl Cluster {
    /// Square uv (0..1, as `mesh::project_shaped` with `half`) to atlas uv.
    pub fn atlas_uv(&self, uv: [f32; 2]) -> [f32; 2] {
        let s = self.side as f32;
        let a = ATLAS as f32;
        [(self.x as f32 + 0.5 + uv[0].clamp(0.0, 1.0) * (s - 1.0)) / a, (self.y as f32 + 0.5 + uv[1].clamp(0.0, 1.0) * (s - 1.0)) / a]
    }
}

fn same_plane(a: &Disc, b: &Disc) -> bool {
    a.normal.dot(b.normal) > 0.98 && (b.origin - a.origin).dot(a.normal).abs() < 2.0
}

/// Groups discs into clusters (indices into `discs`).
pub fn group(discs: &[Disc]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..discs.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..discs.len() {
        for j in i + 1..discs.len() {
            let (a, b) = (&discs[i], &discs[j]);
            if same_plane(a, b) && (a.origin - b.origin).length() < a.radius + b.radius + 4.0 {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut slot = vec![usize::MAX; discs.len()];
    for i in 0..discs.len() {
        let r = root(&mut parent, i);
        if slot[r] == usize::MAX {
            slot[r] = out.len();
            out.push(Vec::new());
        }
        out[slot[r]].push(i);
    }
    out
}

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise in world space (0..1).
fn noise(p: Vec3) -> f32 {
    let f = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
    let t = p - f;
    let s = |v: f32| v * v * (3.0 - 2.0 * v);
    let (sx, sy, sz) = (s(t.x), s(t.y), s(t.z));
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| hash3(x + dx, y + dy, z + dz);
    lerp(
        lerp(lerp(c(0, 0, 0), c(1, 0, 0), sx), lerp(c(0, 1, 0), c(1, 1, 0), sx), sy),
        lerp(lerp(c(0, 0, 1), c(1, 0, 1), sx), lerp(c(0, 1, 1), c(1, 1, 1), sx), sy),
        sz,
    )
}

/// Height of the coat from the summed field: a steep rounded rim, then a
/// gently domed top with a slight ripple.
fn height(field: f32, p: Vec3) -> f32 {
    let edge = EDGE + (noise(p / 14.0) - 0.5) * 0.22 + (noise(p / 5.0) - 0.5) * 0.08;
    let x = field - edge;
    if x <= 0.0 {
        return 0.0;
    }
    let rim = (x / 0.12).min(1.0);
    let body = (x / 1.2).min(1.0);
    let ripple = (noise(p / 9.0) - 0.5) * 0.1;
    (rim * rim * (3.0 - 2.0 * rim) * 0.35 + (1.0 - (1.0 - body).powi(3)) * 0.55 + ripple * rim).clamp(0.0, 1.0)
}

/// Builds the atlas: places each cluster and fills its texels.
/// Returns the clusters (parallel to `groups`) and the RGBA texels.
pub fn build(discs: &[Disc], groups: &[Vec<usize>]) -> (Vec<Option<Cluster>>, Vec<u8>) {
    let mut out = vec![0u8; ATLAS * ATLAS * 4];
    // squares on each plane, in world units
    let squares: Vec<(Vec3, Vec3, f32)> = groups
        .iter()
        .map(|g| {
            let n = discs[g[0]].normal;
            let (u, v) = crate::blood::mesh::axes(n);
            let o = discs[g[0]].origin;
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for &i in g {
                let d = discs[i].origin - o;
                let (a, b, r) = (d.dot(u), d.dot(v), discs[i].radius + 4.0);
                lo = [lo[0].min(a - r), lo[1].min(b - r)];
                hi = [hi[0].max(a + r), hi[1].max(b + r)];
            }
            let center = o + u * ((lo[0] + hi[0]) * 0.5) + v * ((lo[1] + hi[1]) * 0.5);
            (center, n, (hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.5)
        })
        .collect();
    // texel size: as fine as fits (coarser for big clusters / a full atlas)
    let mut texel = TEXEL;
    let placed = loop {
        let sides: Vec<usize> = squares.iter().map(|s| ((s.2 * 2.0 / texel).ceil() as usize + 2).clamp(4, MAX_SIDE)).collect();
        if let Some(p) = pack(&sides) {
            break p.into_iter().zip(sides).map(|((x, y), s)| Some((x, y, s))).collect::<Vec<_>>();
        }
        texel *= 1.5;
        if texel > 64.0 {
            break vec![None; groups.len()];
        }
    };
    let mut clusters = Vec::with_capacity(groups.len());
    for (k, g) in groups.iter().enumerate() {
        let Some((x0, y0, side)) = placed[k] else {
            clusters.push(None);
            continue;
        };
        let (center, n, half) = squares[k];
        let c = Cluster { center, normal: n, half, x: x0, y: y0, side };
        let (u, v) = crate::blood::mesh::axes(n);
        // world position of texel (i, j), matching project_shaped's uv
        let world = |i: usize, j: usize| {
            let (a, b) = (i as f32 / (side - 1) as f32, j as f32 / (side - 1) as f32);
            center + u * ((a - 0.5) * 2.0 * half) + v * ((b - 0.5) * 2.0 * half)
        };
        let step = 2.0 * half / (side - 1) as f32;
        let mut field = vec![[0.0f32; 2]; side * side];
        for &i in g {
            let d = &discs[i];
            let rel = d.origin - center;
            let (ca, cb) = (rel.dot(u) / step + (side - 1) as f32 * 0.5, rel.dot(v) / step + (side - 1) as f32 * 0.5);
            let rt = d.radius / step;
            let (i0, i1) = (((ca - rt).floor().max(0.0)) as usize, ((ca + rt).ceil() as usize).min(side - 1));
            let (j0, j1) = (((cb - rt).floor().max(0.0)) as usize, ((cb + rt).ceil() as usize).min(side - 1));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let q = ((i as f32 - ca).powi(2) + (j as f32 - cb).powi(2)) / (rt * rt);
                    if q < 1.0 {
                        field[j * side + i][d.kind as usize] += (1.0 - q) * (1.0 - q);
                    }
                }
            }
        }
        for j in 0..side {
            for i in 0..side {
                let [b, o] = field[j * side + i];
                let h = height(b + o, world(i, j));
                if h <= 0.0 {
                    continue;
                }
                let w = if b + o > 0.0 { o / (b + o) } else { 0.0 };
                let w = ((w - 0.5) * 6.0 + 0.5).clamp(0.0, 1.0);
                let rgb = [0, 1, 2].map(|ch| COLORS[0][ch] * (1.0 - w) + COLORS[1][ch] * w);
                let px = ((y0 + j) * ATLAS + x0 + i) * 4;
                out[px..px + 4].copy_from_slice(&[(rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8, (h * 255.0) as u8]);
            }
        }
        clusters.push(Some(c));
    }
    (clusters, out)
}

/// Shelf packing of squares into the atlas (a texel of space between).
fn pack(sides: &[usize]) -> Option<Vec<(usize, usize)>> {
    let mut order: Vec<usize> = (0..sides.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sides[i]));
    let mut pos = vec![(0, 0); sides.len()];
    let (mut x, mut y, mut shelf) = (0, 0, 0);
    for i in order {
        let s = sides[i] + 1;
        if x + s > ATLAS {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        if y + s > ATLAS {
            return None;
        }
        pos[i] = (x, y);
        x += s;
        shelf = shelf.max(s);
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disc(kind: u8, x: f32, y: f32) -> Disc {
        Disc { kind, origin: Vec3::new(x, y, 0.0), normal: Vec3::new(0.0, 0.0, 1.0), radius: 30.0 }
    }

    #[test]
    fn overlapping_splats_merge_into_one_coat() {
        let discs = [disc(0, 0.0, 0.0), disc(0, 40.0, 0.0), disc(1, 80.0, 0.0), disc(0, 400.0, 0.0)];
        let groups = group(&discs);
        assert_eq!(groups.len(), 2, "{groups:?}");
        let (clusters, tex) = build(&discs, &groups);
        let c = clusters[0].as_ref().unwrap();
        let alpha = |w: Vec3| {
            let (u, v) = crate::blood::mesh::axes(c.normal);
            let d = w - c.center;
            let uv = [0.5 + d.dot(u) / (2.0 * c.half), 0.5 + d.dot(v) / (2.0 * c.half)];
            let a = c.atlas_uv(uv);
            let (i, j) = ((a[0] * ATLAS as f32) as usize, (a[1] * ATLAS as f32) as usize);
            tex[(j * ATLAS + i) * 4 + 3]
        };
        // the gap between two neighbouring splat centres is fully coated,
        // with no dip at their seam
        let mid = alpha(Vec3::new(20.0, 0.0, 0.0));
        assert!(mid > 100, "seam {mid}");
        assert!(alpha(Vec3::new(0.0, 0.0, 0.0)) > 100);
        assert_eq!(alpha(Vec3::new(0.0, 60.0, 0.0)), 0, "outside");
    }

    #[test]
    fn walls_and_floors_are_separate_clusters() {
        let wall = Disc { kind: 0, origin: Vec3::new(10.0, 0.0, 20.0), normal: Vec3::new(1.0, 0.0, 0.0), radius: 30.0 };
        assert_eq!(group(&[disc(0, 0.0, 0.0), wall]).len(), 2);
    }
}
