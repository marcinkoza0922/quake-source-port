//! Original low-poly induction projector: Quake-scale iron, bronze and copper.
use super::portal_skin::{self as skin, Material as Mat};
use qcore::math::Vec3;
use qformats::mdl::{Frame, Mdl, Pose, StVert, TriVert, Triangle};

#[derive(Default)]
struct Mesh {
    points: Vec<[f32; 3]>,
    st: Vec<StVert>,
    triangles: Vec<Triangle>,
}

impl Mesh {
    fn quad(&mut self, p: [[f32; 3]; 4], material: Mat) {
        self.quad_uv(p, material, None);
    }
    fn quad_uv(&mut self, p: [[f32; 3]; 4], material: Mat, bounds: Option<([f32; 2], [f32; 2])>) {
        let base = self.points.len() as i32;
        let normal =
            (Vec3::from(p[1]) - Vec3::from(p[0])).cross(Vec3::from(p[2]) - Vec3::from(p[0]));
        // Dominant-plane projection keeps each face's texture properly oriented.
        let axes = if normal.x.abs() > normal.y.abs().max(normal.z.abs()) {
            [1, 2]
        } else if normal.y.abs() > normal.z.abs() {
            [0, 2]
        } else {
            [0, 1]
        };
        let lo: [f32; 2] = axes.map(|a| p.iter().map(|v| v[a]).fold(f32::INFINITY, f32::min));
        let hi: [f32; 2] = axes.map(|a| p.iter().map(|v| v[a]).fold(f32::NEG_INFINITY, f32::max));
        let (lo, hi) = bounds.unwrap_or((lo, hi));
        let (s, t) = material.tile();
        self.st.extend(p.iter().map(|v| {
            let uv: [i32; 2] = std::array::from_fn(|i| {
                (1.0 + 29.0 * (v[axes[i]] - lo[i]) / (hi[i] - lo[i]).max(0.01)).round() as i32
            });
            StVert {
                onseam: 0,
                s: s + uv[0],
                t: t + 31 - uv[1],
            }
        }));
        self.points.extend(p);
        self.triangles.push(Triangle {
            faces_front: 1,
            verts: [base, base + 1, base + 2],
        });
        if p[0] != p[3] {
            self.triangles.push(Triangle {
                faces_front: 1,
                verts: [base, base + 2, base + 3],
            });
        }
    }
    fn block(&mut self, a: [f32; 3], b: [f32; 3], m: Mat) {
        let p = |x, y, z| {
            [
                if x { b[0] } else { a[0] },
                if y { b[1] } else { a[1] },
                if z { b[2] } else { a[2] },
            ]
        };
        for f in [
            [
                p(false, false, false),
                p(false, false, true),
                p(false, true, true),
                p(false, true, false),
            ],
            [
                p(true, false, false),
                p(true, true, false),
                p(true, true, true),
                p(true, false, true),
            ],
            [
                p(false, false, false),
                p(true, false, false),
                p(true, false, true),
                p(false, false, true),
            ],
            [
                p(false, true, false),
                p(false, true, true),
                p(true, true, true),
                p(true, true, false),
            ],
            [
                p(false, false, false),
                p(false, true, false),
                p(true, true, false),
                p(true, false, false),
            ],
            [
                p(false, false, true),
                p(true, false, true),
                p(true, true, true),
                p(false, true, true),
            ],
        ] {
            self.quad(f, m);
        }
    }
    /// A tapered octagonal receiver, with real bevels rather than square boxes.
    fn housing(&mut self, back: [f32; 4], front: [f32; 4], m: Mat) {
        let section = |[x, w, bottom, top]: [f32; 4]| {
            let b = 0.9_f32.min(w * 0.4).min((top - bottom) * 0.4);
            [
                [x, -w + b, bottom],
                [x, w - b, bottom],
                [x, w, bottom + b],
                [x, w, top - b],
                [x, w - b, top],
                [x, -w + b, top],
                [x, -w, top - b],
                [x, -w, bottom + b],
            ]
        };
        let (a, b) = (section(back), section(front));
        for i in 0..8 {
            let j = (i + 1) % 8;
            self.quad(
                [a[i], a[j], b[j], b[i]],
                if i % 2 == 1 { Mat::Edge } else { m },
            );
        }
        for i in 1..7 {
            self.quad_uv(
                [b[0], b[i], b[i + 1], b[0]],
                m,
                Some(([-front[1], front[2]], [front[1], front[3]])),
            );
            self.quad_uv(
                [a[0], a[i + 1], a[i], a[0]],
                m,
                Some(([-back[1], back[2]], [back[1], back[3]])),
            );
        }
    }
    /// Hollow eight-sided sleeve along the barrel axis, with contrasting bevels.
    fn sleeve(&mut self, x0: f32, x1: f32, outer: f32, inner: f32, m: Mat) {
        let p = |x, r: f32, i: usize| {
            let t = (i as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            [x, r * t.cos(), r * t.sin()]
        };
        for i in 0..8 {
            let j = i + 1;
            self.quad(
                [
                    p(x0, outer, i),
                    p(x0, outer, j),
                    p(x1, outer, j),
                    p(x1, outer, i),
                ],
                m,
            );
            self.quad(
                [
                    p(x0, inner, j),
                    p(x0, inner, i),
                    p(x1, inner, i),
                    p(x1, inner, j),
                ],
                Mat::Dark,
            );
            self.quad(
                [
                    p(x1, inner, i),
                    p(x1, outer, i),
                    p(x1, outer, j),
                    p(x1, inner, j),
                ],
                Mat::Edge,
            );
            self.quad(
                [
                    p(x0, outer, i),
                    p(x0, inner, i),
                    p(x0, inner, j),
                    p(x0, outer, j),
                ],
                Mat::Edge,
            );
        }
    }
}

pub fn model(palette: &[u8]) -> Mdl {
    let mut mesh = Mesh::default();
    // +X is forward. Broad heavy breech, stepped armor and a narrower emitter.
    mesh.housing([-9.0, 4.0, -3.8, 1.6], [4.0, 3.8, -3.1, 1.5], Mat::Iron);
    mesh.housing([-9.8, 4.15, -3.4, 1.3], [-8.5, 4.15, -3.4, 1.3], Mat::Dark);
    mesh.housing([-6.7, 3.25, 1.0, 2.7], [2.0, 2.8, 1.0, 2.5], Mat::Panel);
    mesh.sleeve(3.5, 18.5, 2.4, 1.9, Mat::Dark);
    mesh.sleeve(4.0, 6.0, 3.8, 2.4, Mat::Copper);
    mesh.sleeve(18.0, 20.5, 3.7, 2.3, Mat::Iron);
    // Recessed colored coils, separated by thick, tarnished induction collars.
    mesh.sleeve(7.0, 16.7, 3.05, 2.6, Mat::Core);
    mesh.block([7.5, -0.55, 2.7], [16.0, 0.55, 3.6], Mat::Core);
    for x in [7.0, 10.0, 13.0, 16.0] {
        mesh.sleeve(x, x + 1.0, 3.5, 2.7, Mat::Copper);
        mesh.block([x, -0.8, 3.1], [x + 1.0, 0.8, 3.75], Mat::Copper);
    }
    for side in [-1.0, 1.0] {
        let y = side * 4.1;
        // Squat armored emitter jaws replace the long white plastic forks.
        let start = mesh.points.len();
        mesh.housing([3.0, 1.0, -2.0, 0.4], [21.5, 0.7, -1.5, 0.1], Mat::Iron);
        for p in &mut mesh.points[start..] {
            p[1] += y;
        }
        mesh.block([5.0, y - 1.15, -2.25], [7.0, y + 1.15, 0.6], Mat::Copper);
        mesh.block([19.0, y - 0.75, -1.35], [22.5, y + 0.75, 0.25], Mat::Edge);
        mesh.block(
            [-6.7, side * 3.9 - 0.12, -1.3],
            [1.5, side * 3.9 + 0.12, 0.8],
            Mat::Vent,
        );
        mesh.block(
            [-5.5, side * 2.8 - 0.4, 2.0],
            [-3.8, side * 2.8 + 0.4, 2.8],
            Mat::Copper,
        );
    }
    mesh.block([-6.0, -2.1, -7.5], [-2.0, 2.1, -3.0], Mat::Grip);
    mesh.block([-2.5, -0.7, 2.5], [3.0, 0.7, 3.0], Mat::Dark);
    mesh.block([-1.6, -0.45, 3.0], [0.1, 0.45, 3.25], Mat::Core);
    // An iron sight and black slot on the breech keep the center clear.
    mesh.block([-8.6, -0.85, 1.3], [-7.4, 0.85, 2.5], Mat::Iron);
    mesh.block([-8.65, -0.3, 2.0], [-8.55, 0.3, 2.55], Mat::Dark);
    let pack = |p: [f32; 3], normal| TriVert {
        v: p.map(|v| ((v + 16.0) * 6.0).round() as u8),
        normal,
    };
    let mut verts = Vec::new();
    for face in mesh.points.as_chunks::<4>().0 {
        let n = (Vec3::from(face[1]) - Vec3::from(face[0]))
            .cross(Vec3::from(face[2]) - Vec3::from(face[0]))
            .normalize();
        let normal = qcore::anorms::AVERTEXNORMALS
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| n.dot(Vec3::from(**a)).total_cmp(&n.dot(Vec3::from(**b))))
            .map_or(0, |(i, _)| i as u8);
        verts.extend(face.iter().map(|p| pack(*p, normal)));
    }
    let min = pack(
        std::array::from_fn(|i| {
            mesh.points
                .iter()
                .map(|p| p[i])
                .fold(f32::INFINITY, f32::min)
        }),
        0,
    );
    let max = pack(
        std::array::from_fn(|i| {
            mesh.points
                .iter()
                .map(|p| p[i])
                .fold(f32::NEG_INFINITY, f32::max)
        }),
        0,
    );
    Mdl {
        scale: [1.0 / 6.0; 3],
        scale_origin: [-16.0; 3],
        bounding_radius: 26.0,
        eye_position: [0.0; 3],
        skin_width: skin::WIDTH as u32,
        skin_height: skin::HEIGHT as u32,
        num_verts: verts.len(),
        synctype: 0,
        flags: 0,
        size: 1.0,
        skins: skin::skins(palette),
        stverts: mesh.st,
        triangles: mesh.triangles,
        frames: vec![Frame {
            bboxmin: min,
            bboxmax: max,
            intervals: vec![],
            poses: vec![Pose {
                name: "inductor".into(),
                bboxmin: min,
                bboxmax: max,
                verts,
            }],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_uvs_and_eight_colored_skins_are_valid() {
        let mut palette = vec![0; 768];
        for i in 0..8 {
            palette[i * 3..i * 3 + 3]
                .copy_from_slice(&crate::addons::portal_draw::colour(i / 2, i % 2));
        }
        let m = model(&palette);
        assert_eq!(m.skins.len(), 8);
        for (i, skin) in m.skins.iter().enumerate() {
            assert_eq!(skin.images[0].len(), skin::WIDTH * skin::HEIGHT);
            let (x, y) = Mat::Core.tile();
            assert_eq!(
                skin.images[0][(y as usize + 16) * skin::WIDTH + x as usize + 16],
                i as u8
            );
        }
        assert_eq!(m.num_verts, m.stverts.len());
        assert!(m.stverts.iter().all(|v| v.s >= 0
            && (v.s as u32) < m.skin_width
            && v.t >= 0
            && (v.t as u32) < m.skin_height));
        assert!(m.triangles.iter().all(|t| {
            t.verts
                .iter()
                .all(|&i| i >= 0 && (i as usize) < m.num_verts)
        }));
        for pose in &m.frames[0].poses {
            assert_eq!(pose.verts.len(), m.num_verts);
            assert!(
                pose.verts
                    .iter()
                    .all(|v| (v.normal as usize) < qcore::anorms::NUMVERTEXNORMALS)
            );
            assert!(pose.verts.iter().all(|v| {
                (0..3).all(|i| {
                    v.v[i] >= m.frames[0].bboxmin.v[i] && v.v[i] <= m.frames[0].bboxmax.v[i]
                })
            }));
        }
    }
}
