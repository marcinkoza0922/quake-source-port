//! Akimbo pistols presentation: the two view models with their recoil, slide
//! and reload animations, muzzle flashes, tracers, smoke, casings, and the
//! synthesised cues ([`qsound::akimbo`]).
use super::akimbo_model::{self, GRIP, MAG_OUT, MUZZLE, PORT, SLIDE, WELL};
use crate::gfx::Renderer;
use qclient::guns::{self, GunEvent};
use qcore::math::{Vec3, angle_vectors};
use qrender::{DLight, FteBatch, FteBlend, FteVertex, ModelId, PicId, RenderEntity, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

/// Seconds a muzzle flash, a tracer, smoke and a casing last.
const FLASH: f32 = 0.06;
const TRACER: f32 = 0.09;
const SMOKE: f32 = 0.7;
const CASING: f32 = 0.9;
const MAX_FX: usize = 64;
/// Sound channel of the left pistol on its owner's entity (the right one
/// uses the next).
const CHANNEL: i32 = 6;
/// When the magazine leaves, comes in, and the slide is worked, as
/// fractions of a reload.
const OUT: f32 = 0.2;
const IN: f32 = 0.62;
const RACK: f32 = 0.82;

/// One pistol of one player.
#[derive(Clone, Copy, Debug)]
struct Gun {
    fired: f64,
    /// Start and length of the running reload.
    reload: Option<(f64, f32)>,
    /// Reload cues played so far.
    cues: u8,
    /// World muzzle and ejection port of the view model and when they were set.
    muzzle: Vec3,
    port: Vec3,
    seen: f64,
}

impl Default for Gun {
    fn default() -> Self {
        Self { fired: -100.0, reload: None, cues: 0, muzzle: Vec3::ZERO, port: Vec3::ZERO, seen: -100.0 }
    }
}

struct Shot {
    time: f64,
    owner: usize,
    part: usize,
    start: Vec3,
    end: Vec3,
    /// Casing start and velocity.
    port: Vec3,
    vel: Vec3,
    seed: f32,
}

#[derive(Default)]
pub struct Presentation {
    guns: Vec<[Gun; 2]>,
    shots: Vec<Shot>,
    count: u32,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pic: Option<PicId>,
}

/// A pistol's pose: origin, its X (muzzle) and Z (up) axes.
#[derive(Clone, Copy, Debug)]
struct Pose {
    origin: Vec3,
    x: Vec3,
    z: Vec3,
}

impl Pose {
    /// World point of model point `p` of the right pistol (`mirrored` for
    /// the left one).
    fn at(&self, p: Vec3, mirrored: bool) -> Vec3 {
        let y = self.z.cross(self.x);
        self.origin + self.x * p.x + y * (if mirrored { -p.y } else { p.y }) + self.z * p.z
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rotates `a` towards `b` by `deg` (both unit, perpendicular).
fn turn(a: Vec3, b: Vec3, deg: f32) -> (Vec3, Vec3) {
    let (s, c) = deg.to_radians().sin_cos();
    (a * c + b * s, b * c - a * s)
}

/// The reload animation `u` (0..1) of a pistol: how far it is tipped (0..1),
/// and the magazine's way below the grip (None: seated).
fn reload_pose(u: f32) -> (f32, Option<f32>) {
    let tip = smooth(0.0, 0.16, u) * (1.0 - smooth(0.86, 1.0, u));
    let mag = if u < OUT || u >= IN {
        None
    } else if u < 0.42 {
        // falls out
        let t = (u - OUT) / (0.42 - OUT);
        Some(t * 4.0 + t * t * 26.0)
    } else {
        // a new one comes up
        let t = smooth(0.42, IN, u);
        Some((1.0 - t) * 16.0)
    };
    (tip, mag)
}

/// Pose of pistol `part` (0 left, 1 right) for a camera: in the lower
/// corners, muzzles slightly inwards. `recoil` 0..1, `tip` 0..1 (reload).
fn pose(view: View, part: usize, bob: f32, recoil: f32, tip: f32) -> Pose {
    let (f, r, u) = angle_vectors(view.angles.into());
    let s = if part == 0 { -1.0 } else { 1.0 };
    let out = r * s;
    // the muzzle points at the crosshair's far end, tops lean inwards; a reload shows the grip
    let x = (f - out * 0.10 + u * 0.06).normalize();
    let y = (u - x * u.dot(x)).normalize();
    let (x, z) = turn(x, y, recoil * 5.0 + tip * 30.0);
    let lean = out - x * out.dot(x) - z * out.dot(z);
    let (z, _) = turn(z, lean.normalize(), -8.0 + tip * 52.0);
    let origin = Vec3::from(view.origin) + f * (25.0 - recoil * 1.2 + tip * 7.0) + out * (9.5 + tip * 3.0)
        - u * (11.5 + bob + recoil * 0.2 - tip * 7.5);
    Pose { origin, x, z }
}

impl Presentation {
    /// Drops every effect in flight (new map).
    pub fn clear(&mut self) {
        self.guns.clear();
        self.shots.clear();
    }

    fn gun(&mut self, owner: usize, part: usize) -> &mut Gun {
        if self.guns.len() <= owner {
            self.guns.resize(owner + 1, Default::default());
        }
        &mut self.guns[owner][part]
    }

    fn play(&mut self, snd: &SoundSystem, cue: usize, owner: usize, part: usize, origin: Vec3, volume: f32) {
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..qsound::akimbo::COUNT).map(|k| Arc::new(qsound::akimbo::render(k, snd.rate()))).collect();
        }
        let sfx = self.cues[cue].clone();
        snd.event(SoundEvent::Start { entity: owner as i32 + 1, channel: CHANNEL + part as i32, sfx, origin: origin.to_array(), volume, attenuation: 1.0 });
    }

    /// One `//qsp_gun` event of this toy.
    pub fn receive(&mut self, e: &GunEvent, time: f64, snd: Option<&SoundSystem>) {
        let (owner, part) = (e.owner as usize, usize::from(e.part == 1));
        let gun = self.gun(owner, part);
        let cue = match e.kind {
            guns::FIRE => {
                gun.fired = time;
                Some((qsound::akimbo::SHOT + part, 0.9))
            }
            guns::DRY => Some((qsound::akimbo::DRY, 0.6)),
            guns::RELOAD => {
                gun.reload = Some((time, e.secs.max(0.1)));
                gun.cues = 0;
                None
            }
            guns::CANCEL | guns::DONE => {
                gun.reload = None;
                None
            }
            _ => None,
        };
        if let (Some((cue, volume)), Some(snd)) = (cue, snd) {
            self.play(snd, cue, owner, part, e.start.into(), volume);
        }
        if e.kind == guns::FIRE {
            self.shot(e, owner, part, time);
        }
    }

    fn shot(&mut self, e: &GunEvent, owner: usize, part: usize, time: f64) {
        let g = *self.gun(owner, part);
        self.count = self.count.wrapping_add(1);
        let rnd = |k: u32| (self.count.wrapping_mul(2_654_435_761).wrapping_add(k * 40_503) >> 8) as f32 % 997.0 / 997.0;
        let (start, end) = (Vec3::from(e.start), Vec3::from(e.end));
        // the view model's own muzzle and port when it is on screen
        let shown = (time - g.seen).abs() < 0.1;
        let dir = (end - start).normalize();
        let side = dir.cross(Vec3::new(0.0, 0.0, 1.0)).normalize() * if part == 0 { -1.0 } else { 1.0 };
        let port = if shown { g.port } else { start - dir * 10.0 };
        let vel = side * (70.0 + 40.0 * rnd(1)) + Vec3::new(0.0, 0.0, 110.0 + 40.0 * rnd(2)) - dir * (20.0 * rnd(3));
        if self.shots.len() >= MAX_FX {
            self.shots.remove(0);
        }
        let seed = rnd(4) * std::f32::consts::TAU;
        self.shots.push(Shot { time, owner, part, start: if shown { g.muzzle } else { start }, end, port, vel, seed });
    }

    /// Muzzle light and the reload cues that are due.
    pub fn lights(&mut self, time: f64, cl: &qclient::ClientState, snd: Option<&SoundSystem>, dls: &mut Vec<DLight>) {
        self.shots.retain(|s| time >= s.time && time - s.time < f64::from(CASING));
        for s in &mut self.shots {
            let age = (time - s.time) as f32;
            if age < FLASH * 1.5 {
                // the flash stays at the muzzle as the view moves
                if let Some(g) = self.guns.get(s.owner).map(|g| g[s.part]).filter(|g| (time - g.seen).abs() < 0.1) {
                    s.start = g.muzzle;
                }
                let k = 1.0 - age / (FLASH * 1.5);
                dls.push(DLight { origin: s.start.to_array(), radius: 80.0 + 120.0 * k, minlight: 0.0, color: [1.0, 0.8, 0.45], key: 0 });
            }
        }
        for owner in 0..self.guns.len() {
            for part in 0..2 {
                let g = &mut self.guns[owner][part];
                let Some((start, secs)) = g.reload else { continue };
                let u = ((time - start) as f32) / secs;
                let marks = [(OUT, qsound::akimbo::MAG_OUT), (IN - 0.04, qsound::akimbo::MAG_IN), (RACK - 0.06, qsound::akimbo::SLIDE)];
                if let Some(&(at, cue)) = marks.get(g.cues as usize)
                    && u >= at
                {
                    g.cues += 1;
                    let origin = cl.entities.get(owner + 1).map_or(Vec3::ZERO, |e| e.origin.into());
                    if let Some(snd) = snd {
                        self.play(snd, cue, owner, part, origin, 0.75);
                    }
                }
            }
        }
    }

    fn model(key: &str, build: impl Fn(&[u8]) -> qformats::mdl::Mdl, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<ModelId> {
        if !lv.by_name.contains_key(key) {
            let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
            let m = r.load_alias(&build(&palette)).ok().map(|id| crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0, md3: false });
            lv.by_name.insert(key.into(), m);
        }
        lv.by_name.get(key).copied().flatten().map(|m| m.id)
    }

    /// `owner`'s view model entities while the toy is held.
    #[allow(clippy::too_many_arguments)]
    pub fn weapons(&mut self, owner: usize, cl: &qclient::ClientState, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Vec<RenderEntity> {
        let models = [
            Self::model("@akimbo_l", |p| akimbo_model::pistol(p, true), lv, fs, r),
            Self::model("@akimbo_r", |p| akimbo_model::pistol(p, false), lv, fs, r),
        ];
        let mag = Self::model("@akimbo_mag", akimbo_model::magazine, lv, fs, r);
        let mags = cl.mags.iter().find(|m| m.owner as usize == owner && m.toy == qserver::addons::Toy::Akimbo.id());
        let empty = [0, 1].map(|k| mags.and_then(|m| m.parts.get(k)).is_some_and(|p| p.rounds == 0));
        let mut out = Vec::new();
        for part in 0..2 {
            let Some(model) = models[part] else { continue };
            let g = self.gun(owner, part);
            let age = (time - g.fired).max(0.0) as f32;
            let recoil = if age < 0.03 { age / 0.03 } else { (-(age - 0.03) * 12.0).exp() };
            let u = g.reload.map(|(t, secs)| (((time - t) as f32) / secs).clamp(0.0, 1.0));
            let (tip, drop) = u.map_or((0.0, None), reload_pose);
            let bob = (time as f32 * 2.1 + part as f32 * 1.9).sin() * 0.2;
            let p = pose(view, part, bob, recoil, tip);
            // the slide kicks back with each shot, stays back when empty,
            // and is worked at the end of a reload
            let shot = (1.0 - (age - 0.02).abs() / 0.05).clamp(0.0, 1.0);
            let rack = u.map_or(0.0, |u| smooth(RACK - 0.07, RACK - 0.01, u) * (1.0 - smooth(RACK, RACK + 0.04, u)));
            let locked = empty[part] && u.is_none_or(|u| u < RACK - 0.01);
            let slide = if locked { 1.0 } else { shot.max(rack) };
            let frame = if drop.is_some() { MAG_OUT } else { 0 };
            out.push(RenderEntity {
                model,
                origin: p.origin.to_array(),
                angles: super::cable_fx::angles_basis(p.x, p.z),
                prev_frame: frame,
                frame: frame + SLIDE,
                lerp: slide,
                viewmodel: true,
                ..Default::default()
            });
            if let (Some(d), Some(model)) = (drop, mag) {
                let up = p.x * GRIP.x + p.z * GRIP.z;
                let fwd = p.x * GRIP.z - p.z * GRIP.x;
                let origin = p.at(WELL, part == 0) - up * d;
                out.push(RenderEntity { model, origin: origin.to_array(), angles: super::cable_fx::angles_basis(fwd, up), viewmodel: true, ..Default::default() });
            }
            let g = self.gun(owner, part);
            g.muzzle = p.at(MUZZLE, part == 0);
            g.port = p.at(PORT, part == 0);
            g.seen = time;
        }
        out
    }

    /// Effect geometry (tracers, flashes, smoke, casings).
    pub fn frame(&mut self, time: f64, view: &View, r: &mut Renderer, geom: &mut Geometry) {
        if self.shots.is_empty() {
            return;
        }
        let Some(pic) = self.pic(r) else { return };
        let (f, right, up) = angle_vectors(view.angles.into());
        let eye = Vec3::from(view.origin);
        let batch = |geom: &mut Geometry, blend, first: u32| {
            let n = geom.1.len() as u32 - first;
            if n > 0 {
                geom.2.push(FteBatch { pic, blend, lines: false, first_index: first, num_indices: n, soft: false, gel: false });
            }
        };
        let first = geom.1.len() as u32;
        for s in &self.shots {
            let age = (time - s.time) as f32;
            if age < SMOKE {
                let k = age / SMOKE;
                for i in 0..3 {
                    let w = s.seed + i as f32 * 2.1;
                    let p = s.start + Vec3::new(w.cos() * 3.0, w.sin() * 3.0, 14.0 + 5.0 * i as f32) * k + (s.end - s.start).normalize() * (2.0 + 3.0 * i as f32);
                    quad(geom, p, right, up, 1.5 + 4.0 * k, [120, 115, 108], (1.0 - k) * 0.16 * smooth(0.0, 0.08, age));
                }
            }
            // the brass tumbles out of the port
            let p = s.port + s.vel * age - Vec3::new(0.0, 0.0, 400.0 * age * age);
            if age < CASING {
                let (a, b) = turn(right, up, (s.seed * 57.3 + age * 1100.0) % 360.0);
                let fade = 1.0 - smooth(CASING * 0.7, CASING, age);
                solid(geom, p, a * 0.75, b * 0.3, [176, 136, 52], fade);
                solid(geom, p + b * 0.12, a * 0.6, b * 0.08, [235, 205, 120], fade);
            }
        }
        batch(geom, FteBlend::Blend, first);
        let first = geom.1.len() as u32;
        for s in &self.shots {
            let age = (time - s.time) as f32;
            let d = s.end - s.start;
            let len = d.length();
            if age < TRACER && len > 48.0 {
                // a streak racing to where the bullet landed
                let dir = d * (1.0 / len);
                let t = age / TRACER;
                let (a, b) = ((t * len).min(len - 8.0), (t * len + 160.0).min(len));
                let side = dir.cross(s.start + dir * a - eye).normalize() * 0.6;
                ribbon(geom, s.start + dir * a, s.start + dir * b, side, [255, 220, 140], 0.75 * (1.0 - t * 0.6));
            }
            if age < FLASH {
                let k = 1.0 - age / FLASH;
                let at = s.start + d.normalize() * 1.5;
                let (a, b) = turn(right, up, (s.seed * 57.3) % 360.0);
                quad(geom, at, a, b, 6.0 + 3.0 * k, [255, 170, 70], 0.8 * k + 0.2);
                quad(geom, at, a, b, 3.0, [255, 245, 200], 1.0);
                // a ragged star
                for i in 0..5 {
                    let w = s.seed + i as f32 * 1.2566;
                    let l = 9.0 + 4.0 * ((s.seed * 7.0 + i as f32 * 2.3).sin());
                    let tip = a * w.cos() + b * w.sin();
                    let perp = f.cross(tip).normalize() * 1.8;
                    ribbon(geom, at, at + tip * l, perp, [255, 205, 110], k);
                }
            }
        }
        batch(geom, FteBlend::AddA, first);
    }

    /// A soft dot, with a solid block in the corner for casings.
    fn pic(&mut self, r: &mut Renderer) -> Option<PicId> {
        if self.pic.is_none() {
            let mut px = Vec::new();
            for y in 0..32 {
                for x in 0..32 {
                    let d = ((x as f32 - 11.5).powi(2) + (y as f32 - 11.5).powi(2)).sqrt() / 11.5;
                    let solid = x >= 26 && y >= 26;
                    px.extend([255, 255, 255, if solid { 255 } else { ((1.0 - d).max(0.0).powf(1.5) * 255.0) as u8 }]);
                }
            }
            self.pic = r.register_pic_rgba_linear(32, 32, &px).ok();
        }
        self.pic
    }
}

/// Atlas corner of the soft dot and centre of the solid block.
const DOT: f32 = 24.0 / 32.0;
const BLOCK: [f32; 2] = [29.0 / 32.0, 29.0 / 32.0];

fn push(g: &mut Geometry, pts: [Vec3; 4], uv: [[f32; 2]; 4], rgb: [u8; 3], alpha: f32) {
    let base = g.0.len() as u32;
    let color = [rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0) as u8];
    g.0.extend(pts.into_iter().zip(uv).map(|(p, uv)| FteVertex { pos: p.to_array(), uv, color }));
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A soft round billboard.
fn quad(g: &mut Geometry, p: Vec3, r: Vec3, u: Vec3, size: f32, rgb: [u8; 3], alpha: f32) {
    let (r, u) = (r * size, u * size);
    push(g, [p - r - u, p + r - u, p + r + u, p - r + u], [[0.0, 0.0], [DOT, 0.0], [DOT, DOT], [0.0, DOT]], rgb, alpha);
}

/// A hard-edged rectangle.
fn solid(g: &mut Geometry, p: Vec3, r: Vec3, u: Vec3, rgb: [u8; 3], alpha: f32) {
    push(g, [p - r - u, p + r - u, p + r + u, p - r + u], [BLOCK; 4], rgb, alpha);
}

/// A soft streak from `a` to `b`, brightest in the middle.
fn ribbon(g: &mut Geometry, a: Vec3, b: Vec3, side: Vec3, rgb: [u8; 3], alpha: f32) {
    push(g, [a - side, b - side, b + side, a + side], [[0.0, 0.0], [DOT, 0.0], [DOT, DOT], [0.0, DOT]], rgb, alpha);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pistols_keep_their_place_on_screen_at_any_look_angle() {
        for pitch in [-85.0, 0.0, 60.0, 85.0] {
            for yaw in [0.0, 130.0] {
                let view = View { origin: [10.0, 20.0, 30.0], angles: [pitch, yaw, 0.0], ..Default::default() };
                let (f, r, u) = angle_vectors(view.angles.into());
                for (part, s) in [(0, -1.0), (1, 1.0)] {
                    for (recoil, tip) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)] {
                        let p = pose(view, part, 0.0, recoil, tip);
                        assert!((p.x.length() - 1.0).abs() < 1e-3 && (p.z.length() - 1.0).abs() < 1e-3 && p.x.dot(p.z).abs() < 1e-3);
                        let level = pose(View { origin: [10.0, 20.0, 30.0], ..Default::default() }, part, 0.0, recoil, tip);
                        let d = p.origin - Vec3::from(view.origin);
                        let e = level.origin - Vec3::from(view.origin);
                        assert!((d.dot(f) - e.x).abs() < 1e-2 && (d.dot(r) + e.y).abs() < 1e-2 && (d.dot(u) - e.z).abs() < 1e-2);
                        assert!(d.dot(r) * s > 5.0 && d.dot(f) > 10.0 && d.dot(u) < -3.0);
                    }
                }
            }
        }
    }

    #[test]
    fn the_magazine_is_out_only_in_the_middle_of_a_reload() {
        assert_eq!(reload_pose(0.0), (0.0, None));
        assert_eq!(reload_pose(1.0), (0.0, None));
        assert!(reload_pose(OUT).1.is_some_and(|d| d < 0.1));
        assert!(reload_pose(0.4).1.is_some_and(|d| d > 15.0));
        assert!(reload_pose(IN - 0.001).1.is_some_and(|d| d < 0.1));
        assert_eq!(reload_pose(IN).1, None);
    }

    #[test]
    fn events_drive_each_gun_separately() {
        let mut p = Presentation::default();
        let e = |part, kind| GunEvent { owner: 1, toy: 9, part, kind, secs: 1.5, flesh: false, start: [0.0; 3], end: [100.0, 0.0, 0.0], normal: [0.0; 3] };
        p.receive(&e(0, guns::FIRE), 5.0, None);
        p.receive(&e(1, guns::RELOAD), 5.0, None);
        assert_eq!((p.guns[1][0].fired, p.guns[1][0].reload), (5.0, None));
        assert_eq!(p.guns[1][1].reload, Some((5.0, 1.5)));
        assert_eq!(p.shots.len(), 1);
        p.receive(&e(1, guns::CANCEL), 5.2, None);
        assert_eq!(p.guns[1][1].reload, None);
        for _ in 0..200 {
            p.receive(&e(0, guns::FIRE), 6.0, None);
        }
        assert_eq!(p.shots.len(), MAX_FX);
        let mut dls = Vec::new();
        p.lights(6.0, &qclient::ClientState::default(), None, &mut dls);
        assert!(!dls.is_empty());
        p.lights(9.0, &qclient::ClientState::default(), None, &mut dls);
        assert!(p.shots.is_empty());
    }
}
