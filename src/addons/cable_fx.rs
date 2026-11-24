//! Client presentation of the male-to-male cable: the view model (two
//! gloved hands, one plug each, the plugs jiggling on damped springs), the
//! plug seated in its socket, the rope ([`super::cable_rope_fx`]), the
//! stock lightning bolt from the free plug's pins, the socket highlight
//! ([`super::cable_glow`]) and the synthesised plug/yank/miss cues.
use qcore::math::{Vec3, angle_vectors};
use crate::gfx::Renderer;
use qrender::{PicId, RenderEntity, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

use super::cable_glow::SocketGlow;
use super::cable_rope_fx::{Geometry, RopeSim, draw, rubber_pic};

/// Plug model half length (pins along -X) and the cable hanging between
/// the hands while nothing is plugged in.
const PLUG_HALF: f32 = 6.0;
const HAND_LOOP: f32 = 70.0;
/// Where the plug sits in a fist model: towards the knuckles, the palm and
/// out of the thumb side.
const GRIP_UP: f32 = 2.5;
const GRIP_IN: f32 = 0.5;
const GRIP_OUT: f32 = 3.0;
/// How much further out the free plug sits when it points ahead.
const GRIP_TAIL: f32 = 3.5;
/// Seconds to swap between the two-handed and the plugged pose.
const POSE_TIME: f32 = 0.25;

/// A damped spring offset in two dimensions (view right, view up).
#[derive(Clone, Copy, Debug, Default)]
struct Spring {
    x: [f32; 2],
    v: [f32; 2],
}

impl Spring {
    fn step(&mut self, dt: f32, kick: [f32; 2], k: f32, c: f32) {
        for ((x, v), kick) in self.x.iter_mut().zip(&mut self.v).zip(kick) {
            *v += kick + (-k * *x - c * *v) * dt;
            *x = (*x + *v * dt).clamp(-6.0, 6.0);
        }
    }
}

/// One local player's view model state.
#[derive(Clone, Debug, Default)]
struct Held {
    time: f64,
    angles: Vec3,
    origin: Vec3,
    vel: Vec3,
    /// 0 two-handed, 1 plugged (right hand only).
    pose: f32,
    plugs: [Spring; 2],
    hands: Spring,
    /// World points of the right plug's pins and back, left plug's back.
    tip: Vec3,
    back: Vec3,
    left_back: Vec3,
    /// The cable between the hands.
    hand_loop: RopeSim,
}

#[derive(Default)]
pub struct CableFx {
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    held: Vec<Held>,
    /// Plugged ropes by owner.
    ropes: Vec<(u8, RopeSim)>,
    plugged: Vec<bool>,
    pic: Option<PicId>,
    glow: SocketGlow,
}

/// Entity angles (pitch, yaw, roll as R_RotateForEntity applies them)
/// that turn the model's +X to `x` and its +Z as near `z` as possible.
pub(super) fn angles_basis(x: Vec3, z: Vec3) -> [f32; 3] {
    let [pitch, yaw, _] = angles_of(x);
    let (s, c) = yaw.to_radians().sin_cos();
    let left = Vec3::new(-s, c, 0.0);
    let up = x.normalize().cross(left);
    [pitch, yaw, (-z.dot(left)).atan2(z.dot(up)).to_degrees()]
}

/// The pose of a gloved fist at `at` (its model: knuckles +Z, palm +X,
/// right thumb +Y and left thumb -Y) whose forearm runs along `arm` and
/// whose grip runs along `grip` (exact), and the centre of the plug in it.
fn fist(at: Vec3, arm: Vec3, grip: Vec3, left: bool) -> ([f32; 3], Vec3) {
    let y = if left { grip * -1.0 } else { grip };
    let palm = y.cross(arm).normalize();
    let knuckles = palm.cross(y).normalize();
    (angles_basis(palm, knuckles), at + knuckles * GRIP_UP + palm * GRIP_IN + grip * GRIP_OUT)
}

/// Entity angles (pitch up positive) of direction `d`, unrounded.
fn angles_of(d: Vec3) -> [f32; 3] {
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = d.z.atan2((d.x * d.x + d.y * d.y).sqrt()).to_degrees();
    [pitch, yaw, 0.0]
}

impl CableFx {
    pub fn clear(&mut self) {
        self.ropes.clear();
        self.plugged.clear();
        self.held.iter_mut().for_each(|h| *h = Held::default());
    }

    /// Plug, yank and miss cues.
    pub fn receive(&mut self, c: &qclient::CableView, snd: Option<&SoundSystem>) {
        let o = c.owner as usize;
        if self.plugged.len() <= o {
            self.plugged.resize(o + 1, false);
        }
        let cue = match c.state {
            1 if !self.plugged[o] => Some(0),
            0 => Some(1),
            2 => Some(2),
            _ => None,
        };
        self.plugged[o] = c.state == 1;
        if c.state != 1 {
            self.ropes.retain(|(w, _)| *w != c.owner);
        }
        let (Some(cue), Some(snd)) = (cue, snd) else { return };
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..3).map(|k| Arc::new(qsound::cable::render(k, snd.rate()))).collect();
        }
        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx: self.cues[cue].clone(), origin: c.socket, volume: 1.0, attenuation: 1.0 });
    }

    fn models(lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<[qrender::ModelId; 3]> {
        use super::toy_model::get;
        Some([
            get("@cable_hand_l", include_bytes!("../../assets/cable/hand_left/model.mdl.zlib"), include_bytes!("../../assets/cable/hand_left/texture.rgb.zlib"), false, lv, fs, r)?,
            get("@cable_hand_r", include_bytes!("../../assets/cable/hand_right/model.mdl.zlib"), include_bytes!("../../assets/cable/hand_right/texture.rgb.zlib"), false, lv, fs, r)?,
            get("@cable_plug", include_bytes!("../../assets/cable/plug/model.mdl.zlib"), include_bytes!("../../assets/cable/plug/texture.rgb.zlib"), false, lv, fs, r)?,
        ])
    }

    /// The view model of local player `owner` (hands and plugs). `plugged`:
    /// his cable is in a socket; `firing`: the bolt is on.
    #[allow(clippy::too_many_arguments)]
    pub fn weapon(&mut self, owner: usize, cl: &qclient::ClientState, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Vec<RenderEntity> {
        let Some([hand_l, hand_r, plug]) = Self::models(lv, fs, r) else { return Vec::new() };
        let cable = cl.cables.iter().find(|c| c.owner as usize == owner);
        let (plugged, firing) = (cable.is_some(), cable.is_some_and(|c| c.fire));
        if self.held.len() <= owner {
            self.held.resize(owner + 1, Held::default());
        }
        let h = &mut self.held[owner];
        let eye = Vec3::from(view.origin);
        let angles = Vec3::from(view.angles);
        let dt = ((time - h.time) as f32).clamp(0.0, 0.05);
        if dt > 0.0 && h.time > 0.0 {
            // swing: turning and accelerating kick the springs
            let turn = |a: f32, b: f32| ((a - b + 540.0) % 360.0) - 180.0;
            let (dyaw, dpitch) = (turn(angles.y, h.angles.y), turn(angles.x, h.angles.x));
            let vel = (eye - h.origin) * (1.0 / dt);
            let acc = (vel - h.vel) * (1.0 / dt);
            let (_, rt, up) = angle_vectors(angles);
            let push = [acc.dot(rt) * -0.0004, acc.dot(up) * -0.0004];
            let kick = [dyaw * 0.9 + push[0], dpitch * 0.9 + push[1]];
            let buzz = if firing { [(time * 97.0).sin() as f32 * 0.6, (time * 131.0).cos() as f32 * 0.6] } else { [0.0; 2] };
            for p in &mut h.plugs {
                p.step(dt, [kick[0] + buzz[0], kick[1] + buzz[1]], 160.0, 7.0);
            }
            h.hands.step(dt, [kick[0] * 0.15, kick[1] * 0.15], 90.0, 14.0);
            h.vel = vel;
            let target = if plugged { 1.0 } else { 0.0 };
            h.pose += (target - h.pose).clamp(-dt / POSE_TIME, dt / POSE_TIME);
        }
        h.time = time;
        h.angles = angles;
        h.origin = eye;
        let (f, rt, up) = angle_vectors(angles);
        let t = h.pose * h.pose * (3.0 - 2.0 * h.pose);
        let sway = rt * h.hands.x[0] + up * h.hands.x[1];
        let bob = up * ((time as f32 * 2.2).sin() * 0.25);
        let jiggle = |s: &Spring| (rt * s.x[0] + up * s.x[1]) * (0.5 / (PLUG_HALF * 2.5));
        let mut out = Vec::new();
        // Each fist holds its plug in a hammer grip: `arm` runs from the
        // wrist to the knuckles, `grip` from the little finger to the
        // thumb (the plug's pins). Left hand: holds the other plug pins up
        // until it is plugged in, then drops away.
        let arm_l = f + up * 0.45 + rt * 0.55;
        let grip_l = (up + f * 0.55 + rt * 0.25 + jiggle(&h.plugs[0])).normalize();
        let gl = eye + f * 18.0 - rt * (10.0 + 6.0 * t) - up * (10.5 + 24.0 * t) + sway + bob;
        let (pose_l, plug_l) = fist(gl, arm_l, grip_l, true);
        h.left_back = plug_l - grip_l * PLUG_HALF;
        if t < 0.95 {
            out.push(RenderEntity { model: hand_l, origin: gl.to_array(), angles: pose_l, viewmodel: true, ..Default::default() });
            out.push(RenderEntity { model: plug, origin: plug_l.to_array(), angles: angles_of(grip_l * -1.0), viewmodel: true, ..Default::default() });
        }
        // right hand: from holding its plug pins up to pointing it ahead,
        // the wrist cocked so the pins show past the fist
        let arm_r = (f + up * 0.45 - rt * 0.55).lerp(f * 0.8 + up * 0.9 - rt * 0.1, t);
        let grip_r = (up + f * 0.55 - rt * 0.25).normalize().lerp((f + up * 0.25 - rt * 0.15).normalize(), t);
        let grip_r = (grip_r + jiggle(&h.plugs[1])).normalize();
        let gr = eye + f * (18.0 + 4.0 * t) + rt * (10.0 + 1.0 * t) - up * (10.5 + 2.5 * t) + sway + bob;
        let (pose_r, plug_r) = fist(gr, arm_r, grip_r, false);
        // held by its tail once it points ahead
        let plug_r = plug_r + grip_r * (GRIP_TAIL * t);
        out.push(RenderEntity { model: hand_r, origin: gr.to_array(), angles: pose_r, viewmodel: true, ..Default::default() });
        out.push(RenderEntity { model: plug, origin: plug_r.to_array(), angles: angles_of(grip_r * -1.0), viewmodel: true, ..Default::default() });
        h.tip = plug_r + grip_r * PLUG_HALF;
        h.back = plug_r - grip_r * PLUG_HALF;
        out
    }

    /// World entities: plugs in sockets and lightning bolts. Also steps the
    /// rope simulations (once per client time).
    #[allow(clippy::too_many_arguments)]
    pub fn entities(&mut self, cl: &qclient::ClientState, me: usize, holding: bool, time: f64, gravity: f32, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer, ents: &mut Vec<RenderEntity>) {
        if cl.cables.is_empty() && !holding {
            return;
        }
        let plug = Self::models(lv, fs, r).map(|m| m[2]);
        let bolt = lv.named(fs, r, "progs/bolt2.mdl").map(|m| m.id);
        let cm = lv.collision_model();
        let hit = |a: Vec3, b: Vec3| -> Option<(Vec3, Vec3)> {
            let tr = cm?.hull(0, 0).trace(a, b);
            (tr.fraction < 1.0 && !tr.startsolid).then_some((tr.endpos, tr.plane_normal))
        };
        for c in &cl.cables {
            let socket = Vec3::from(c.socket);
            let n = Vec3::from(c.normal);
            if let Some(plug) = plug {
                let centre = socket + n * (PLUG_HALF - 1.5);
                ents.push(RenderEntity { model: plug, origin: centre.to_array(), angles: angles_of(n), ..Default::default() });
            }
            let body = cl.entities.get(c.owner as usize + 1).map(|e| Vec3::from(e.origin));
            let Some(body) = body else { continue };
            let mut anchors = vec![socket + n * (PLUG_HALF * 2.0 - 1.0)];
            anchors.extend(c.pivots.iter().map(|&p| Vec3::from(p)));
            anchors.push(body + Vec3::new(0.0, 0.0, 6.0));
            let sim = match self.ropes.iter().position(|(o, _)| *o == c.owner) {
                Some(i) => &mut self.ropes[i].1,
                None => {
                    self.ropes.push((c.owner, RopeSim::default()));
                    &mut self.ropes.last_mut().unwrap().1
                }
            };
            if time > sim.time || sim.points().is_empty() {
                sim.step(&anchors, c.length, time, gravity, &hit);
            }
            // the bolt: from the free plug's pins (the viewer's own view
            // model) or roughly the other player's hands
            if c.fire && let Some(bolt) = bolt {
                let start = if c.owner as usize == me && let Some(h) = self.held.get(me) {
                    h.tip
                } else {
                    let yaw = cl.entities.get(c.owner as usize + 1).map_or(0.0, |e| e.angles[1]);
                    let (f, _, _) = angle_vectors(Vec3::new(0.0, yaw, 0.0));
                    body + Vec3::new(0.0, 0.0, 14.0) + f * 18.0
                };
                bolt_segments(start, Vec3::from(c.end), bolt, time, ents);
            }
        }
        // the cable hanging between the hands while nothing is plugged in
        if holding && let Some(h) = self.held.get_mut(me) && h.pose < 0.95 {
            let anchors = [h.left_back, h.back];
            if time > h.hand_loop.time || h.hand_loop.points().is_empty() {
                h.hand_loop.carry(&anchors);
                h.hand_loop.step(&anchors, HAND_LOOP, time, gravity, &hit);
            }
        }
    }

    /// Rope ribbons and (while holding an unplugged cable) the socket
    /// highlight.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, cl: &qclient::ClientState, me: usize, holding: bool, metal: bool, lv: &crate::assets::Level, view: &View, r: &mut Renderer, geom: &mut Geometry) {
        if cl.cables.is_empty() && !holding {
            return;
        }
        let Some(pic) = self.pic.or_else(|| rubber_pic(r)) else { return };
        self.pic = Some(pic);
        let eye = Vec3::from(view.origin);
        for (owner, sim) in &self.ropes {
            let mine = *owner as usize == me && holding;
            let last = mine.then(|| self.held.get(me).map(|h| h.back)).flatten();
            draw(sim.points(), last, eye, pic, geom);
        }
        let plugged = cl.cables.iter().any(|c| c.owner as usize == me);
        if holding && let Some(h) = self.held.get(me) {
            if h.pose < 0.95 {
                draw(h.hand_loop.points(), None, eye, pic, geom);
            }
            if !plugged {
                let (f, _, _) = angle_vectors(Vec3::from(view.angles));
                self.glow.frame(cl, lv, eye, f, metal, cl.time, r, geom);
            }
        }
    }
}

/// CL_UpdateTEnts beam segments of `model` from `start` to `end`.
fn bolt_segments(start: Vec3, end: Vec3, model: qrender::ModelId, time: f64, ents: &mut Vec<RenderEntity>) {
    let d = end - start;
    let len = d.length();
    if len < 1.0 {
        return;
    }
    let dir = d * (1.0 / len);
    let [pitch, yaw, _] = angles_of(dir);
    let mut seed = (time * 1000.0) as u32 | 1;
    let mut s = 0.0;
    while s < len && ents.len() < 4096 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let roll = (seed % 360) as f32;
        ents.push(RenderEntity { model, origin: (start + dir * s).to_array(), angles: [pitch, yaw, roll], ..Default::default() });
        s += 30.0;
    }
}
