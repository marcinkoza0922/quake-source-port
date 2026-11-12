//! Isolated player jump probes. Use the server's movement routines,
//! freezing solid entities and omitting QC callbacks and triggers.
//! A probe cannot collect an item, fire a target or advance the live server.

use crate::nav::Forbid;
use qcore::math::Vec3;
use qserver::{ClientMove, Server};
use qworld::{Edicts, Ent, E, F, V, World, PhysParams, FL_ONGROUND, MOVETYPE_WALK, SOLID_BSP, SOLID_BBOX, SOLID_SLIDEBOX, CONTENTS_EMPTY, CONTENTS_WATER};
use qworld::hull::{HULL_MINS, HULL_MAXS};

pub const DT: f64 = 0.05;
const PLAYER: Ent = 1;
const JUMP_SPEED: f32 = 270.0; // default for synthetic tests; live probes use a measured impulse

#[derive(Clone)]
struct Body {
    vectors: [Vec3; V::MoveDir as usize + 1],
    floats: [f32; F::IdealPitch as usize + 1],
    entities: [Ent; E::GoalEntity as usize + 1],
}

impl Default for Body {
    fn default() -> Self {
        Self { vectors: [Vec3::ZERO; V::MoveDir as usize + 1], floats: [0.0; F::IdealPitch as usize + 1], entities: [0; E::GoalEntity as usize + 1] }
    }
}

struct Bodies(Vec<Body>);
impl Edicts for Bodies {
    fn num_edicts(&self) -> u32 { self.0.len() as u32 }
    fn is_free(&self, e: Ent) -> bool { e as usize >= self.0.len() }
    fn v(&self, e: Ent, f: V) -> Vec3 { self.0[e as usize].vectors[f as usize] }
    fn set_v(&mut self, e: Ent, f: V, v: Vec3) { self.0[e as usize].vectors[f as usize] = v; }
    fn f(&self, e: Ent, f: F) -> f32 { self.0[e as usize].floats[f as usize] }
    fn set_f(&mut self, e: Ent, f: F, v: f32) { self.0[e as usize].floats[f as usize] = v; }
    fn e(&self, e: Ent, f: E) -> Ent { self.0[e as usize].entities[f as usize] }
    fn set_e(&mut self, e: Ent, f: E, v: Ent) { self.0[e as usize].entities[f as usize] = v; }
    fn has_touch(&self, _: Ent) -> bool { false }
    fn touch(&mut self, _: &mut World, _: Ent, _: Ent) {}
    fn blocked(&mut self, _: &mut World, _: Ent, _: Ent) {}
    fn think(&mut self, _: &mut World, _: Ent, _: f64) {}
}

pub struct Probe {
    world: World,
    bodies: Bodies,
    params: PhysParams,
    jump_speed: f32,
    player_gravity: f32,
    triggers: Vec<Forbid>,
}

pub struct Jump {
    pub moves: Vec<ClientMove>,
    /// Position before each move, plus the final landing.
    pub positions: Vec<Vec3>,
}

impl Probe {
    pub fn snapshot(sv: &Server, jump_speed: f32) -> Option<Self> {
        let level = sv.level.as_ref()?;
        let vm = &level.vm;
        let fields = &sv.core.sv.fields;
        let mut probe = Self::new(World::new(level.world.worldmodel.clone()), sv.core.sv.phys);
        probe.jump_speed = jump_speed;
        probe.player_gravity = vm.field_ofs("gravity").map_or(0.0, |f| vm.ent_f(PLAYER, f));
        probe.world.rotate_bmodels = level.world.rotate_bmodels;
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) { continue; }
            let solid = vm.ent_f(e, fields.solid) as i32;
            if solid == qworld::SOLID_TRIGGER && matches!(vm.classname(e).as_str(), "trigger_push" | "trigger_teleport") {
                probe.triggers.push(Forbid { mins: Vec3::from_array(vm.ent_v(e, fields.absmin)), maxs: Vec3::from_array(vm.ent_v(e, fields.absmax)) });
            }
            if e == PLAYER || vm.is_free(e) || !matches!(solid, SOLID_BSP | SOLID_BBOX | SOLID_SLIDEBOX) { continue; }
            let model = vm.ent_f(e, fields.modelindex) as usize;
            if solid == SOLID_BSP {
                let Some(m) = level.world.model(model) else { continue };
                probe.world.set_model(model, Some(m.clone()));
            }
            let n = probe.bodies.num_edicts();
            probe.bodies.0.push(Body::default());
            for (v, f) in [(V::Origin, fields.origin), (V::Mins, fields.mins), (V::Maxs, fields.maxs), (V::Angles, fields.angles)] {
                probe.bodies.set_v(n, v, Vec3::from_array(vm.ent_v(e, f)));
            }
            probe.bodies.set_f(n, F::Solid, solid as f32);
            probe.bodies.set_f(n, F::Flags, vm.ent_f(e, fields.flags));
            probe.bodies.set_f(n, F::ModelIndex, model as f32);
            probe.world.link_edict(&mut probe.bodies, n, false);
        }
        Some(probe)
    }

    fn new(world: World, mut params: PhysParams) -> Self {
        params.frametime = DT;
        let mut bodies = Bodies(vec![Body::default(), Body::default()]);
        bodies.set_f(0, F::Solid, SOLID_BSP as f32);
        bodies.set_f(0, F::ModelIndex, 1.0);
        Self { world, bodies, params, jump_speed: JUMP_SPEED, player_gravity: 0.0, triggers: Vec::new() }
    }

    /// Start at rest, trying immediate takeoff and short run-ups. Only a
    /// supported landing on the destination counts, never flying past it.
    /// Keep a 20-unit margin around forbidden triggers: playback permits that
    /// much positional drift, and touching one would invalidate the probe.
    pub fn jump(&mut self, from: Vec3, to: Vec3, forbid: &[Forbid]) -> Option<Jump> {
        for delay in [0, 2, 4] {
            if let Some(jump) = self.attempt(from, to, delay, forbid) { return Some(jump); }
        }
        None
    }

    fn attempt(&mut self, from: Vec3, to: Vec3, delay: usize, forbid: &[Forbid]) -> Option<Jump> {
        self.world.unlink(PLAYER);
        self.bodies.0[PLAYER as usize] = Body::default();
        self.bodies.set_v(PLAYER, V::Origin, from);
        self.bodies.set_v(PLAYER, V::OldOrigin, from);
        self.bodies.set_v(PLAYER, V::Mins, HULL_MINS[1]);
        self.bodies.set_v(PLAYER, V::Maxs, HULL_MAXS[1]);
        self.bodies.set_f(PLAYER, F::Health, 100.0);
        self.bodies.set_f(PLAYER, F::Gravity, self.player_gravity);
        self.bodies.set_f(PLAYER, F::MoveType, MOVETYPE_WALK as f32);
        self.bodies.set_f(PLAYER, F::Solid, SOLID_SLIDEBOX as f32);
        self.bodies.set_flags(PLAYER, FL_ONGROUND);
        if self.world.trace(&self.bodies, from, HULL_MINS[1], HULL_MAXS[1], from, qworld::MoveType::Normal, Some(PLAYER)).startsolid {
            return None;
        }
        let floor = self.world.trace(&self.bodies, from, HULL_MINS[1], HULL_MAXS[1], from - Vec3::new(0.0, 0.0, 4.0), qworld::MoveType::Normal, Some(PLAYER));
        if floor.fraction == 1.0 || floor.plane_normal.z < 0.7 { return None; }
        self.world.link_edict(&mut self.bodies, PLAYER, false);
        let mut result = Jump { moves: Vec::new(), positions: vec![from] };
        let mut launched = false;
        let mut params = self.params;
        for tick in 0..32 {
            let here = self.bodies.v(PLAYER, V::Origin);
            let d = to - here;
            let mv = ClientMove { angles: [0.0, d.y.atan2(d.x).to_degrees(), 0.0], forward: 320.0, buttons: if tick == delay { 2 } else { 0 }, ..Default::default() };
            params.time = self.params.time + tick as f64 * DT;
            self.bodies.set_v(PLAYER, V::VAngle, Vec3::from_array(mv.angles));
            qworld::user::client_think(&self.world, &mut self.bodies, &params, PLAYER, &qworld::user::UserCmd { forwardmove: mv.forward, ..Default::default() });
            if tick == delay {
                if self.bodies.flags(PLAYER) & FL_ONGROUND == 0 { return None; }
                self.bodies.set_flags(PLAYER, self.bodies.flags(PLAYER) & !FL_ONGROUND);
                let mut velocity = self.bodies.v(PLAYER, V::Velocity);
                velocity.z += self.jump_speed;
                self.bodies.set_v(PLAYER, V::Velocity, velocity);
                launched = true;
            }
            qworld::phys::physics_client_move(&mut self.world, &mut self.bodies, &params, PLAYER);
            let here = self.bodies.v(PLAYER, V::Origin);
            result.moves.push(mv);
            result.positions.push(here);
            let contents = self.world.point_contents(here);
            if !matches!(contents, CONTENTS_EMPTY | CONTENTS_WATER) || self.bodies.f(PLAYER, F::WaterLevel) >= 2.0
                || forbid.iter().chain(&self.triggers).any(|f| f.contains_margin(here, 20.0)) || here.z < from.z.min(to.z) - 64.0 {
                return None;
            }
            if launched && self.bodies.flags(PLAYER) & FL_ONGROUND != 0 {
                let d = here - to;
                return (d.x.hypot(d.y) < 16.0 && d.z.abs() < 4.0).then_some(result);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qworld::hull::{CNode, CollisionModel, HPlane, SubModel};
    use qworld::{CONTENTS_SOLID, CONTENTS_EMPTY};
    use std::sync::Arc;

    fn boxes(boxes: &[(Vec3, Vec3)]) -> World {
        fn hull(boxes: &[(Vec3, Vec3)], planes: &mut Vec<HPlane>, mins: Vec3, maxs: Vec3) -> Vec<CNode> {
            let mut nodes = Vec::new();
            for (i, &(lo, hi)) in boxes.iter().enumerate() {
                let outside = if i + 1 == boxes.len() { CONTENTS_EMPTY } else { ((i + 1) * 6) as i32 };
                for axis in 0..3 {
                    for upper in [false, true] {
                        let mut normal = Vec3::ZERO;
                        normal[axis] = 1.0;
                        let plane = planes.len() as u32;
                        planes.push(HPlane { normal, dist: if upper { hi[axis] - mins[axis] } else { lo[axis] - maxs[axis] }, kind: axis as i32 });
                        let inside = if axis == 2 && upper { CONTENTS_SOLID } else { nodes.len() as i32 + 1 };
                        nodes.push(CNode { plane, children: if upper { [outside, inside] } else { [inside, outside] } });
                    }
                }
            }
            nodes
        }
        let mut cm = CollisionModel::default();
        cm.hull0 = hull(boxes, &mut cm.planes, Vec3::ZERO, Vec3::ZERO);
        cm.clipnodes = hull(boxes, &mut cm.planes, HULL_MINS[1], HULL_MAXS[1]);
        cm.submodels.push(SubModel { mins: Vec3::splat(-4096.0), maxs: Vec3::splat(4096.0), origin: Vec3::ZERO, headnode: [0; 4] });
        World::new(Arc::new(cm))
    }

    #[test]
    fn jump_requires_a_supported_landing() {
        let w = boxes(&[(Vec3::new(-256.0, -128.0, -64.0), Vec3::new(256.0, 128.0, 0.0))]);
        let mut p = Probe::new(w, PhysParams::default());
        let from = Vec3::new(0.0, 0.0, 24.03125);
        let to = Vec3::new(104.0, 0.0, 24.03125);
        let jump = p.jump(from, to, &[]).expect("stock jump on flat ground");
        assert!(jump.positions.iter().any(|p| p.z > from.z + 20.0));
        assert!((jump.positions.last().unwrap().z - to.z).abs() < 1.0);
        assert!(p.jump(from, Vec3::new(104.0, 0.0, 64.0), &[]).is_none());
        assert!(p.jump(Vec3::new(0.0, 0.0, 80.0), to, &[]).is_none());
    }

    #[test]
    fn low_ceiling_and_forbidden_trigger_reject_jump() {
        let floor = (Vec3::new(-256.0, -128.0, -64.0), Vec3::new(256.0, 128.0, 0.0));
        let ceiling = (Vec3::new(-256.0, -128.0, 60.0), Vec3::new(256.0, 128.0, 100.0));
        let from = Vec3::new(0.0, 0.0, 24.03125);
        let to = Vec3::new(104.0, 0.0, 24.03125);
        let left = (Vec3::new(-256.0, -128.0, -64.0), Vec3::new(-32.0, 128.0, 0.0));
        let right = (Vec3::new(64.0, -128.0, -64.0), Vec3::new(320.0, 128.0, 0.0));
        let mut p = Probe::new(boxes(&[left, right, ceiling]), PhysParams::default());
        assert!(p.jump(Vec3::new(-64.0, 0.0, from.z), Vec3::new(80.0, 0.0, to.z), &[]).is_none());
        let mut p = Probe::new(boxes(&[floor]), PhysParams::default());
        let forbid = Forbid { mins: Vec3::new(40.0, -64.0, 0.0), maxs: Vec3::new(60.0, 64.0, 128.0) };
        assert!(p.jump(from, to, &[forbid]).is_none());
        assert!(p.jump(from, to, &[]).is_some()); // a failed probe cannot poison the next one
    }

    #[test]
    fn crosses_gap_but_cannot_land_in_midair() {
        let w = boxes(&[
            (Vec3::new(-256.0, -128.0, -64.0), Vec3::new(-32.0, 128.0, 0.0)),
            (Vec3::new(64.0, -128.0, -64.0), Vec3::new(320.0, 128.0, 0.0)),
        ]);
        let mut p = Probe::new(w, PhysParams::default());
        let from = Vec3::new(-64.0, 0.0, 24.03125);
        assert!(p.jump(from, Vec3::new(80.0, 0.0, 24.03125), &[]).is_some());
        assert!(p.jump(from, Vec3::new(16.0, 0.0, 24.03125), &[]).is_none());
    }
    #[test]
    fn nearby_trigger_has_room_for_playback_drift() {
        let w = boxes(&[(Vec3::new(-256.0, -128.0, -64.0), Vec3::new(256.0, 128.0, 0.0))]);
        let mut p = Probe::new(w, PhysParams::default());
        let from = Vec3::new(0.0, 0.0, 24.03125);
        let to = Vec3::new(104.0, 0.0, 24.03125);
        let trigger = Forbid { mins: Vec3::new(130.0, -64.0, 0.0), maxs: Vec3::new(140.0, 64.0, 128.0) };
        let jump = p.jump(from, to, &[]).unwrap();
        assert!(jump.positions.iter().all(|&p| !trigger.contains(p)));
        assert!(p.jump(from, to, &[trigger]).is_none());
    }

    #[test]
    fn frozen_monster_blocks_the_probe() {
        let w = boxes(&[(Vec3::new(-256.0, -128.0, -64.0), Vec3::new(256.0, 128.0, 0.0))]);
        let mut p = Probe::new(w, PhysParams::default());
        let monster = p.bodies.num_edicts();
        p.bodies.0.push(Body::default());
        p.bodies.set_f(monster, F::Solid, SOLID_SLIDEBOX as f32);
        p.bodies.set_v(monster, V::Mins, Vec3::new(24.0, -128.0, 0.0));
        p.bodies.set_v(monster, V::Maxs, Vec3::new(48.0, 128.0, 256.0));
        p.world.link_edict(&mut p.bodies, monster, false);
        let from = Vec3::new(0.0, 0.0, 24.03125);
        let to = Vec3::new(104.0, 0.0, 24.03125);
        assert!(p.jump(from, to, &[]).is_none());
        p.world.unlink(monster);
        assert!(p.jump(from, to, &[]).is_some());
    }

}
