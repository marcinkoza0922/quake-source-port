//! Clipping hulls (gl_model.c hull setup) and hull traces (world.c).

use crate::{CONTENTS_SOLID, DIST_EPSILON, Ent, Vec3};
use qformats::Bsp;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HPlane {
    pub normal: Vec3,
    pub dist: f32,
    /// 0..2 axial, else general.
    pub kind: i32,
}

impl HPlane {
    /// Signed distance with ironwail's DoublePrecisionDotProduct, rounded to float.
    #[inline]
    pub fn diff(&self, p: Vec3) -> f32 {
        if (0..3).contains(&self.kind) {
            p[self.kind as usize] - self.dist
        } else {
            let n = self.normal;
            (n.x as f64 * p.x as f64 + n.y as f64 * p.y as f64 + n.z as f64 * p.z as f64
                - self.dist as f64) as f32
        }
    }
}

/// Clip node; a child < 0 is a contents value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CNode {
    pub plane: u32,
    pub children: [i32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct SubModel {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub origin: Vec3,
    pub headnode: [i32; 4],
}

/// Collision data of a brush model (world + inline submodels).
#[derive(Debug, Default)]
pub struct CollisionModel {
    pub planes: Vec<HPlane>,
    /// Hull 0: copy of the render nodes (Mod_MakeHull0).
    pub hull0: Vec<CNode>,
    /// Hulls 1 and 2 share this array.
    pub clipnodes: Vec<CNode>,
    /// Render node children (< 0: leaf `-(i+1)`), for touched-leaf queries.
    pub node_children: Vec<[i32; 2]>,
    pub leaf_contents: Vec<i32>,
    pub submodels: Vec<SubModel>,
    /// Optional BSPX `BRUSHLIST` collision structures per submodel (exact
    /// swept-box collision for non-standard hull sizes or omitted clipnodes).
    pub brushes: Option<Vec<crate::brush::BrushModel>>,
}

pub const HULL_MINS: [Vec3; 3] =
    [Vec3::ZERO, Vec3::new(-16.0, -16.0, -24.0), Vec3::new(-32.0, -32.0, -24.0)];
pub const HULL_MAXS: [Vec3; 3] =
    [Vec3::ZERO, Vec3::new(16.0, 16.0, 32.0), Vec3::new(32.0, 32.0, 64.0)];

impl CollisionModel {
    pub fn from_bsp(b: &Bsp) -> Self {
        let planes = b
            .planes
            .iter()
            .map(|p| HPlane { normal: p.normal.into(), dist: p.dist, kind: p.kind })
            .collect();
        let leaf_c = |c: i32| b.leafs.get((-(c + 1)) as usize).map_or(CONTENTS_SOLID, |l| l.contents);
        let hull0 = b
            .nodes
            .iter()
            .map(|n| CNode {
                plane: n.plane as u32,
                children: n.children.map(|c| if c < 0 { leaf_c(c) } else { c }),
            })
            .collect();
        let clipnodes =
            b.clipnodes.iter().map(|c| CNode { plane: c.plane as u32, children: c.children }).collect();
        let brushes = b.brushlist.as_ref().map(|models| {
            let max_m = models
                .iter()
                .map(|m| m.model_num as usize + 1)
                .max()
                .unwrap_or(0)
                .max(b.models.len());
            let mut per_sub = vec![crate::brush::BrushModel::default(); max_m];
            for m in models {
                if let Some(slot) = per_sub.get_mut(m.model_num as usize) {
                    *slot = crate::brush::BrushModel::from_bspx(&m.brushes);
                }
            }
            per_sub
        });
        Self {
            planes,
            hull0,
            clipnodes,
            node_children: b.nodes.iter().map(|n| n.children).collect(),
            leaf_contents: b.leafs.iter().map(|l| l.contents).collect(),
            submodels: b
                .models
                .iter()
                // Mod_LoadSubmodels: spread the mins / maxs by a pixel.
                .map(|m| SubModel {
                    mins: Vec3::from(m.mins) - Vec3::splat(1.0),
                    maxs: Vec3::from(m.maxs) + Vec3::splat(1.0),
                    origin: m.origin.into(),
                    headnode: m.headnode,
                })
                .collect(),
            brushes,
        }
    }

    /// Standard Quake hull index if `(mins, maxs)` matches hull 0, 1 or 2 exactly.
    pub fn standard_hull(mins: Vec3, maxs: Vec3) -> Option<usize> {
        let size = maxs - mins;
        if size == Vec3::ZERO {
            Some(0)
        } else if size == HULL_MAXS[1] - HULL_MINS[1] {
            Some(1)
        } else if size == HULL_MAXS[2] - HULL_MINS[2] {
            Some(2)
        } else {
            None
        }
    }

    /// Whether a trace against `sub` with `(mins, maxs)` should use `BRUSHLIST`
    /// exact brush collision (non-standard hull size, or omitted clipnodes).
    pub fn should_use_brushes(&self, sub: usize, mins: Vec3, maxs: Vec3) -> bool {
        if self.brushes.is_none() {
            return false;
        }
        match Self::standard_hull(mins, maxs) {
            None => true,
            Some(0) => self.hull0.is_empty(),
            Some(h) => {
                self.clipnodes.is_empty()
                    || self.submodels.get(sub).is_some_and(|m| m.headnode[h] < 0)
            }
        }
    }

    /// Trace against `BRUSHLIST` for submodel `sub` in world coordinates,
    /// handling translation by `origin` and optional rotation by `angles`.
    #[allow(clippy::too_many_arguments)]
    pub fn trace_brushes_transformed(
        &self,
        sub: usize,
        origin: Vec3,
        angles: Option<Vec3>,
        start: Vec3,
        mins: Vec3,
        maxs: Vec3,
        end: Vec3,
    ) -> Option<Trace> {
        if let Some(angles) = angles {
            let (f, r, u) = qcore::math::angle_vectors(angles);
            let local = |p: Vec3| {
                let d = p - origin;
                Vec3::new(d.dot(f), -d.dot(r), d.dot(u))
            };
            let mut t = self.trace_brushes(sub, local(start), mins, maxs, local(end))?;
            if t.fraction != 1.0 {
                let n = t.plane_normal;
                t.plane_normal = f * n.x - r * n.y + u * n.z;
                t.endpos = start + (end - start) * t.fraction;
                t.plane_dist = t.plane_normal.dot(t.endpos);
            } else {
                t.endpos = end;
            }
            Some(t)
        } else {
            let mut t = self.trace_brushes(sub, start - origin, mins, maxs, end - origin)?;
            if t.fraction != 1.0 || t.startsolid {
                t.endpos += origin;
                t.plane_dist += t.plane_normal.dot(origin);
            } else {
                t.endpos = end;
            }
            Some(t)
        }
    }

    /// Trace against `BRUSHLIST` for submodel `sub` in submodel-local coordinates.
    pub fn trace_brushes(
        &self,
        sub: usize,
        start: Vec3,
        mins: Vec3,
        maxs: Vec3,
        end: Vec3,
    ) -> Option<Trace> {
        let list = self.brushes.as_ref()?;
        match list.get(sub) {
            Some(bm) => Some(bm.trace(start, mins, maxs, end)),
            None => {
                // Submodels omitted from BRUSHLIST are non-solid (e.g. func_illusionary).
                let mut tr = Trace::new(end);
                tr.allsolid = false;
                tr.inopen = true;
                Some(tr)
            }
        }
    }

    /// Hull `h` (0..2) of submodel `sub`.
    pub fn hull(&self, sub: usize, h: usize) -> Hull<'_> {
        let first = self.submodels.get(sub).map_or(0, |m| m.headnode[h]);
        Hull {
            nodes: if h == 0 { &self.hull0 } else { &self.clipnodes },
            planes: &self.planes,
            first,
            clip_mins: HULL_MINS[h],
            clip_maxs: HULL_MAXS[h],
        }
    }

    /// SV_TruePointContents (world hull 0 from node 0).
    pub fn point_contents(&self, p: Vec3) -> i32 {
        let mut h = self.hull(0, 0);
        h.first = 0;
        h.point_contents(0, p)
    }

    /// SV_FindTouchedLeafs: leaf numbers (leaf index - 1) touched by a box, max `max`.
    pub fn touched_leafs(&self, mins: Vec3, maxs: Vec3, out: &mut Vec<u32>, max: usize) {
        if self.node_children.is_empty() {
            return;
        }
        self.leafs_r(0, mins, maxs, out, max);
    }

    fn leafs_r(&self, num: i32, mins: Vec3, maxs: Vec3, out: &mut Vec<u32>, max: usize) {
        if out.len() == max {
            return;
        }
        if num < 0 {
            let li = (-(num + 1)) as usize;
            if li == 0 || self.leaf_contents.get(li).is_none_or(|&c| c == CONTENTS_SOLID) {
                return;
            }
            out.push(li as u32 - 1);
            return;
        }
        let Some(ch) = self.node_children.get(num as usize) else { return };
        let sides = box_on_plane_side(mins, maxs, &self.planes[self.hull0[num as usize].plane as usize]);
        if sides & 1 != 0 {
            self.leafs_r(ch[0], mins, maxs, out, max);
        }
        if sides & 2 != 0 {
            self.leafs_r(ch[1], mins, maxs, out, max);
        }
    }

    /// SV_BoxInPVS.
    pub fn box_in_pvs(&self, mins: Vec3, maxs: Vec3, pvs: &[u8]) -> bool {
        self.pvs_r(0, mins, maxs, pvs)
    }

    fn pvs_r(&self, num: i32, mins: Vec3, maxs: Vec3, pvs: &[u8]) -> bool {
        if num < 0 {
            let li = (-(num + 1)) as usize;
            if li == 0 || self.leaf_contents.get(li).is_none_or(|&c| c == CONTENTS_SOLID) {
                return false;
            }
            let l = li - 1;
            return pvs.get(l >> 3).is_some_and(|b| b & (1 << (l & 7)) != 0);
        }
        let Some(ch) = self.node_children.get(num as usize) else { return false };
        let sides = box_on_plane_side(mins, maxs, &self.planes[self.hull0[num as usize].plane as usize]);
        (sides & 1 != 0 && self.pvs_r(ch[0], mins, maxs, pvs))
            || (sides & 2 != 0 && self.pvs_r(ch[1], mins, maxs, pvs))
    }
}

/// BOX_ON_PLANE_SIDE: 1 front, 2 back, 3 both.
pub fn box_on_plane_side(mins: Vec3, maxs: Vec3, p: &HPlane) -> i32 {
    if (0..3).contains(&p.kind) {
        let k = p.kind as usize;
        if p.dist <= mins[k] {
            return 1;
        }
        if p.dist >= maxs[k] {
            return 2;
        }
        return 3;
    }
    let (mut d1, mut d2) = (0.0f32, 0.0f32);
    for i in 0..3 {
        let n = p.normal[i];
        if n >= 0.0 {
            d1 += n * maxs[i];
            d2 += n * mins[i];
        } else {
            d1 += n * mins[i];
            d2 += n * maxs[i];
        }
    }
    let mut s = 0;
    if d1 >= p.dist {
        s = 1;
    }
    if d2 < p.dist {
        s |= 2;
    }
    s
}

/// A hull view: nodes + planes + head node.
#[derive(Clone, Copy, Debug)]
pub struct Hull<'a> {
    pub nodes: &'a [CNode],
    pub planes: &'a [HPlane],
    pub first: i32,
    pub clip_mins: Vec3,
    pub clip_maxs: Vec3,
}

/// SV_InitBoxHull clipnodes.
pub static BOX_NODES: [CNode; 6] = [
    CNode { plane: 0, children: [-1, 1] },
    CNode { plane: 1, children: [2, -1] },
    CNode { plane: 2, children: [-1, 3] },
    CNode { plane: 3, children: [4, -1] },
    CNode { plane: 4, children: [-1, 5] },
    CNode { plane: 5, children: [-2, -1] },
];

/// SV_HullForBox planes (kept on the caller's stack).
pub fn box_planes(mins: Vec3, maxs: Vec3) -> [HPlane; 6] {
    let ax = |i: usize, d: f32| {
        let mut n = Vec3::ZERO;
        n[i] = 1.0;
        HPlane { normal: n, dist: d, kind: i as i32 }
    };
    [ax(0, maxs.x), ax(0, mins.x), ax(1, maxs.y), ax(1, mins.y), ax(2, maxs.z), ax(2, mins.z)]
}

impl<'a> Hull<'a> {
    pub fn for_box(planes: &'a [HPlane; 6]) -> Self {
        Hull { nodes: &BOX_NODES, planes, first: 0, clip_mins: Vec3::ZERO, clip_maxs: Vec3::ZERO }
    }

    #[inline]
    fn node(&self, num: i32) -> Option<(&CNode, &HPlane)> {
        let n = self.nodes.get(num as usize)?;
        Some((n, self.planes.get(n.plane as usize)?))
    }

    /// SV_HullPointContents. Bad node numbers count as solid.
    pub fn point_contents(&self, mut num: i32, p: Vec3) -> i32 {
        while num >= 0 {
            let Some((n, pl)) = self.node(num) else { return CONTENTS_SOLID };
            num = if pl.diff(p) < 0.0 { n.children[1] } else { n.children[0] };
        }
        num
    }

    /// Full trace from `p1` to `p2` (SV_ClipMoveToEntity core, hull-local).
    pub fn trace(&self, p1: Vec3, p2: Vec3) -> Trace {
        let mut t = Trace::new(p2);
        self.recursive_check(self.first, 0.0, 1.0, p1, p2, &mut t);
        t
    }

    /// SV_RecursiveHullCheck, bit-for-bit.
    pub fn recursive_check(&self, num: i32, p1f: f32, p2f: f32, p1: Vec3, p2: Vec3, tr: &mut Trace) -> bool {
        if num < 0 {
            if num != CONTENTS_SOLID {
                tr.allsolid = false;
                if num == crate::CONTENTS_EMPTY {
                    tr.inopen = true;
                } else {
                    tr.inwater = true;
                }
            } else {
                tr.startsolid = true;
            }
            return true;
        }
        let Some((node, plane)) = self.node(num) else {
            tr.startsolid = true;
            return true;
        };
        let t1 = plane.diff(p1);
        let t2 = plane.diff(p2);
        if t1 >= 0.0 && t2 >= 0.0 {
            return self.recursive_check(node.children[0], p1f, p2f, p1, p2, tr);
        }
        if t1 < 0.0 && t2 < 0.0 {
            return self.recursive_check(node.children[1], p1f, p2f, p1, p2, tr);
        }
        let mut frac = if t1 < 0.0 { (t1 + DIST_EPSILON) / (t1 - t2) } else { (t1 - DIST_EPSILON) / (t1 - t2) };
        frac = frac.clamp(0.0, 1.0);
        let mut midf = p1f + (p2f - p1f) * frac;
        let lerp = |f: f32| {
            Vec3::new(p1.x + f * (p2.x - p1.x), p1.y + f * (p2.y - p1.y), p1.z + f * (p2.z - p1.z))
        };
        let mut mid = lerp(frac);
        let side = (t1 < 0.0) as usize;
        if !self.recursive_check(node.children[side], p1f, midf, p1, mid, tr) {
            return false;
        }
        if self.point_contents(node.children[side ^ 1], mid) != CONTENTS_SOLID {
            return self.recursive_check(node.children[side ^ 1], midf, p2f, mid, p2, tr);
        }
        if tr.allsolid {
            return false;
        }
        if side == 0 {
            tr.plane_normal = plane.normal;
            tr.plane_dist = plane.dist;
        } else {
            tr.plane_normal = -plane.normal;
            tr.plane_dist = -plane.dist;
        }
        while self.point_contents(self.first, mid) == CONTENTS_SOLID {
            frac -= 0.1;
            if frac < 0.0 {
                tr.fraction = midf;
                tr.endpos = mid;
                return false;
            }
            midf = p1f + (p2f - p1f) * frac;
            mid = lerp(frac);
        }
        tr.fraction = midf;
        tr.endpos = mid;
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub allsolid: bool,
    pub startsolid: bool,
    pub inopen: bool,
    pub inwater: bool,
    pub fraction: f32,
    pub endpos: Vec3,
    pub plane_normal: Vec3,
    pub plane_dist: f32,
    /// Hit entity (Some(0) = world).
    pub ent: Option<Ent>,
}

impl Trace {
    /// Initial state used by SV_ClipMoveToEntity.
    pub fn new(end: Vec3) -> Self {
        Trace {
            allsolid: true,
            startsolid: false,
            inopen: false,
            inwater: false,
            fraction: 1.0,
            endpos: end,
            plane_normal: Vec3::ZERO,
            plane_dist: 0.0,
            ent: None,
        }
    }
}
