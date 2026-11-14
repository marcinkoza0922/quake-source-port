//! Powerwasher: a water cannon. Fire shoots a straight, lightning-gun
//! style jet with a limited reach ([`STREAM`]), alt-fire a wide fan of five
//! shorter jets. Each jet is traced in short segments from the muzzle. It
//! pushes back the first monster or player it hits (no damage), turns
//! projectiles flying near it along the jet, and washes gel
//! ([`super::gel::Gel::wash`]) off where it hits a wall; clients wash blood
//! off around the same points.
//!
//! While spraying the server sends `//qsp_water owner mode x y z...` every
//! frame (mode 1 jet, 2 fan; one end point per jet) and
//! `//qsp_water owner 0` when it stops. Nothing is saved.

use qcore::math::{Vec3, angle_vectors, vec_to_angles};
use qworld::{E, Edicts, Ent, F, MoveType, V, World};

use super::gel::Gel;
use crate::client::Client;

const MOVETYPE_STEP: i32 = 4;
const MOVETYPE_WALK: i32 = 3;
const MOVETYPE_TOSS: i32 = 6;
const MOVETYPE_FLYMISSILE: i32 = 9;
const MOVETYPE_BOUNCE: i32 = 10;
const FL_ONGROUND: i32 = 512;
const SOLID_BSP: f32 = 4.0;

/// One jet: reach, push acceleration on a target and wash radius at the hit.
#[derive(Clone, Copy, Debug)]
pub struct Jet {
    pub range: f32,
    pub push: f32,
    pub wash: f32,
}
pub const STREAM: Jet = Jet { range: 560.0, push: 3200.0, wash: 44.0 };
pub const FAN: Jet = Jet { range: 320.0, push: 1100.0, wash: 40.0 };
/// Fan jets' yaw offsets (degrees).
pub const FAN_YAW: [f32; 5] = [-14.0, -7.0, 0.0, 7.0, 14.0];
/// Pushed targets go no faster than this along the jet.
const PUSH_MAX: f32 = 650.0;
/// Projectiles within this distance of a jet are turned.
const DEFLECT_R: f32 = 30.0;
const STEPS: usize = 12;

#[derive(Clone, Debug, Default)]
pub struct Washers {
    /// Mode each player sprayed last frame (0 none).
    mode: Vec<u8>,
}

/// Points along a jet, from `start` in direction `dir` to its reach.
pub fn arc(start: Vec3, dir: Vec3, jet: Jet) -> [Vec3; STEPS + 1] {
    std::array::from_fn(|k| start + dir * jet.range * k as f32 / STEPS as f32)
}

/// Distance from `p` to segment `a`..`b`.
fn seg_dist(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

impl Washers {
    pub(crate) fn frame<D: Edicts + ?Sized>(&mut self, w: &mut World, ents: &mut D, clients: &mut [Client], gel: &mut Gel, scale: f32, dt: f32) {
        if self.mode.len() < clients.len() {
            self.mode.resize(clients.len(), 0);
        }
        let mut lines = Vec::new();
        for (i, cl) in clients.iter().enumerate() {
            let e = i as Ent + 1;
            let b = cl.cmd.buttons;
            let held = cl.spawned && super::Toy::from_buttons(b) == Some(super::Toy::Washer) && ents.f(e, F::Health) > 0.0;
            let mode = match (held, b & super::BUTTON_TOY_FIRE != 0, b & super::BUTTON_TOY_ALT != 0) {
                (true, true, _) => 1,
                (true, false, true) => 2,
                _ => 0,
            };
            if mode == 0 {
                if self.mode[i] != 0 {
                    lines.push(format!("//qsp_water {i} 0\n"));
                }
                self.mode[i] = 0;
                continue;
            }
            self.mode[i] = mode;
            let angles = ents.v(e, V::VAngle);
            let (fwd, _, up) = angle_vectors(angles);
            let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
            let start = eye + fwd * 14.0 - up * 7.0;
            let (jet, yaws): (Jet, &[f32]) = if mode == 1 { (STREAM, &[0.0]) } else { (FAN, &FAN_YAW) };
            let mut line = format!("//qsp_water {i} {mode}");
            for &yaw in yaws {
                let dir = angle_vectors(angles + Vec3::new(0.0, yaw, 0.0)).0;
                let end = spray(w, ents, gel, e, start, dir, jet, scale, dt);
                line += &format!(" {:.1} {:.1} {:.1}", end.x, end.y, end.z);
            }
            lines.push(line + "\n");
        }
        for cl in clients.iter_mut().filter(|c| c.spawned) {
            for l in &lines {
                cl.message.byte(crate::msg::SVC_STUFFTEXT);
                cl.message.string(l.as_bytes());
            }
        }
    }

    pub(crate) fn reset(&mut self) {
        self.mode.clear();
    }
}

/// Traces one jet; pushes what it hits, turns nearby projectiles and
/// washes gel at a wall. Returns where the water lands.
#[allow(clippy::too_many_arguments)]
fn spray<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, gel: &mut Gel, me: Ent, start: Vec3, dir: Vec3, jet: Jet, scale: f32, dt: f32) -> Vec3 {
    let pts = arc(start, dir, jet);
    let mut end = pts[STEPS];
    let mut target = None;
    let mut wall = false;
    let mut reached = STEPS;
    for k in 0..STEPS {
        let tr = w.trace(ents, pts[k], Vec3::ZERO, Vec3::ZERO, pts[k + 1], MoveType::Normal, Some(me));
        if tr.allsolid && k == 0 {
            return pts[0];
        }
        if tr.fraction < 1.0 {
            end = tr.endpos;
            reached = k + 1;
            match tr.ent {
                Some(0) | None => wall = true,
                Some(h) if ents.f(h, F::Solid) == SOLID_BSP => wall = true,
                Some(h) => target = Some((h, (pts[k + 1] - pts[k]).normalize())),
            }
            break;
        }
    }
    if let Some((h, d)) = target {
        push(w, ents, h, d, jet.push * scale * dt);
    }
    if wall {
        gel.wash(end, jet.wash);
    }
    deflect(ents, me, &pts[..=reached.min(STEPS)], end, dt);
    end
}

fn push<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, h: Ent, d: Vec3, amount: f32) {
    let mt = ents.f(h, F::MoveType) as i32;
    if !matches!(mt, MOVETYPE_WALK | MOVETYPE_STEP | MOVETYPE_TOSS | MOVETYPE_BOUNCE) {
        return;
    }
    let flat = Vec3::new(d.x, d.y, 0.0).normalize();
    let flat = if flat.length_sq() > 0.5 { flat } else { d };
    let mut v = ents.v(h, V::Velocity);
    let along = v.dot(flat);
    if along < PUSH_MAX {
        v += flat * amount.min(PUSH_MAX - along);
    }
    if ents.flags(h) & FL_ONGROUND != 0 {
        // lift off the floor so step physics lets the push slide them
        v.z = v.z.max(110.0);
        ents.set_flags(h, ents.flags(h) & !FL_ONGROUND);
        ents.set_e(h, E::GroundEntity, 0);
    }
    ents.set_v(h, V::Velocity, v);
    w.link_edict(ents, h, false);
}

/// Projectiles near the jet are turned towards its direction.
fn deflect<D: Edicts + ?Sized>(ents: &mut D, me: Ent, pts: &[Vec3], end: Vec3, dt: f32) {
    let mut path: Vec<Vec3> = pts.to_vec();
    if let Some(last) = path.last_mut() {
        *last = end;
    }
    for e in 1..ents.num_edicts() {
        if ents.is_free(e) || e == me {
            continue;
        }
        let mt = ents.f(e, F::MoveType) as i32;
        if mt != MOVETYPE_FLYMISSILE && mt != MOVETYPE_BOUNCE {
            continue;
        }
        let p = ents.v(e, V::Origin);
        let Some(k) = (0..path.len().saturating_sub(1)).find(|&k| seg_dist(p, path[k], path[k + 1]) < DEFLECT_R) else { continue };
        let along = (path[k + 1] - path[k]).normalize();
        let v = ents.v(e, V::Velocity);
        let speed = v.length().max(300.0);
        let turned = (v.normalize() + along * (12.0 * dt).min(1.0) * 3.0).normalize() * speed;
        ents.set_v(e, V::Velocity, turned);
        if mt == MOVETYPE_FLYMISSILE {
            ents.set_v(e, V::Angles, vec_to_angles(turned));
        }
        // the shooter now owns it, so it can fly back through monsters
        ents.set_e(e, E::Owner, me);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jets_are_straight_with_limited_reach() {
        let p = arc(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), STREAM);
        let end = p[STEPS];
        assert!((end.x - STREAM.range).abs() < 1e-3 && end.z == 0.0, "{end:?}");
        assert!(seg_dist(Vec3::new(5.0, 3.0, 0.0), Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)) - 3.0 < 1e-4);
    }
}
