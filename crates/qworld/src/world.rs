//! Area tree, entity linking and SV_Move (world.c).

use crate::hull::{CollisionModel, HPlane, Hull, Trace, box_planes};
use crate::{
    CONTENTS_CURRENT_0, CONTENTS_CURRENT_DOWN, CONTENTS_WATER, E, Edicts, Ent, F, FL_ITEM,
    FL_MONSTER, SOLID_BSP, SOLID_NOT, SOLID_TRIGGER, V, Vec3,
};
use qcore::math::angle_vectors;
use std::sync::Arc;

const AREA_DEPTH: u32 = 4;
const AREA_NODES: usize = 2 << AREA_DEPTH;
const HEADS: usize = AREA_NODES * 2;
const NONE: u32 = u32::MAX;
pub const MAX_ENT_LEAFS: usize = 32;

/// SV_Move type (MOVE_NORMAL / MOVE_NOMONSTERS / MOVE_MISSILE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    Normal,
    NoMonsters,
    Missile,
}

impl From<i32> for MoveType {
    fn from(v: i32) -> Self {
        match v {
            1 => MoveType::NoMonsters,
            2 => MoveType::Missile,
            _ => MoveType::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AreaNode {
    /// -1 = leaf.
    axis: i32,
    dist: f32,
    children: [u8; 2],
}

#[derive(Clone, Copy, Debug)]
struct Link {
    prev: u32,
    next: u32,
}

const UNLINKED: Link = Link { prev: NONE, next: NONE };

struct MoveClip {
    boxmins: Vec3,
    boxmaxs: Vec3,
    mins: Vec3,
    maxs: Vec3,
    mins2: Vec3,
    maxs2: Vec3,
    start: Vec3,
    end: Vec3,
    trace: Trace,
    ty: MoveType,
    pass: Option<Ent>,
}

/// Server collision world: brush models, area tree with intrusive link lists
/// (same iteration order as the C linked lists), reusable scratch buffers.
pub struct World {
    pub worldmodel: Arc<CollisionModel>,
    /// Per modelindex: brush model + submodel number (None for alias/sprite).
    models: Vec<Option<(Arc<CollisionModel>, usize)>>,
    nodes: [AreaNode; AREA_NODES],
    numnodes: usize,
    links: Vec<Link>,
    touch_scratch: Vec<Ent>,
    leaf_scratch: Vec<u32>,
    pub(crate) push_scratch: Vec<(Ent, Vec3)>,
    /// Extension: trace against SOLID_BSP entities in their rotated frame
    /// (off = vanilla, which ignores angles).
    pub rotate_bmodels: bool,
}

impl World {
    /// SV_ClearWorld for `worldmodel` (also registered as modelindex 1).
    pub fn new(worldmodel: Arc<CollisionModel>) -> Self {
        let mut w = World {
            worldmodel: worldmodel.clone(),
            models: vec![None, Some((worldmodel, 0))],
            nodes: [AreaNode::default(); AREA_NODES],
            numnodes: 0,
            links: Vec::new(),
            touch_scratch: Vec::new(),
            leaf_scratch: Vec::new(),
            push_scratch: Vec::new(),
            rotate_bmodels: false,
        };
        w.clear();
        w
    }

    /// Unlink everything and rebuild the area tree.
    pub fn clear(&mut self) {
        self.numnodes = 0;
        self.links.clear();
        self.links.resize(HEADS, UNLINKED);
        for h in 0..HEADS {
            self.links[h] = Link { prev: h as u32, next: h as u32 };
        }
        let (mins, maxs) =
            self.worldmodel.submodels.first().map_or((Vec3::ZERO, Vec3::ZERO), |m| (m.mins, m.maxs));
        self.create_node(0, mins, maxs);
    }

    fn create_node(&mut self, depth: u32, mins: Vec3, maxs: Vec3) -> u8 {
        let n = self.numnodes;
        self.numnodes += 1;
        if depth == AREA_DEPTH {
            self.nodes[n] = AreaNode { axis: -1, dist: 0.0, children: [0; 2] };
            return n as u8;
        }
        let size = maxs - mins;
        let axis = if size.x > size.y { 0 } else { 1 };
        let dist = 0.5 * (maxs[axis] + mins[axis]);
        let (mut maxs1, mut mins2) = (maxs, mins);
        maxs1[axis] = dist;
        mins2[axis] = dist;
        let c0 = self.create_node(depth + 1, mins2, maxs);
        let c1 = self.create_node(depth + 1, mins, maxs1);
        self.nodes[n] = AreaNode { axis: axis as i32, dist, children: [c0, c1] };
        n as u8
    }

    /// Register model `index` (precache slot). `sub` is the submodel number for "*N".
    pub fn set_model(&mut self, index: usize, model: Option<(Arc<CollisionModel>, usize)>) {
        if self.models.len() <= index {
            self.models.resize(index + 1, None);
        }
        self.models[index] = model;
    }

    pub fn model(&self, index: usize) -> Option<&(Arc<CollisionModel>, usize)> {
        self.models.get(index)?.as_ref()
    }

    /// SV_PointContents (currents map to water).
    pub fn point_contents(&self, p: Vec3) -> i32 {
        let c = self.worldmodel.point_contents(p);
        if (CONTENTS_CURRENT_DOWN..=CONTENTS_CURRENT_0).contains(&c) { CONTENTS_WATER } else { c }
    }

    /// SV_TruePointContents.
    pub fn true_point_contents(&self, p: Vec3) -> i32 {
        self.worldmodel.point_contents(p)
    }

    // ---- links ----

    fn li(e: Ent) -> usize {
        HEADS + e as usize
    }

    pub fn is_linked(&self, e: Ent) -> bool {
        self.links.get(Self::li(e)).is_some_and(|l| l.prev != NONE)
    }

    /// SV_UnlinkEdict.
    pub fn unlink(&mut self, e: Ent) {
        if !self.is_linked(e) {
            return;
        }
        let i = Self::li(e);
        let Link { prev, next } = self.links[i];
        self.links[prev as usize].next = next;
        self.links[next as usize].prev = prev;
        self.links[i] = UNLINKED;
    }

    fn insert_before(&mut self, e: Ent, head: usize) {
        let i = Self::li(e);
        if self.links.len() <= i {
            self.links.resize(i + 1, UNLINKED);
        }
        let prev = self.links[head].prev;
        self.links[i] = Link { prev, next: head as u32 };
        self.links[prev as usize].next = i as u32;
        self.links[head].prev = i as u32;
    }

    /// SV_LinkEdict.
    pub fn link_edict<D: Edicts + ?Sized>(&mut self, ents: &mut D, e: Ent, touch_triggers: bool) {
        self.unlink(e);
        if e == 0 || ents.is_free(e) {
            return;
        }
        let org = ents.v(e, V::Origin);
        let mut absmin = org + ents.v(e, V::Mins);
        let mut absmax = org + ents.v(e, V::Maxs);
        if ents.flags(e) & FL_ITEM != 0 {
            absmin.x -= 15.0;
            absmin.y -= 15.0;
            absmax.x += 15.0;
            absmax.y += 15.0;
        } else {
            absmin -= Vec3::splat(1.0);
            absmax += Vec3::splat(1.0);
        }
        ents.set_v(e, V::AbsMin, absmin);
        ents.set_v(e, V::AbsMax, absmax);

        let mut leafs = std::mem::take(&mut self.leaf_scratch);
        leafs.clear();
        if ents.f(e, F::ModelIndex) != 0.0 {
            self.worldmodel.touched_leafs(absmin, absmax, &mut leafs, MAX_ENT_LEAFS);
        }
        ents.set_leafs(e, &leafs);
        self.leaf_scratch = leafs;

        let solid = ents.fi(e, F::Solid);
        if solid == SOLID_NOT {
            return;
        }
        let mut n = 0usize;
        loop {
            let node = self.nodes[n];
            if node.axis == -1 {
                break;
            }
            let a = node.axis as usize;
            if absmin[a] > node.dist {
                n = node.children[0] as usize;
            } else if absmax[a] < node.dist {
                n = node.children[1] as usize;
            } else {
                break;
            }
        }
        let head = n * 2 + (solid != SOLID_TRIGGER) as usize;
        self.insert_before(e, head);
        if touch_triggers {
            self.touch_links(ents, e);
        }
    }

    fn area_triggers<D: Edicts + ?Sized>(&self, ents: &D, e: Ent, n: usize, out: &mut Vec<Ent>) {
        let (amin, amax) = (ents.v(e, V::AbsMin), ents.v(e, V::AbsMax));
        let head = n * 2;
        let mut l = self.links[head].next as usize;
        while l != head {
            let t = (l - HEADS) as Ent;
            l = self.links[l].next as usize;
            if t == e || !ents.has_touch(t) || ents.fi(t, F::Solid) != SOLID_TRIGGER {
                continue;
            }
            if !boxes_touch(amin, amax, ents.v(t, V::AbsMin), ents.v(t, V::AbsMax)) {
                continue;
            }
            out.push(t);
        }
        let node = self.nodes[n];
        if node.axis == -1 {
            return;
        }
        let a = node.axis as usize;
        if amax[a] > node.dist {
            self.area_triggers(ents, e, node.children[0] as usize, out);
        }
        if amin[a] < node.dist {
            self.area_triggers(ents, e, node.children[1] as usize, out);
        }
    }

    /// Triggers whose box touches `e`'s absbox (SV_AreaTriggerEdicts).
    pub fn area_trigger_edicts<D: Edicts + ?Sized>(&self, ents: &D, e: Ent, out: &mut Vec<Ent>) {
        self.area_triggers(ents, e, 0, out);
    }

    /// SV_TouchLinks.
    pub fn touch_links<D: Edicts + ?Sized>(&mut self, ents: &mut D, e: Ent) {
        let mut list = std::mem::take(&mut self.touch_scratch);
        list.clear();
        self.area_triggers(ents, e, 0, &mut list);
        for &t in &list {
            if t == e || !ents.has_touch(t) || ents.fi(t, F::Solid) != SOLID_TRIGGER {
                continue;
            }
            let (amin, amax) = (ents.v(e, V::AbsMin), ents.v(e, V::AbsMax));
            if !boxes_touch(amin, amax, ents.v(t, V::AbsMin), ents.v(t, V::AbsMax)) {
                continue;
            }
            ents.touch(self, t, e);
        }
        self.touch_scratch = list;
    }

    // ---- clipping ----

    /// SV_ClipMoveToEntity (+ optional bmodel rotation).
    pub fn clip_move_to_entity<D: Edicts + ?Sized>(
        &self,
        ents: &D,
        ent: Ent,
        start: Vec3,
        mins: Vec3,
        maxs: Vec3,
        end: Vec3,
    ) -> Trace {
        let bp: [HPlane; 6];
        let origin = ents.v(ent, V::Origin);
        let bmodel = if ent == 0 {
            Some((&self.worldmodel, 0))
        } else if ents.fi(ent, F::Solid) == SOLID_BSP {
            self.model(ents.f(ent, F::ModelIndex) as usize).map(|(m, s)| (m, *s))
        } else {
            None
        };
        let (hull, offset) = match bmodel {
            Some((m, sub)) => {
                let sx = maxs.x - mins.x;
                let h = if sx < 3.0 {
                    0
                } else if sx <= 32.0 {
                    1
                } else {
                    2
                };
                let hull = m.hull(sub, h);
                let off = hull.clip_mins - mins + origin;
                (hull, off)
            }
            None => {
                bp = box_planes(ents.v(ent, V::Mins) - maxs, ents.v(ent, V::Maxs) - mins);
                (Hull::for_box(&bp), origin)
            }
        };
        let angles = ents.v(ent, V::Angles);
        let rot = self.rotate_bmodels && ent != 0 && bmodel.is_some() && angles != Vec3::ZERO;
        let mut tr = Trace::new(end);
        if rot {
            let (f, r, u) = angle_vectors(angles);
            let adj = offset - origin;
            let local = |p: Vec3| {
                let d = p - origin;
                Vec3::new(d.dot(f), -d.dot(r), d.dot(u)) - adj
            };
            hull.recursive_check(hull.first, 0.0, 1.0, local(start), local(end), &mut tr);
            if tr.fraction != 1.0 {
                let n = tr.plane_normal;
                tr.plane_normal = f * n.x - r * n.y + u * n.z;
                tr.endpos = start + (end - start) * tr.fraction;
                tr.plane_dist = tr.plane_normal.dot(tr.endpos);
            }
        } else {
            hull.recursive_check(hull.first, 0.0, 1.0, start - offset, end - offset, &mut tr);
            if tr.fraction != 1.0 {
                tr.endpos += offset;
            }
        }
        if tr.fraction < 1.0 || tr.startsolid {
            tr.ent = Some(ent);
        }
        tr
    }

    fn clip_to_links<D: Edicts + ?Sized>(&self, ents: &D, n: usize, clip: &mut MoveClip) {
        let head = n * 2 + 1;
        let mut l = self.links[head].next as usize;
        while l != head {
            let t = (l - HEADS) as Ent;
            l = self.links[l].next as usize;
            let solid = ents.fi(t, F::Solid);
            // C: Sys_Error on triggers in the solid list; we skip them.
            if solid == SOLID_NOT || solid == SOLID_TRIGGER || Some(t) == clip.pass {
                continue;
            }
            if clip.ty == MoveType::NoMonsters && solid != SOLID_BSP {
                continue;
            }
            if !boxes_touch(clip.boxmins, clip.boxmaxs, ents.v(t, V::AbsMin), ents.v(t, V::AbsMax)) {
                continue;
            }
            if let Some(p) = clip.pass
                && ents.v(p, V::Size).x != 0.0
                && ents.v(t, V::Size).x == 0.0
            {
                continue;
            }
            if clip.trace.allsolid {
                return;
            }
            if let Some(p) = clip.pass
                && (ents.e(t, E::Owner) == p || ents.e(p, E::Owner) == t)
            {
                continue;
            }
            let mut tr = if ents.flags(t) & FL_MONSTER != 0 {
                self.clip_move_to_entity(ents, t, clip.start, clip.mins2, clip.maxs2, clip.end)
            } else {
                self.clip_move_to_entity(ents, t, clip.start, clip.mins, clip.maxs, clip.end)
            };
            if tr.allsolid || tr.startsolid || tr.fraction < clip.trace.fraction {
                tr.ent = Some(t);
                if clip.trace.startsolid {
                    clip.trace = tr;
                    clip.trace.startsolid = true;
                } else {
                    clip.trace = tr;
                }
            } else if tr.startsolid {
                clip.trace.startsolid = true;
            }
        }
        let node = self.nodes[n];
        if node.axis == -1 {
            return;
        }
        let a = node.axis as usize;
        if clip.boxmaxs[a] > node.dist {
            self.clip_to_links(ents, node.children[0] as usize, clip);
        }
        if clip.boxmins[a] < node.dist {
            self.clip_to_links(ents, node.children[1] as usize, clip);
        }
    }

    /// SV_Move. `pass` is the passedict (Some(0) for the world is valid and,
    /// as in C, skips everything owned by the world).
    #[allow(clippy::too_many_arguments)]
    pub fn trace<D: Edicts + ?Sized>(
        &self,
        ents: &D,
        start: Vec3,
        mins: Vec3,
        maxs: Vec3,
        end: Vec3,
        ty: MoveType,
        pass: Option<Ent>,
    ) -> Trace {
        let trace = self.clip_move_to_entity(ents, 0, start, mins, maxs, end);
        let (mins2, maxs2) =
            if ty == MoveType::Missile { (Vec3::splat(-15.0), Vec3::splat(15.0)) } else { (mins, maxs) };
        let (mut boxmins, mut boxmaxs) = (Vec3::ZERO, Vec3::ZERO);
        for i in 0..3 {
            if end[i] > start[i] {
                boxmins[i] = start[i] + mins2[i] - 1.0;
                boxmaxs[i] = end[i] + maxs2[i] + 1.0;
            } else {
                boxmins[i] = end[i] + mins2[i] - 1.0;
                boxmaxs[i] = start[i] + maxs2[i] + 1.0;
            }
        }
        let mut clip =
            MoveClip { boxmins, boxmaxs, mins, maxs, mins2, maxs2, start, end, trace, ty, pass };
        self.clip_to_links(ents, 0, &mut clip);
        clip.trace
    }

    /// SV_TestEntityPosition: blocking entity (0 = world) if `e` is stuck.
    pub fn test_entity_position<D: Edicts + ?Sized>(&self, ents: &D, e: Ent) -> Option<Ent> {
        let o = ents.v(e, V::Origin);
        let tr = self.trace(ents, o, ents.v(e, V::Mins), ents.v(e, V::Maxs), o, MoveType::Normal, Some(e));
        tr.startsolid.then(|| tr.ent.unwrap_or(0))
    }
}

#[inline]
fn boxes_touch(amin: Vec3, amax: Vec3, bmin: Vec3, bmax: Vec3) -> bool {
    !(amin.x > bmax.x
        || amin.y > bmax.y
        || amin.z > bmax.z
        || amax.x < bmin.x
        || amax.y < bmin.y
        || amax.z < bmin.z)
}
