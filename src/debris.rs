//! Client-side debris physics (`r_debris`): ragdolls for corpses, tumbling
//! gibs, and metal shards from exploding boxes. Purely cosmetic: the server
//! never sees any of it. Server entities that a ragdoll or gib replaces are
//! hidden through [`ClientState::debris_owned`] and released (shown again)
//! as soon as the server changes them. Bodies collide with the world and,
//! optionally, with moving brush entities (doors, lifts), which also carry
//! them ([`scene`]). Bullets, nails and explosions push things along the
//! inferred line of fire ([`fire`]); an explosion right on a ragdoll gibs it
//! with the game's own gib models ([`gibs`]). [`Settings`] holds the
//! `r_debris_*` sub-options.
//!
//! When `r_debris` is 0, [`Debris::update`] returns at once and owns nothing.
use std::collections::HashMap;
use std::rc::Rc;

use qclient::ClientState;
use qcore::math::Vec3;
use qrender::{DynPose, ModelId, RenderEntity, Renderer};
use qworld::CollisionModel;

use crate::assets::{Kind, Level};

mod body;
mod events;
mod fire;
mod fragment;
mod gibs;
mod info;
pub(crate) mod math;
mod ragdoll;
mod rig;
mod scene;
mod settings;
#[cfg(test)]
mod tests;

use body::Body;
use info::ModelInfo;
use math::{Quat, Rng, entity_rotation};
use ragdoll::{Ragdoll, mat_to_quat};
use rig::Rig;
use scene::{Mover, Scene};
use settings::{Tier, tier_for};
pub use settings::{Quality, Settings, Start};

/// Fixed simulation step (High tier).
pub const STEP: f32 = 1.0 / 60.0;
const MAX_STEPS: usize = 6;
const MAX_GIBS: usize = 128;
const MAX_SHARDS: usize = 96;
/// Seconds over which a retiring corpse fades to zero alpha.
const CORPSE_FADE: f32 = 1.5;
/// A corpse is taken over after holding a death frame this long (seconds),
/// or sooner once it reaches the last frame of a death sequence.
const HOLD_FINAL: f64 = 0.12;
const HOLD_ANY: f64 = 1.0;
/// Explosions this recent fling corpses that are taken over.
const BOOM_MEMORY: f64 = 0.6;
/// Bullet and nail hits are remembered this long, so the killing shot
/// still pushes a corpse taken over at death.
const SHOT_MEMORY: f64 = 0.3;
/// Client-side gibbing waits this long so a server-side gib wins.
const GIB_DELAY: f64 = 0.1;
const BLOOD: i32 = 73;
/// FTE trail keys for bodies, clear of entity numbers.
const TRAIL_KEYS: i32 = 0x4000_0000;
/// An explosion this close to a ragdoll's bones gibs it.
const DIRECT_HIT: f32 = 28.0;

/// World collision for the simulation: (fraction, endpos, normal).
pub trait Collide {
    fn trace(&self, a: Vec3, b: Vec3) -> (f32, Vec3, Vec3);
    fn solid(&self, p: Vec3) -> bool;
    fn water(&self, p: Vec3) -> bool;
}

impl Collide for CollisionModel {
    fn trace(&self, a: Vec3, b: Vec3) -> (f32, Vec3, Vec3) {
        let t = self.hull(0, 0).trace(a, b);
        if t.allsolid || t.startsolid {
            return (0.0, a, Vec3::new(0.0, 0.0, 1.0));
        }
        (t.fraction, t.endpos, t.plane_normal)
    }
    fn solid(&self, p: Vec3) -> bool {
        self.point_contents(p) == qworld::CONTENTS_SOLID
    }
    fn water(&self, p: Vec3) -> bool {
        matches!(self.point_contents(p), qworld::CONTENTS_WATER | qworld::CONTENTS_SLIME | qworld::CONTENTS_LAVA)
    }
}

struct Corpse {
    ent: usize,
    model: usize,
    id: ModelId,
    skin: u32,
    colormap: Option<[u8; 2]>,
    alpha: f32,
    life: f32,
    doll: Ragdoll,
    /// Pending client gib: (when, explosion centre).
    gib: Option<(f64, Vec3)>,
}

#[derive(Clone, Copy)]
struct Track {
    model: usize,
    frame: i32,
    since: f64,
    /// Mixed start: decided on the first death frame whether to go limp.
    limp: Option<bool>,
}

#[derive(Default)]
pub struct Debris {
    active: bool,
    cm: Option<Rc<CollisionModel>>,
    infos: HashMap<usize, Option<Rc<ModelInfo>>>,
    rigs: HashMap<(usize, i32, usize), Option<Rc<Rig>>>,
    tracks: HashMap<usize, Track>,
    corpses: Vec<Corpse>,
    bodies: Vec<Body>,
    /// Corpses and expired server gibs hidden until the server changes them:
    /// `None` hides any death frame of `model`; `Some(frame)` hides that frame.
    gibbed: HashMap<usize, (usize, Option<i32>)>,
    /// Small brush entities last frame: entity → (model, world bounds).
    brushes: HashMap<usize, (usize, Vec3, Vec3)>,
    brush_bounds: HashMap<usize, Option<(Vec3, Vec3)>>,
    booms: Vec<(Vec3, f64)>,
    /// Recent bullet/nail hits: (line-of-fire origin, impact, time).
    shots: Vec<(Vec3, Vec3, f64)>,
    shard: Option<ModelId>,
    /// The game's gib models: (name stem, model).
    gib_models: Option<Rc<[(String, ModelId)]>>,
    /// Head model per corpse model index.
    heads: HashMap<usize, Option<ModelId>>,
    /// Inline brush entities this frame.
    movers: Vec<Mover>,
    /// Fast projectiles last frame: (origin, velocity).
    missiles: Vec<(Vec3, Vec3)>,
    /// Recent muzzle flashes: (eye, or None for the player's view; time; entity).
    flashes: Vec<(Option<Vec3>, f64, usize)>,
    rng: Option<Rng>,
    step: f32,
    acc: f32,
    last_time: f64,
    blood_bursts: Vec<Vec3>,
}

/// Per-frame inputs from the host.
pub struct Frame<'a> {
    pub cl: &'a mut ClientState,
    pub lv: &'a mut Level,
    pub fs: &'a qcore::fs::Fs,
    pub r: &'a mut Renderer,
    pub view: Vec3,
    pub gravity: f32,
    pub set: Settings,
}

impl Debris {
    /// Forgets everything (map change, or `r_debris 0`), showing any hidden
    /// server entities again.
    pub fn clear(&mut self, cl: &mut ClientState) {
        let shard = self.shard.take();
        *self = Debris::default();
        self.shard = shard;
        cl.debris_owned.clear();
    }

    /// New map: model indices and the collision model change.
    pub fn new_map(&mut self, cl: &mut ClientState) {
        self.clear(cl);
        self.shard = None;
    }

    pub fn update(&mut self, enabled: bool, f: Frame<'_>) {
        self.blood_bursts.clear();
        if !enabled {
            if self.active {
                self.clear(f.cl);
            }
            return;
        }
        let Frame { cl, lv, fs, r, view, gravity, set } = f;
        if self.cm.is_none() {
            let Some(bsp) = lv.bsp.as_ref() else { return };
            self.cm = Some(Rc::new(CollisionModel::from_bsp(bsp)));
            self.last_time = cl.time;
        }
        self.active = true;
        let tier = tier_for(&set);
        if self.step <= 0.0 {
            self.step = tier.step;
        } else if (self.step - tier.step).abs() > 1e-6 {
            for c in &mut self.corpses {
                c.doll.set_step(tier.step);
            }
            self.step = tier.step;
        }
        let time = cl.time;
        let dt = (time - self.last_time).clamp(0.0, 0.1) as f32;
        self.last_time = time;
        if cl.debris_owned.len() < cl.entities.len() {
            cl.debris_owned.resize(cl.entities.len(), false);
        }
        if !set.ragdolls {
            for c in self.corpses.drain(..) {
                cl.debris_owned[c.ent] = false;
            }
        }
        self.release(cl);
        self.enforce_tier(cl, &tier);
        // impacts first: a killing hit arrives with the death frame, and
        // take_over replays remembered hits on the new ragdoll
        self.impacts(cl, view, time, &set);
        self.take_over(cl, lv, fs, time, &set, &tier);
        self.update_movers(cl, tier.movers);
        if set.shrapnel {
            self.brush_breaks(cl, lv, fs, r, &tier);
        }
        self.track_shooters(cl, lv, time);
        self.gib_pending(cl, lv, fs, r, time, &tier);
        self.simulate(cl, dt, gravity, &tier);
    }

    fn enforce_tier(&mut self, cl: &mut ClientState, tier: &Tier) {
        for c in &mut self.corpses {
            c.life = c.life.min(tier.corpse_life);
        }
        for b in &mut self.bodies {
            let cap = if b.ent.is_some() {
                tier.gib_life
            } else if b.trail {
                tier.client_gib_life.0 + tier.client_gib_life.1
            } else {
                tier.shard_life.0 + tier.shard_life.1
            };
            b.life = b.life.min(cap);
        }
        self.trim_shards(tier.max_shards);
        self.trim_client_gibs(tier.max_gibs);
        while self.corpses.len() > tier.max_ragdolls {
            self.retire_oldest_corpse(cl);
        }
    }

    fn retire_oldest_corpse(&mut self, cl: &mut ClientState) {
        let Some((idx, _)) = self
            .corpses
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.life.total_cmp(&b.life))
        else {
            return;
        };
        let c = self.corpses.swap_remove(idx);
        let mtime = cl.mtime[0];
        let live = cl.entities.get(c.ent).is_some_and(|e| e.model == c.model && e.msgtime == mtime);
        if live {
            self.gibbed.insert(c.ent, (c.model, None));
        } else {
            cl.debris_owned[c.ent] = false;
        }
    }

    fn trim_shards(&mut self, max_shards: usize) {
        let shards = self.bodies.iter().filter(|b| b.ent.is_none() && !b.trail).count();
        if shards > max_shards {
            let mut lives: Vec<f32> = self.bodies.iter().filter(|b| b.ent.is_none() && !b.trail).map(|b| b.life).collect();
            lives.sort_by(f32::total_cmp);
            let cut = lives[shards - max_shards];
            self.bodies.retain(|b| b.ent.is_some() || b.trail || b.life >= cut);
        }
    }

    fn trim_client_gibs(&mut self, max_gibs: usize) {
        let gibs = self.bodies.iter().filter(|b| b.ent.is_none() && b.trail).count();
        if gibs > max_gibs {
            let mut lives: Vec<f32> = self.bodies.iter().filter(|b| b.ent.is_none() && b.trail).map(|b| b.life).collect();
            lives.sort_by(f32::total_cmp);
            let cut = lives[gibs - max_gibs];
            self.bodies.retain(|b| b.ent.is_some() || !b.trail || b.life >= cut);
        }
    }

    fn rng(&mut self) -> &mut Rng {
        self.rng.get_or_insert(Rng(0x9e37_79b9))
    }

    fn info(&mut self, cl: &ClientState, fs: &qcore::fs::Fs, model: usize) -> Option<Rc<ModelInfo>> {
        self.infos
            .entry(model)
            .or_insert_with(|| cl.model_name(model).and_then(|n| ModelInfo::load(fs, n)).map(Rc::new))
            .clone()
    }

    /// Lets go of anything the server has changed or removed.
    fn release(&mut self, cl: &mut ClientState) {
        let infos = &self.infos;
        let is_death = |model: usize, frame: i32| {
            infos.get(&model).and_then(|i| i.as_ref()).is_some_and(|i| i.is_death(frame))
        };
        let owned = &mut cl.debris_owned;
        let ents = &cl.entities;
        let mtime = cl.mtime[0];
        let live = |ent: usize, model: usize| ents.get(ent).filter(|e| e.model == model && e.msgtime == mtime);
        self.corpses.retain(|c| {
            let keep = live(c.ent, c.model).is_some_and(|e| is_death(c.model, e.frame));
            if !keep {
                owned[c.ent] = false;
            }
            keep
        });
        let gibbed = &mut self.gibbed;
        self.bodies.retain(|b| {
            let Some((ent, model)) = b.ent else { return b.life > 0.0 };
            let Some(e) = live(ent, model) else {
                owned[ent] = false;
                return false;
            };
            if b.life <= 0.0 {
                gibbed.insert(ent, (model, Some(e.frame)));
                return false;
            }
            true
        });
        self.gibbed.retain(|&ent, &mut (model, frame)| {
            let keep = live(ent, model).is_some_and(|e| match frame {
                Some(f) => e.frame == f,
                None => is_death(model, e.frame),
            });
            if !keep {
                owned[ent] = false;
            }
            keep
        });
    }

    fn simulate(&mut self, cl: &mut ClientState, dt: f32, gravity: f32, tier: &Tier) {
        let Some(cm) = self.cm.clone() else { return };
        let scene = Scene { cm: &cm, movers: &self.movers };
        self.acc = (self.acc + dt).min(tier.step * tier.max_steps as f32);
        while self.acc >= tier.step {
            self.acc -= tier.step;
            for c in &mut self.corpses {
                c.doll.step(gravity, &scene, tier.iters, tier.skip_down);
            }
            for b in &mut self.bodies {
                b.step(gravity, &scene, tier.step, tier.skip_down);
            }
        }
        for c in &mut self.corpses {
            if c.life.is_finite() && (c.doll.asleep || c.life <= CORPSE_FADE) {
                c.life -= dt;
            }
        }
        let mtime = cl.mtime[0];
        let mut i = 0;
        while i < self.corpses.len() {
            if self.corpses[i].life <= 0.0 {
                let c = self.corpses.swap_remove(i);
                let live = cl.entities.get(c.ent).is_some_and(|e| e.model == c.model && e.msgtime == mtime);
                if live {
                    self.gibbed.insert(c.ent, (c.model, None));
                } else {
                    cl.debris_owned[c.ent] = false;
                }
            } else {
                i += 1;
            }
        }
        for (k, b) in self.bodies.iter_mut().enumerate() {
            if b.trail && !b.asleep && (b.pos - b.trail_org).length_sq() > 9.0 {
                let (a, z) = (b.trail_org.to_array(), b.pos.to_array());
                // FTE trail state is per key; bounded by the body cap
                if !cl.fte_trail("TR_BLOOD", a, z, TRAIL_KEYS + k as i32) {
                    cl.rocket_trail(a, z, 2);
                }
                b.trail_org = b.pos;
            }
        }
    }

    /// Explosive dismemberments performed by the cosmetic simulation.
    pub fn blood_bursts(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.blood_bursts.iter().copied()
    }

    /// Moving fleshy debris, including client-created gibs without an edict.
    pub fn blood_sources(&self) -> impl Iterator<Item = (u32, Vec3)> + '_ {
        self.bodies.iter().enumerate().filter(|(_, b)| b.trail && !b.asleep)
            .map(|(i, b)| (0x8000_0000 + i as u32, b.pos))
    }

    /// Render entities for everything simulated; ragdoll poses go to `verts`.
    /// When `frustum` is `Some`, offscreen ragdolls skip CPU skinning (`pack()`).
    pub fn entities(&mut self, out: &mut Vec<RenderEntity>, verts: &mut Vec<u32>, frustum: Option<&qrender::ViewFrustum>) {
        if !self.active {
            return;
        }
        for c in &mut self.corpses {
            if let Some(f) = frustum {
                let (center, radius) = c.doll.bounds();
                if !f.contains_sphere(center.to_array(), radius) {
                    continue;
                }
            }
            let fade = (c.life / CORPSE_FADE).clamp(0.0, 1.0);
            if fade <= 0.0 {
                continue;
            }
            let p = c.doll.pack();
            let ofs = verts.len() as u32;
            verts.extend_from_slice(&p.verts);
            out.push(RenderEntity {
                model: c.id,
                key: c.ent as u32,
                origin: p.center.to_array(),
                skin: c.skin,
                colormap: c.colormap,
                alpha: c.alpha * fade,
                dyn_pose: Some(DynPose { ofs, origin: p.origin.to_array(), scale: p.scale.to_array(), radius: p.radius }),
                ..Default::default()
            });
        }
        for b in &self.bodies {
            if let Some(f) = frustum
                && !f.contains_sphere(b.pos.to_array(), b.radius + 16.0)
            {
                continue;
            }
            out.push(RenderEntity {
                model: b.model,
                key: b.ent.map_or(0, |(i, _)| i as u32),
                origin: b.pos.to_array(),
                angles: b.angles(),
                frame: b.frame,
                prev_frame: b.frame,
                lerp: 1.0,
                skin: b.skin,
                alpha: b.life.clamp(0.0, 1.0),
                ..Default::default()
            });
        }
    }
}
