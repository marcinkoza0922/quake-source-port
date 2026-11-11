//! Client movement (sv_user.c): friction, acceleration, water/air/noclip moves.

use crate::phys::PhysParams;
use crate::world::MoveType;
use crate::{Edicts, Ent, F, FL_ONGROUND, FL_WATERJUMP, MOVETYPE_NOCLIP, MOVETYPE_NONE, MOVETYPE_WALK, V, Vec3, World};
use qcore::math::angle_vectors;

/// usercmd_t movement part.
#[derive(Clone, Copy, Debug, Default)]
pub struct UserCmd {
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
}

const ON_EPSILON: f32 = 0.1;

/// SV_UserFriction.
pub fn user_friction<D: Edicts + ?Sized>(w: &World, ents: &mut D, p: &PhysParams, e: Ent) {
    let mut vel = ents.v(e, V::Velocity);
    let origin = ents.v(e, V::Origin);
    let speed = (vel.x * vel.x + vel.y * vel.y).sqrt();
    if speed == 0.0 {
        return;
    }
    let mut start = Vec3::new(origin.x + vel.x / speed * 16.0, origin.y + vel.y / speed * 16.0, 0.0);
    start.z = origin.z + ents.v(e, V::Mins).z;
    let stop = Vec3::new(start.x, start.y, start.z - 34.0);
    let tr = w.trace(ents, start, Vec3::ZERO, Vec3::ZERO, stop, MoveType::NoMonsters, Some(e));
    let friction = if tr.fraction == 1.0 { p.friction * p.edgefriction } else { p.friction };
    let control = if speed < p.stopspeed { p.stopspeed } else { speed };
    let mut newspeed = (speed as f64 - p.frametime * control as f64 * friction as f64) as f32;
    if newspeed < 0.0 {
        newspeed = 0.0;
    }
    newspeed /= speed;
    vel *= newspeed;
    ents.set_v(e, V::Velocity, vel);
}

/// SV_Accelerate.
pub fn accelerate(vel: &mut Vec3, p: &PhysParams, wishspeed: f32, wishdir: Vec3) {
    let addspeed = wishspeed - vel.dot(wishdir);
    if addspeed <= 0.0 {
        return;
    }
    let accelspeed = ((p.accelerate as f64 * p.frametime * wishspeed as f64) as f32).min(addspeed);
    *vel += wishdir * accelspeed;
}

/// SV_AirAccelerate (`wishveloc` is normalized in place, as in C).
pub fn air_accelerate(vel: &mut Vec3, p: &PhysParams, wishspeed: f32, wishveloc: &mut Vec3) {
    let (n, len) = wishveloc.normalize_len();
    *wishveloc = n;
    let wishspd = len.min(30.0);
    let addspeed = wishspd - vel.dot(n);
    if addspeed <= 0.0 {
        return;
    }
    let accelspeed = ((p.accelerate as f64 * wishspeed as f64 * p.frametime) as f32).min(addspeed);
    *vel += n * accelspeed;
}

/// DropPunchAngle.
pub fn drop_punch_angle<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent) {
    let (n, len) = ents.v(e, V::PunchAngle).normalize_len();
    let len = ((len as f64 - 10.0 * p.frametime) as f32).max(0.0);
    ents.set_v(e, V::PunchAngle, n * len);
}

/// SV_WaterMove.
pub fn water_move<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent, cmd: &UserCmd) {
    let (forward, right, _) = angle_vectors(ents.v(e, V::VAngle));
    let mut wishvel = forward * cmd.forwardmove + right * cmd.sidemove;
    if cmd.forwardmove == 0.0 && cmd.sidemove == 0.0 && cmd.upmove == 0.0 {
        wishvel.z -= 60.0;
    } else {
        wishvel.z += cmd.upmove;
    }
    let mut wishspeed = wishvel.length();
    if wishspeed > p.maxspeed {
        wishvel *= p.maxspeed / wishspeed;
        wishspeed = p.maxspeed;
    }
    wishspeed *= 0.7;
    let mut vel = ents.v(e, V::Velocity);
    let speed = vel.length();
    let newspeed = if speed != 0.0 {
        let ns = ((speed as f64 - p.frametime * speed as f64 * p.friction as f64) as f32).max(0.0);
        vel *= ns / speed;
        ns
    } else {
        0.0
    };
    ents.set_v(e, V::Velocity, vel);
    if wishspeed == 0.0 {
        return;
    }
    let addspeed = wishspeed - newspeed;
    if addspeed <= 0.0 {
        return;
    }
    let wishvel = wishvel.normalize();
    let accelspeed = ((p.accelerate as f64 * wishspeed as f64 * p.frametime) as f32).min(addspeed);
    ents.set_v(e, V::Velocity, vel + wishvel * accelspeed);
}

/// SV_WaterJump.
pub fn water_jump<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent) {
    if p.time > ents.f(e, F::TeleportTime) as f64 || ents.f(e, F::WaterLevel) == 0.0 {
        ents.set_flags(e, ents.flags(e) & !FL_WATERJUMP);
        ents.set_f(e, F::TeleportTime, 0.0);
    }
    let md = ents.v(e, V::MoveDir);
    let mut v = ents.v(e, V::Velocity);
    v.x = md.x;
    v.y = md.y;
    ents.set_v(e, V::Velocity, v);
}

/// SV_NoclipMove (johnfitz alternate noclip).
pub fn noclip_move<D: Edicts + ?Sized>(ents: &mut D, p: &PhysParams, e: Ent, cmd: &UserCmd) {
    let (f, r, _) = angle_vectors(ents.v(e, V::VAngle));
    let mut v = f * cmd.forwardmove + r * cmd.sidemove;
    v.z += cmd.upmove * 2.0;
    if v.length() > p.maxspeed {
        v = v.normalize() * p.maxspeed;
    }
    ents.set_v(e, V::Velocity, v);
}

/// SV_AirMove.
pub fn air_move<D: Edicts + ?Sized>(w: &World, ents: &mut D, p: &PhysParams, e: Ent, cmd: &UserCmd, onground: bool) {
    let (forward, right, _) = angle_vectors(ents.v(e, V::Angles));
    let mut fmove = cmd.forwardmove;
    let smove = cmd.sidemove;
    if p.time < ents.f(e, F::TeleportTime) as f64 && fmove < 0.0 {
        fmove = 0.0;
    }
    let mut wishvel = forward * fmove + right * smove;
    let mt = ents.fi(e, F::MoveType);
    wishvel.z = if mt != MOVETYPE_WALK { cmd.upmove } else { 0.0 };
    let (wishdir, mut wishspeed) = wishvel.normalize_len();
    if wishspeed > p.maxspeed {
        wishvel *= p.maxspeed / wishspeed;
        wishspeed = p.maxspeed;
    }
    if mt == MOVETYPE_NOCLIP {
        ents.set_v(e, V::Velocity, wishvel);
    } else if onground {
        user_friction(w, ents, p, e);
        let mut v = ents.v(e, V::Velocity);
        accelerate(&mut v, p, wishspeed, wishdir);
        ents.set_v(e, V::Velocity, v);
    } else {
        let mut v = ents.v(e, V::Velocity);
        air_accelerate(&mut v, p, wishspeed, &mut wishvel);
        ents.set_v(e, V::Velocity, v);
    }
}

/// V_CalcRoll.
pub fn calc_roll(angles: Vec3, velocity: Vec3, rollangle: f32, rollspeed: f32) -> f32 {
    let (_, right, _) = angle_vectors(angles);
    let side = velocity.dot(right);
    let sign = if side < 0.0 { -1.0 } else { 1.0 };
    let side = side.abs();
    let v = if side < rollspeed { side * rollangle / rollspeed } else { rollangle };
    v * sign
}

/// SV_ClientThink for player `e`.
pub fn client_think<D: Edicts + ?Sized>(w: &World, ents: &mut D, p: &PhysParams, e: Ent, cmd: &UserCmd) {
    let mt = ents.fi(e, F::MoveType);
    if mt == MOVETYPE_NONE {
        return;
    }
    let onground = ents.flags(e) & FL_ONGROUND != 0;
    drop_punch_angle(ents, p, e);
    if ents.f(e, F::Health) <= 0.0 {
        return;
    }
    let v_angle = ents.v(e, V::VAngle) + ents.v(e, V::PunchAngle);
    let mut angles = ents.v(e, V::Angles);
    angles.z = calc_roll(angles, ents.v(e, V::Velocity), p.rollangle, p.rollspeed) * 4.0;
    if ents.f(e, F::FixAngle) == 0.0 {
        angles.x = -v_angle.x / 3.0;
        angles.y = v_angle.y;
    }
    ents.set_v(e, V::Angles, angles);
    if ents.flags(e) & FL_WATERJUMP != 0 {
        water_jump(ents, p, e);
        return;
    }
    if mt == MOVETYPE_NOCLIP && p.altnoclip {
        noclip_move(ents, p, e, cmd);
    } else if ents.f(e, F::WaterLevel) >= 2.0 && mt != MOVETYPE_NOCLIP {
        water_move(ents, p, e, cmd);
    } else {
        air_move(w, ents, p, e, cmd, onground);
    }
}

/// SV_SetIdealPitch.
pub fn set_ideal_pitch<D: Edicts + ?Sized>(w: &World, ents: &mut D, p: &PhysParams, e: Ent) {
    const MAX_FORWARD: usize = 6;
    if ents.flags(e) & FL_ONGROUND == 0 {
        return;
    }
    let a = (ents.v(e, V::Angles).y as f64 * std::f64::consts::PI * 2.0 / 360.0) as f32 as f64;
    let (sinval, cosval) = (a.sin() as f32, a.cos() as f32);
    let org = ents.v(e, V::Origin);
    let mut z = [0.0f32; MAX_FORWARD];
    for (i, zi) in z.iter_mut().enumerate() {
        let k = (i + 3) as f32 * 12.0;
        let top = Vec3::new(org.x + cosval * k, org.y + sinval * k, org.z + ents.v(e, V::ViewOfs).z);
        let bottom = Vec3::new(top.x, top.y, top.z - 160.0);
        let tr = w.trace(ents, top, Vec3::ZERO, Vec3::ZERO, bottom, MoveType::NoMonsters, Some(e));
        if tr.allsolid || tr.fraction == 1.0 {
            return;
        }
        *zi = top.z + tr.fraction * (bottom.z - top.z);
    }
    let (mut dir, mut steps) = (0i32, 0);
    for j in 1..MAX_FORWARD {
        let step = (z[j] - z[j - 1]) as i32;
        let s = step as f32;
        if s > -ON_EPSILON && s < ON_EPSILON {
            continue;
        }
        let d = (step - dir) as f32;
        if dir != 0 && (d > ON_EPSILON || d < -ON_EPSILON) {
            return;
        }
        steps += 1;
        dir = step;
    }
    if dir == 0 {
        ents.set_f(e, F::IdealPitch, 0.0);
        return;
    }
    if steps < 2 {
        return;
    }
    ents.set_f(e, F::IdealPitch, -dir as f32 * p.idealpitchscale);
}
