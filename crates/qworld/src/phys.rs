//! Server physics (sv_phys.c). All functions take the world, the edicts and
//! the per-frame parameters (cvars + time).

use crate::world::MoveType;
use crate::{
    CONTENTS_EMPTY, CONTENTS_WATER, DIST_EPSILON, E, Edicts, Ent, F, FL_FLY, FL_ONGROUND, FL_SWIM,
    FL_WATERJUMP, MOVETYPE_BOUNCE, MOVETYPE_FLY, MOVETYPE_FLYMISSILE, MOVETYPE_GIB, MOVETYPE_NOCLIP,
    MOVETYPE_NONE, MOVETYPE_PUSH, MOVETYPE_STEP, MOVETYPE_TOSS, MOVETYPE_WALK, SOLID_BBOX, SOLID_BSP,
    SOLID_NOT, SOLID_SLIDEBOX, SOLID_TRIGGER, STEPSIZE, Trace, V, Vec3, World,
};
use qcore::math::angle_vectors;
mod push;
pub use push::*;
mod walk;
pub use walk::*;

/// Cvars and timing used by physics. `Default` gives the stock values.
#[derive(Clone, Copy, Debug)]
pub struct PhysParams {
    /// qcvm->time.
    pub time: f64,
    /// host_frametime.
    pub frametime: f64,
    pub gravity: f32,
    pub friction: f32,
    pub stopspeed: f32,
    pub maxvelocity: f32,
    pub nostep: bool,
    pub edgefriction: f32,
    pub accelerate: f32,
    pub maxspeed: f32,
    /// sv_gameplayfix_elevators: 0 off, 1 clients, 2 all.
    pub gameplayfix_elevators: f32,
    pub maxclients: u32,
    pub altnoclip: bool,
    pub rollangle: f32,
    pub rollspeed: f32,
    pub idealpitchscale: f32,
}

impl Default for PhysParams {
    fn default() -> Self {
        PhysParams {
            time: 0.0,
            frametime: 0.0,
            gravity: 800.0,
            friction: 4.0,
            stopspeed: 100.0,
            maxvelocity: 2000.0,
            nostep: false,
            edgefriction: 2.0,
            accelerate: 10.0,
            maxspeed: 320.0,
            gameplayfix_elevators: 2.0,
            maxclients: 1,
            altnoclip: true,
            rollangle: 2.0,
            rollspeed: 200.0,
            idealpitchscale: 0.8,
        }
    }
}

const STOP_EPSILON: f32 = 0.1;
const MAX_CLIP_PLANES: usize = 5;

fn ma(a: Vec3, s: f32, b: Vec3) -> Vec3 {
    Vec3::new(a.x + s * b.x, a.y + s * b.y, a.z + s * b.z)
}

/// SV_CheckVelocity (NaNs zeroed, clamp to maxvelocity).
pub fn check_velocity<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent) {
    let mut v = ents.v(e, V::Velocity);
    let mut o = ents.v(e, V::Origin);
    for i in 0..3 {
        if v[i].is_nan() {
            v[i] = 0.0;
        }
        if o[i].is_nan() {
            o[i] = 0.0;
        }
        if v[i] > p.maxvelocity {
            v[i] = p.maxvelocity;
        } else if v[i] < -p.maxvelocity {
            v[i] = -p.maxvelocity;
        }
    }
    ents.set_v(e, V::Velocity, v);
    ents.set_v(e, V::Origin, o);
}

/// SV_RunThink. Returns false if the entity removed itself.
pub fn run_think<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) -> bool {
    let mut thinktime = ents.f(e, F::NextThink);
    if thinktime <= 0.0 || thinktime as f64 > p.time + p.frametime {
        return true;
    }
    if (thinktime as f64) < p.time {
        thinktime = p.time as f32;
    }
    ents.set_f(e, F::NextThink, 0.0);
    ents.think(w, e, thinktime as f64);
    !ents.is_free(e)
}

/// SV_Impact.
pub fn impact<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e1: Ent, e2: Ent) {
    if ents.has_touch(e1) && ents.fi(e1, F::Solid) != SOLID_NOT {
        ents.touch(w, e1, e2);
    }
    if ents.has_touch(e2) && ents.fi(e2, F::Solid) != SOLID_NOT {
        ents.touch(w, e2, e1);
    }
}

/// ClipVelocity: returns (out, blocked flags: 1 floor, 2 step).
pub fn clip_velocity(inv: Vec3, normal: Vec3, overbounce: f32) -> (Vec3, i32) {
    let mut blocked = 0;
    if normal.z > 0.0 {
        blocked |= 1;
    }
    if normal.z == 0.0 {
        blocked |= 2;
    }
    let backoff = inv.dot(normal) * overbounce;
    let mut out = Vec3::ZERO;
    for i in 0..3 {
        out[i] = inv[i] - normal[i] * backoff;
        if out[i] > -STOP_EPSILON && out[i] < STOP_EPSILON {
            out[i] = 0.0;
        }
    }
    (out, blocked)
}

/// SV_FlyMove: 1 floor, 2 wall/step, 4 dead stop (3/7 as in C).
pub fn fly_move<D: Edicts + ?Sized>(
    w: &mut World,
    ents: &mut D,
    e: Ent,
    time: f32,
    mut steptrace: Option<&mut Trace>,
) -> i32 {
    let mut blocked = 0;
    let mut original_velocity = ents.v(e, V::Velocity);
    let primal_velocity = original_velocity;
    let mut planes = [Vec3::ZERO; MAX_CLIP_PLANES];
    let mut numplanes = 0;
    let mut time_left = time;
    for _ in 0..4 {
        let vel = ents.v(e, V::Velocity);
        if vel == Vec3::ZERO {
            break;
        }
        let org = ents.v(e, V::Origin);
        let end = ma(org, time_left, vel);
        let tr = w.trace(ents, org, ents.v(e, V::Mins), ents.v(e, V::Maxs), end, MoveType::Normal, Some(e));
        if tr.allsolid {
            ents.set_v(e, V::Velocity, Vec3::ZERO);
            return 3;
        }
        if tr.fraction > 0.0 {
            ents.set_v(e, V::Origin, tr.endpos);
            original_velocity = vel;
            numplanes = 0;
        }
        if tr.fraction == 1.0 {
            break;
        }
        let hit = tr.ent.unwrap_or(0);
        if tr.plane_normal.z > 0.7 {
            blocked |= 1;
            if ents.fi(hit, F::Solid) == SOLID_BSP {
                ents.set_flags(e, ents.flags(e) | FL_ONGROUND);
                ents.set_e(e, E::GroundEntity, hit);
            }
        }
        if tr.plane_normal.z == 0.0 {
            blocked |= 2;
            if let Some(st) = steptrace.as_deref_mut() {
                *st = tr;
            }
        }
        impact(w, ents, e, hit);
        if ents.is_free(e) {
            break;
        }
        time_left -= time_left * tr.fraction;
        if numplanes >= MAX_CLIP_PLANES {
            ents.set_v(e, V::Velocity, Vec3::ZERO);
            return 3;
        }
        planes[numplanes] = tr.plane_normal;
        numplanes += 1;
        let mut new_velocity = Vec3::ZERO;
        let mut i = 0;
        while i < numplanes {
            new_velocity = clip_velocity(original_velocity, planes[i], 1.0).0;
            let ok = (0..numplanes).all(|j| j == i || new_velocity.dot(planes[j]) >= 0.0);
            if ok {
                break;
            }
            i += 1;
        }
        let vel = if i != numplanes {
            new_velocity
        } else {
            if numplanes != 2 {
                ents.set_v(e, V::Velocity, Vec3::ZERO);
                return 7;
            }
            let dir = planes[0].cross(planes[1]);
            let d = dir.dot(ents.v(e, V::Velocity));
            dir * d
        };
        ents.set_v(e, V::Velocity, vel);
        if vel.dot(primal_velocity) <= 0.0 {
            ents.set_v(e, V::Velocity, Vec3::ZERO);
            return blocked;
        }
    }
    blocked
}

/// SV_AddGravity.
pub fn add_gravity<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent) {
    let g = ents.f(e, F::Gravity);
    let g = if g != 0.0 { g } else { 1.0 };
    let mut v = ents.v(e, V::Velocity);
    v.z = (v.z as f64 - g as f64 * p.gravity as f64 * p.frametime) as f32;
    ents.set_v(e, V::Velocity, v);
}

/// SV_CheckWaterTransition.
pub fn check_water_transition<D: Edicts + ?Sized>(w: &World, ents: &mut D, e: Ent) {
    let cont = w.point_contents(ents.v(e, V::Origin));
    let wt = ents.f(e, F::WaterType);
    if wt == 0.0 {
        ents.set_f(e, F::WaterType, cont as f32);
        ents.set_f(e, F::WaterLevel, 1.0);
        return;
    }
    if cont <= CONTENTS_WATER {
        if wt == CONTENTS_EMPTY as f32 {
            ents.start_sound(e, "misc/h2ohit1.wav");
        }
        ents.set_f(e, F::WaterType, cont as f32);
        ents.set_f(e, F::WaterLevel, 1.0);
    } else {
        if wt != CONTENTS_EMPTY as f32 {
            ents.start_sound(e, "misc/h2ohit1.wav");
        }
        ents.set_f(e, F::WaterType, CONTENTS_EMPTY as f32);
        ents.set_f(e, F::WaterLevel, cont as f32);
    }
}

/// SV_Physics_Toss (toss, bounce, gib, fly, flymissile).
pub fn physics_toss<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    if !run_think(w, ents, p, e) {
        return;
    }
    if ents.flags(e) & FL_ONGROUND != 0 {
        return;
    }
    check_velocity(ents, p, e);
    let mt = ents.fi(e, F::MoveType);
    if mt != MOVETYPE_FLY && mt != MOVETYPE_FLYMISSILE {
        add_gravity(ents, p, e);
    }
    let ft = p.frametime as f32;
    ents.set_v(e, V::Angles, ma(ents.v(e, V::Angles), ft, ents.v(e, V::AVelocity)));
    let mv = ents.v(e, V::Velocity) * ft;
    let tr = push_entity(w, ents, e, mv);
    if tr.fraction == 1.0 || ents.is_free(e) {
        return;
    }
    let mt = ents.fi(e, F::MoveType);
    let backoff = if mt == MOVETYPE_BOUNCE { 1.5 } else { 1.0 };
    let (v, _) = clip_velocity(ents.v(e, V::Velocity), tr.plane_normal, backoff);
    ents.set_v(e, V::Velocity, v);
    if tr.plane_normal.z > 0.7 && (v.z < 60.0 || mt != MOVETYPE_BOUNCE) {
        ents.set_flags(e, ents.flags(e) | FL_ONGROUND);
        ents.set_e(e, E::GroundEntity, tr.ent.unwrap_or(0));
        ents.set_v(e, V::Velocity, Vec3::ZERO);
        ents.set_v(e, V::AVelocity, Vec3::ZERO);
    }
    check_water_transition(w, ents, e);
}

/// SV_Physics_Step.
pub fn physics_step<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    if ents.flags(e) & (FL_ONGROUND | FL_FLY | FL_SWIM) == 0 {
        let hitsound = ents.v(e, V::Velocity).z < p.gravity * -0.1;
        add_gravity(ents, p, e);
        check_velocity(ents, p, e);
        fly_move(w, ents, e, p.frametime as f32, None);
        w.link_edict(ents, e, true);
        if ents.flags(e) & FL_ONGROUND != 0 && hitsound {
            ents.start_sound(e, "demon/dland2.wav");
        }
    }
    run_think(w, ents, p, e);
    check_water_transition(w, ents, e);
}

/// Per-entity dispatch of SV_Physics for non-client entities.
pub fn physics_entity<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    match ents.fi(e, F::MoveType) {
        MOVETYPE_PUSH => physics_pusher(w, ents, p, e),
        MOVETYPE_NONE => physics_none(w, ents, p, e),
        MOVETYPE_NOCLIP => physics_noclip(w, ents, p, e),
        MOVETYPE_STEP => physics_step(w, ents, p, e),
        MOVETYPE_TOSS | MOVETYPE_GIB | MOVETYPE_BOUNCE | MOVETYPE_FLY | MOVETYPE_FLYMISSILE => {
            physics_toss(w, ents, p, e)
        }
        _ => {} // C: Sys_Error
    }
}

// ---- client movement ----

/// SV_CheckStuck.
pub fn check_stuck<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent) {
    if w.test_entity_position(ents, e).is_none() {
        ents.set_v(e, V::OldOrigin, ents.v(e, V::Origin));
        return;
    }
    let org = ents.v(e, V::Origin);
    ents.set_v(e, V::Origin, ents.v(e, V::OldOrigin));
    if w.test_entity_position(ents, e).is_none() {
        w.link_edict(ents, e, true);
        return;
    }
    for z in 0..18 {
        for i in -1..=1 {
            for j in -1..=1 {
                ents.set_v(e, V::Origin, Vec3::new(org.x + i as f32, org.y + j as f32, org.z + z as f32));
                if w.test_entity_position(ents, e).is_none() {
                    w.link_edict(ents, e, true);
                    return;
                }
            }
        }
    }
    ents.set_v(e, V::Origin, org);
}

/// SV_CheckWater: sets waterlevel/watertype; true if waterlevel > 1.
pub fn check_water<D: Edicts + ?Sized>(w: &World, ents: &mut D, e: Ent) -> bool {
    let o = ents.v(e, V::Origin);
    let (mins, maxs) = (ents.v(e, V::Mins), ents.v(e, V::Maxs));
    let mut point = Vec3::new(o.x, o.y, o.z + mins.z + 1.0);
    ents.set_f(e, F::WaterLevel, 0.0);
    ents.set_f(e, F::WaterType, CONTENTS_EMPTY as f32);
    let cont = w.point_contents(point);
    if cont <= CONTENTS_WATER {
        ents.set_f(e, F::WaterType, cont as f32);
        ents.set_f(e, F::WaterLevel, 1.0);
        point.z = o.z + (mins.z + maxs.z) * 0.5;
        if w.point_contents(point) <= CONTENTS_WATER {
            ents.set_f(e, F::WaterLevel, 2.0);
            point.z = o.z + ents.v(e, V::ViewOfs).z;
            if w.point_contents(point) <= CONTENTS_WATER {
                ents.set_f(e, F::WaterLevel, 3.0);
            }
        }
    }
    ents.f(e, F::WaterLevel) > 1.0
}

/// SV_WallFriction.
pub fn wall_friction<D: Edicts + ?Sized>(ents: &mut D, e: Ent, tr: &Trace) {
    let (forward, _, _) = angle_vectors(ents.v(e, V::VAngle));
    let d = tr.plane_normal.dot(forward) + 0.5;
    if d >= 0.0 {
        return;
    }
    let mut vel = ents.v(e, V::Velocity);
    let i = tr.plane_normal.dot(vel);
    let side = vel - tr.plane_normal * i;
    vel.x = side.x * (1.0 + d);
    vel.y = side.y * (1.0 + d);
    ents.set_v(e, V::Velocity, vel);
}

/// SV_TryUnstick.
pub fn try_unstick<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent, oldvel: Vec3) -> i32 {
    const DIRS: [(f32, f32); 8] =
        [(2., 0.), (0., 2.), (-2., 0.), (0., -2.), (2., 2.), (-2., 2.), (2., -2.), (-2., -2.)];
    let oldorg = ents.v(e, V::Origin);
    let mut steptrace = Trace::new(Vec3::ZERO);
    for (x, y) in DIRS {
        push_entity(w, ents, e, Vec3::new(x, y, 0.0));
        ents.set_v(e, V::Velocity, Vec3::new(oldvel.x, oldvel.y, 0.0));
        let clip = fly_move(w, ents, e, 0.1, Some(&mut steptrace));
        let o = ents.v(e, V::Origin);
        if (oldorg.y - o.y).abs() > 4.0 || (oldorg.x - o.x).abs() > 4.0 {
            return clip;
        }
        ents.set_v(e, V::Origin, oldorg);
    }
    ents.set_v(e, V::Velocity, Vec3::ZERO);
    7
}
