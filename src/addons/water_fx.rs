//! Client presentation of the powerwasher: water jets as streams of
//! glossy droplets (gel-shaded, translucent) arcing from the muzzle to the
//! end points the server sends every frame, splashes there, the jet hiss
//! loop and splash sounds, and the imported view model. The host also
//! washes blood marks off around [`WaterFx::wash_points`].
use super::gel_fx::{DOME, atlas, tile_uv};
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

const LOOP_CHANNEL: i32 = 7;
/// Droplets drawn per jet and their size near the muzzle / at the end.
const DROPS: usize = 180;
const SIZE: (f32, f32) = (1.3, 4.2);
pub const WASH_RADIUS: f32 = 44.0;

#[derive(Clone, Debug, Default)]
struct Jet {
    mode: u8,
    ends: Vec<Vec3>,
    seen: f64,
}

struct Splash {
    pos: Vec3,
    vel: Vec3,
    born: f64,
}

#[derive(Default)]
pub struct WaterFx {
    jets: Vec<Jet>,
    splashes: Vec<Splash>,
    last_splash_sound: f64,
    seed: u32,
    time: f64,
    pic: Option<PicId>,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
}

impl WaterFx {
    pub fn clear(&mut self) {
        self.jets.clear();
        self.splashes.clear();
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    fn cue(&mut self, snd: &SoundSystem, k: usize) -> Arc<Sfx> {
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..qsound::gel::COUNT).map(|k| Arc::new(qsound::gel::render(k, snd.rate()))).collect();
        }
        self.cues[k].clone()
    }

    /// One `//qsp_water owner mode [x y z]...` line (without the prefix).
    pub fn receive(&mut self, line: &str, time: f64, snd: Option<&SoundSystem>) {
        let n: Vec<f32> = line.split_whitespace().filter_map(|w| w.parse().ok()).filter(|v: &f32| v.is_finite()).collect();
        let (Some(&o), Some(&mode)) = (n.first(), n.get(1)) else { return };
        let o = (o as usize).min(63);
        if self.jets.len() <= o {
            self.jets.resize(o + 1, Jet::default());
        }
        let mode = mode as u8;
        let before = self.jets[o].mode;
        let ends: Vec<Vec3> = n[2..].as_chunks::<3>().0.iter().map(|c| Vec3::new(c[0], c[1], c[2])).take(8).collect();
        if let Some(snd) = snd
            && before != mode
        {
            let entity = o as i32 + 1;
            if mode == 0 {
                snd.event(SoundEvent::Stop { entity, channel: LOOP_CHANNEL });
            } else {
                let sfx = self.cue(snd, if mode == 1 { qsound::gel::JET } else { qsound::gel::FAN });
                let origin = ends.first().map_or([0.0; 3], |e| e.to_array());
                snd.event(SoundEvent::Start { entity, channel: LOOP_CHANNEL, sfx, origin, volume: 0.55, attenuation: 1.2 });
            }
        }
        if let (Some(snd), Some(&e)) = (snd, ends.first())
            && time - self.last_splash_sound > 0.22
        {
            self.last_splash_sound = time;
            let sfx = self.cue(snd, qsound::gel::SPLASH);
            snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin: e.to_array(), volume: 0.5, attenuation: 1.2 });
        }
        for &e in &ends {
            for _ in 0..2 {
                let v = Vec3::new(self.random() - 0.5, self.random() - 0.5, self.random() * 0.8 + 0.2) * 260.0;
                if self.splashes.len() < 400 {
                    self.splashes.push(Splash { pos: e, vel: v, born: time });
                }
            }
        }
        self.jets[o] = Jet { mode, ends, seen: time };
    }

    /// Where water lands this frame (to wash blood off).
    pub fn wash_points(&self, time: f64) -> impl Iterator<Item = Vec3> + '_ {
        self.jets.iter().filter(move |j| j.mode != 0 && time - j.seen < 0.2).flat_map(|j| j.ends.iter().copied())
    }

    /// Player `owner` is spraying now.
    pub fn spraying(&self, owner: usize, time: f64) -> bool {
        self.jets.get(owner).is_some_and(|j| j.mode != 0 && time - j.seen < 0.2)
    }

    pub fn weapon(&self, owner: usize, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
        let id = super::toy_model::get("@powerwasher", include_bytes!("../../assets/washer/model.mdl.zlib"), include_bytes!("../../assets/washer/texture.rgb.zlib"), false, lv, fs, r)?;
        let shake = if self.spraying(owner, time) { (time as f32 * 70.0).sin() * 0.3 - 0.8 } else { 0.0 };
        let bob = (time as f32 * 2.5).sin() * 0.15;
        let (f, _, u) = angle_vectors(view.angles.into());
        let origin = (Vec3::from(view.origin) + f * (24.0 + shake) - u * (17.0 + bob)).to_array();
        let angles = [-view.angles[0], view.angles[1], view.angles[2]];
        Some(RenderEntity { model: id, origin, angles, viewmodel: true, ..Default::default() })
    }

    /// Droplet streams and splashes. `me` is the local player (0-based),
    /// whose jets leave the view model's muzzle.
    pub fn frame(&mut self, cl: &qclient::ClientState, me: usize, view: &View, gravity: f32, r: &mut Renderer, geom: &mut Geometry) {
        let dt = (cl.time - self.time).clamp(0.0, 0.1) as f32;
        self.time = cl.time;
        let now = cl.time;
        for j in &mut self.jets {
            // the server stops sending when a player leaves mid-spray
            if j.mode != 0 && now - j.seen > 0.5 {
                j.mode = 0;
            }
        }
        self.splashes.retain_mut(|s| {
            s.vel.z -= gravity * dt;
            s.pos += s.vel * dt;
            now - s.born < 0.45
        });
        if self.splashes.is_empty() && self.jets.iter().all(|j| j.mode == 0) {
            return;
        }
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let Ok(p) = r.register_pic_rgba_linear(256, 256, &atlas()) else { return };
                self.pic = Some(p);
                p
            }
        };
        let (vf, vr, vu) = angle_vectors(view.angles.into());
        let eye = Vec3::from(view.origin);
        let first = geom.1.len() as u32;
        let t = now as f32;
        for (o, j) in self.jets.iter().enumerate().filter(|(_, j)| j.mode != 0) {
            let (start, aim) = if o == me {
                (eye + vf * 34.0 - vu * 9.0, vf)
            } else {
                let Some(e) = cl.entities.get(o + 1) else { continue };
                (Vec3::from(e.origin) + Vec3::new(0.0, 0.0, 16.0), Vec3::ZERO)
            };
            for (k, &end) in j.ends.iter().enumerate() {
                let len = (end - start).length();
                // the arc leaves along the aim and sags onto the end point
                let ctrl = if aim == Vec3::ZERO { (start + end) * 0.5 + Vec3::new(0.0, 0.0, len * 0.08) } else { start + aim * len * 0.5 };
                for d in 0..DROPS {
                    let h = hash(d as u32 * 7 + k as u32 * 131);
                    let u = (d as f32 / DROPS as f32 + t * 2.2 + h * 0.03).fract();
                    let p = start * (1.0 - u).powi(2) + ctrl * 2.0 * u * (1.0 - u) + end * u * u;
                    let spread = (if j.mode == 1 { 1.0 } else { 2.0 }) * (0.6 + u * u * 6.0);
                    let jitter = (vr * (hash(d as u32 + 17) - 0.5) + vu * (hash(d as u32 + 29) - 0.5)) * spread;
                    let size = SIZE.0 + (SIZE.1 - SIZE.0) * u;
                    drop(geom, p + jitter, vr, vu, size * (0.7 + h * 0.6), 170);
                }
            }
        }
        for s in &self.splashes {
            let age = ((now - s.born) / 0.45) as f32;
            drop(geom, s.pos, vr, vu, 1.6 + age * 1.5, ((1.0 - age) * 170.0) as u8);
        }
        let count = geom.1.len() as u32 - first;
        if count > 0 {
            geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: count, soft: false, gel: true });
        }
    }
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// One glossy water droplet facing the camera.
fn drop(g: &mut Geometry, c: Vec3, r: Vec3, u: Vec3, s: f32, alpha: u8) {
    let base = g.0.len() as u32;
    for (x, y, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
        g.0.push(FteVertex { pos: (c + r * x * s + u * y * s).to_array(), uv: tile_uv(uv, DOME), color: [150, 200, 235, alpha] });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_lines_track_jets() {
        let mut w = WaterFx::default();
        w.receive("0 2 1 2 3 4 5 6", 1.0, None);
        assert_eq!(w.wash_points(1.0).count(), 2);
        assert!(w.spraying(0, 1.05));
        w.receive("0 0", 1.1, None);
        assert_eq!(w.wash_points(1.1).count(), 0);
    }
}
