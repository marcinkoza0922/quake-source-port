//! Temp entities, dynamic lights and particles (cl_tent.c, cl_main.c
//! CL_AllocDlight/CL_DecayLights, r_part.c simulation only).

use qecs::ParticleSpawn;

use crate::anorms::{AVERTEXNORMALS, NUMVERTEXNORMALS};
use crate::state::{ClientState, MAX_TEMP_ENTITIES};

pub const PT_STATIC: u8 = 0;
pub const PT_GRAV: u8 = 1;
pub const PT_SLOWGRAV: u8 = 2;
pub const PT_FIRE: u8 = 3;
pub const PT_EXPLODE: u8 = 4;
pub const PT_EXPLODE2: u8 = 5;
pub const PT_BLOB: u8 = 6;
pub const PT_BLOB2: u8 = 7;

const RAMP1: [u8; 8] = [0x6f, 0x6d, 0x6b, 0x69, 0x67, 0x65, 0x63, 0x61];
const RAMP2: [u8; 8] = [0x6f, 0x6e, 0x6d, 0x6c, 0x6b, 0x6a, 0x68, 0x66];
const RAMP3: [u8; 8] = [0x6d, 0x6b, 6, 5, 4, 3, 0, 0];

/// C `rand()` stand-in (MSVC LCG, 0..=0x7fff). Deterministic per seed.
#[derive(Debug, Clone, Copy)]
pub struct QRand(pub u32);

impl Default for QRand {
    fn default() -> Self {
        QRand(1)
    }
}

impl QRand {
    #[inline]
    pub fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
    pub fn srand(&mut self, seed: u32) {
        self.0 = seed;
    }
}

/// beam_t.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub entity: i32,
    pub model: &'static str,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub starttime: f64,
    pub endtime: f64,
}

/// dlight_t.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dlight {
    pub key: i32,
    pub origin: [f32; 3],
    pub radius: f32,
    pub die: f64,
    pub decay: f32,
    pub minlight: f32,
    pub color: [f32; 3],
}

impl Default for Dlight {
    fn default() -> Self {
        Self { key: 0, origin: [0.0; 3], radius: 0.0, die: 0.0, decay: 0.0, minlight: 0.0, color: [1.0; 3] }
    }
}

/// A beam segment to draw (CL_NewTempEntity output).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempEntity {
    pub model: &'static str,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
}

fn normalize(v: [f32; 3]) -> ([f32; 3], f32) {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { (v, 0.0) } else { ([v[0] / l, v[1] / l, v[2] / l], l) }
}

impl ClientState {
    #[inline]
    fn spawn(&mut self, org: [f32; 3], vel: [f32; 3], color: i32, die: f64, kind: u8, ramp: f32) -> bool {
        self.particles.spawn(ParticleSpawn { org, vel, color: color as u8, die: die as f32, kind, ramp }).is_some()
    }

    #[inline]
    fn r(&mut self) -> i32 {
        self.rng.rand()
    }

    /// `org + ((rand()%32)-16)` / vel `(rand()%512)-256` used by explosions.
    fn explode_ov(&mut self, org: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let mut o = [0.0; 3];
        let mut v = [0.0; 3];
        for j in 0..3 {
            o[j] = org[j] + ((self.r() % 32) - 16) as f32;
            v[j] = ((self.r() % 512) - 256) as f32;
        }
        (o, v)
    }

    /// CL_AllocDlight: reuse the light with the same nonzero key, else a dead
    /// one, else the first.
    pub fn alloc_dlight(&mut self, key: i32) -> &mut Dlight {
        let t = self.time;
        let s = self.dlights.as_slice();
        let idx = (key != 0)
            .then(|| s.iter().position(|d| d.key == key))
            .flatten()
            .or_else(|| s.iter().position(|d| d.die < t));
        let fresh = Dlight { key, ..Default::default() };
        let i = match idx {
            Some(i) => i,
            None if !self.dlights.is_full() => {
                self.dlights.insert(fresh);
                self.dlights.len() - 1
            }
            None => 0,
        };
        let d = &mut self.dlights.as_mut_slice()[i];
        *d = fresh;
        d
    }

    /// CL_DecayLights.
    pub fn decay_lights(&mut self) {
        let t = self.time;
        let dt = (self.time - self.oldtime).max(0.0) as f32;
        self.dlights.retain(|d| {
            d.radius -= dt * d.decay;
            d.die >= t && d.radius > 0.0
        });
    }

    /// R_RunParticleEffect.
    pub fn run_particle_effect(&mut self, org: [f32; 3], dir: [f32; 3], color: i32, count: i32) {
        let t = self.time;
        for i in 0..count {
            if count == 1024 {
                let (o, v) = self.explode_ov(org);
                let ramp = (self.r() & 3) as f32;
                let k = if i & 1 != 0 { PT_EXPLODE } else { PT_EXPLODE2 };
                if !self.spawn(o, v, RAMP1[0] as i32, t + 5.0, k, ramp) {
                    return;
                }
            } else {
                let die = t + 0.1 * (self.r() % 5) as f64;
                let c = (color & !7) + (self.r() & 7);
                let mut o = [0.0; 3];
                for j in 0..3 {
                    o[j] = org[j] + ((self.r() & 15) - 8) as f32;
                }
                let v = [dir[0] * 15.0, dir[1] * 15.0, dir[2] * 15.0];
                if !self.spawn(o, v, c, die, PT_SLOWGRAV, 0.0) {
                    return;
                }
            }
        }
    }

    /// R_ParticleExplosion.
    pub fn particle_explosion(&mut self, org: [f32; 3]) {
        self.run_particle_effect(org, [0.0; 3], 0, 1024);
    }

    /// R_ParticleExplosion2.
    pub fn particle_explosion2(&mut self, org: [f32; 3], color_start: i32, color_length: i32) {
        let t = self.time;
        for i in 0..512 {
            let c = color_start + i % color_length.max(1);
            let (o, v) = self.explode_ov(org);
            if !self.spawn(o, v, c, t + 0.3, PT_BLOB, 0.0) {
                return;
            }
        }
    }

    /// R_BlobExplosion.
    pub fn blob_explosion(&mut self, org: [f32; 3]) {
        let t = self.time;
        for i in 0..1024 {
            let die = t + 1.0 + (self.r() & 8) as f64 * 0.05;
            let (k, c) = if i & 1 != 0 { (PT_BLOB, 66 + self.r() % 6) } else { (PT_BLOB2, 150 + self.r() % 6) };
            let (o, v) = self.explode_ov(org);
            if !self.spawn(o, v, c, die, k, 0.0) {
                return;
            }
        }
    }

    /// R_LavaSplash.
    pub fn lava_splash(&mut self, org: [f32; 3]) {
        let t = self.time;
        for i in -16..16 {
            for j in -16..16 {
                let die = t + 2.0 + (self.r() & 31) as f64 * 0.02;
                let c = 224 + (self.r() & 7);
                let dir = [(j * 8 + (self.r() & 7)) as f32, (i * 8 + (self.r() & 7)) as f32, 256.0];
                let o = [org[0] + dir[0], org[1] + dir[1], org[2] + (self.r() & 63) as f32];
                let (n, _) = normalize(dir);
                let vel = (50 + (self.r() & 63)) as f32;
                if !self.spawn(o, [n[0] * vel, n[1] * vel, n[2] * vel], c, die, PT_SLOWGRAV, 0.0) {
                    return;
                }
            }
        }
    }

    /// R_TeleportSplash.
    pub fn teleport_splash(&mut self, org: [f32; 3]) {
        let t = self.time;
        for i in (-16..16).step_by(4) {
            for j in (-16..16).step_by(4) {
                for k in (-24..32).step_by(4) {
                    let die = t + 0.2 + (self.r() & 7) as f64 * 0.02;
                    let c = 7 + (self.r() & 7);
                    let dir = [(j * 8) as f32, (i * 8) as f32, (k * 8) as f32];
                    let o = [
                        org[0] + (i + (self.r() & 3)) as f32,
                        org[1] + (j + (self.r() & 3)) as f32,
                        org[2] + (k + (self.r() & 3)) as f32,
                    ];
                    let (n, _) = normalize(dir);
                    let vel = (50 + (self.r() & 63)) as f32;
                    if !self.spawn(o, [n[0] * vel, n[1] * vel, n[2] * vel], c, die, PT_SLOWGRAV, 0.0) {
                        return;
                    }
                }
            }
        }
    }

    /// R_RocketTrail. `ty` ≥ 128 means dense (step 1).
    pub fn rocket_trail(&mut self, start: [f32; 3], end: [f32; 3], mut ty: i32) {
        let t = self.time;
        let (vec, mut len) = normalize([end[0] - start[0], end[1] - start[1], end[2] - start[2]]);
        let dec = if ty < 128 {
            3.0
        } else {
            ty -= 128;
            1.0
        };
        let mut s = start;
        while len > 0.0 {
            len -= dec;
            let jitter = |me: &mut Self, s: [f32; 3]| {
                let mut o = [0.0; 3];
                for j in 0..3 {
                    o[j] = s[j] + ((me.r() % 6) - 3) as f32;
                }
                o
            };
            let ok = match ty {
                0 | 1 => {
                    let ramp = ((self.r() & 3) + if ty == 1 { 2 } else { 0 }) as f32;
                    let o = jitter(self, s);
                    self.spawn(o, [0.0; 3], RAMP3[ramp as usize] as i32, t + 2.0, PT_FIRE, ramp)
                }
                2 | 4 => {
                    let c = 67 + (self.r() & 3);
                    let o = jitter(self, s);
                    if ty == 4 {
                        len -= 3.0;
                    }
                    self.spawn(o, [0.0; 3], c, t + 2.0, PT_GRAV, 0.0)
                }
                3 | 5 => {
                    let tc = self.tracercount;
                    let c = if ty == 3 { 52 } else { 230 } + ((tc & 4) << 1);
                    self.tracercount += 1;
                    let v = if self.tracercount & 1 != 0 {
                        [30.0 * vec[1], 30.0 * -vec[0], 0.0]
                    } else {
                        [30.0 * -vec[1], 30.0 * vec[0], 0.0]
                    };
                    self.spawn(s, v, c, t + 0.5, PT_STATIC, 0.0)
                }
                6 => {
                    let c = 9 * 16 + 8 + (self.r() & 3);
                    let mut o = [0.0; 3];
                    for j in 0..3 {
                        o[j] = s[j] + ((self.r() & 15) - 8) as f32;
                    }
                    self.spawn(o, [0.0; 3], c, t + 0.3, PT_STATIC, 0.0)
                }
                _ => true,
            };
            if !ok {
                return;
            }
            for j in 0..3 {
                s[j] += vec[j];
            }
        }
    }

    /// R_EntityParticles (EF_BRIGHTFIELD).
    pub fn entity_particles(&mut self, origin: [f32; 3]) {
        if self.avelocities.is_empty() {
            for _ in 0..NUMVERTEXNORMALS {
                let a = [(self.r() & 255) as f32 * 0.01, (self.r() & 255) as f32 * 0.01, (self.r() & 255) as f32 * 0.01];
                self.avelocities.push(a);
            }
        }
        let (dist, beamlength) = (64.0, 16.0);
        let t = self.time;
        for (i, n) in AVERTEXNORMALS.iter().enumerate() {
            let av = self.avelocities[i];
            let (sy, cy) = ((t as f32) * av[0]).sin_cos();
            let (sp, cp) = ((t as f32) * av[1]).sin_cos();
            let fwd = [cp * cy, cp * sy, -sp];
            let o = [
                origin[0] + n[0] * dist + fwd[0] * beamlength,
                origin[1] + n[1] * dist + fwd[1] * beamlength,
                origin[2] + n[2] * dist + fwd[2] * beamlength,
            ];
            if !self.spawn(o, [0.0; 3], 0x6f, t + 0.01, PT_EXPLODE, 0.0) {
                return;
            }
        }
    }

    /// CL_RunParticles: kill expired, integrate, per-kind ramps and gravity.
    pub fn run_particles(&mut self) {
        let ft = (self.time - self.oldtime) as f32;
        let (time1, time2, time3) = (ft * 5.0, ft * 10.0, ft * 15.0);
        let grav = ft * self.sv_gravity * 0.05;
        let dvel = 4.0 * ft;
        let p = &mut self.particles;
        p.kill_expired(self.time as f32);
        p.integrate(ft);
        let mut i = 0;
        while i < p.len() {
            let mut dead = false;
            let mut ramp_to = |ramp: &mut f32, add: f32, max: f32, table: &[u8; 8], color: &mut u8| {
                *ramp += add;
                if *ramp >= max {
                    dead = true;
                } else {
                    *color = table[*ramp as usize];
                }
            };
            match p.kind[i] {
                PT_FIRE => {
                    ramp_to(&mut p.ramp[i], time1, 6.0, &RAMP3, &mut p.color[i]);
                    p.vz[i] += grav;
                }
                PT_EXPLODE => {
                    ramp_to(&mut p.ramp[i], time2, 8.0, &RAMP1, &mut p.color[i]);
                    p.vx[i] += p.vx[i] * dvel;
                    p.vy[i] += p.vy[i] * dvel;
                    p.vz[i] += p.vz[i] * dvel;
                    p.vz[i] -= grav;
                }
                PT_EXPLODE2 => {
                    ramp_to(&mut p.ramp[i], time3, 8.0, &RAMP2, &mut p.color[i]);
                    p.vx[i] -= p.vx[i] * ft;
                    p.vy[i] -= p.vy[i] * ft;
                    p.vz[i] -= p.vz[i] * ft;
                    p.vz[i] -= grav;
                }
                PT_BLOB => {
                    p.vx[i] += p.vx[i] * dvel;
                    p.vy[i] += p.vy[i] * dvel;
                    p.vz[i] += p.vz[i] * dvel;
                    p.vz[i] -= grav;
                }
                PT_BLOB2 => {
                    p.vx[i] -= p.vx[i] * dvel;
                    p.vy[i] -= p.vy[i] * dvel;
                    p.vz[i] -= grav;
                }
                PT_GRAV | PT_SLOWGRAV => p.vz[i] -= grav,
                _ => {}
            }
            if dead {
                p.kill(i);
            } else {
                i += 1;
            }
        }
    }

    /// CL_UpdateTEnts: beam segments into [`ClientState::temp_entities`].
    pub fn update_tents(&mut self) {
        self.temp_entities.clear();
        self.rng.srand((self.time * 1000.0) as i32 as u32);
        let t = self.time;
        let view = self.viewentity as i32;
        let view_origin = self.entities.get(self.viewentity).map(|e| e.origin);
        let mut out = std::mem::take(&mut self.temp_entities);
        let beams: Vec<_> = self.beams.as_mut_slice().iter_mut().filter(|b| b.starttime <= t && b.endtime >= t).map(|b| {
            if b.entity == view && let Some(o) = view_origin {
                b.start = o;
            }
            *b
        }).collect();
        'outer: for b in beams {
            let dist = [b.end[0] - b.start[0], b.end[1] - b.start[1], b.end[2] - b.start[2]];
            let (yaw, pitch) = if dist[0] == 0.0 && dist[1] == 0.0 {
                (0.0, if dist[2] > 0.0 { 90.0 } else { 270.0 })
            } else {
                let mut yaw = (dist[1].atan2(dist[0]).to_degrees()) as i32 as f32;
                if yaw < 0.0 {
                    yaw += 360.0;
                }
                let fwd = (dist[0] * dist[0] + dist[1] * dist[1]).sqrt();
                let mut pitch = (dist[2].atan2(fwd).to_degrees()) as i32 as f32;
                if pitch < 0.0 {
                    pitch += 360.0;
                }
                (yaw, pitch)
            };
            let (dir, mut d) = normalize(dist);
            let mut org = b.start;
            while d > 0.0 {
                if out.len() >= MAX_TEMP_ENTITIES {
                    break 'outer;
                }
                let roll = (self.rng.rand() % 360) as f32;
                out.push(TempEntity { model: b.model, origin: org, angles: [pitch, yaw, roll] });
                for j in 0..3 {
                    org[j] += dir[j] * 30.0;
                }
                d -= 30.0;
            }
        }
        self.temp_entities = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_and_lights() {
        let mut cl = ClientState::new();
        cl.time = 1.0;
        cl.oldtime = 1.0;
        cl.particle_explosion([0.0; 3]);
        assert_eq!(cl.particles.len(), 1024);
        cl.teleport_splash([0.0; 3]);
        cl.lava_splash([0.0; 3]);
        cl.rocket_trail([0.0; 3], [30.0, 0.0, 0.0], 0);
        cl.rocket_trail([0.0; 3], [30.0, 0.0, 0.0], 3);
        cl.entity_particles([0.0; 3]);
        let n = cl.particles.len();
        assert!(n > 1024 + 896 + 1024, "{n}");
        cl.explosion_light_for_test();
        assert_eq!(cl.dlights.len(), 1);
        for _ in 0..400 {
            cl.oldtime = cl.time;
            cl.time += 1.0 / 72.0;
            cl.run_particles();
            cl.decay_lights();
        }
        assert!(cl.particles.is_empty(), "{}", cl.particles.len());
        assert!(cl.dlights.is_empty());
    }

    impl ClientState {
        fn explosion_light_for_test(&mut self) {
            let d = self.alloc_dlight(5);
            d.radius = 350.0;
            d.decay = 300.0;
            d.die = 1.5;
            assert_eq!(self.alloc_dlight(5).key, 5);
            let d = self.alloc_dlight(5);
            d.radius = 350.0;
            d.decay = 300.0;
            d.die = 1.5;
        }
    }
}
