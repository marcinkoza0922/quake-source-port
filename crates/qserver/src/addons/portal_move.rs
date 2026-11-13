//! Crossing portals. Collision still stops a mover at the wall, so the
//! check runs after physics with the velocity from *before* the frame: a
//! mover touching a portal's rectangle while heading into it (or resting
//! on a floor portal) comes out of the linked portal with that velocity
//! rotated, which keeps momentum.

use qcore::math::{Vec3, angle_vectors, vec_to_angles};
use qworld::{E, Edicts, Ent, F, V, World};

use super::portal::{HALF_H, HALF_W, Portal, Portals};

const MOVETYPE_WALK: i32 = 3;
const MOVETYPE_STEP: i32 = 4;
const MOVETYPE_FLY: i32 = 5;
const MOVETYPE_TOSS: i32 = 6;
const MOVETYPE_BOUNCE: i32 = 10;
const FL_ONGROUND: i32 = 512;
/// Slowest exit, so nothing sits in the exit plane and drops straight back.
const MIN_EXIT_SPEED: f32 = 60.0;
/// Slowest exit out of a floor portal: a launch about as high as a jump
/// (~81 units at sv_gravity 800), so a mover that dropped or stepped into a
/// floor portal pops out with room to steer away instead of falling
/// straight back in and ping-ponging between two floor portals.
const MIN_FLOOR_EXIT_SPEED: f32 = 360.0;

fn movable<D: Edicts + ?Sized>(ents: &D, e: Ent) -> bool {
    let mt = ents.f(e, F::MoveType) as i32;
    matches!(mt, MOVETYPE_WALK | MOVETYPE_STEP | MOVETYPE_FLY | MOVETYPE_TOSS | MOVETYPE_BOUNCE)
}

/// Record movers' origin and velocity before physics (only while some
/// pair is linked).
pub(crate) fn snapshot<D: Edicts + ?Sized>(st: &mut Portals, ents: &D) {
    st.snapshot.clear();
    if !st.any_linked() {
        return;
    }
    for e in 1..ents.num_edicts() {
        if !ents.is_free(e) && movable(ents, e) {
            st.snapshot.push((e, ents.v(e, V::Origin), ents.v(e, V::Velocity)));
        }
    }
}

/// Half extent of the box (`mins`, `maxs`) along `n`.
fn extent(mins: Vec3, maxs: Vec3, n: Vec3) -> f32 {
    let h = (maxs - mins) * 0.5;
    h.x * n.x.abs() + h.y * n.y.abs() + h.z * n.z.abs()
}

/// Rotate `v` from portal `a`'s frame into the frame leaving `b`
/// (in through `a`'s face, out of `b`'s face).
pub fn carry(a: &Portal, b: &Portal, v: Vec3) -> Vec3 {
    let (n, r, u) = (v.dot(a.normal), v.dot(a.right), v.dot(a.up));
    b.normal * -n + b.right * -r + b.up * u
}

/// Teleport movers that went into a portal this frame. `idle` lists player
/// entities giving no movement input: leaving a floor portal they lose any
/// sideways drift (they never touch ground to feel friction), so between
/// two floor portals they bob straight up and down until they move.
pub(crate) fn cross<D: Edicts + ?Sized>(st: &mut Portals, w: &mut World, ents: &mut D, idle: &[Ent]) {
    if st.snapshot.is_empty() {
        return;
    }
    let pairs: Vec<_> = st.pairs.iter().enumerate().filter_map(|(o,p)| Some((o,[p[0]?, p[1]?]))).collect();
    let mut effects = Vec::new();
    for &(e, _, vel) in &st.snapshot {
        if ents.is_free(e) || !movable(ents, e) {
            continue;
        }
        let (mins, maxs) = (ents.v(e, V::Mins), ents.v(e, V::Maxs));
        let mid = (mins + maxs) * 0.5;
        let centre = ents.v(e, V::Origin) + mid;
        let onground = ents.flags(e) & FL_ONGROUND != 0;
        let hit = pairs.iter().flat_map(|(o,p)| [(*o,0,p[0],p[1]), (*o,1,p[1],p[0])]).find(|(_, _, a, _)| {
            let d = centre - a.origin;
            let along = d.dot(a.normal);
            let ext = extent(mins, maxs, a.normal);
            let into = vel.dot(a.normal) < -1.0 || (a.normal.z > 0.7 && onground);
            into && along <= ext + 4.0 && along > -ext && d.dot(a.right).abs() <= HALF_W && d.dot(a.up).abs() <= HALF_H
        });
        let Some((owner, slot, a, b)) = hit else { continue };
        let d = centre - a.origin;
        let ext = extent(mins, maxs, b.normal);
        let lateral = |k: f32| (-d.dot(a.right) * k, d.dot(a.up) * k);
        let old = ents.v(e, V::Origin);
        let mut placed = false;
        // where it lines up, then centred, then shifted along the exit's up
        // axis (a tall monster out of a low wall portal clears the floor)
        let (r1, u1) = lateral(1.0);
        for (r, u) in [(r1, u1), (0.0, 0.0), (0.0, 16.0), (0.0, 32.0), (0.0, -16.0)] {
            let c = b.at(r.clamp(-HALF_W, HALF_W), u.clamp(-HALF_H, HALF_H)) + b.normal * (ext + 1.0);
            ents.set_v(e, V::Origin, c - mid);
            if w.test_entity_position(ents, e).is_none() {
                placed = true;
                break;
            }
        }
        if !placed {
            ents.set_v(e, V::Origin, old);
            continue;
        }
        let mut v = carry(&a, &b, vel);
        if b.normal.z > 0.7 && idle.contains(&e) {
            v = b.normal * v.dot(b.normal);
        }
        let out = v.dot(b.normal);
        let min = if b.normal.z > 0.7 { MIN_FLOOR_EXIT_SPEED } else { MIN_EXIT_SPEED };
        if out < min {
            v += b.normal * (min - out);
        }
        ents.set_v(e, V::Velocity, v);
        turn(ents, e, &a, &b);
        ents.set_flags(e, ents.flags(e) & !FL_ONGROUND);
        ents.set_e(e, E::GroundEntity, 0);
        w.link_edict(ents, e, true);
        effects.push((owner, slot, a.origin, b.origin));
    }
    st.snapshot.clear();
    for (owner, slot, start, end) in effects {
        st.effect(owner, slot, 2, start, end);
    }
}

/// Turn the mover (and a player's view) by the portal rotation.
fn turn<D: Edicts + ?Sized>(ents: &mut D, e: Ent, a: &Portal, b: &Portal) {
    let is_player = ents.f(e, F::MoveType) as i32 == MOVETYPE_WALK;
    let src = if is_player { ents.v(e, V::VAngle) } else { ents.v(e, V::Angles) };
    let (fwd, _, _) = angle_vectors(src);
    let mut ang = vec_to_angles(carry(a, b, fwd));
    // vectoangles gives pitch positive = up; view angles are the opposite
    if is_player {
        ang.x = -ang.x;
        if ang.x < -180.0 {
            ang.x += 360.0;
        }
        ents.set_v(e, V::Angles, Vec3::new(0.0, ang.y, 0.0));
        ents.set_v(e, V::VAngle, ang);
        ents.set_f(e, F::FixAngle, 1.0);
    } else {
        let mut cur = ents.v(e, V::Angles);
        cur.y = ang.y;
        ents.set_v(e, V::Angles, cur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_to_wall_turns_falling_into_horizontal_speed() {
        let z = Vec3::new(0.0, 0.0, 1.0);
        let floor = Portal::new(Vec3::ZERO, z, Vec3::new(1.0, 0.0, 0.0));
        let wall = Portal::new(Vec3::new(500.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), z);
        let v = carry(&floor, &wall, Vec3::new(0.0, 0.0, -600.0));
        assert!((v - Vec3::new(-600.0, 0.0, 0.0)).length() < 1e-3, "{v:?}");
        // walking into one wall comes out of a facing wall at the same speed
        let w2 = Portal::new(Vec3::new(-500.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), z);
        let v = carry(&wall, &w2, Vec3::new(320.0, 0.0, 0.0));
        assert!((v - Vec3::new(320.0, 0.0, 0.0)).length() < 1e-3, "{v:?}");
    }
}
