//! Assault rifle presentation: the view model, muzzle flash, tracers, brass,
//! smoke and sounds. The pose is `rifle_pose`, the sight input `rifle_ads`.
use crate::gfx::Renderer;
use qclient::guns::{self, GunEvent};
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, View};
use qsound::{Sfx, SoundEvent, SoundSystem, rifle as cue};
use std::sync::Arc;

use super::rifle_model::{MUZZLE, PORT, models};
use super::rifle_pose::{self, Motion, Pose, pose};

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

/// Muzzle flash lifetime, tracer speed and length, brass and smoke lifetimes.
const FLASH: f32 = 0.045;
const TRACER_SPEED: f32 = 9000.0;
const TRACER_LEN: f32 = 110.0;
const BRASS: f32 = 0.7;
const SMOKE: f32 = 0.45;
/// Sound channels on the owner (QC uses 0..=4): shots alternate on two.
const SHOT_CHANNELS: [i32; cue::SHOTS] = [6, 7, 8];
const WORK_CHANNEL: i32 = 5;
const MAX_SHOTS: usize = 64;
const MAX_PLAYERS: usize = 16;

/// The sight's state of one player, for [`Presentation::weapons`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    /// Sight raised, 0..1 (eased).
    pub aim: f32,
    /// The Viewmodels setting's side offset.
    pub side: f32,
}

struct Shot {
    owner: usize,
    start: Vec3,
    end: Vec3,
    time: f64,
    seed: u32,
    /// Where its brass left and the way right and up there.
    port: Option<(Vec3, Vec3, Vec3)>,
}

#[derive(Clone, Copy, Default)]
struct Owner {
    last: f64,
    shots: u32,
    reload: Option<(f64, f32, bool)>,
    /// Next reload cue: 0 magazine out, 1 in, 2 bolt, 3 none.
    stage: u8,
    aim: f32,
    raised: bool,
    /// When the view model was last placed, and its pose.
    placed: f64,
    pose: Option<Pose>,
    /// The muzzle on the player's body and when it was set.
    body: Option<(Vec3, f64)>,
    /// Rounds were left when the reload began.
    dry: bool,
}

#[derive(Default)]
pub struct Presentation {
    owners: [Owner; MAX_PLAYERS],
    shots: Vec<Shot>,
    /// Cues to play at the next frame: owner, cue.
    queue: Vec<(usize, usize)>,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pic: Option<PicId>,
}

impl Presentation {
    /// Drops every effect in flight (new map).
    pub fn clear(&mut self) {
        self.shots.clear();
        self.queue.clear();
        self.owners = Default::default();
    }

    fn play(&mut self, snd: Option<&SoundSystem>, owner: usize, k: usize, origin: [f32; 3]) {
        let Some(snd) = snd else { return };
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..cue::CUES).map(|k| Arc::new(cue::render(k, snd.rate()))).collect();
        }
        // shots take turns on channels and takes: none cuts the last one
        // short, and full auto doesn't sound looped
        let take = self.owners[owner].shots as usize % cue::SHOTS;
        let (k, channel, volume) = if k == cue::SHOT { (cue::SHOT + take, SHOT_CHANNELS[take], 0.75) } else { (k, WORK_CHANNEL, 0.8) };
        snd.event(SoundEvent::Start { entity: owner as i32 + 1, channel, sfx: self.cues[k].clone(), origin, volume, attenuation: 0.8 });
    }

    /// One `//qsp_gun` event of this toy.
    pub fn receive(&mut self, e: &GunEvent, time: f64, snd: Option<&SoundSystem>) {
        let i = (e.owner as usize).min(MAX_PLAYERS - 1);
        match e.kind {
            guns::FIRE => {
                let o = &mut self.owners[i];
                o.shots = o.shots.wrapping_add(1);
                o.last = time;
                let pose = o.pose.filter(|_| time - o.placed < 0.2);
                let body = o.body.filter(|b| (time - b.1).abs() < 0.1).map(|b| b.0);
                let start = pose.map(|p| p.at(MUZZLE)).or(body).unwrap_or(e.start.into());
                let port = pose.map(|p| (p.at(PORT), p.axes[1] * -1.0, p.axes[2]));
                if self.shots.len() >= MAX_SHOTS {
                    self.shots.remove(0);
                }
                let seed = o.shots.wrapping_mul(0x9e37_79b9) ^ i as u32;
                self.shots.push(Shot { owner: i, start, end: Vec3::from(e.end), time, seed, port });
                self.play(snd, i, cue::SHOT, e.start);
            }
            guns::DRY => self.play(snd, i, cue::DRY, e.start),
            guns::RELOAD => {
                let o = &mut self.owners[i];
                o.reload = Some((time, e.secs.max(0.1), o.dry));
                o.stage = 0;
            }
            guns::CANCEL | guns::DONE => self.owners[i].reload = None,
            _ => {}
        }
    }

    /// `owner`'s view model entities while the rifle is held.
    #[allow(clippy::too_many_arguments)]
    pub fn weapons(&mut self, owner: usize, hold: Hold, cl: &qclient::ClientState, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Vec<RenderEntity> {
        let i = owner.min(MAX_PLAYERS - 1);
        let Some([body, sight, mag]) = models(lv, fs, r) else { return Vec::new() };
        let o = &mut self.owners[i];
        let id = qserver::addons::Toy::Rifle.id();
        if let Some(m) = cl.mags.iter().find(|m| m.toy == id && m.owner as usize == owner) {
            o.dry = m.parts.first().is_some_and(|p| p.rounds == 0);
        }
        // cloth cue as the sight goes up or down
        let raised = if o.raised { hold.aim > 0.6 } else { hold.aim > 0.05 };
        if raised != o.raised && time - o.placed < 0.2 {
            self.queue.push((i, if raised { cue::AIM_IN } else { cue::AIM_OUT }));
        }
        o.raised = raised;
        o.aim = hold.aim;
        // a reload lost with the toy put away (the cancel comes later)
        let reload = o.reload.map(|(t, secs, dry)| (((time - t) as f32 / secs).clamp(0.0, 1.0), dry)).filter(|&(u, _)| u < 1.0);
        let v = cl.velocity;
        let p = pose(&view, &Motion {
            aim: hold.aim,
            side: hold.side,
            shot: (time - o.last).max(0.0) as f32,
            reload,
            time: (time % 3600.0) as f32,
            speed: (v[0] * v[0] + v[1] * v[1]).sqrt(),
        });
        (o.pose, o.placed) = (Some(p), time);
        let ent = |model, at: Vec3| RenderEntity { model, origin: at.to_array(), angles: p.angles(), viewmodel: true, ..Default::default() };
        let mut out = vec![ent(body, p.origin), ent(sight, p.origin)];
        if let Some(off) = reload.map_or(Some(Vec3::ZERO), |(u, _)| rifle_pose::magazine(u)) {
            out.push(ent(mag, p.at(off)));
        }
        out
    }

    /// The view the sight's dot is drawn in: 0..1 how visible it is along
    /// `dir` from `view`'s eye.
    pub fn dot(&self, owner: usize, time: f64, eye: Vec3, dir: Vec3) -> f32 {
        let o = &self.owners[owner.min(MAX_PLAYERS - 1)];
        match o.pose {
            Some(p) if o.aim > 0.4 && (time - o.placed).abs() < 0.1 => p.through(eye, dir),
            _ => 0.0,
        }
    }

    /// Seconds since `owner`'s last shot and its number.
    pub fn last_shot(&self, owner: usize, time: f64) -> (f32, u32) {
        let o = &self.owners[owner.min(MAX_PLAYERS - 1)];
        ((time - o.last) as f32, o.shots)
    }

    /// Where `owner`'s rifle ends on the player's body this frame.
    pub fn body_muzzle(&mut self, owner: usize, at: Vec3, time: f64) {
        self.owners[owner.min(MAX_PLAYERS - 1)].body = Some((at, time));
    }

    /// `owner`'s view model was placed this frame.
    pub fn placed(&self, owner: usize, time: f64) -> bool {
        let o = &self.owners[owner.min(MAX_PLAYERS - 1)];
        o.pose.is_some() && (time - o.placed).abs() < 0.1
    }

    /// The soft round picture the flashes and the dot use.
    pub fn pic(&self) -> Option<PicId> {
        self.pic
    }

    /// Effect geometry (tracers, flashes, smoke, casings) and reload cues.
    pub fn frame(&mut self, time: f64, view: &View, r: &mut Renderer, snd: Option<&SoundSystem>, geom: &mut Geometry) {
        for i in 0..MAX_PLAYERS {
            let Some((t, secs, dry)) = self.owners[i].reload else { continue };
            let u = (time - t) as f32 / secs;
            let o = &mut self.owners[i];
            let next = [rifle_pose::MAG_OUT, rifle_pose::MAG_IN, rifle_pose::BOLT];
            if (o.stage as usize) < next.len() && u >= next[o.stage as usize] {
                let k = [cue::MAG_OUT, cue::MAG_IN, cue::BOLT][o.stage as usize];
                o.stage += 1;
                if k != cue::BOLT || dry {
                    self.queue.push((i, k));
                }
            }
            if u > 1.5 {
                self.owners[i].reload = None;
            }
        }
        for (i, k) in std::mem::take(&mut self.queue) {
            let at = self.owners[i].pose.map_or([0.0; 3], |p| p.origin.to_array());
            self.play(snd, i, k, at);
        }
        self.shots.retain(|s| time >= s.time && time - s.time < f64::from(BRASS));
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let mut pixels = Vec::new();
                for y in 0..32 {
                    for x in 0..32 {
                        let d = ((x as f32 - 15.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt() / 15.5;
                        pixels.extend([255, 255, 255, ((1.0 - d).max(0.0).powf(1.6) * 255.0) as u8]);
                    }
                }
                let Ok(p) = r.register_pic_rgba_linear(32, 32, &pixels) else { return };
                self.pic = Some(p);
                p
            }
        };
        if self.shots.is_empty() {
            return;
        }
        let eye = Vec3::from(view.origin);
        let (_, right, up) = angle_vectors(view.angles.into());
        // brass and smoke blend; flashes and tracers add
        let first = geom.1.len() as u32;
        for s in &self.shots {
            let age = (time - s.time) as f32;
            brass(geom, s, age, eye);
            if age < SMOKE {
                let k = age / SMOKE;
                let at = s.start + Vec3::new(0.0, 0.0, 10.0 * age) + (s.end - s.start).normalize() * (6.0 * k);
                quad(geom, at, right * (1.5 + 4.5 * k), up * (1.5 + 4.5 * k), [150, 145, 140], 0.10 * (1.0 - k));
            }
        }
        batch(geom, pic, FteBlend::Blend, first);
        let first = geom.1.len() as u32;
        for s in &self.shots {
            let age = (time - s.time) as f32;
            flash(geom, s, age, self.owners[s.owner].aim, right, up);
            tracer(geom, s, age, eye);
        }
        batch(geom, pic, FteBlend::AddA, first);
    }
}

fn batch(g: &mut Geometry, pic: PicId, blend: FteBlend, first: u32) {
    let n = g.1.len() as u32 - first;
    if n > 0 {
        g.2.push(FteBatch { pic, blend, lines: false, first_index: first, num_indices: n, soft: false, gel: false });
    }
}

/// A star of crossed streaks and a hot core, different each shot.
fn flash(g: &mut Geometry, s: &Shot, age: f32, aim: f32, right: Vec3, up: Vec3) {
    if age > FLASH {
        return;
    }
    let fade = 1.0 - age / FLASH * 0.7;
    let size = (1.0 - 0.5 * aim) * (0.85 + (s.seed >> 8 & 63) as f32 / 160.0);
    let dir = (s.end - s.start).normalize();
    let at = s.start + dir * 1.0;
    let spin = (s.seed >> 16 & 255) as f32 / 255.0;
    for k in 0..3 {
        let a = (spin + k as f32 / 3.0) * std::f32::consts::PI;
        let (sn, cs) = a.sin_cos();
        let (x, y) = (right * cs + up * sn, up * cs - right * sn);
        quad(g, at, x * (7.5 * size), y * (1.7 * size), [255, 190, 90], 0.85 * fade);
    }
    quad(g, at, right * (3.4 * size), up * (3.4 * size), [255, 240, 200], fade);
    quad(g, at + dir * 2.0, right * (9.0 * size), up * (9.0 * size), [255, 150, 60], 0.22 * fade);
}

/// A bright streak flying from the muzzle to the impact.
fn tracer(g: &mut Geometry, s: &Shot, age: f32, eye: Vec3) {
    let d = s.end - s.start;
    let len = d.length();
    let head = age * TRACER_SPEED + 60.0;
    if len < 80.0 || head - TRACER_LEN > len {
        return;
    }
    let dir = d / len;
    let (a, b) = (s.start + dir * (head - TRACER_LEN).clamp(20.0, len), s.start + dir * head.min(len));
    let mid = (a + b) / 2.0;
    let side = dir.cross(mid - eye).normalize();
    let width = 0.35 + (mid - eye).length() * 0.0012;
    quad(g, mid, (b - a) * 0.5, side * width, [255, 215, 140], 0.75);
}

/// The spent case, thrown to the right and tumbling.
fn brass(g: &mut Geometry, s: &Shot, age: f32, eye: Vec3) {
    let Some((from, right, up)) = s.port else { return };
    let j = |k: u32| (s.seed >> k & 31) as f32 / 31.0 - 0.5;
    let v = right * (75.0 + 30.0 * j(0)) + up * (70.0 + 30.0 * j(5)) + right.cross(up) * (14.0 * j(10));
    let at = from + v * age - Vec3::new(0.0, 0.0, 330.0 * age * age);
    let a = age * (26.0 + 12.0 * j(15)) + j(3) * 6.0;
    let axis = right.cross(up) * a.cos() + up * a.sin();
    let side = axis.cross(at - eye);
    if side.length() < 1e-3 {
        return;
    }
    let side = side.normalize();
    // layers of the soft picture: a solid little case with a bright line
    for _ in 0..3 {
        quad(g, at, axis * 0.62, side * 0.24, [84, 58, 22], 1.0);
    }
    quad(g, at, axis * 0.4, side * 0.09, [176, 134, 66], 0.8);
}

/// A quad spanned by two half axes.
fn quad(g: &mut Geometry, p: Vec3, x: Vec3, y: Vec3, rgb: [u8; 3], alpha: f32) {
    let base = g.0.len() as u32;
    for (a, b, uv) in [(-1.0, -1.0, [0.0, 0.0]), (1.0, -1.0, [1.0, 0.0]), (1.0, 1.0, [1.0, 1.0]), (-1.0, 1.0, [0.0, 1.0])] {
        g.0.push(FteVertex { pos: (p + x * a + y * b).to_array(), uv, color: [rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0) as u8] });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
