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
