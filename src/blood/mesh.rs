//! Clipped mesh patches with barycentric attachment to animated triangles.
use qcore::math::Vec3;
use qformats::Mdl;
use qrender::RenderEntity;

#[derive(Clone, Copy)]
pub struct Point {
    pub pos: Vec3,
    pub uv: [f32; 2],
    pub bary: Vec3,
}

pub struct Patch {
    pub tri: [usize; 3],
    pub side: f32,
    pub points: Vec<Point>,
}

pub fn clip(mut poly: Vec<Point>) -> Vec<Point> {
    for axis in 0..2 {
        for bound in [0.0, 1.0] {
            let mut out = Vec::new();
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                let d = |p: Point| {
                    if bound == 0.0 {
                        p.uv[axis]
                    } else {
                        1.0 - p.uv[axis]
                    }
                };
                let (da, db) = (d(a), d(b));
                if da >= 0.0 {
                    out.push(a);
                }
                if (da >= 0.0) != (db >= 0.0) {
                    let t = da / (da - db);
                    out.push(Point {
                        pos: a.pos.lerp(b.pos, t),
                        bary: a.bary.lerp(b.bary, t),
                        uv: std::array::from_fn(|k| a.uv[k] + (b.uv[k] - a.uv[k]) * t),
                    });
                }
            }
            poly = out;
        }
    }
    poly
}

pub fn axes(n: Vec3) -> (Vec3, Vec3) {
    let u = n
        .cross(if n.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        })
        .normalize();
    (u, n.cross(u))
}

#[cfg(test)]
pub fn project(poly: &[Vec3], center: Vec3, n: Vec3, size: f32) -> Vec<Point> {
    project_shaped(poly, center, n, size, [0.0, 1.0])
}

pub fn project_shaped(
    poly: &[Vec3],
    center: Vec3,
    n: Vec3,
    size: f32,
    shape: [f32; 2],
) -> Vec<Point> {
    let (u, v) = axes(n);
    let (sn, cs) = shape[0].sin_cos();
    let (u, v) = ((u * cs + v * sn) / shape[1], v * cs - u * sn);
    clip(
        poly.iter()
            .enumerate()
            .map(|(i, &p)| {
                let d = p - center;
                let mut bary = Vec3::ZERO;
                if i < 3 {
                    bary[i] = 1.0;
                }
                Point {
                    pos: p,
                    uv: [0.5 + d.dot(u) / (size * 2.0), 0.5 + d.dot(v) / (size * 2.0)],
                    bary,
                }
            })
            .collect(),
    )
}

pub fn positions(m: &Mdl, e: &RenderEntity, dynverts: &[u32], time: f64) -> Vec<Vec3> {
    if let Some(d) = e.dyn_pose {
        return (0..m.num_verts)
            .map(|i| {
                let p = dynverts.get(d.ofs as usize + i).copied().unwrap_or(0);
                Vec3::from(d.origin)
                    + Vec3::new(
                        (p & 255) as f32 * d.scale[0],
                        ((p >> 8) & 255) as f32 * d.scale[1],
                        ((p >> 16) & 255) as f32 * d.scale[2],
                    )
            })
            .collect();
    }
    let pose = |f: u32| {
        let f = &m.frames[(f as usize).min(m.frames.len() - 1)];
        let t = time + e.syncbase as f64;
        let full = f.intervals.last().copied().unwrap_or(0.1) as f64;
        let t = t.rem_euclid(full.max(0.001));
        let i = f
            .intervals
            .iter()
            .position(|&iv| t < iv as f64)
            .unwrap_or(0)
            .min(f.poses.len() - 1);
        &f.poses[i]
    };
    let (a, b) = (pose(e.prev_frame), pose(e.frame));
    let (f, r, u) = qcore::math::angle_vectors(Vec3::new(-e.angles[0], e.angles[1], e.angles[2]));
    (0..m.num_verts)
        .map(|i| {
            let p = Vec3::new(
                a.verts[i].v[0] as f32,
                a.verts[i].v[1] as f32,
                a.verts[i].v[2] as f32,
            )
            .lerp(
                Vec3::new(
                    b.verts[i].v[0] as f32,
                    b.verts[i].v[1] as f32,
                    b.verts[i].v[2] as f32,
                ),
                e.lerp,
            );
            let p = Vec3::from(m.scale_origin)
                + Vec3::new(p.x * m.scale[0], p.y * m.scale[1], p.z * m.scale[2]);
            Vec3::from(e.origin) + f * p.x - r * p.y + u * p.z
        })
        .collect()
}

pub fn ray(a: Vec3, b: Vec3, p: [Vec3; 3]) -> Option<(f32, Vec3)> {
    let d = b - a;
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-6 {
        return None;
    }
    let s = a - p[0];
    let u = s.dot(h) / det;
    let q = s.cross(e1);
    let v = d.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && (0.0..=1.0).contains(&t))
        .then_some((t, e1.cross(e2).normalize()))
}

/// Closest skin point for QC damage against a collision box. Animation can
/// leave empty space on the firing line even though the server dealt damage.
pub fn closest_triangle(p: Vec3, tri: [Vec3; 3]) -> Vec3 {
    let [a, b, c] = tri;
    let n = (b - a).cross(c - a);
    let area = n.length_sq();
    if area > 1e-8 {
        let q = p - n * ((p - a).dot(n) / area);
        if [(a, b), (b, c), (c, a)]
            .iter()
            .all(|&(u, v)| (v - u).cross(q - u).dot(n) >= -1e-6)
        {
            return q;
        }
    }
    [(a, b), (b, c), (c, a)]
        .into_iter()
        .map(|(u, v)| {
            let d = v - u;
            let t = if d.length_sq() > 1e-8 {
                (p - u).dot(d) / d.length_sq()
            } else {
                0.0
            };
            u + d * t.clamp(0.0, 1.0)
        })
        .min_by(|a, b| (*a - p).length_sq().total_cmp(&(*b - p).length_sq()))
        .unwrap_or(a)
}

const DEATH_NAMES: [&[u8]; 5] = [b"death", b"deth", b"dth", b"die", b"dead"];

/// ASCII case-insensitive death-pose name check across id1 and mod monsters.
pub(crate) fn is_death_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    DEATH_NAMES.iter().any(|needle| {
        bytes
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
    })
}

/// True for `.mdl` asset paths (excludes inline brush `*N`, sprites, and BSPs).
pub(crate) fn is_mdl_path(name: &str) -> bool {
    !name.starts_with('*')
        && name.len() >= 4
        && name.as_bytes()[name.len() - 4..].eq_ignore_ascii_case(b".mdl")
}

/// Squared Euclidean distance from `p` to closed line segment `[a, b]`.
pub(crate) fn point_segment_dist_sq(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= 1e-8 {
        return (p - a).length_sq();
    }
    let t = (p - a).dot(ab) / len2;
    (p - (a + ab * t.clamp(0.0, 1.0))).length_sq()
}

/// Cached alias model metadata used to skip non-flesh models, per-frame string
/// scans, and vertex transformation when an entity cannot reach a ray or hit.
pub(crate) struct CachedMdl {
    pub mdl: Mdl,
    pub is_flesh: bool,
    pub has_death_pose: bool,
    pub is_head: bool,
    pub is_gib_or_head: bool,
    pub death_frames: Vec<bool>,
    pub radius: f32,
}

impl CachedMdl {
    pub(crate) fn from_mdl(mdl: Mdl, is_head: bool) -> Self {
        let death_frames: Vec<bool> = mdl
            .frames
            .iter()
            .map(|f| f.poses.iter().any(|p| is_death_name(&p.name)))
            .collect();
        let has_death_pose = death_frames.iter().any(|&d| d);
        let is_gib = mdl.flags & (4 | 32) != 0;
        let mut max_r2 = 0.0_f32;
        let so = Vec3::from(mdl.scale_origin);
        let sc = Vec3::from(mdl.scale);
        for f in &mdl.frames {
            for p in &f.poses {
                for v in &p.verts {
                    let pos = so
                        + Vec3::new(
                            v.v[0] as f32 * sc.x,
                            v.v[1] as f32 * sc.y,
                            v.v[2] as f32 * sc.z,
                        );
                    max_r2 = max_r2.max(pos.length_sq());
                }
            }
        }
        Self {
            mdl,
            is_flesh: is_gib || has_death_pose,
            has_death_pose,
            is_head,
            is_gib_or_head: is_gib || is_head,
            death_frames,
            radius: max_r2.sqrt(),
        }
    }

    pub(crate) fn is_corpse(&self, e: &RenderEntity) -> bool {
        e.dyn_pose.is_some()
            || self.mdl.flags & (4 | 32) != 0
            || self
                .death_frames
                .get(e.frame as usize)
                .copied()
                .unwrap_or(false)
    }

    /// Conservative world-space bounding radius around `e.origin`.
    pub(crate) fn entity_radius(&self, e: &RenderEntity) -> f32 {
        match e.dyn_pose {
            Some(d) if d.radius > 0.0 => d.radius + 1.0,
            Some(_) => f32::INFINITY,
            None => self.radius + 1.0,
        }
    }
}
