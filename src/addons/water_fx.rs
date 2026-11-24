//! Client presentation of the powerwasher: each jet is a straight,
//! lightning-gun style column of water from the muzzle to the end point
//! the server sends every frame (a camera-facing ribbon across the round
//! dome tile, so the gel shading makes it look cylindrical; its width
//! ripples along the length and flares at the tip), a few loose droplets
//! around it, splashes at the end, the jet hiss
//! loop and splash sounds, and the imported view model. The host also
//! washes blood marks off around [`WaterFx::wash_points`].
use super::gel_fx::{DOME, atlas, tile_uv};
use qcore::math::{Vec3, angle_vectors};
use crate::gfx::Renderer;
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

const LOOP_CHANNEL: i32 = 7;
/// Ribbon segments per jet, loose droplets around it, and splash cap per quality tier.
const SEGMENTS_HIGH: usize = 16;
const SEGMENTS_LOW: usize = 10;
const DROPS_HIGH: usize = 14;
const DROPS_LOW: usize = 6;
const MAX_SPLASHES_HIGH: usize = 120;
const MAX_SPLASHES_LOW: usize = 48;
const WIDTH: (f32, f32) = (1.6, 4.5);
const WATER: [u8; 3] = [150, 200, 235];
pub const WASH_RADIUS: f32 = 44.0;

#[derive(Clone, Copy, Debug, Default)]
struct Jet {
    mode: u8,
    end_count: u8,
    ends: [Vec3; 8],
    seen: f64,
}

impl Jet {
    fn ends(&self) -> &[Vec3] {
        &self.ends[..self.end_count as usize]
    }
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
        let mut nums = line.split_whitespace().filter_map(|w| w.parse::<f32>().ok()).filter(|v| v.is_finite());
        let (Some(o), Some(mode)) = (nums.next(), nums.next()) else { return };
        let o = (o as usize).min(63);
        if self.jets.len() <= o {
            self.jets.resize(o + 1, Jet::default());
        }
        let mode = mode as u8;
        let before = self.jets[o].mode;
        let mut ends = [Vec3::ZERO; 8];
        let mut end_count = 0usize;
        while end_count < ends.len() {
            let (Some(x), Some(y), Some(z)) = (nums.next(), nums.next(), nums.next()) else { break };
            ends[end_count] = Vec3::new(x, y, z);
            end_count += 1;
        }
        if let Some(snd) = snd
            && before != mode
        {
            let entity = o as i32 + 1;
            if mode == 0 {
                snd.event(SoundEvent::Stop { entity, channel: LOOP_CHANNEL });
            } else {
                let sfx = self.cue(snd, if mode == 1 { qsound::gel::JET } else { qsound::gel::FAN });
                let origin = if end_count > 0 { ends[0].to_array() } else { [0.0; 3] };
                snd.event(SoundEvent::Start { entity, channel: LOOP_CHANNEL, sfx, origin, volume: 0.55, attenuation: 1.2 });
            }
        }
        if let Some(snd) = snd
            && end_count > 0
            && time - self.last_splash_sound > 0.22
        {
            self.last_splash_sound = time;
            let sfx = self.cue(snd, qsound::gel::SPLASH);
            snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin: ends[0].to_array(), volume: 0.5, attenuation: 1.2 });
        }
        for &e in &ends[..end_count] {
            let v = Vec3::new(self.random() - 0.5, self.random() - 0.5, self.random() * 0.8 + 0.2) * 260.0;
            if self.splashes.len() < MAX_SPLASHES_HIGH {
                self.splashes.push(Splash { pos: e, vel: v, born: time });
            }
        }
        self.jets[o] = Jet { mode, end_count: end_count as u8, ends, seen: time };
    }

    /// Where water lands this frame (to wash blood off).
    pub fn wash_points(&self, time: f64) -> impl Iterator<Item = Vec3> + '_ {
        self.jets.iter().filter(move |j| j.mode != 0 && time - j.seen < 0.2).flat_map(|j| j.ends().iter().copied())
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
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, cl: &qclient::ClientState, me: usize, view: &View, gravity: f32, quality: u32, r: &mut Renderer, geom: &mut Geometry) {
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
        let high = quality > 0;
        let max_splashes = if high { MAX_SPLASHES_HIGH } else { MAX_SPLASHES_LOW };
        if self.splashes.len() > max_splashes {
            let excess = self.splashes.len() - max_splashes;
            self.splashes.drain(..excess);
        }
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
        self.emit_geometry(cl, me, view, quality, pic, geom);
    }

    fn emit_geometry(&self, cl: &qclient::ClientState, me: usize, view: &View, quality: u32, pic: PicId, geom: &mut Geometry) {
        let high = quality > 0;
        let (vf, vr, vu) = angle_vectors(view.angles.into());
        let eye = Vec3::from(view.origin);
        let segments = if high { SEGMENTS_HIGH } else { SEGMENTS_LOW };
        let drops = if high { DROPS_HIGH } else { DROPS_LOW };
        let t = self.time as f32;
        let ribbon_first = geom.1.len() as u32;
        for (o, j) in self.jets.iter().enumerate().filter(|(_, j)| j.mode != 0) {
            let Some((start, aim)) = jet_origin(cl, o, me, eye, vf, vu) else { continue };
            let fan = if j.mode == 1 { 1.0 } else { 0.7 };
            for (k, &end) in j.ends().iter().enumerate() {
                ribbon(geom, eye, start, aim, end, segments, fan, k as f32, t, high);
            }
        }
        let ribbon_count = geom.1.len() as u32 - ribbon_first;
        if high && ribbon_count > 0 {
            geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: ribbon_first, num_indices: ribbon_count, soft: false, gel: true });
        }
        let drops_first = geom.1.len() as u32;
        let drop_alpha = if high { 185 } else { 195 };
        for (o, j) in self.jets.iter().enumerate().filter(|(_, j)| j.mode != 0) {
            let Some((start, aim)) = jet_origin(cl, o, me, eye, vf, vu) else { continue };
            for (k, &end) in j.ends().iter().enumerate() {
                let len = (end - start).length();
                let bend = if aim == Vec3::ZERO { (start + end) * 0.5 } else { start + aim * len.min(48.0) };
                for d in 0..drops {
                    let h = hash(d as u32 * 7 + k as u32 * 131);
                    let u = (d as f32 / drops as f32 + t * 3.0 + h * 0.03).fract();
                    let spread = 2.0 + u * u * 9.0;
                    let jitter = (vr * (hash(d as u32 + 17) - 0.5) + vu * (hash(d as u32 + 29) - 0.5)) * spread;
                    drop(geom, jet_point(start, bend, end, len, u) + jitter, vr, vu, 0.8 + u * 1.4, drop_alpha);
                }
            }
        }
        for s in &self.splashes {
            let age = ((self.time - s.born) / 0.45) as f32;
            drop(geom, s.pos, vr, vu, 1.6 + age * 1.5, ((1.0 - age) * 200.0) as u8);
        }
        if high {
            let count = geom.1.len() as u32 - drops_first;
            if count > 0 {
                geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: drops_first, num_indices: count, soft: false, gel: false });
            }
        } else {
            let count = geom.1.len() as u32 - ribbon_first;
            if count > 0 {
                geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: ribbon_first, num_indices: count, soft: false, gel: false });
            }
        }
    }
}

fn jet_origin(cl: &qclient::ClientState, owner: usize, me: usize, eye: Vec3, vf: Vec3, vu: Vec3) -> Option<(Vec3, Vec3)> {
    if owner == me {
        Some((eye + vf * 34.0 - vu * 9.0, vf))
    } else {
        let e = cl.entities.get(owner + 1)?;
        Some((Vec3::from(e.origin) + Vec3::new(0.0, 0.0, 16.0), Vec3::ZERO))
    }
}

/// The jet leaves the muzzle along the aim, bending onto the server's line
/// over the first 48 units.
fn jet_point(start: Vec3, bend: Vec3, end: Vec3, len: f32, u: f32) -> Vec3 {
    let w = (u * len / 48.0).min(1.0);
    let near = start * (1.0 - u) + bend * u;
    let far = start * (1.0 - u) + end * u;
    near * (1.0 - w) + far * w
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

/// A camera-facing strip along `jet_point` with `segments` subdivisions,
/// mapped across the middle of the dome tile so it shades as a cylinder.
#[allow(clippy::too_many_arguments)]
fn ribbon(g: &mut Geometry, eye: Vec3, start: Vec3, aim: Vec3, end: Vec3, segments: usize, fan: f32, phase: f32, t: f32, high: bool) {
    let len = (end - start).length();
    let bend = if aim == Vec3::ZERO { (start + end) * 0.5 } else { start + aim * len.min(48.0) };
    let mut pts = [Vec3::ZERO; SEGMENTS_HIGH + 1];
    for (i, pt) in pts.iter_mut().enumerate().take(segments + 1) {
        *pt = jet_point(start, bend, end, len, i as f32 / segments as f32);
    }
    let base = g.0.len() as u32;
    let alpha_scale = if high { 200.0 } else { 225.0 };
    for (i, &p) in pts.iter().enumerate().take(segments + 1) {
        let u = i as f32 / segments as f32;
        let along = pts[(i + 1).min(segments)] - pts[i.saturating_sub(1)];
        let side = along.cross(eye - p).normalize();
        let side = if side.length_sq() > 0.5 { side } else { Vec3::new(0.0, 0.0, 1.0) };
        let ripple = (u * 31.0 - t * 31.0 + phase).sin() * 0.25 + (u * 47.0 - t * 53.0).sin() * 0.12;
        let flare = ((u - 0.85) / 0.15).max(0.0) * 1.5;
        let w = (WIDTH.0 + (WIDTH.1 - WIDTH.0) * u) * fan * (1.0 + ripple + flare);
        // fade in at the nozzle and out at the tip
        let a = (u * 12.0).min(1.0) * ((1.0 - u) * 6.0).min(1.0);
        let color = [WATER[0], WATER[1], WATER[2], (alpha_scale * a) as u8];
        g.0.push(FteVertex { pos: (p - side * w).to_array(), uv: tile_uv([0.0, 0.5], DOME), color });
        g.0.push(FteVertex { pos: (p + side * w).to_array(), uv: tile_uv([1.0, 0.5], DOME), color });
    }
    for i in 0..segments as u32 {
        let v = base + i * 2;
        g.1.extend([v, v + 1, v + 3, v, v + 3, v + 2]);
    }
}

/// One water droplet facing the camera.
fn drop(g: &mut Geometry, c: Vec3, r: Vec3, u: Vec3, s: f32, alpha: u8) {
    let base = g.0.len() as u32;
    for (x, y, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
        g.0.push(FteVertex { pos: (c + r * x * s + u * y * s).to_array(), uv: tile_uv(uv, DOME), color: [WATER[0], WATER[1], WATER[2], alpha] });
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

    #[test]
    fn quality_tiers_scale_geometry_and_gel_batches() {
        let mut w = WaterFx::default();
        // 5-jet fan spawns 5 splashes per receive, up to MAX_SPLASHES_HIGH.
        for step in 0..30 {
            w.receive("0 2 100 0 0 100 10 0 100 -10 0 100 0 10 100 0 -10", 1.0 + step as f64 * 0.01, None);
        }
        w.time = 1.3;
        assert_eq!(w.splashes.len(), MAX_SPLASHES_HIGH);
        let cl = qclient::ClientState::default();
        let view = View::default();

        let mut high = Geometry::default();
        w.emit_geometry(&cl, 0, &view, 1, PicId(0), &mut high);
        assert_eq!(high.2.len(), 2);
        assert!(high.2[0].gel);
        assert!(!high.2[1].gel);
        assert_eq!(high.2[0].num_indices, 5 * SEGMENTS_HIGH as u32 * 6);
        assert_eq!(high.2[1].num_indices, (5 * DROPS_HIGH as u32 + MAX_SPLASHES_HIGH as u32) * 6);

        let mut low = Geometry::default();
        w.splashes.drain(..w.splashes.len() - MAX_SPLASHES_LOW);
        w.emit_geometry(&cl, 0, &view, 0, PicId(0), &mut low);
        assert_eq!(low.2.len(), 1);
        assert!(!low.2[0].gel);
        assert_eq!(low.2[0].num_indices, (5 * SEGMENTS_LOW as u32 + 5 * DROPS_LOW as u32 + MAX_SPLASHES_LOW as u32) * 6);
    }
}
