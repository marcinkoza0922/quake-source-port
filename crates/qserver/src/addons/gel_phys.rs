//! What gel does to movers.
//!
//! Repulsion (blue): physics runs as usual; afterwards a mover that hit a
//! blue floor falling faster than [`LAND_MIN`] is sent back up at the speed
//! it landed (at least [`BOUNCE_MIN`]), one that ran into a blue wall
//! faster than [`WALL_MIN`] rebounds off it with a little lift, and a
//! player jumping off blue gets [`JUMP`] instead of the normal jump.
//! Simply walking onto blue gel does not bounce (as in Portal 2).
//!
//! Propulsion (orange): a player standing on it moves with
//! [`SPEED_FACTOR`]× the move input and top speed and a tenth of the
//! friction, so speed builds up quickly; off the gel the momentum carries
//! through the air and normal friction slows it on plain floor.

use qcore::math::Vec3;
use qworld::{E, Edicts, Ent, F, V};

use super::gel::{BLUE, Gel, ORANGE, Splat};

const MOVETYPE_WALK: i32 = 3;
const MOVETYPE_STEP: i32 = 4;
const MOVETYPE_TOSS: i32 = 6;
const MOVETYPE_BOUNCE: i32 = 10;
const FL_ONGROUND: i32 = 512;

/// Slowest landing that bounces.
pub const LAND_MIN: f32 = 170.0;
/// Slowest bounce off a blue floor (~126 units up at sv_gravity 800).
pub const BOUNCE_MIN: f32 = 450.0;
/// Fastest bounce.
const BOUNCE_MAX: f32 = 1500.0;
/// Jumping off blue gel (~290 units up).
pub const JUMP: f32 = 680.0;
/// Slowest run into a blue wall that rebounds.
const WALL_MIN: f32 = 180.0;
/// Move input, top speed and friction on orange gel.
pub const SPEED_FACTOR: f32 = 2.6;
const FRICTION_FACTOR: f32 = 0.1;

fn movable<D: Edicts + ?Sized>(ents: &D, e: Ent) -> bool {
    let mt = ents.f(e, F::MoveType) as i32;
    matches!(mt, MOVETYPE_WALK | MOVETYPE_STEP | MOVETYPE_TOSS | MOVETYPE_BOUNCE)
}

/// Does a box (absolute `lo`..`hi`) touch splat `s`?
pub fn touches(s: &Splat, lo: Vec3, hi: Vec3) -> bool {
    let centre = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    let n = s.normal;
    let ext = half.x * n.x.abs() + half.y * n.y.abs() + half.z * n.z.abs();
    let gap = (centre - s.origin).dot(n) - ext;
    if !(-6.0..=4.0).contains(&gap) {
        return false;
    }
    // nearest box point to the splat centre, measured along the surface
    let q = Vec3::new(s.origin.x.clamp(lo.x, hi.x), s.origin.y.clamp(lo.y, hi.y), s.origin.z.clamp(lo.z, hi.z));
    let d = q - s.origin;
    (d - n * d.dot(n)).length() < s.radius * 0.85
}

fn bbox<D: Edicts + ?Sized>(ents: &D, e: Ent, origin: Vec3) -> (Vec3, Vec3) {
    (origin + ents.v(e, V::Mins), origin + ents.v(e, V::Maxs))
}

/// The gel under a box's feet (floor splats only), if any; blue wins.
pub fn floor_kind(gel: &Gel, lo: Vec3, hi: Vec3) -> Option<u8> {
    let lo = lo - Vec3::new(0.0, 0.0, 2.0);
    let mut found = None;
    for s in gel.splats.iter().filter(|s| s.normal.z > 0.7) {
        if touches(s, lo, hi) {
            if s.kind == BLUE {
                return Some(BLUE);
            }
            found = Some(ORANGE);
        }
    }
    found
}

/// Before physics: remember movers (only while there is blue gel).
pub(crate) fn snapshot<D: Edicts + ?Sized>(gel: &mut Gel, ents: &D) {
    gel.snapshot.clear();
    if !gel.splats.iter().any(|s| s.kind == BLUE) {
        return;
    }
    for e in 1..ents.num_edicts() {
        if !ents.is_free(e) && movable(ents, e) {
            let og = ents.flags(e) & FL_ONGROUND != 0;
            gel.snapshot.push((e, ents.v(e, V::Origin), ents.v(e, V::Velocity), og));
        }
    }
}

/// Player input on orange gel: scales the move and loosens friction.
pub(crate) fn tune<D: Edicts + ?Sized>(gel: &Gel, ents: &D, e: Ent, p: &mut qworld::phys::PhysParams, cmd: &mut qworld::user::UserCmd) {
    if gel.splats.is_empty() || ents.flags(e) & FL_ONGROUND == 0 {
        return;
    }
    let (lo, hi) = bbox(ents, e, ents.v(e, V::Origin));
    if floor_kind(gel, lo, hi) == Some(ORANGE) {
        p.maxspeed *= SPEED_FACTOR;
        p.friction *= FRICTION_FACTOR;
        cmd.forwardmove *= SPEED_FACTOR;
        cmd.sidemove *= SPEED_FACTOR;
    }
}

/// After physics: bounces off blue gel, and footstep sounds on any gel.
pub(crate) fn after<D: Edicts + ?Sized>(gel: &mut Gel, ents: &mut D, players: usize, now: f64) {
    let snap = std::mem::take(&mut gel.snapshot);
    for &(e, pre_origin, pre, was_on) in &snap {
        if ents.is_free(e) || !movable(ents, e) {
            continue;
        }
        let origin = ents.v(e, V::Origin);
        let mut vel = ents.v(e, V::Velocity);
        let on = ents.flags(e) & FL_ONGROUND != 0;
        let (lo, hi) = bbox(ents, e, origin);
        let player = (e as usize) <= players;
        let mut kick = None;
        let floor_now = floor_kind(gel, lo, hi) == Some(BLUE);
        let landed = pre.z < -LAND_MIN && (on || vel.z > pre.z * 0.5);
        if landed && floor_now {
            vel.z = (-pre.z).clamp(BOUNCE_MIN, BOUNCE_MAX);
            vel.x = pre.x;
            vel.y = pre.y;
            kick = Some(vel.z);
        } else if player && was_on && vel.z > 150.0 && pre.z < 60.0 {
            let (plo, phi) = bbox(ents, e, pre_origin);
            if floor_kind(gel, plo, phi) == Some(BLUE) {
                vel.z = JUMP;
                kick = Some(JUMP);
            }
        }
        if kick.is_none() {
            for s in gel.splats.iter().filter(|s| s.kind == BLUE && s.normal.z.abs() < 0.7) {
                let into = -pre.dot(s.normal);
                if into > WALL_MIN && vel.dot(s.normal) > -into * 0.5 && touches(s, lo, hi) {
                    vel = vel - s.normal * vel.dot(s.normal) + s.normal * into * 0.95;
                    vel.z = vel.z.max(pre.z) + if player { 160.0 } else { 60.0 };
                    kick = Some(into);
                    break;
                }
            }
        }
        if let Some(speed) = kick {
            ents.set_v(e, V::Velocity, vel);
            ents.set_flags(e, ents.flags(e) & !FL_ONGROUND);
            ents.set_e(e, E::GroundEntity, 0);
            let feet = Vec3::new(origin.x, origin.y, lo.z);
            gel.event(format!("//qsp_gel j {BLUE} {:.1} {:.1} {:.1} {:.0}\n", feet.x, feet.y, feet.z, speed));
        }
    }
    gel.snapshot = snap;
    gel.snapshot.clear();
    steps(gel, ents, players, now);
}

/// Footsteps for players walking on gel: blue stretches, orange zips
/// faster the quicker you go.
fn steps<D: Edicts + ?Sized>(gel: &mut Gel, ents: &D, players: usize, now: f64) {
    if gel.splats.is_empty() {
        return;
    }
    for i in 0..players.min(gel.next_step.len()) {
        let e = i as Ent + 1;
        if ents.is_free(e) || ents.flags(e) & FL_ONGROUND == 0 || ents.f(e, F::Health) <= 0.0 {
            continue;
        }
        let v = ents.v(e, V::Velocity);
        let speed = (v.x * v.x + v.y * v.y).sqrt();
        if speed < 60.0 || now < gel.next_step[i] {
            continue;
        }
        let (lo, hi) = bbox(ents, e, ents.v(e, V::Origin));
        let Some(kind) = floor_kind(gel, lo, hi) else { continue };
        let gap = if kind == BLUE { 0.42 } else { 0.3 } * (320.0 / speed.max(160.0)).min(2.0);
        gel.next_step[i] = now + f64::from(gap.max(0.12));
        let o = ents.v(e, V::Origin);
        gel.event(format!("//qsp_gel s {kind} {:.1} {:.1} {:.1} {:.0}\n", o.x, o.y, lo.z, speed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_touch_floor_and_wall_splats() {
        let mut g = Gel::default();
        g.add(ORANGE, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), 32.0);
        g.add(BLUE, Vec3::new(100.0, 0.0, 40.0), Vec3::new(-1.0, 0.0, 0.0), 32.0);
        let player = |x: f32, z: f32| (Vec3::new(x - 16.0, -16.0, z - 24.0), Vec3::new(x + 16.0, 16.0, z + 32.0));
        let (lo, hi) = player(10.0, 24.0);
        assert_eq!(floor_kind(&g, lo, hi), Some(ORANGE));
        let (lo, hi) = player(80.0, 24.0);
        assert_eq!(floor_kind(&g, lo, hi), None);
        // standing against the wall splat
        let (lo, hi) = player(84.0, 30.0);
        assert!(touches(&g.splats[1], lo, hi));
        // a metre above the floor gel is not on it
        let (lo, hi) = player(0.0, 64.0);
        assert_eq!(floor_kind(&g, lo, hi), None);
    }
}
