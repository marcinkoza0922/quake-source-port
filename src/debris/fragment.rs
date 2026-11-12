//! A procedural alias model of jagged metal shards (one shape per frame),
//! thrown when an exploding box breaks. Built in code so it works with any
//! mod; the skin uses the active palette's grey and brown rows.
use qformats::mdl::{Frame, Mdl, Pose, Skin, StVert, Triangle, TriVert};

use super::math::Rng;

pub const SHAPES: u32 = 6;
const SKIN: u32 = 16;
/// Model units per packed coordinate step.
const SCALE: f32 = 0.08;
const ORIGIN: f32 = -10.0;

pub fn shard_model() -> Mdl {
    let mut rng = Rng(0x5eed_1234);
    // A bent, uneven plate: 4 corners top and bottom, 8 verts.
    let frames = (0..SHAPES)
        .map(|f| {
            let w = 3.0 + rng.unit() * 6.0;
            let h = 2.0 + rng.unit() * 5.0;
            let bend = rng.signed() * 2.5;
            let thick = 0.6 + rng.unit() * 0.6;
            let mut corners = Vec::new();
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let jx = 1.0 + rng.signed() * 0.35;
                let jy = 1.0 + rng.signed() * 0.35;
                let x: f32 = sx * w * jx;
                let y: f32 = sy * h * jy;
                let z = bend * (x / w).powi(2) + rng.signed() * 0.6;
                corners.push([x, y, z]);
            }
            let mut verts = Vec::with_capacity(8);
            for dz in [thick * 0.5, -thick * 0.5] {
                for c in &corners {
                    verts.push(pack([c[0], c[1], c[2] + dz], super::math::normal_index(qcore::math::Vec3::new(0.0, 0.0, dz.signum()))));
                }
            }
            let pose = Pose { name: format!("shard{f}"), bboxmin: pack([ORIGIN; 3], 0), bboxmax: pack([-ORIGIN; 3], 0), verts };
            Frame { bboxmin: pose.bboxmin, bboxmax: pose.bboxmax, intervals: Vec::new(), poses: vec![pose] }
        })
        .collect();
    let quads = [[0, 1, 2, 3], [7, 6, 5, 4], [0, 4, 5, 1], [1, 5, 6, 2], [2, 6, 7, 3], [3, 7, 4, 0]];
    let triangles = quads
        .iter()
        .flat_map(|q| {
            [[q[0], q[2], q[1]], [q[0], q[3], q[2]]].map(|v| Triangle { faces_front: 1, verts: v })
        })
        .collect();
    let stverts = (0..8)
        .map(|i| StVert { onseam: 0, s: [1, 14, 14, 1][i % 4] + (i / 4) as i32, t: [1, 1, 14, 14][i % 4] })
        .collect();
    let mut px = Vec::with_capacity((SKIN * SKIN) as usize);
    for _ in 0..SKIN * SKIN {
        let r = rng.unit();
        // mostly dark grey metal (palette row 0) with rust (row 5)
        let c = if r < 0.7 { 3 + (rng.unit() * 5.0) as u8 } else { 84 + (rng.unit() * 6.0) as u8 };
        px.push(c);
    }
    Mdl {
        scale: [SCALE; 3],
        scale_origin: [ORIGIN; 3],
        bounding_radius: 12.0,
        eye_position: [0.0; 3],
        skin_width: SKIN,
        skin_height: SKIN,
        num_verts: 8,
        synctype: 0,
        flags: 0,
        size: 4.0,
        skins: vec![Skin { intervals: Vec::new(), images: vec![px] }],
        stverts,
        triangles,
        frames,
    }
}

fn pack(p: [f32; 3], normal: u8) -> TriVert {
    TriVert { v: p.map(|c| ((c - ORIGIN) / SCALE).round().clamp(0.0, 255.0) as u8), normal }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shard_model_is_consistent() {
        let m = super::shard_model();
        assert_eq!(m.frames.len(), super::SHAPES as usize);
        assert!(m.frames.iter().all(|f| f.poses[0].verts.len() == m.num_verts));
        assert!(m.triangles.iter().all(|t| t.verts.iter().all(|&v| (v as usize) < m.num_verts)));
    }
}
