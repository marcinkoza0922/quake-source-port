//! Particle simulation (r_part_fte.c: trailstates, RunParticleEffectState,
//! ParticleTrailSpawn, UpdateParticle, UpdateParticleTypes, decals, rain).
//!
//! Particles, beam segments and clipped decals live in index arenas; each
//! type keeps index-linked lists like the C code, because beams refer to
//! particles and dead particles are only recycled at the end of a frame.
use crate::types::*;
use crate::util::{Rng, pcos, perpendicular, psin, rotate_around, vector_vectors};
use crate::{FileSource, Registry, TypeId};
use qcore::anorms::{AVERTEXNORMALS, NUMVERTEXNORMALS};
use qcore::math::Vec3;
use std::collections::HashMap;
use std::f32::consts::PI;

const NIL: u32 = u32::MAX;

pub const BS_LASTSEG: u8 = 0x1;
pub const BS_DEAD: u8 = 0x2;
pub const BS_NODRAW: u8 = 0x4;

pub const MAX_PARTICLES: usize = 1 << 18;
pub const MAX_DECALS: usize = 1 << 18;
pub const MAX_BEAMSEGS: usize = 1 << 11;
pub const MAX_TRAILSTATES: usize = 1 << 10;

#[derive(Clone, Debug, Default)]
pub struct Particle {
    next: u32,
    pub die: f32,
    pub org: Vec3,
    pub rgba: [f32; 4],
    pub scale: f32,
    pub s1: f32,
    pub t1: f32,
    pub s2: f32,
    pub t2: f32,
    /// Previous traced position (throttles traces).
    pub oldorg: Vec3,
    pub vel: Vec3,
    pub angle: f32,
    pub rotationspeed: f32,
    /// Next emit time (types with `emitinterval` >= 0).
    pub nextemit: f32,
    /// Trail state (types with `emitinterval` < 0).
    pub trail: Option<TrailHandle>,
}

#[derive(Clone, Debug, Default)]
pub struct BeamSeg {
    next: u32,
    /// Particle index.
    pub p: u32,
    pub flags: u8,
    pub dir: Vec3,
    pub texture_s: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Decal {
    next: u32,
    pub die: f32,
    /// 0: world, >0: entity the vertices are relative to.
    pub entity: i32,
    pub vertex: [Vec3; 3],
    pub texcoords: [[f32; 2]; 3],
    pub valpha: [f32; 3],
    pub rgba: [f32; 4],
}

/// Owner-side reference to a trailstate (the C `trailstate_t **` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrailHandle {
    idx: u16,
    serial: u32,
}

#[derive(Clone, Debug, Default)]
struct TrailState {
    serial: u32,
    assoc: Option<TrailHandle>,
    lastbeam: u32,
    /// lastdist / statetime.
    state1: f32,
    /// laststop / emittime.
    state2: f32,
}

#[derive(Clone, Copy, Debug)]
struct TypeRt {
    particles: u32,
    beams: u32,
    decals: u32,
    in_run: bool,
    countoverflow: f32,
}

impl Default for TypeRt {
    fn default() -> Self {
        TypeRt { particles: NIL, beams: NIL, decals: NIL, in_run: false, countoverflow: 0.0 }
    }
}

/// Cvar-backed settings.
#[derive(Clone, Debug)]
pub struct Settings {
    /// r_part_density
    pub density: f32,
    /// r_part_contentswitch
    pub contentswitch: bool,
    /// r_bouncysparks
    pub bouncysparks: bool,
    /// r_particle_tracelimit (per frame)
    pub trace_limit: u32,
    /// r_part_rain
    pub rain: bool,
    /// r_part_rain_quantity
    pub rain_quantity: f32,
    /// r_decal_noperpendicular
    pub decal_noperpendicular: bool,
    /// r_lightflicker
    pub lightflicker: bool,
    /// r_part_maxparticles
    pub max_particles: usize,
    /// r_part_maxdecals
    pub max_decals: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            density: 1.0,
            contentswitch: true,
            bouncysparks: true,
            trace_limit: 16_777_216,
            rain: true,
            rain_quantity: 1.0,
            decal_noperpendicular: true,
            lightflicker: true,
            max_particles: 65536,
            max_decals: 8192,
        }
    }
}

/// Result of a world trace (CL_TraceLine).
#[derive(Clone, Copy, Debug)]
pub struct Trace {
    /// 1 when nothing was hit.
    pub frac: f32,
    pub endpos: Vec3,
    pub normal: Vec3,
    /// Entity hit (0 = world).
    pub entity: i32,
}

/// What the particle system needs from the client world.
pub trait World {
    /// Whether a world model is loaded (`cl.worldmodel`).
    fn has_world(&self) -> bool {
        true
    }
    fn trace_line(&self, start: Vec3, end: Vec3) -> Trace;
    /// FTECONTENTS_* mask at `p`.
    fn point_contents(&self, p: Vec3) -> u32;
    /// Origin of a brush entity with a model, None if it has none.
    fn entity_origin(&self, _entity: i32) -> Option<Vec3> {
        None
    }
    /// Calls `out` with each surface polygon (in the model's space) of
    /// `entity`'s model (0 = world) near `center` within `radius`, skipping
    /// sky/water, and with `noperp` only surfaces facing against `normal`
    /// (dot(surface normal, normal) <= -0.5). Q1BSP_ClipDecalToNodes.
    fn decal_polys(&self, _entity: i32, _center: Vec3, _radius: f32, _normal: Vec3, _noperp: bool, _out: &mut dyn FnMut(&[Vec3])) {}
}

/// A world with nothing in it.
pub struct EmptyWorld;

impl World for EmptyWorld {
    fn has_world(&self) -> bool {
        false
    }
    fn trace_line(&self, _start: Vec3, end: Vec3) -> Trace {
        Trace { frac: 1.0, endpos: end, normal: Vec3::ZERO, entity: 0 }
    }
    fn point_contents(&self, _p: Vec3) -> u32 {
        FTECONTENTS_EMPTY
    }
}

/// Receives renderable state during [`System::frame`].
pub trait Sink {
    fn particle(&mut self, _ty: TypeId, _p: &Particle) {}
    /// Segment from `p` (of `seg`) to `q` (of `next`).
    fn beam(&mut self, _ty: TypeId, _seg: &BeamSeg, _next: &BeamSeg, _p: &Particle, _q: &Particle) {}
    fn decal(&mut self, _ty: TypeId, _d: &Decal) {}
}

pub struct NullSink;
impl Sink for NullSink {}

#[derive(Clone, Debug, PartialEq)]
pub struct DlightSpawn {
    pub key: i32,
    pub org: Vec3,
    pub radius: f32,
    /// Absolute client time.
    pub die: f64,
    pub decay: f32,
    pub rgb: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundSpawn {
    pub name: String,
    pub org: Vec3,
    pub vol: f32,
    pub atten: f32,
}

/// One rain-emitting triangle (skytris_t).
#[derive(Clone, Debug)]
pub struct SkyTri {
    pub org: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub area: f32,
    pub nexttime: f64,
    pub ptype: TypeId,
    /// Outward surface normal (already flipped for SURF_PLANEBACK).
    pub normal: Vec3,
}

/// Fans a surface polygon into sky triangles (R_Part_SkyTri).
pub fn sky_tris_for_polygon(verts: &[Vec3], normal: Vec3, ptype: TypeId) -> Vec<SkyTri> {
    let mut out = Vec::new();
    if verts.len() >= 64 {
        return out;
    }
    let v1 = 0;
    let mut v2 = 1;
    for v3 in 2..verts.len() {
        let org = verts[v1];
        let x = verts[v2] - org;
        let y = verts[v3] - org;
        let (xm, ym) = (x.length(), y.length());
        let theta = (x.dot(y) / (xm * ym)).acos();
        let area = psin(theta) * xm * ym;
        if area > 0.0 {
            out.push(SkyTri { org, x, y, area, nexttime: 0.0, ptype, normal });
        }
        v2 = v3;
    }
    out
}

#[derive(Debug, Default)]
struct SkyModel {
    tris: Vec<SkyTri>,
    skytime: f64,
}

/// A brush entity whose sky triangles emit rain this frame.
#[derive(Clone, Copy, Debug)]
pub struct RainEmitter {
    pub model: usize,
    /// forward, right, up (AngleVectors).
    pub axis: [Vec3; 3],
    pub origin: Vec3,
}

/// Per-frame inputs to [`System::frame`].
#[derive(Clone, Debug, Default)]
pub struct FrameInput {
    pub cl_time: f64,
    pub vieworg: Vec3,
    pub rain: Vec<RainEmitter>,
}

/// Classic effect fallbacks (PE_DEFAULT etc.).
#[derive(Clone, Copy, Debug, Default)]
pub struct PeTypes {
    pub default: Option<TypeId>,
    pub size2: Option<TypeId>,
    pub size3: Option<TypeId>,
    pub defaulttrail: Option<TypeId>,
}

struct DeferredTrail {
    start: Vec3,
    end: Vec3,
    ty: TypeId,
    /// Particle owning the trailstate.
    owner: Option<u32>,
}

struct DeferredDecal {
    ty: TypeId,
    entity: i32,
    center: Vec3,
    normal: Vec3,
    scale: f32,
}

struct DecalCtx {
    ty: TypeId,
    entity: i32,
    center: Vec3,
    normal: Vec3,
    tangent1: Vec3,
    tangent2: Vec3,
    scale0: f32,
    scale1: f32,
    scale2: f32,
    bias1: f32,
    bias2: f32,
}

/// The live particle state.
pub struct System {
    pub settings: Settings,
    parts: Vec<Particle>,
    free_parts: Vec<u32>,
    beams: Vec<BeamSeg>,
    free_beams: Vec<u32>,
    decals: Vec<Decal>,
    free_decals: Vec<u32>,
    trails: Vec<TrailState>,
    ts_cycle: usize,
    rt: Vec<TypeRt>,
    run_list: Vec<TypeId>,
    /// Particle clock (advances by the clamped frame time).
    pub particletime: f32,
    oldtime: f64,
    cl_time: f64,
    frametime: f32,
    flurrytime: f32,
    doflurry: bool,
    rng: Rng,
    urng: Rng,
    avelocities: Vec<[f32; 2]>,
    particle_recycle: usize,
    decal_recycle: usize,
    flicker: (i64, u32),
    traces_used: u32,
    kill_list: Vec<u32>,
    q_effects: Vec<(Vec3, Vec3, f32, TypeId)>,
    q_trails: Vec<DeferredTrail>,
    q_decals: Vec<DeferredDecal>,
    sky: HashMap<usize, SkyModel>,
    pub pe: PeTypes,
    /// Dynamic lights requested by effects since the last drain.
    pub dlights: Vec<DlightSpawn>,
    /// Sounds requested by effects since the last drain.
    pub sounds: Vec<SoundSpawn>,
}

impl Default for System {
    fn default() -> Self {
        System::new(Settings::default())
    }
}

fn rgb_rand(rng: &mut Rng, t: &PartType) -> [f32; 3] {
    let s = rng.frandom();
    t.rgbrandsync.map(|sync| s * sync + rng.frandom() * (1.0 - sync))
}

impl System {
    pub fn new(settings: Settings) -> Self {
        let mut trails = vec![TrailState::default(); MAX_TRAILSTATES];
        for t in &mut trails {
            t.lastbeam = NIL;
        }
        System {
            settings,
            parts: Vec::new(),
            free_parts: Vec::new(),
            beams: Vec::new(),
            free_beams: Vec::new(),
            decals: Vec::new(),
            free_decals: Vec::new(),
            trails,
            ts_cycle: 0,
            rt: Vec::new(),
            run_list: Vec::new(),
            particletime: 0.0,
            oldtime: 0.0,
            cl_time: 0.0,
            frametime: 0.0,
            flurrytime: 0.0,
            doflurry: false,
            rng: Rng::new(0x1234_5678),
            urng: Rng::new(0x9e37_79b9),
            avelocities: Vec::new(),
            particle_recycle: 0,
            decal_recycle: 0,
            flicker: (-1, 0),
            traces_used: 0,
            kill_list: Vec::new(),
            q_effects: Vec::new(),
            q_trails: Vec::new(),
            q_decals: Vec::new(),
            sky: HashMap::new(),
            pe: PeTypes::default(),
            dlights: Vec::new(),
            sounds: Vec::new(),
        }
    }

    pub fn seed(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.urng = Rng::new(seed.wrapping_mul(2_654_435_761));
    }

    fn max_particles(&self) -> usize {
        self.settings.max_particles.clamp(1, MAX_PARTICLES)
    }
    fn max_decals(&self) -> usize {
        self.settings.max_decals.clamp(1, MAX_DECALS)
    }

    /// PScript_ClearParticles: drops everything, resets the clock.
    pub fn clear(&mut self, cl_time: f64) {
        let s = self.settings.clone();
        let sky = std::mem::take(&mut self.sky);
        let pe = self.pe;
        *self = System::new(s);
        self.sky = sky;
        self.pe = pe;
        self.particletime = cl_time as f32;
        self.oldtime = cl_time;
        self.cl_time = cl_time;
    }

    /// Re-resolves the PE_* fallbacks (done when effects change).
    pub fn refresh_pe(&mut self, reg: &mut Registry, files: &mut dyn FileSource) {
        self.pe = PeTypes {
            default: reg.find("PE_DEFAULT", files),
            size2: reg.find("PE_SIZE2", files),
            size3: reg.find("PE_SIZE3", files),
            defaulttrail: reg.find("PE_DEFAULTTRAIL", files),
        };
    }

    pub fn set_sky_tris(&mut self, model: usize, mut tris: Vec<SkyTri>) {
        let m = self.sky.entry(model).or_default();
        for t in &mut tris {
            t.nexttime = m.skytime;
        }
        m.tris = tris;
    }

    pub fn clear_sky_tris(&mut self) {
        self.sky.clear();
    }

    // ---- statistics / access ----

    /// Number of live (allocated) particles.
    pub fn num_particles(&self) -> usize {
        self.parts.len() - self.free_parts.len()
    }
    pub fn num_decals(&self) -> usize {
        self.decals.len() - self.free_decals.len()
    }
    pub fn num_beams(&self) -> usize {
        self.beams.len() - self.free_beams.len()
    }
    /// Types currently in the run list.
    pub fn run_list(&self) -> &[TypeId] {
        &self.run_list
    }
    /// Live particles of a type, newest first.
    pub fn particles_of(&self, ty: TypeId) -> impl Iterator<Item = &Particle> {
        let mut i = self.rt.get(ty).map_or(NIL, |r| r.particles);
        std::iter::from_fn(move || {
            if i == NIL {
                return None;
            }
            let p = &self.parts[i as usize];
            i = p.next;
            Some(p)
        })
    }
    pub fn decals_of(&self, ty: TypeId) -> impl Iterator<Item = &Decal> {
        let mut i = self.rt.get(ty).map_or(NIL, |r| r.decals);
        std::iter::from_fn(move || {
            if i == NIL {
                return None;
            }
            let d = &self.decals[i as usize];
            i = d.next;
            Some(d)
        })
    }
    pub fn beams_of(&self, ty: TypeId) -> impl Iterator<Item = &BeamSeg> {
        let mut i = self.rt.get(ty).map_or(NIL, |r| r.beams);
        std::iter::from_fn(move || {
            if i == NIL {
                return None;
            }
            let b = &self.beams[i as usize];
            i = b.next;
            Some(b)
        })
    }
    pub fn particle(&self, idx: u32) -> &Particle {
        &self.parts[idx as usize]
    }

    // ---- arenas ----

    fn ensure_rt(&mut self, n: usize) {
        if self.rt.len() < n {
            self.rt.resize(n, TypeRt::default());
        }
    }

    fn has_free_particle(&self) -> bool {
        !self.free_parts.is_empty() || self.parts.len() < self.max_particles()
    }
    fn has_free_beam(&self) -> bool {
        !self.free_beams.is_empty() || self.beams.len() < MAX_BEAMSEGS
    }
    fn has_free_decal(&self) -> bool {
        !self.free_decals.is_empty() || self.decals.len() < self.max_decals()
    }

    /// Takes a particle and links it at the head of `ty`'s list.
    fn alloc_particle(&mut self, ty: TypeId) -> u32 {
        let i = match self.free_parts.pop() {
            Some(i) => i,
            None => {
                self.parts.push(Particle::default());
                (self.parts.len() - 1) as u32
            }
        };
        let p = &mut self.parts[i as usize];
        *p = Particle { next: self.rt[ty].particles, ..Particle::default() };
        self.rt[ty].particles = i;
        i
    }

    fn alloc_beam(&mut self) -> u32 {
        match self.free_beams.pop() {
            Some(i) => i,
            None => {
                self.beams.push(BeamSeg::default());
                (self.beams.len() - 1) as u32
            }
        }
    }

    fn alloc_decal(&mut self, ty: TypeId) -> u32 {
        let i = match self.free_decals.pop() {
            Some(i) => i,
            None => {
                self.decals.push(Decal::default());
                (self.decals.len() - 1) as u32
            }
        };
        self.decals[i as usize] = Decal { next: self.rt[ty].decals, ..Decal::default() };
        self.rt[ty].decals = i;
        i
    }

    fn add_to_run_list(&mut self, ty: TypeId, after_head: bool) {
        if self.rt[ty].in_run {
            return;
        }
        if after_head && !self.run_list.is_empty() {
            self.run_list.insert(1, ty);
        } else {
            self.run_list.insert(0, ty);
        }
        self.rt[ty].in_run = true;
    }

    // ---- trailstates ----

    fn clean_trailstate(&mut self, i: usize) {
        let lb = self.trails[i].lastbeam;
        if lb != NIL {
            let b = &mut self.beams[lb as usize];
            b.flags &= !BS_LASTSEG;
            b.flags |= BS_NODRAW;
        }
        let serial = self.trails[i].serial.wrapping_add(1);
        self.trails[i] = TrailState { serial, assoc: None, lastbeam: NIL, state1: 0.0, state2: 0.0 };
    }

    fn valid(&self, h: TrailHandle) -> bool {
        self.trails[h.idx as usize].serial == h.serial
    }

    /// Returns the trailstate for `slot`, allocating one if missing or stolen.
    fn trailstate(&mut self, slot: &mut Option<TrailHandle>) -> usize {
        if let Some(h) = *slot
            && self.valid(h)
        {
            return h.idx as usize;
        }
        if self.ts_cycle >= self.trails.len() {
            self.ts_cycle = 0;
        }
        let i = self.ts_cycle;
        self.clean_trailstate(i);
        self.ts_cycle += 1;
        *slot = Some(TrailHandle { idx: i as u16, serial: self.trails[i].serial });
        i
    }

    /// PScript_DelinkTrailstate: releases an owner's trailstate chain.
    pub fn delink_trailstate(&mut self, slot: &mut Option<TrailHandle>) {
        let Some(h) = slot.take() else { return };
        if !self.valid(h) {
            return;
        }
        let mut assoc = self.trails[h.idx as usize].assoc;
        self.clean_trailstate(h.idx as usize);
        let mut guard = 0;
        while let Some(a) = assoc {
            if !self.valid(a) || guard > MAX_TRAILSTATES {
                break;
            }
            assoc = self.trails[a.idx as usize].assoc;
            self.clean_trailstate(a.idx as usize);
            guard += 1;
        }
    }

    // ---- spawning ----

    fn contents(&self, world: &dyn World, p: Vec3) -> u32 {
        world.point_contents(p)
    }

    fn effect_spawned(&mut self, t: &PartType, org: Vec3, dlkey: i32) {
        if t.dl_radius[0] != 0.0 || t.dl_radius[1] != 0.0 {
            let tick = (self.cl_time * 20.0) as i64;
            if self.flicker.0 != tick {
                self.flicker = (tick, self.rng.rand());
            }
            let f = if self.settings.lightflicker {
                ((self.flicker.1 as i64 + dlkey as i64 * 2000) & 0xffff) as f32 * (1.0 / 65535.0)
            } else {
                0.5
            };
            self.dlights.push(DlightSpawn {
                key: dlkey,
                org,
                radius: t.dl_radius[0] + f * t.dl_radius[1],
                die: self.cl_time + t.dl_time as f64,
                decay: t.dl_decay[3],
                rgb: t.dl_rgb,
            });
        }
        if !t.sounds.is_empty() {
            let total: f32 = t.sounds.iter().map(|s| s.weight).sum();
            let w = self.rng.frandom() * total;
            let mut acc = 0.0;
            for s in &t.sounds {
                acc += s.weight;
                if w <= acc {
                    if !s.name.is_empty() && s.vol > 0.0 {
                        self.sounds.push(SoundSpawn { name: s.name.clone(), org, vol: s.vol, atten: s.atten });
                    }
                    break;
                }
            }
        }
    }

    /// Shared initial state of a new particle (die offset kept in `p.die`).
    fn init_particle(&mut self, reg: &Registry, t: &PartType, i: u32) {
        let rng = &mut self.rng;
        let die = t.randdie * rng.frandom();
        let scale = t.scale + t.scalerand * rng.frandom();
        let mut a = if t.die != 0.0 { t.alpha + die * t.alphachange } else { t.alpha };
        a += t.alpharand * rng.frandom();
        let p = &mut self.parts[i as usize];
        p.die = die;
        p.scale = scale;
        p.rgba[3] = a;
        if t.emittime >= 0.0 {
            p.nextemit = self.particletime + t.emitstart - die;
        }
        p.rotationspeed = t.rotationmin + rng.frandom() * t.rotationrand;
        p.angle = t.rotationstartmin + rng.frandom() * t.rotationstartrand;
        p.s1 = t.s1;
        p.t1 = t.t1;
        p.s2 = t.s2;
        p.t2 = t.t2;
        let _ = reg;
    }

    fn apply_colour(&mut self, reg: &Registry, t: &PartType, i: u32, extra_cidx: i32) {
        let p = &mut self.parts[i as usize];
        if t.colorindex >= 0 {
            let r = if t.colorrand > 0 { (self.rng.rand() % t.colorrand as u32) as i32 } else { 0 };
            let cidx = t.colorindex + r + extra_cidx;
            if cidx > 255 {
                p.rgba[3] /= 2.0;
            }
            let c = reg.palette_rgb(cidx as usize);
            p.rgba[..3].copy_from_slice(&c);
        } else {
            p.rgba[..3].copy_from_slice(&t.rgb);
        }
        let v = rgb_rand(&mut self.rng, t);
        let die = p.die;
        for (k, c) in p.rgba[..3].iter_mut().enumerate() {
            *c += v[k] * t.rgbrand[k] + t.rgbchange[k] * die;
        }
    }

    fn worldspace_rand(&mut self, t: &PartType, i: u32) {
        if t.flags & PT_WORLDSPACERAND == 0 {
            return;
        }
        let mut v;
        loop {
            v = Vec3::new(self.rng.crand(), self.rng.crand(), self.rng.crand());
            if v.dot(v) <= 1.0 {
                break;
            }
        }
        let p = &mut self.parts[i as usize];
        p.org += v.mul_elem(Vec3::from(t.orgwrand));
        p.vel += v.mul_elem(Vec3::from(t.velwrand));
        p.vel += Vec3::from(t.velbias);
    }

    /// PScript_RunParticleEffectState. Returns false if the type is invalid
    /// or not loaded (the C return value 1).
    #[allow(clippy::too_many_arguments)]
    pub fn run_effect(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        org: Vec3,
        dir: Option<Vec3>,
        count: f32,
        ty: TypeId,
        tsk: Option<&mut Option<TrailHandle>>,
    ) -> bool {
        let Some(t0) = reg.types.get(ty) else { return false };
        if !t0.is_loaded() {
            return false;
        }
        self.ensure_rt(reg.types.len());
        let mut ty = ty;
        if self.settings.contentswitch
            && let Some(iw) = t0.inwater
            && world.has_world()
            && self.contents(world, org) & FTECONTENTS_FLUID != 0
        {
            ty = iw;
        }
        let mut tsk = if reg.types[ty].flags & PT_NOSTATE != 0 { None } else { tsk };
        let mut ts = tsk.as_mut().map(|s| self.trailstate(s));
        let mut axis = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)];
        let mut dir = dir;
        let mut guard = 0;

        loop {
            guard += 1;
            if guard > reg.types.len() + 1 {
                break;
            }
            let t = &reg.types[ty];
            let mut skip = false;
            if self.settings.contentswitch && t.flags & (PT_TRUNDERWATER | PT_TROVERWATER) != 0 && world.has_world() {
                let cont = self.contents(world, org);
                if (t.flags & PT_TROVERWATER != 0 && cont & t.fluidmask != 0)
                    || (t.flags & PT_TRUNDERWATER != 0 && cont & t.fluidmask == 0)
                {
                    skip = true;
                }
            }
            if !skip {
                if let Some(d) = dir
                    && d != Vec3::ZERO
                {
                    axis[2] = d.normalize();
                    axis[0] = perpendicular(axis[2]).normalize();
                    axis[1] = axis[2].cross(axis[0]).normalize();
                }
                self.effect_spawned(t, org, 0);

                if t.looks.kind == RenderType::CDecal {
                    if !self.has_free_decal() {
                        return true;
                    }
                    let d = self.spawn_cdecal(reg, world, ty, org, dir);
                    if d.is_none() {
                        return true;
                    }
                    dir = d;
                    match t.assoc {
                        Some(a) => {
                            ty = a;
                            continue;
                        }
                        None => break,
                    }
                }
                if !self.spawn_effect_particles(reg, ty, org, dir, count, &axis, ts) {
                    return true;
                }
            }
            // next associated effect
            let Some(a) = t.assoc else { break };
            if let Some(cur) = ts {
                let mut slot = self.trails[cur].assoc;
                let n = self.trailstate(&mut slot);
                self.trails[cur].assoc = slot;
                ts = Some(n);
            }
            ty = a;
        }
        true
    }

    /// Spawns the particles of one effect stage. Returns false when the rest
    /// of the effect chain must be skipped (spawn time limit or failed
    /// spawn chance).
    #[allow(clippy::too_many_arguments)]
    fn spawn_effect_particles(
        &mut self,
        reg: &Registry,
        ty: TypeId,
        org: Vec3,
        dir: Option<Vec3>,
        count: f32,
        axis: &[Vec3; 3],
        ts: Option<usize>,
    ) -> bool {
        let t = &reg.types[ty];
        let mut spawnspc: i32 = 8;
        let (mut j, mut k, mut l): (i32, i32, i32) = (0, 0, 0);
        let mut m: f32 = 0.0;
        let mut pcount = t.countextra + self.settings.density * count * (t.count + t.countrand * self.rng.frandom());
        if t.flags & PT_INVFRAMETIME != 0 {
            pcount /= self.frametime.max(1e-6);
        }
        if let Some(ts) = ts {
            pcount += self.trails[ts].state2;
        }
        match t.spawnmode {
            SpawnMode::UniCircle => {
                m = pcount;
                if t.looks.kind == RenderType::Beam {
                    m -= 1.0;
                }
                m = if m < 1.0 { 0.0 } else { PI * 2.0 / m };
                if t.spawnparam1 != 0.0 {
                    m *= t.spawnparam1;
                }
            }
            SpawnMode::TeleBox | SpawnMode::LavaSplash => {
                if t.spawnmode == SpawnMode::TeleBox {
                    spawnspc = 4;
                    l = -t.areaspreadvert as i32;
                }
                j = -t.areaspread as i32;
                k = j;
                m = if t.spawnparam1 != 0.0 { t.spawnparam1 } else { 0.55752 };
                if t.spawnparam2 != 0.0 {
                    spawnspc = t.spawnparam2 as i32;
                }
            }
            SpawnMode::Field => {
                if self.avelocities.is_empty() {
                    for _ in 0..NUMVERTEXNORMALS {
                        let a = (self.rng.rand() & 255) as f32 * 0.01;
                        let b = (self.rng.rand() & 255) as f32 * 0.01;
                        self.avelocities.push([a, b]);
                    }
                }
                j = 0;
                m = 0.0;
            }
            _ => {}
        }
        let spawnspc = spawnspc.max(1);

        if t.spawntime != 0.0
            && let Some(ts) = ts
        {
            if self.trails[ts].state1 > self.particletime {
                return false;
            }
            self.trails[ts].state1 = self.particletime + t.spawntime;
        }

        let mut i: i32 = 0;
        // C `break`s out of the whole effect loop here.
        if t.spawnchance < self.rng.frandom() {
            return false;
        }
        {
            if t.die == 0.0 && t.count == 1.0 && t.countrand == 0.0 && pcount < 1.0 {
                pcount = 1.0;
            }
            let mut b = NIL;
            let mut bfirst = NIL;
            let cl_time = self.cl_time as f32;
            let mut ofsvec: Vec3;
            while (i as f32) < pcount {
                if !self.has_free_particle() {
                    break;
                }
                if t.looks.kind == RenderType::Beam && !self.has_free_beam() {
                    break;
                }
                let p = self.alloc_particle(ty);
                if t.looks.kind == RenderType::Beam {
                    let nb = self.alloc_beam();
                    if b != NIL {
                        self.beams[b as usize].next = nb;
                    } else {
                        bfirst = nb;
                    }
                    b = nb;
                    self.beams[b as usize] = BeamSeg { next: NIL, p, flags: 0, dir: Vec3::ZERO, texture_s: i as f32 };
                }
                self.init_particle(reg, t, p);
                if t.randsmax != 1 {
                    let o = t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
                    let pp = &mut self.parts[p as usize];
                    pp.s1 += o;
                    pp.s2 += o;
                }
                self.apply_colour(reg, t, p, 0);

                let rng = &mut self.rng;
                let arsvec;
                match t.spawnmode {
                    SpawnMode::Box => {
                        ofsvec = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom());
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                    SpawnMode::TeleBox => {
                        ofsvec = Vec3::new(k as f32, j as f32, (l + 4) as f32).normalize();
                        ofsvec *= 1.0 - rng.frandom() * m;
                        arsvec = Vec3::new(
                            (j + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (k + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (l + (rng.rand() % spawnspc as u32) as i32) as f32,
                        );
                        j += spawnspc;
                        if j as f32 >= t.areaspread {
                            j = -t.areaspread as i32;
                            k += spawnspc;
                            if k as f32 >= t.areaspread {
                                k = -t.areaspread as i32;
                                l += spawnspc;
                                if l as f32 >= t.areaspreadvert {
                                    l = -t.areaspreadvert as i32;
                                }
                            }
                        }
                    }
                    SpawnMode::LavaSplash => {
                        let o = Vec3::new(
                            (k + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (j + (rng.rand() % spawnspc as u32) as i32) as f32,
                            256.0,
                        );
                        arsvec = Vec3::new(o.x, o.y, rng.frandom() * t.areaspreadvert);
                        ofsvec = o.normalize() * (1.0 - rng.frandom() * m);
                        j += spawnspc;
                        if j as f32 >= t.areaspread {
                            j = -t.areaspread as i32;
                            k += spawnspc;
                            if k as f32 >= t.areaspread {
                                k = -t.areaspread as i32;
                            }
                        }
                    }
                    SpawnMode::UniCircle => {
                        ofsvec = Vec3::new(pcos(m * i as f32), psin(m * i as f32), 0.0);
                        arsvec = ofsvec * t.areaspread;
                    }
                    SpawnMode::Field => {
                        let av = self.avelocities[i as usize % NUMVERTEXNORMALS];
                        let a0 = cl_time * av[0] + m;
                        let a1 = cl_time * av[1] + m;
                        let a2 = pcos(a1);
                        ofsvec = Vec3::new(a2 * pcos(a0), a2 * psin(a0), -psin(a1));
                        let orgadd = t.spawnparam2 * psin(cl_time + j as f32 + m);
                        let n = AVERTEXNORMALS[j as usize];
                        arsvec = Vec3::new(
                            n[0] * (t.areaspread + orgadd) + ofsvec.x * t.spawnparam1,
                            n[1] * (t.areaspread + orgadd) + ofsvec.y * t.spawnparam1,
                            n[2] * (t.areaspreadvert + orgadd) + ofsvec.z * t.spawnparam1,
                        );
                        ofsvec = ofsvec.normalize();
                        j += 1;
                        if j as usize >= NUMVERTEXNORMALS {
                            j = 0;
                            m += 0.176_289_1;
                        }
                    }
                    SpawnMode::DistBall => {
                        let rdist = t.spawnparam2 - rng.crandom() * (1.0 - rng.crandom() * t.spawnparam1);
                        let z = if t.areaspreadvert != 0.0 { None } else { Some(0.0) };
                        let x = rng.hrandom();
                        let y = rng.hrandom();
                        let z = z.unwrap_or_else(|| rng.hrandom());
                        ofsvec = Vec3::new(x, y, z).normalize() * rdist;
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                    _ => {
                        let x = rng.hrandom();
                        let y = rng.hrandom();
                        let z = if t.areaspreadvert != 0.0 { rng.hrandom() } else { 0.0 };
                        ofsvec = Vec3::new(x, y, z).normalize();
                        if t.spawnmode != SpawnMode::Circle {
                            ofsvec *= rng.frandom();
                        }
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                }

                let orgadd = t.orgadd + rng.frandom() * t.randomorgadd;
                let mut veladd = t.veladd + rng.frandom() * t.randomveladd;
                if let Some(d) = dir {
                    veladd *= d.length();
                }
                let pp = &mut self.parts[p as usize];
                pp.vel = Vec3::ZERO
                    .mul_add(ofsvec.x * t.spawnvel, axis[0])
                    .mul_add(ofsvec.y * t.spawnvel, axis[1])
                    .mul_add(veladd + ofsvec.z * t.spawnvelvert, axis[2]);
                pp.org = org.mul_add(arsvec.x, axis[0]).mul_add(arsvec.y, axis[1]).mul_add(orgadd + arsvec.z, axis[2]);
                self.worldspace_rand(t, p);
                let pp = &mut self.parts[p as usize];
                pp.org += Vec3::from(t.orgbias);
                pp.die = self.particletime + t.die - pp.die;
                pp.oldorg = pp.org;
                i += 1;
            }

            if t.looks.kind == RenderType::Beam && b != NIL {
                if t.spawnmode == SpawnMode::UniCircle {
                    let ars = Vec3::new(pcos(m * (i - 2) as f32), psin(m * (i - 2) as f32), 0.0);
                    let po = self.parts[self.beams[b as usize].p as usize].org;
                    self.beams[bfirst as usize].dir = (po - ars).normalize();
                }
                self.beams[b as usize].flags |= BS_NODRAW;
                self.beams[b as usize].next = self.rt[ty].beams;
                self.rt[ty].beams = bfirst;
            }
        }

        if let Some(ts) = ts {
            self.trails[ts].state2 = pcount - i as f32;
        }
        if self.rt[ty].particles != NIL || self.rt[ty].decals != NIL {
            self.add_to_run_list(ty, true);
        }
        true
    }

    /// The PT_CDECAL branch of RunParticleEffectState. Returns the
    /// direction used for the decal (C reassigns `dir`), None if no model.
    fn spawn_cdecal(&mut self, reg: &Registry, world: &dyn World, ty: TypeId, org: Vec3, dir: Option<Vec3>) -> Option<Vec3> {
        let t = &reg.types[ty];
        let mut center = org;
        let mut entity = 0;
        let bestdir;
        match dir {
            Some(d) if d != Vec3::ZERO => {
                let tr = world.trace_line(org - d, org + d);
                center = tr.endpos;
                bestdir = tr.normal;
                entity = tr.entity;
            }
            _ => {
                let mut best = Vec3::new(0.0, 0.73, 0.73).normalize();
                let mut bestfrac = 1.0;
                for n in 0..6 {
                    let mut e = Vec3::ZERO;
                    e[n % 3] = if n >= 3 { 16.0 } else { -16.0 };
                    let tr = world.trace_line(org - e, org + e);
                    if bestfrac > tr.frac {
                        bestfrac = tr.frac;
                        best = tr.normal;
                        center = tr.endpos;
                        entity = tr.entity;
                    }
                }
                bestdir = best;
            }
        }
        let used_dir = match dir {
            Some(d) if d != Vec3::ZERO => d,
            _ => bestdir,
        };
        if entity != 0 {
            match world.entity_origin(entity) {
                Some(o) => center -= o,
                None => entity = 0,
            }
        }
        if entity == 0 && !world.has_world() {
            return None;
        }
        let normal = (used_dir * -1.0).normalize();
        let vec = Vec3::splat(0.5).normalize();
        let tangent1 = normal.cross(vec);
        let tangent2 = rotate_around(normal, tangent1, self.rng.frandom() * 360.0);
        let tangent1 = normal.cross(tangent2).normalize();
        let tangent2 = tangent2.normalize();
        let m = t.scale + self.rng.frandom() * t.scalerand;
        let scale1 = t.s2 - t.s1;
        let scale2 = t.t2 - t.t1;
        let mut bias1 = t.s1 + scale1 / 2.0;
        if t.randsmax != 1 {
            bias1 += t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
        }
        let ctx = DecalCtx {
            ty,
            entity,
            center,
            normal,
            tangent1,
            tangent2,
            scale0: 2.0 / m,
            scale1: scale1 / m,
            scale2: scale2 / m,
            bias1,
            bias2: t.t1 + scale2 / 2.0,
        };
        self.clip_decal(reg, world, &ctx, m);
        Some(used_dir)
    }

    /// Mod_ClipDecal + PScript_AddDecals.
    fn clip_decal(&mut self, reg: &Registry, world: &dyn World, ctx: &DecalCtx, size: f32) {
        let t = &reg.types[ctx.ty];
        let mut planes = [
            (ctx.tangent1, 0.0),
            (ctx.tangent1 * -1.0, 0.0),
            (ctx.tangent2, 0.0),
            (ctx.tangent2 * -1.0, 0.0),
            (ctx.normal, 0.0),
            (ctx.normal * -1.0, 0.0),
        ];
        let mut radius = 0.0f32;
        for (n, d) in &mut planes {
            let (nn, r) = n.normalize_len();
            *n = nn;
            let r = r * size / 2.0;
            radius = radius.max(r);
            *d = -(r - ctx.center.dot(*n));
        }
        let noperp = self.settings.decal_noperpendicular;
        let mut tris: Vec<[Vec3; 3]> = Vec::new();
        world.decal_polys(ctx.entity, ctx.center, radius, ctx.normal, noperp, &mut |poly| {
            fragment_clip_poly(poly, &planes, ctx.normal, noperp, &mut tris);
        });
        let _ = t;
        self.add_decals(reg, ctx, &tris);
    }

    fn add_decals(&mut self, reg: &Registry, ctx: &DecalCtx, tris: &[[Vec3; 3]]) {
        let t = &reg.types[ctx.ty];
        for tri in tris {
            if !self.has_free_decal() {
                break;
            }
            let di = self.alloc_decal(ctx.ty);
            let noperp = self.settings.decal_noperpendicular;
            let die = t.randdie * self.rng.frandom();
            let mut a = if t.die != 0.0 { t.alpha + die * t.alphachange } else { t.alpha };
            a += t.alpharand * self.rng.frandom();
            let mut rgba = [0.0; 4];
            rgba[3] = a;
            if t.colorindex >= 0 {
                let r = if t.colorrand > 0 { (self.rng.rand() % t.colorrand as u32) as i32 } else { 0 };
                let cidx = t.colorindex + r;
                if cidx > 255 {
                    rgba[3] /= 2.0;
                }
                rgba[..3].copy_from_slice(&reg.palette_rgb(cidx as usize));
            } else {
                rgba[..3].copy_from_slice(&t.rgb);
            }
            let v = rgb_rand(&mut self.rng, t);
            for k in 0..3 {
                rgba[k] += v[k] * t.rgbrand[k] + t.rgbchange[k] * die;
            }
            let d = &mut self.decals[di as usize];
            d.vertex = *tri;
            for (i, &v) in tri.iter().enumerate() {
                let vec = v - ctx.center;
                d.texcoords[i] = [vec.dot(ctx.tangent1) * ctx.scale1 + ctx.bias1, vec.dot(ctx.tangent2) * ctx.scale2 + ctx.bias2];
                d.valpha[i] = if noperp { 1.0 } else { 1.0 - (vec.dot(ctx.normal) * ctx.scale0).abs() };
            }
            d.entity = ctx.entity;
            d.rgba = rgba;
            d.die = self.particletime + t.die - die;
            if t.looks.kind != RenderType::CDecal {
                d.die += 20.0;
            }
            self.add_to_run_list(ctx.ty, false);
        }
    }

    /// Wall-splat decal from a particle hit (deferred decal queue).
    fn spawn_hit_decal(&mut self, reg: &Registry, world: &dyn World, dd: &DeferredDecal) {
        let t = &reg.types[dd.ty];
        let mut center = dd.center;
        let mut entity = dd.entity;
        if entity != 0 {
            match world.entity_origin(entity) {
                Some(o) => center -= o,
                None => entity = 0,
            }
        }
        let normal = (dd.normal * -1.0).normalize();
        let vec = Vec3::new(0.5, 0.5, 0.431).normalize();
        let tangent1 = normal.cross(vec);
        let tangent2 = rotate_around(normal, tangent1, self.rng.frandom() * 360.0);
        let tangent1 = normal.cross(tangent2).normalize();
        let tangent2 = tangent2.normalize();
        let m = dd.scale * (1.5 + self.rng.frandom() * 0.5) * 0.5;
        let scale1 = t.s2 - t.s1;
        let scale2 = t.t2 - t.t1;
        let ctx = DecalCtx {
            ty: dd.ty,
            entity,
            center,
            normal,
            tangent1,
            tangent2,
            scale0: 2.0 / m,
            scale1: scale1 / m,
            scale2: scale2 / m,
            bias1: t.s1 + scale1 * 0.5,
            bias2: t.t1 + scale2 * 0.5,
        };
        self.clip_decal(reg, world, &ctx, m);
    }

    /// PScript_ParticleTrail: spawns a trail between two points.
    /// `dlkey` keys any dynamic light (usually the entity number).
    #[allow(clippy::too_many_arguments)]
    pub fn trail(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        start: Vec3,
        end: Vec3,
        ty: TypeId,
        timeinterval: f32,
        dlkey: i32,
        tsk: Option<&mut Option<TrailHandle>>,
    ) -> bool {
        let Some(t) = reg.types.get(ty) else { return false };
        if !t.is_loaded() {
            return false;
        }
        self.ensure_rt(reg.types.len());
        let mut ty = ty;
        if self.settings.contentswitch
            && let Some(iw) = t.inwater
            && world.has_world()
            && self.contents(world, start) & FTECONTENTS_FLUID != 0
        {
            ty = iw;
        }
        self.trail_spawn(reg, world, start, end, ty, timeinterval, dlkey, tsk, 0);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn trail_spawn(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        start0: Vec3,
        end: Vec3,
        ty: TypeId,
        timeinterval: f32,
        dlkey: i32,
        tsk: Option<&mut Option<TrailHandle>>,
        depth: usize,
    ) {
        let t = &reg.types[ty];
        let tsk = if t.flags & PT_NOSTATE != 0 { None } else { tsk };
        let mut ts = tsk.map(|s| self.trailstate(s));

        self.effect_spawned(t, start0, dlkey);

        if let Some(a) = t.assoc
            && depth < reg.types.len()
        {
            match ts {
                Some(cur) => {
                    let mut slot = self.trails[cur].assoc;
                    self.trail(reg, world, start0, end, a, timeinterval, dlkey, Some(&mut slot));
                    self.trails[cur].assoc = slot;
                }
                None => {
                    self.trail(reg, world, start0, end, a, timeinterval, dlkey, None);
                }
            }
        }

        if self.settings.contentswitch && t.flags & (PT_TRUNDERWATER | PT_TROVERWATER) != 0 && world.has_world() {
            let cont = self.contents(world, start0);
            if (t.flags & PT_TROVERWATER != 0 && cont & t.fluidmask != 0)
                || (t.flags & PT_TRUNDERWATER != 0 && cont & t.fluidmask == 0)
            {
                return;
            }
        }

        if t.spawntime != 0.0
            && let Some(cur) = ts
        {
            if self.trails[cur].state1 > self.particletime {
                return;
            }
            self.trails[cur].state1 = self.particletime + t.spawntime;
            ts = None;
        }

        if t.spawnchance < self.rng.frandom() {
            return;
        }
        if t.die == 0.0 {
            ts = None;
        }

        let mut start = start0;
        let (vec, mut len) = (end - start).normalize_len();
        let mut step;
        let mut count: f32;
        if t.countspacing != 0.0 {
            step = t.countspacing / self.settings.density;
            if t.countextra != 0.0 {
                count = t.countextra;
                if step > 0.0 {
                    count += len / step;
                }
                step = len / count;
            }
        } else {
            let s = t.count * self.settings.density * timeinterval + t.countextra + self.rt[ty].countoverflow;
            count = (s as i32) as f32;
            self.rt[ty].countoverflow = s - count;
            if count <= 0.0 {
                return;
            }
            step = len / count;
        }
        if t.flags & PT_AVERAGETRAIL != 0 {
            let mut tavg = len / step;
            tavg /= tavg.ceil();
            step *= tavg;
            len += step;
        }
        let mut vstep = vec * step;

        let (mut right, mut up) = (Vec3::ZERO, Vec3::ZERO);
        let mut tdegree = 2.0 * PI / 256.0;
        let mut sdegree = 0.0;
        if t.spawnmode == SpawnMode::Spiral {
            (right, up) = vector_vectors(vec);
            if t.spawnparam1 != 0.0 {
                tdegree = 2.0 * PI / t.spawnparam1;
            }
            sdegree = t.spawnparam2 * (PI / 180.0);
        } else if t.spawnmode == SpawnMode::Circle {
            (right, up) = vector_vectors(vec);
        }

        let stop;
        if let Some(cur) = ts {
            stop = self.trails[cur].state2 + len;
            self.trails[cur].state2 = stop;
            len = self.trails[cur].state1;
        } else {
            stop = len;
            len = 0.0;
        }
        let nrfirst = if t.flags & PT_NOSPREADFIRST != 0 { len + step * 1.5 } else { len };
        let nrlast = if t.flags & PT_NOSPREADLAST != 0 { stop } else { stop + step };

        let mut b = NIL;
        let mut bfirst = NIL;
        if len < stop {
            count = (stop - len) / step;
        } else {
            count = 0.0;
            step = 0.0;
            vstep = Vec3::ZERO;
        }
        let veladd = -t.veladd;

        while count > 0.0 {
            count -= 1.0;
            len += step;
            if !self.has_free_particle() {
                len = stop;
                break;
            }
            if t.looks.kind == RenderType::Beam && !self.has_free_beam() {
                len = stop;
                break;
            }
            let p = self.alloc_particle(ty);
            if t.looks.kind == RenderType::Beam {
                let nb = self.alloc_beam();
                if b != NIL {
                    self.beams[b as usize].next = nb;
                } else {
                    bfirst = nb;
                }
                b = nb;
                self.beams[b as usize] = BeamSeg { next: NIL, p, flags: 0, dir: vec, texture_s: len };
            }
            self.init_particle(reg, t, p);
            let tcount =
                if t.spawnparam1 != 0.0 { (len * t.count / t.spawnparam1) as i32 } else { (len * t.count) as i32 };
            let extra = if t.flags & PT_CITRACER != 0 { (tcount & 4) << 1 } else { 0 };
            self.apply_colour(reg, t, p, extra);
            if t.randsmax != 1 {
                let o = t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
                let pp = &mut self.parts[p as usize];
                pp.s1 += o;
                pp.s2 += o;
                let mut guard = 0;
                while pp.s1 >= 1.0 && guard < 64 {
                    pp.s1 -= 1.0;
                    pp.s2 -= 1.0;
                    pp.t1 += t.texsstride;
                    pp.t2 += t.texsstride;
                    guard += 1;
                }
            }

            let rng = &mut self.rng;
            let (org, vel);
            if len < nrfirst || len >= nrlast {
                vel = vec * veladd;
                org = start;
            } else {
                match t.spawnmode {
                    SpawnMode::Tracer => {
                        let s = if tcount & 1 != 0 { 1.0 } else { -1.0 };
                        let side = Vec3::new(vec.y * s, -vec.x * s, 0.0);
                        vel = Vec3::new(side.x * t.spawnvel + vec.x * veladd, side.y * t.spawnvel + vec.y * veladd, vec.z * veladd);
                        org = Vec3::new(side.x * t.areaspread + start.x, side.y * t.areaspread + start.y, start.z);
                    }
                    SpawnMode::Spiral => {
                        let (tc, tsn) = (pcos(len * tdegree + sdegree), psin(len * tdegree + sdegree));
                        org = start + right * (tc * t.areaspread) + up * (tsn * t.areaspread);
                        vel = vec * veladd + right * (tc * t.spawnvel) + up * (tsn * t.spawnvel);
                    }
                    SpawnMode::Ball => {
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom()).normalize() * rng.frandom();
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                    SpawnMode::Circle => {
                        let (tc, tsn) = (pcos(len * tdegree) * t.areaspread, psin(len * tdegree) * t.areaspread);
                        let a = len * tdegree;
                        org = Vec3::new(
                            start.x + right.x * tc + up.x * tsn + vstep.x * a,
                            start.y + right.y * tc + up.y * tsn + vstep.y * a,
                            start.z + right.z * tc + up.z * tsn + vstep.z * a * 50.0,
                        );
                        let (tc, tsn) = (pcos(len * tdegree) * t.spawnvel, psin(len * tdegree) * t.spawnvel);
                        vel = vec * veladd + right * tc + up * tsn;
                    }
                    SpawnMode::DistBall => {
                        let rdist = t.spawnparam2 - rng.crandom() * (1.0 - rng.crandom() * t.spawnparam1);
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom()).normalize() * rdist;
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                    _ => {
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom());
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                }
            }
            let pp = &mut self.parts[p as usize];
            pp.vel = vel;
            pp.org = if len >= nrfirst && len < nrlast && t.orgadd != 0.0 { org + vec * t.orgadd } else { org };
            self.worldspace_rand(t, p);
            let pp = &mut self.parts[p as usize];
            pp.org += Vec3::from(t.orgbias);

            start += vstep;
            if t.countrand != 0.0 {
                let rstep = self.rng.frandom() / t.countrand;
                start = start.mul_add(rstep, vec);
                step += rstep;
            }
            let pp = &mut self.parts[p as usize];
            pp.die = self.particletime + t.die - pp.die;
            pp.oldorg = pp.org;
        }

        if let Some(cur) = ts {
            self.trails[cur].state1 = len;
            if t.looks.kind == RenderType::Beam {
                if b != NIL {
                    let lb = self.trails[cur].lastbeam;
                    if self.rt[ty].beams != NIL {
                        if lb != NIL {
                            self.beams[b as usize].next = self.beams[lb as usize].next;
                            self.beams[lb as usize].next = bfirst;
                            self.beams[lb as usize].flags &= !BS_LASTSEG;
                        } else {
                            self.beams[b as usize].next = self.rt[ty].beams;
                            self.rt[ty].beams = bfirst;
                        }
                    } else {
                        self.rt[ty].beams = bfirst;
                        self.beams[b as usize].next = NIL;
                    }
                    self.beams[b as usize].flags |= BS_LASTSEG;
                    self.trails[cur].lastbeam = b;
                }
                let lb = self.trails[cur].lastbeam;
                if (!self.has_free_particle() || !self.has_free_beam()) && lb != NIL {
                    self.beams[lb as usize].flags &= !BS_LASTSEG;
                    self.beams[lb as usize].flags |= BS_NODRAW;
                    self.trails[cur].lastbeam = NIL;
                }
            }
        } else if t.looks.kind == RenderType::Beam && b != NIL {
            self.beams[b as usize].flags |= BS_NODRAW;
            self.beams[b as usize].next = self.rt[ty].beams;
            self.rt[ty].beams = bfirst;
        }
        self.add_to_run_list(ty, false);
    }

    /// PScript_RunParticleEffect: classic `particle` messages via `pe_<color>`
    /// or the PE_* fallbacks. Returns false if nothing handled it.
    #[allow(clippy::too_many_arguments)]
    pub fn run_classic_effect(
        &mut self,
        reg: &mut Registry,
        files: &mut dyn FileSource,
        world: &dyn World,
        org: Vec3,
        dir: Vec3,
        color: i32,
        count: i32,
    ) -> bool {
        if let Some(t) = reg.find(&format!("pe_{color}"), files)
            && self.run_effect(reg, world, org, Some(dir), count as f32, t, None)
        {
            return true;
        }
        let pick = if count > 130 && self.pe.size3.is_some() {
            self.pe.size3
        } else if count > 20 && self.pe.size2.is_some() {
            self.pe.size2
        } else {
            self.pe.default
        };
        let Some(t) = pick else { return false };
        reg.types[t].colorindex = color & !7;
        reg.types[t].colorrand = 8;
        self.run_effect(reg, world, org, Some(dir), count as f32, t, None)
    }

    /// PScript_RunParticleWeather (te_particlerain / te_particlesnow).
    #[allow(clippy::too_many_arguments)]
    pub fn run_weather(
        &mut self,
        reg: &mut Registry,
        files: &mut dyn FileSource,
        world: &dyn World,
        minb: Vec3,
        maxb: Vec3,
        dir: Vec3,
        count: f32,
        colour: i32,
        efname: &str,
    ) {
        let t = match reg.find(&format!("te_{efname}_{colour}"), files) {
            Some(t) => t,
            None => {
                let Some(t) = reg.find(&format!("te_{efname}"), files).or(self.pe.default) else { return };
                reg.types[t].colorindex = colour;
                t
            }
        };
        let pc = reg.types[t].count;
        let invcount = 1.0 / pc;
        let count = count * pc;
        let mut i = 0.0;
        while i < count {
            if !self.has_free_particle() {
                return;
            }
            let mut org = Vec3::ZERO;
            for j in 0..3 {
                let num = self.rng.frandom();
                org[j] = minb[j] + num * (maxb[j] - minb[j]);
            }
            self.run_effect(reg, world, org, Some(dir), invcount, t, None);
            i += 1.0;
        }
    }

    // ---- update ----

    /// P_AddRainParticles for one emitter.
    fn add_rain(&mut self, reg: &Registry, world: &dyn World, em: &RainEmitter, vieworg: Vec3) {
        if self.settings.rain_quantity == 0.0 {
            return;
        }
        let Some(mut sm) = self.sky.remove(&em.model) else { return };
        sm.skytime += self.frametime as f64;
        'tris: for st in &mut sm.tris {
            let Some(t) = reg.types.get(st.ptype) else { continue };
            if !t.is_loaded() {
                continue;
            }
            let mut guard = 0;
            while st.nexttime < sm.skytime {
                if !self.has_free_particle() || guard > 4096 {
                    break 'tris;
                }
                guard += 1;
                st.nexttime += 10000.0 / (st.area * self.settings.rain_quantity * t.rainfrequency) as f64;
                let x = self.rng.frandom() * self.rng.frandom();
                let y = self.rng.frandom() * (1.0 - x);
                let o = st.org.mul_add(x, st.x).mul_add(y, st.y);
                let a = &em.axis;
                let worg = Vec3::new(o.dot(a[0]), -o.dot(a[1]), o.dot(a[2])) + em.origin;
                if (worg - vieworg).length() > (1024.0 + 512.0) * self.rng.frandom() {
                    continue;
                }
                let n = st.normal;
                let wnorm = Vec3::new(n.dot(a[0]), -n.dot(a[1]), n.dot(a[2]));
                let worg = worg.mul_add(0.5, wnorm);
                if world.point_contents(worg) & FTECONTENTS_SOLID == 0 {
                    self.run_effect(reg, world, worg, Some(wnorm), 1.0, st.ptype, None);
                }
            }
        }
        self.sky.insert(em.model, sm);
    }

    /// PScript_UpdateParticle.
    fn update_particle(&mut self, reg: &Registry, world: &dyn World, pi: u32, ty: TypeId) {
        let t = &reg.types[ty];
        let ft = self.frametime;
        let pt = self.particletime;
        let p = &mut self.parts[pi as usize];
        let oldorg = p.org;
        if t.flags & PT_VELOCITY != 0 {
            p.org += p.vel * ft;
            p.vel.z -= t.gravity * ft;
            if t.flags & PT_FRICTION != 0 {
                p.vel = p.vel.mul_elem(Vec3::new(1.0 - t.friction[0] * ft, 1.0 - t.friction[1] * ft, 1.0 - t.friction[2] * ft));
            }
            if t.flurry != 0.0 && self.doflurry {
                p.vel.x += self.urng.ucrandom() * t.flurry;
                p.vel.y += self.urng.ucrandom() * t.flurry;
            }
        }
        p.angle += p.rotationspeed * ft;
        let n = t.ramp.len();
        let age_frac = |die: f32| n as f32 * (t.die - (die - pt)) / t.die;
        match t.rampmode {
            RampMode::Nearest if n > 0 => {
                let r = &t.ramp[(age_frac(p.die).max(0.0) as usize).min(n - 1)];
                p.rgba = [r.rgb[0], r.rgb[1], r.rgb[2], r.alpha];
                p.scale = r.scale;
            }
            RampMode::Lerp if n > 0 => {
                let mut frac = age_frac(p.die);
                let s1 = (frac.max(0.0) as usize).min(n - 1);
                let s2 = (s1 + 1).min(n - 1);
                frac -= s1 as f32;
                let (a, b) = (&t.ramp[s1], &t.ramp[s2]);
                for k in 0..3 {
                    p.rgba[k] = a.rgb[k] + (b.rgb[k] - a.rgb[k]) * frac;
                }
                p.rgba[3] = a.alpha + (b.alpha - a.alpha) * frac;
                p.scale = a.scale + (b.scale - a.scale) * frac;
            }
            RampMode::Delta if n > 0 => {
                let r = &t.ramp[(age_frac(p.die).max(0.0) as usize).min(n - 1)];
                for k in 0..3 {
                    p.rgba[k] += ft * r.rgb[k];
                }
                p.rgba[3] -= ft * r.alpha;
                p.scale += ft * r.scale;
            }
            _ => {
                if pt < p.die - t.die + t.rgbchangetime {
                    for k in 0..3 {
                        p.rgba[k] += ft * t.rgbchange[k];
                    }
                }
                p.rgba[3] += ft * t.alphachange;
                p.scale += ft * t.scaledelta;
            }
        }

        if let Some(e) = t.emit {
            if t.emittime < 0.0 {
                self.q_trails.push(DeferredTrail { start: oldorg, end: p.org, ty: e, owner: Some(pi) });
            } else if p.nextemit < pt {
                p.nextemit = pt + t.emittime + self.urng.ufrandom() * t.emitrand;
                self.q_effects.push((p.org, p.vel, 1.0, e));
            }
        }

        let Some(ct) = t.cliptype else { return };
        if !self.settings.bouncysparks {
            return;
        }
        let p = &mut self.parts[pi as usize];
        let moved = p.org - p.oldorg;
        if t.clipbounce == 0.0 || moved.dot(moved) > 100.0 {
            self.traces_used += 1;
            let tr = if self.traces_used <= self.settings.trace_limit {
                world.trace_line(p.oldorg, p.org)
            } else {
                Trace { frac: 1.0, endpos: p.org, normal: Vec3::ZERO, entity: 0 }
            };
            if tr.frac < 1.0 {
                if t.clipbounce < 0.0 {
                    p.die = -1.0;
                    if t.clipbounce == -2.0 {
                        self.q_decals.push(DeferredDecal {
                            ty,
                            entity: tr.entity,
                            center: p.org,
                            normal: tr.normal,
                            scale: p.scale,
                        });
                    }
                    return;
                } else if ct == ty {
                    let dist = p.vel.dot(tr.normal) * -t.clipbounce;
                    p.vel = p.vel.mul_add(dist, tr.normal);
                    p.org = tr.endpos;
                    if t.texname.is_empty() && p.vel.length() < 1000.0 * ft && t.looks.kind == RenderType::Normal {
                        p.die = -1.0;
                        return;
                    }
                } else {
                    p.die = -1.0;
                    p.vel = p.vel.normalize();
                    let cc = t.clipcount / reg.types[ct].count;
                    let d = if t.clipbounce != 0.0 { tr.normal * t.clipbounce } else { p.vel };
                    self.q_effects.push((tr.endpos, d, cc, ct));
                    return;
                }
            }
            p.oldorg = p.org;
        }
    }

    /// Unlinks expired particles of `ty` into the kill list and returns the
    /// live ones.
    fn reap(&mut self, reg: &Registry, ty: TypeId, out: &mut Vec<(u32, TypeId)>) {
        let pt = self.particletime;
        let emit_trail = reg.types[ty].emittime < 0.0;
        let mut prev = NIL;
        let mut cur = self.rt[ty].particles;
        while cur != NIL {
            let next = self.parts[cur as usize].next;
            if self.parts[cur as usize].die < pt {
                if emit_trail {
                    let mut tr = self.parts[cur as usize].trail.take();
                    self.delink_trailstate(&mut tr);
                }
                if prev == NIL {
                    self.rt[ty].particles = next;
                } else {
                    self.parts[prev as usize].next = next;
                }
                self.kill_list.push(cur);
            } else {
                out.push((cur, ty));
                prev = cur;
            }
            cur = next;
        }
    }

    fn free_beam(&mut self, b: u32) {
        self.free_beams.push(b);
    }

    /// One frame: spawns rain, advances particles, feeds renderables to
    /// `sink`, runs effects queued by particles, and advances the clock.
    pub fn frame(&mut self, reg: &Registry, world: &dyn World, input: &FrameInput, sink: &mut dyn Sink) {
        self.ensure_rt(reg.types.len());
        self.cl_time = input.cl_time;
        self.frametime = ((input.cl_time - self.oldtime) as f32).clamp(0.0, 1.0);
        self.oldtime = input.cl_time;
        self.kill_list.clear();

        self.flurrytime -= self.frametime;
        if self.flurrytime < 0.0 {
            self.doflurry = true;
            self.flurrytime = 0.1 + self.rng.frandom() * 0.3;
        } else {
            self.doflurry = false;
        }

        // Full pools: mark some as dead so new ones can spawn next frame.
        if !self.has_free_decal() && !self.decals.is_empty() {
            for _ in 0..256 {
                self.decals[self.decal_recycle].die = -1.0;
                self.decal_recycle = (self.decal_recycle + 1) % self.decals.len();
            }
        }
        if !self.has_free_particle() && !self.parts.is_empty() {
            for _ in 0..256 {
                self.parts[self.particle_recycle].die = -1.0;
                self.particle_recycle = (self.particle_recycle + 1) % self.parts.len();
            }
        }

        if self.settings.rain {
            for em in &input.rain {
                self.add_rain(reg, world, em, input.vieworg);
            }
        }

        // Setup: reap and flatten.
        let mut updates = Vec::new();
        let run = self.run_list.clone();
        for &ty in &run {
            if reg.types[ty].die == 0.0 {
                continue;
            }
            self.reap(reg, ty, &mut updates);
        }
        self.traces_used = 0;
        for &(p, ty) in &updates {
            self.update_particle(reg, world, p, ty);
        }

        self.update_types(reg, sink);

        // Deferred spawns.
        let ft = self.frametime;
        for tr in std::mem::take(&mut self.q_trails) {
            let mut slot = tr.owner.and_then(|p| self.parts[p as usize].trail);
            self.trail(reg, world, tr.start, tr.end, tr.ty, ft, 0, Some(&mut slot));
            if let Some(p) = tr.owner {
                self.parts[p as usize].trail = slot;
            }
        }
        for (org, dir, count, ty) in std::mem::take(&mut self.q_effects) {
            self.run_effect(reg, world, org, Some(dir), count, ty, None);
        }
        for dd in std::mem::take(&mut self.q_decals) {
            self.spawn_hit_decal(reg, world, &dd);
        }

        let kills = std::mem::take(&mut self.kill_list);
        self.free_parts.extend(kills.iter().rev());
        self.particletime += self.frametime;
    }

    /// PScript_UpdateParticleTypes minus geometry: decals, draining of
    /// zero-lifetime types, beam bookkeeping, run-list maintenance.
    fn update_types(&mut self, reg: &Registry, sink: &mut dyn Sink) {
        let pt = self.particletime;
        let ft = self.frametime;
        let run = std::mem::take(&mut self.run_list);
        let mut keep = Vec::with_capacity(run.len());
        for ty in run {
            let t = &reg.types[ty];

            // Clipped decals.
            if self.rt[ty].decals != NIL {
                let mut prev = NIL;
                let mut cur = self.rt[ty].decals;
                while cur != NIL {
                    let next = self.decals[cur as usize].next;
                    if self.decals[cur as usize].die < pt {
                        if prev == NIL {
                            self.rt[ty].decals = next;
                        } else {
                            self.decals[prev as usize].next = next;
                        }
                        self.free_decals.push(cur);
                        cur = next;
                        continue;
                    }
                    let d = &mut self.decals[cur as usize];
                    if d.die - pt <= t.die {
                        ramp_decal(t, d, pt, ft);
                    }
                    sink.decal(ty, d);
                    prev = cur;
                    cur = next;
                }
            }

            let draws_points = matches!(
                t.looks.kind,
                RenderType::UDecal
                    | RenderType::Normal
                    | RenderType::Spark
                    | RenderType::SparkFan
                    | RenderType::TexturedSpark
            );

            if draws_points && t.die != 0.0 {
                let mut i = self.rt[ty].particles;
                while i != NIL {
                    let p = &self.parts[i as usize];
                    sink.particle(ty, p);
                    i = p.next;
                }
            } else if t.die == 0.0 {
                // Zero-lifetime: draw once, then kill.
                let mut i = self.rt[ty].particles;
                while i != NIL {
                    let p = &self.parts[i as usize];
                    if draws_points {
                        sink.particle(ty, p);
                    }
                    if let Some(e) = t.emit
                        && t.emitstart <= 0.0
                    {
                        self.q_effects.push((p.org, p.vel, 1.0, e));
                    }
                    let next = p.next;
                    self.kill_list.push(i);
                    i = next;
                }
                self.rt[ty].particles = NIL;

                while self.rt[ty].beams != NIL && self.beams[self.rt[ty].beams as usize].flags & BS_DEAD != 0 {
                    let b = self.rt[ty].beams;
                    self.rt[ty].beams = self.beams[b as usize].next;
                    self.free_beam(b);
                }
                let mut b = self.rt[ty].beams;
                while b != NIL {
                    if self.beams[b as usize].flags & BS_NODRAW == 0 {
                        let nb = self.beams[b as usize].next;
                        if nb != NIL {
                            self.draw_beam(ty, b, nb, sink);
                        }
                    }
                    loop {
                        let k = self.beams[b as usize].next;
                        if k != NIL && self.beams[k as usize].flags & BS_DEAD != 0 {
                            self.beams[b as usize].next = self.beams[k as usize].next;
                            self.free_beam(k);
                            continue;
                        }
                        break;
                    }
                    self.beams[b as usize].flags |= BS_DEAD;
                    b = self.beams[b as usize].next;
                }
            } else {
                // Beams of types with a lifetime.
                loop {
                    let k = self.rt[ty].beams;
                    if k == NIL {
                        break;
                    }
                    let bk = &self.beams[k as usize];
                    if (bk.flags & BS_DEAD != 0 || self.parts[bk.p as usize].die < pt) && bk.flags & BS_LASTSEG == 0 {
                        self.rt[ty].beams = bk.next;
                        self.free_beam(k);
                        continue;
                    }
                    break;
                }
                let mut b = self.rt[ty].beams;
                let mut guard = 0usize;
                while b != NIL && guard <= MAX_BEAMSEGS * 2 {
                    guard += 1;
                    let next = self.beams[b as usize].next;
                    if next == NIL {
                        if self.parts[self.beams[b as usize].p as usize].die < pt {
                            self.beams[b as usize].flags |= BS_DEAD;
                        }
                        break;
                    }
                    if self.beams[b as usize].flags & (BS_LASTSEG | BS_DEAD | BS_NODRAW) != 0 {
                        loop {
                            let k = self.beams[b as usize].next;
                            if k != NIL
                                && self.beams[k as usize].flags & BS_DEAD != 0
                                && self.beams[k as usize].flags & BS_LASTSEG == 0
                            {
                                self.beams[b as usize].next = self.beams[k as usize].next;
                                self.free_beam(k);
                                continue;
                            }
                            break;
                        }
                        if self.beams[b as usize].next == NIL {
                            continue;
                        }
                    } else {
                        if self.beams[next as usize].flags & BS_DEAD == 0 {
                            self.draw_beam(ty, b, next, sink);
                        }
                        if self.parts[self.beams[b as usize].p as usize].die < pt {
                            self.beams[b as usize].flags |= BS_DEAD;
                        }
                    }
                    if self.parts[self.beams[b as usize].p as usize].die < pt {
                        self.beams[b as usize].flags |= BS_DEAD;
                    }
                    b = self.beams[b as usize].next;
                }
            }

            let r = &mut self.rt[ty];
            if r.particles == NIL && r.beams == NIL && r.decals == NIL {
                r.in_run = false;
            } else {
                keep.push(ty);
            }
        }
        // Types added during this pass (none expected) go first.
        let added: Vec<_> = std::mem::take(&mut self.run_list);
        self.run_list = added.into_iter().chain(keep).collect();
    }

    fn draw_beam(&mut self, ty: TypeId, b: u32, nb: u32, sink: &mut dyn Sink) {
        let po = self.parts[self.beams[b as usize].p as usize].org;
        let qo = self.parts[self.beams[nb as usize].p as usize].org;
        self.beams[nb as usize].dir = (qo - po).normalize();
        let (seg, next) = (&self.beams[b as usize], &self.beams[nb as usize]);
        sink.beam(ty, seg, next, &self.parts[seg.p as usize], &self.parts[next.p as usize]);
    }
}

fn ramp_decal(t: &PartType, d: &mut Decal, pt: f32, ft: f32) {
    let n = t.ramp.len();
    let frac = n as f32 * (t.die - (d.die - pt)) / t.die;
    match t.rampmode {
        RampMode::Nearest if n > 0 => {
            let r = &t.ramp[(frac.max(0.0) as usize).min(n - 1)];
            d.rgba = [r.rgb[0], r.rgb[1], r.rgb[2], r.alpha];
        }
        RampMode::Lerp if n > 0 => {
            let s1 = (frac.max(0.0) as usize).min(n - 1);
            let s2 = (s1 + 1).min(n - 1);
            let f = frac - s1 as f32;
            let (a, b) = (&t.ramp[s1], &t.ramp[s2]);
            for k in 0..3 {
                d.rgba[k] = a.rgb[k] + (b.rgb[k] - a.rgb[k]) * f;
            }
            d.rgba[3] = a.alpha + (b.alpha - a.alpha) * f;
        }
        RampMode::Delta if n > 0 => {
            let r = &t.ramp[(frac.max(0.0) as usize).min(n - 1)];
            for k in 0..3 {
                d.rgba[k] += ft * r.rgb[k];
            }
            d.rgba[3] -= ft * r.alpha;
        }
        _ => {
            if pt < d.die - t.die + t.rgbchangetime {
                for k in 0..3 {
                    d.rgba[k] += ft * t.rgbchange[k];
                }
            }
            d.rgba[3] += ft * t.alphachange;
        }
    }
}

const FRAG_EPSILON: f32 = 1.0 / 32.0;
const MAXFRAGMENTVERTS: usize = 128 * 3;

/// Fragment_ClipPolyToPlane.
fn clip_poly_to_plane(input: &[Vec3], plane: Vec3, dist: f32, out: &mut Vec<Vec3>) {
    out.clear();
    let n = input.len();
    let dots: Vec<f32> = input.iter().map(|v| v.dot(plane) - dist).collect();
    let keep: Vec<u8> = dots
        .iter()
        .map(|&d| if d < -FRAG_EPSILON { 0 } else if d > FRAG_EPSILON { 1 } else { 2 })
        .collect();
    let clipped = keep.iter().filter(|&&k| k == 0).count();
    if clipped == n {
        return;
    }
    if clipped == 0 {
        out.extend_from_slice(input);
        return;
    }
    for i in 0..n {
        let (k0, k1) = (keep[i], keep[(i + 1) % n]);
        if k0 == 2 {
            out.push(input[i]);
            continue;
        }
        if k0 == 1 {
            out.push(input[i]);
        }
        if k1 == 2 || k0 == k1 {
            continue;
        }
        let (d0, d1) = (dots[i], dots[(i + 1) % n]);
        let mut d = d0 - d1;
        if d != 0.0 {
            d = d0 / d;
        }
        out.push(input[i].lerp(input[(i + 1) % n], d));
    }
}

/// Fragment_ClipPoly: clips a surface polygon to the decal box and fans it
/// into triangles.
fn fragment_clip_poly(poly: &[Vec3], planes: &[(Vec3, f32); 6], normal: Vec3, noperp: bool, tris: &mut Vec<[Vec3; 3]>) {
    if poly.len() > MAXFRAGMENTVERTS || poly.len() < 3 {
        return;
    }
    if noperp {
        let d1 = poly[1] - poly[0];
        let mut n = Vec3::ZERO;
        let mut found = false;
        for v in &poly[2..] {
            n = d1.cross(*v - poly[0]);
            if n.dot(n) > 0.1 {
                found = true;
                break;
            }
        }
        if !found || n.normalize().dot(normal) < 0.1 {
            return;
        }
    }
    let mut a = poly.to_vec();
    let mut b = Vec::with_capacity(poly.len() + 6);
    for (pn, pd) in planes {
        clip_poly_to_plane(&a, *pn, *pd, &mut b);
        if b.len() < 3 {
            return;
        }
        std::mem::swap(&mut a, &mut b);
    }
    let mut n = a.len();
    while n > 2 {
        n -= 1;
        tris.push([a[0], a[n - 1], a[n]]);
    }
}

#[cfg(test)]
mod tests;
