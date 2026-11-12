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
mod defs;
pub use defs::*;
mod effects;
mod effect_particles;
mod trail;
mod weather;
mod update;
mod frag;
use frag::*;

const NIL: u32 = u32::MAX;

pub const BS_LASTSEG: u8 = 0x1;
pub const BS_DEAD: u8 = 0x2;
pub const BS_NODRAW: u8 = 0x4;

pub const MAX_PARTICLES: usize = 1 << 18;
pub const MAX_DECALS: usize = 1 << 18;
pub const MAX_BEAMSEGS: usize = 1 << 11;
pub const MAX_TRAILSTATES: usize = 1 << 10;

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

}

#[cfg(test)]
mod tests;
