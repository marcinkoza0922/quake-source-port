//! Railgun presentation: the Quake III style spiral trail (bright core
//! beam wrapped in a particle spiral, fading over about a second), impact
//! sparks and glow ring, sounds, the idle hum and the view model.
use qclient::RailEffect;
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

/// How long a trail lasts (seconds).
pub const TRAIL: f32 = 1.0;
/// Spark lifetime.
const SPARKS: f32 = 0.6;
/// Impact glow lifetime.
const MARK: f32 = 2.0;
/// Sound channel for the hum on the owner's entity (QC uses 0..=4).
const HUM_CHANNEL: i32 = 6;
const FIRE_CHANNEL: i32 = 7;

/// `addon_rail_color` choices: (core, spiral).
const COLOURS: [([u8; 3], [u8; 3]); 6] = [
    ([200, 230, 255], [40, 110, 255]),  // blue
    ([210, 255, 210], [40, 220, 60]),   // green
    ([255, 215, 200], [240, 40, 30]),   // red
    ([255, 250, 200], [240, 200, 30]),  // yellow
    ([245, 210, 255], [170, 50, 240]),  // purple
    ([255, 255, 255], [200, 200, 210]), // white
];

/// Trail colours for `owner`: the chosen colour, rotated per split-screen
/// player so each one's slugs are told apart.
pub fn colour(choice: usize, owner: usize) -> ([u8; 3], [u8; 3]) {
    COLOURS[(choice + owner) % COLOURS.len()]
}

struct Shot {
    event: RailEffect,
    time: f64,
    colour: ([u8; 3], [u8; 3]),
}

pub struct Presentation {
    shots: Vec<Shot>,
    last: [f64; 256],
    hum: [bool; 256],
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pic: Option<PicId>,
}

impl Default for Presentation {
    fn default() -> Self {
        Self { shots: Vec::new(), last: [-100.0; 256], hum: [false; 256], rate: 0, cues: Vec::new(), pic: None }
    }
}

impl Presentation {
    pub fn clear(&mut self) {
        self.shots.clear();
        self.last.fill(-100.0);
        self.hum.fill(false);
    }

    fn cue(&mut self, snd: &SoundSystem, k: usize) -> Arc<Sfx> {
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..4).map(|k| Arc::new(qsound::rail::render(k, snd.rate()))).collect();
        }
        self.cues[k].clone()
    }

    pub fn receive(&mut self, event: RailEffect, time: f64, choice: usize, snd: Option<&SoundSystem>) {
        let ent = event.owner as i32 + 1;
        if let Some(snd) = snd {
            let k = if event.kind == 1 { qsound::rail::FIRE } else { qsound::rail::CLICK };
            let sfx = self.cue(snd, k);
            snd.event(SoundEvent::Start { entity: ent, channel: FIRE_CHANNEL, sfx, origin: event.start, volume: 1.0, attenuation: 0.5 });
            if event.kind == 1 && event.normal != [0.0; 3] {
                let sfx = self.cue(snd, qsound::rail::IMPACT);
                snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin: event.end, volume: 0.7, attenuation: 1.0 });
            }
        }
        if event.kind != 1 {
            return;
        }
        self.last[event.owner as usize] = time;
        if self.shots.len() >= 32 {
            self.shots.remove(0);
        }
        let colour = colour(choice, event.owner as usize);
        self.shots.push(Shot { event, time, colour });
    }

    /// Start or stop `owner`'s idle hum as the railgun is held or put away.
    pub fn hum(&mut self, owner: usize, held: bool, snd: Option<&SoundSystem>) {
        let owner = owner.min(255);
        if self.hum[owner] == held {
            return;
        }
        let Some(snd) = snd else { return };
        self.hum[owner] = held;
        let entity = owner as i32 + 1;
        if held {
            let sfx = self.cue(snd, qsound::rail::HUM);
            snd.event(SoundEvent::Start { entity, channel: HUM_CHANNEL, sfx, origin: [0.0; 3], volume: 0.25, attenuation: 2.0 });
        } else {
            snd.event(SoundEvent::Stop { entity, channel: HUM_CHANNEL });
        }
    }

    pub fn weapon(&self, owner: usize, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
        let key = "@railgun";
        if !lv.by_name.contains_key(key) {
            let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
            let model = r
                .load_alias(&super::rail_model::model(&palette))
                .ok()
                .map(|id| crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0 });
            lv.by_name.insert(key.into(), model);
        }
        let model = lv.by_name.get(key).copied().flatten()?;
        let age = (time - self.last[owner.min(255)]).max(0.0) as f32;
        let recoil = (-age * 10.0).exp() * 2.0;
        let bob = (time as f32 * 2.0).sin() * 0.15;
        let (origin, mut angles) = crate::view::weapon::portal_pose(view, bob, recoil);
        // a little right and turned in so the side and its glow strip show
        let (_, right, _) = qcore::math::angle_vectors(view.angles.into());
        let origin = (qcore::math::Vec3::from(origin) + right * 6.0).to_array();
        angles[1] += 6.0;
        Some(RenderEntity { model: model.id, origin, angles, viewmodel: true, ..Default::default() })
    }

    pub fn frame(&mut self, time: f64, view: &View, r: &mut Renderer, geom: &mut Geometry) {
        self.shots.retain(|s| time >= s.time && time - s.time < MARK as f64);
        if self.shots.is_empty() {
            return;
        }
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let mut pixels = Vec::new();
                for y in 0..16 {
                    for x in 0..16 {
                        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt() / 7.5;
                        pixels.extend([255, 255, 255, ((1.0 - d).max(0.0).powi(2) * 255.0) as u8]);
                    }
                }
                let Ok(p) = r.register_pic_rgba_linear(16, 16, &pixels) else { return };
                self.pic = Some(p);
                p
            }
        };
        let (_, right, up) = angle_vectors(view.angles.into());
        let first = geom.1.len() as u32;
        for s in &self.shots {
            let age = (time - s.time) as f32;
            trail(geom, s, age, right, up);
        }
        geom.2.push(FteBatch { pic, blend: FteBlend::AddA, lines: false, first_index: first, num_indices: geom.1.len() as u32 - first, soft: false });
    }
}

/// Points of the spiral around the slug's line: one every `step` units,
/// `radius` out, turning a full circle every `pitch` units.
pub fn spiral(start: Vec3, end: Vec3, step: f32, radius: f32, pitch: f32) -> Vec<Vec3> {
    let d = end - start;
    let len = d.length();
    if len < 1.0 {
        return Vec::new();
    }
    let dir = d / len;
    let axis = if dir.z.abs() < 0.9 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let a = dir.cross(axis).normalize();
    let b = dir.cross(a);
    let n = ((len / step) as usize).min(1024);
    (0..n)
        .map(|i| {
            let x = i as f32 * step;
            let th = x / pitch * std::f32::consts::TAU;
            start + dir * x + (a * th.cos() + b * th.sin()) * radius
        })
        .collect()
}

fn trail(g: &mut Geometry, s: &Shot, age: f32, right: Vec3, up: Vec3) {
    let e = &s.event;
    let (core, spin) = s.colour;
    let start = Vec3::from(e.start);
    let end = Vec3::from(e.end);
    if age < TRAIL {
        let fade = 1.0 - age / TRAIL;
        // core beam: dense small bright dots, fading fastest
        let len = (end - start).length();
        let n = ((len / 4.0) as usize).clamp(2, 2048);
        let core_a = (1.0 - age / (TRAIL * 0.5)).max(0.0);
        if core_a > 0.0 {
            for i in 0..=n {
                let p = start + (end - start) * (i as f32 / n as f32);
                dot(g, p, right, up, 1.6, core, core_a);
            }
        }
        // spiral: grows outward a little as it fades, like Q3's rings
        let radius = 4.0 + age * 6.0;
        for p in spiral(start, end, 3.0, radius, 48.0) {
            dot(g, p, right, up, 2.2, spin, fade * 0.8);
        }
    }
    let n = Vec3::from(e.normal);
    if n.length() > 0.5 {
        // glowing scorch ring on the wall
        let mark = (1.0 - age / MARK).max(0.0);
        dot(g, end + n * 1.0, right, up, 6.0, spin, mark * 0.7);
        dot(g, end + n * 1.0, right, up, 2.5, core, mark);
        if age < SPARKS {
            let a = if n.z.abs() < 0.9 { n.cross(Vec3::new(0.0, 0.0, 1.0)).normalize() } else { n.cross(Vec3::new(0.0, 1.0, 0.0)).normalize() };
            let b = n.cross(a);
            for i in 0..16 {
                let th = i as f32 * 2.39996;
                let sp = 60.0 + (i * 37 % 11) as f32 * 14.0;
                let v = (n * 1.2 + a * th.cos() + b * th.sin()) * sp;
                let p = end + v * age - Vec3::new(0.0, 0.0, 400.0 * age * age);
                dot(g, p, right, up, 1.4, [255, 220, 150], 1.0 - age / SPARKS);
            }
        }
    }
    // pierced bodies: a short red burst
    if age < 0.5 {
        for h in &e.hits {
            for i in 0..8 {
                let th = i as f32 * 0.785;
                let p = Vec3::from(*h) + Vec3::new(th.cos(), th.sin(), 0.5) * age * 50.0;
                dot(g, p, right, up, 2.5, [160, 10, 10], 1.0 - age / 0.5);
            }
        }
    }
}

fn dot(g: &mut Geometry, p: Vec3, r: Vec3, u: Vec3, size: f32, rgb: [u8; 3], alpha: f32) {
    let base = g.0.len() as u32;
    for (x, y, uv) in [(-1.0, -1.0, [0.0, 0.0]), (1.0, -1.0, [1.0, 0.0]), (1.0, 1.0, [1.0, 1.0]), (-1.0, 1.0, [0.0, 1.0])] {
        g.0.push(FteVertex { pos: (p + r * x * size + u * y * size).to_array(), uv, color: [rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0) as u8] });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spiral_wraps_the_line_at_its_radius() {
        let pts = spiral(Vec3::ZERO, Vec3::new(96.0, 0.0, 0.0), 3.0, 4.0, 48.0);
        assert_eq!(pts.len(), 32);
        for p in &pts {
            assert!(((p.y * p.y + p.z * p.z).sqrt() - 4.0).abs() < 1e-3);
        }
        // one full turn every 48 units
        assert!((pts[16] - pts[0] - Vec3::new(48.0, 0.0, 0.0)).length() < 1e-3);
        assert!(spiral(Vec3::ZERO, Vec3::ZERO, 3.0, 4.0, 48.0).is_empty());
    }

    #[test]
    fn split_screen_players_get_different_colours() {
        assert_ne!(colour(0, 0), colour(0, 1));
        assert_eq!(colour(5, 1), colour(0, 0));
    }
}
