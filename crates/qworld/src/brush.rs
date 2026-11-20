//! BSPX `BRUSHLIST` exact convex-brush collision (Q2/Q3/FTE `CM_ClipBoxToBrush`),
//! accelerated by a flat binary AABB BVH per submodel.

use crate::hull::Trace;
use crate::{
    CONTENTS_CLIP, CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_SOLID, CONTENTS_WATER, DIST_EPSILON,
    Vec3,
};
use qformats::bspx::BspxBrush;

#[derive(Debug, Clone, Copy)]
struct Brush {
    mins: Vec3,
    maxs: Vec3,
    contents: i32,
    first_plane: u32,
    num_planes: u16,
}

#[derive(Debug, Clone, Copy)]
struct BvhNode {
    mins: Vec3,
    maxs: Vec3,
    /// Leaf: first brush index; internal: left child index.
    first_or_left: u32,
    /// Leaf: brush count (`> 0`); internal: `0` (right child in `right`).
    count: u32,
    right: u32,
}

/// Exact convex-brush collision data for one inline BSP submodel (`BRUSHLIST`).
#[derive(Debug, Default, Clone)]
pub struct BrushModel {
    brushes: Vec<Brush>,
    planes: Vec<(Vec3, f32)>,
    nodes: Vec<BvhNode>,
}

const LEAF_MAX: usize = 4;

impl BrushModel {
    pub fn from_bspx(src: &[BspxBrush]) -> Self {
        let mut brushes = Vec::with_capacity(src.len());
        let mut planes = Vec::new();
        for b in src {
            let first_plane = planes.len() as u32;
            for p in &b.planes {
                planes.push((Vec3::from(p.normal), p.dist));
            }
            brushes.push(Brush {
                mins: Vec3::from(b.mins),
                maxs: Vec3::from(b.maxs),
                contents: b.contents as i32,
                first_plane,
                num_planes: b.planes.len() as u16,
            });
        }
        let mut nodes = Vec::new();
        if !brushes.is_empty() {
            let n = brushes.len();
            nodes.reserve(n * 2);
            build_bvh(&mut brushes, 0, n, &mut nodes);
        }
        Self { brushes, planes, nodes }
    }

    pub fn is_empty(&self) -> bool {
        self.brushes.is_empty()
    }

    /// Swept AABB `[mins, maxs]` trace from `start` to `end` in submodel-local space.
    pub fn trace(&self, start: Vec3, mins: Vec3, maxs: Vec3, end: Vec3) -> Trace {
        let mut tr = Trace::new(end);
        tr.allsolid = false;
        if self.nodes.is_empty() {
            tr.inopen = true;
            return tr;
        }
        let has_size = mins != maxs;
        let qmin = start.min(end) + mins - Vec3::splat(1.0);
        let qmax = start.max(end) + maxs + Vec3::splat(1.0);

        let mut stack = [0u32; 64];
        let mut sp = 1usize;
        stack[0] = 0;
        while sp > 0 {
            sp -= 1;
            let node = self.nodes[stack[sp] as usize];
            if !aabb_overlap(qmin, qmax, node.mins, node.maxs) {
                continue;
            }
            if node.count > 0 {
                let first = node.first_or_left as usize;
                let last = first + node.count as usize;
                for b in &self.brushes[first..last] {
                    if !aabb_overlap(qmin, qmax, b.mins, b.maxs) {
                        continue;
                    }
                    if b.contents == CONTENTS_SOLID || (b.contents == CONTENTS_CLIP && has_size) {
                        clip_box_to_brush(b, &self.planes, start, mins, maxs, end, &mut tr);
                        if tr.allsolid {
                            tr.fraction = 0.0;
                            tr.endpos = start;
                            return tr;
                        }
                    } else if matches!(b.contents, CONTENTS_WATER | CONTENTS_SLIME | CONTENTS_LAVA)
                        && box_hits_brush(b, &self.planes, start, mins, maxs, end)
                    {
                        tr.inwater = true;
                    }
                }
            } else if sp + 2 <= stack.len() {
                stack[sp] = node.right;
                stack[sp + 1] = node.first_or_left;
                sp += 2;
            }
        }
        if tr.allsolid {
            tr.fraction = 0.0;
            tr.endpos = start;
        } else {
            if !tr.inwater {
                tr.inopen = true;
            }
            if tr.fraction < 1.0 {
                tr.endpos = start + (end - start) * tr.fraction;
            }
        }
        tr
    }
}

fn build_bvh(brushes: &mut [Brush], start: usize, end: usize, nodes: &mut Vec<BvhNode>) -> u32 {
    let mut mins = Vec3::splat(f32::INFINITY);
    let mut maxs = Vec3::splat(f32::NEG_INFINITY);
    for b in &brushes[start..end] {
        mins = mins.min(b.mins);
        maxs = maxs.max(b.maxs);
    }
    let idx = nodes.len() as u32;
    let count = end - start;
    if count <= LEAF_MAX {
        nodes.push(BvhNode {
            mins,
            maxs,
            first_or_left: start as u32,
            count: count as u32,
            right: 0,
        });
        return idx;
    }
    let mut cmin = Vec3::splat(f32::INFINITY);
    let mut cmax = Vec3::splat(f32::NEG_INFINITY);
    for b in &brushes[start..end] {
        let c = (b.mins + b.maxs) * 0.5;
        cmin = cmin.min(c);
        cmax = cmax.max(c);
    }
    let ext = cmax - cmin;
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    let mid = start + count / 2;
    brushes[start..end].select_nth_unstable_by(mid - start, |a, b| {
        let ca = a.mins[axis] + a.maxs[axis];
        let cb = b.mins[axis] + b.maxs[axis];
        ca.total_cmp(&cb)
    });
    nodes.push(BvhNode { mins, maxs, first_or_left: 0, count: 0, right: 0 });
    let left = build_bvh(brushes, start, mid, nodes);
    let right = build_bvh(brushes, mid, end, nodes);
    nodes[idx as usize].first_or_left = left;
    nodes[idx as usize].right = right;
    idx
}

#[inline]
fn aabb_overlap(amin: Vec3, amax: Vec3, bmin: Vec3, bmax: Vec3) -> bool {
    !(amin.x > bmax.x
        || amin.y > bmax.y
        || amin.z > bmax.z
        || amax.x < bmin.x
        || amax.y < bmin.y
        || amax.z < bmin.z)
}

#[inline]
fn brush_axial_planes(b: &Brush) -> [(Vec3, f32); 6] {
    [
        (Vec3::new(1.0, 0.0, 0.0), b.maxs.x),
        (Vec3::new(-1.0, 0.0, 0.0), -b.mins.x),
        (Vec3::new(0.0, 1.0, 0.0), b.maxs.y),
        (Vec3::new(0.0, -1.0, 0.0), -b.mins.y),
        (Vec3::new(0.0, 0.0, 1.0), b.maxs.z),
        (Vec3::new(0.0, 0.0, -1.0), -b.mins.z),
    ]
}

fn box_hits_brush(
    b: &Brush,
    planes: &[(Vec3, f32)],
    start: Vec3,
    mins: Vec3,
    maxs: Vec3,
    end: Vec3,
) -> bool {
    let mut tmp = Trace::new(end);
    tmp.allsolid = false;
    clip_box_to_brush(b, planes, start, mins, maxs, end, &mut tmp);
    tmp.startsolid || tmp.fraction < 1.0
}

/// Swept box vs convex brush (Q2/Q3/FTE `CM_ClipBoxToBrush`).
fn clip_box_to_brush(
    b: &Brush,
    planes: &[(Vec3, f32)],
    start: Vec3,
    mins: Vec3,
    maxs: Vec3,
    end: Vec3,
    tr: &mut Trace,
) {
    let mut enter_frac = -1.0f32;
    let mut leave_frac = 1.0f32;
    let mut starts_out = false;
    let mut gets_out = false;
    let mut clip_normal = Vec3::ZERO;
    let mut clip_dist = 0.0f32;

    let axial = brush_axial_planes(b);
    let extra = &planes[b.first_plane as usize..b.first_plane as usize + b.num_planes as usize];
    for &(normal, base_dist) in axial.iter().chain(extra.iter()) {
        let ofs = Vec3::new(
            if normal.x < 0.0 { maxs.x } else { mins.x },
            if normal.y < 0.0 { maxs.y } else { mins.y },
            if normal.z < 0.0 { maxs.z } else { mins.z },
        );
        let dist = base_dist - ofs.dot(normal);
        let d1 = (normal.x as f64 * start.x as f64
            + normal.y as f64 * start.y as f64
            + normal.z as f64 * start.z as f64
            - dist as f64) as f32;
        let d2 = (normal.x as f64 * end.x as f64
            + normal.y as f64 * end.y as f64
            + normal.z as f64 * end.z as f64
            - dist as f64) as f32;

        if d1 > 0.0 {
            starts_out = true;
        }
        if d2 > 0.0 {
            gets_out = true;
        }
        if d1 > 0.0 && (d2 >= DIST_EPSILON || d2 >= d1) {
            return;
        }
        if d1 <= 0.0 && d2 <= 0.0 {
            continue;
        }
        if d1 > d2 {
            let f = ((d1 - DIST_EPSILON) / (d1 - d2)).max(0.0);
            if f > enter_frac {
                enter_frac = f;
                clip_normal = normal;
                clip_dist = dist;
            }
        } else {
            let f = ((d1 + DIST_EPSILON) / (d1 - d2)).min(1.0);
            if f < leave_frac {
                leave_frac = f;
            }
        }
    }

    if !starts_out {
        tr.startsolid = true;
        if !gets_out {
            tr.allsolid = true;
            tr.fraction = 0.0;
        }
        return;
    }
    if enter_frac < leave_frac && enter_frac > -1.0 && enter_frac < tr.fraction {
        tr.fraction = enter_frac.max(0.0);
        tr.plane_normal = clip_normal;
        tr.plane_dist = clip_dist;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qformats::bspx::BrushPlane;

    #[test]
    fn non_standard_box_fits_under_lintel_and_hits_slope() {
        // Lintel from z=40..80, x=0..64, y=-64..64.
        let lintel = BspxBrush {
            mins: [0.0, -64.0, 40.0],
            maxs: [64.0, 64.0, 80.0],
            contents: CONTENTS_SOLID as i16,
            planes: vec![],
        };
        // 45-degree ramp in x=100..200, z=0..100: plane (-1/sqrt2, 0, 1/sqrt2), dist = -100/sqrt2.
        let inv_s2 = std::f32::consts::FRAC_1_SQRT_2;
        let ramp = BspxBrush {
            mins: [100.0, -64.0, 0.0],
            maxs: [200.0, 64.0, 100.0],
            contents: CONTENTS_SOLID as i16,
            planes: vec![BrushPlane {
                normal: [-inv_s2, 0.0, inv_s2],
                dist: -100.0 * inv_s2,
            }],
        };
        let bm = BrushModel::from_bspx(&[lintel, ramp]);

        // Crouching player (non-standard 32x32x36: mins -16,-16,0 maxs 16,16,36) passes cleanly under z=40!
        let tr = bm.trace(
            Vec3::new(-32.0, 0.0, 0.0),
            Vec3::new(-16.0, -16.0, 0.0),
            Vec3::new(16.0, 16.0, 36.0),
            Vec3::new(80.0, 0.0, 0.0),
        );
        assert_eq!(tr.fraction, 1.0);
        assert!(!tr.startsolid);

        // Taller non-standard box (maxs.z = 48) stops at x = -16 (lintel x=0 minus maxs.x=16).
        let tr2 = bm.trace(
            Vec3::new(-64.0, 0.0, 0.0),
            Vec3::new(-16.0, -16.0, 0.0),
            Vec3::new(16.0, 16.0, 48.0),
            Vec3::new(80.0, 0.0, 0.0),
        );
        assert!(tr2.fraction < 1.0);
        assert!((tr2.endpos.x - (-16.0)).abs() < 0.1, "got {}", tr2.endpos.x);
        assert_eq!(tr2.plane_normal, Vec3::new(-1.0, 0.0, 0.0));

        // Falling onto the 45-degree ramp with a small custom box (-8,-8,-8..8,8,8) at x=150:
        // ramp surface at x=150 is z=50; box bottom corner (x=158, z-8) touches plane when z-8 = 58 -> z=66.
        let tr3 = bm.trace(
            Vec3::new(150.0, 0.0, 120.0),
            Vec3::splat(-8.0),
            Vec3::splat(8.0),
            Vec3::new(150.0, 0.0, 0.0),
        );
        assert!(tr3.fraction < 1.0);
        assert!((tr3.endpos.z - 66.0).abs() < 0.2, "got z={}", tr3.endpos.z);
        assert!((tr3.plane_normal.z - inv_s2).abs() < 1e-4);
    }

    #[test]
    fn clip_brush_blocks_boxes_not_points() {
        let clip = BspxBrush {
            mins: [0.0, -32.0, 0.0],
            maxs: [32.0, 32.0, 64.0],
            contents: CONTENTS_CLIP as i16,
            planes: vec![],
        };
        let bm = BrushModel::from_bspx(&[clip]);
        let pt = bm.trace(Vec3::new(-16.0, 0.0, 32.0), Vec3::ZERO, Vec3::ZERO, Vec3::new(48.0, 0.0, 32.0));
        assert_eq!(pt.fraction, 1.0);
        let bx = bm.trace(
            Vec3::new(-32.0, 0.0, 32.0),
            Vec3::splat(-8.0),
            Vec3::splat(8.0),
            Vec3::new(48.0, 0.0, 32.0),
        );
        assert!(bx.fraction < 1.0);
        assert!((bx.endpos.x - (-8.0)).abs() < 0.1);
    }

    #[test]
    fn world_trace_uses_brushlist_for_non_standard_hull() {
        use crate::{CollisionModel, E, Edicts, Ent, F, MoveType, V, World};
        use qformats::bsp::Bsp;
        use qformats::bspx::BspxModelBrushes;
        use std::sync::Arc;

        struct DummyEdicts;
        impl Edicts for DummyEdicts {
            fn num_edicts(&self) -> u32 { 1 }
            fn is_free(&self, _: Ent) -> bool { false }
            fn v(&self, _: Ent, _: V) -> Vec3 { Vec3::ZERO }
            fn set_v(&mut self, _: Ent, _: V, _: Vec3) {}
            fn f(&self, _: Ent, _: F) -> f32 { 0.0 }
            fn set_f(&mut self, _: Ent, _: F, _: f32) {}
            fn e(&self, _: Ent, _: E) -> Ent { 0 }
            fn set_e(&mut self, _: Ent, _: E, _: Ent) {}
            fn has_touch(&self, _: Ent) -> bool { false }
            fn touch(&mut self, _: &mut World, _: Ent, _: Ent) {}
            fn blocked(&mut self, _: &mut World, _: Ent, _: Ent) {}
            fn think(&mut self, _: &mut World, _: Ent, _: f64) {}
        }

        // World with NO clipnodes, only a BRUSHLIST floor brush at z=-64..0.
        let bsp = Bsp {
            models: vec![qformats::bsp::Model {
                mins: [-512.0; 3],
                maxs: [512.0; 3],
                origin: [0.0; 3],
                headnode: [-1; 4],
                visleafs: 0,
                first_face: 0,
                num_faces: 0,
            }],
            brushlist: Some(vec![BspxModelBrushes {
                model_num: 0,
                brushes: vec![BspxBrush {
                    mins: [-256.0, -256.0, -64.0],
                    maxs: [256.0, 256.0, 0.0],
                    contents: CONTENTS_SOLID as i16,
                    planes: vec![],
                }],
            }]),
            ..Default::default()
        };
        let cm = Arc::new(CollisionModel::from_bsp(&bsp));
        assert!(cm.should_use_brushes(0, Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0)));
        // Omitted clipnodes also trigger BRUSHLIST for standard hull 1.
        assert!(cm.should_use_brushes(0, Vec3::new(-16.0, -16.0, -24.0), Vec3::new(16.0, 16.0, 32.0)));
        let w = World::new(cm);
        let tr = w.trace(
            &DummyEdicts,
            Vec3::new(0.0, 0.0, 50.0),
            Vec3::new(-10.0, -10.0, -10.0),
            Vec3::new(10.0, 10.0, 10.0),
            Vec3::new(0.0, 0.0, -50.0),
            MoveType::Normal,
            None,
        );
        assert!(tr.fraction < 1.0);
        assert_eq!(tr.ent, Some(0));
        assert!((tr.endpos.z - 10.0).abs() < 0.1, "got z={}", tr.endpos.z);
    }
}
