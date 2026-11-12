use super::*;

/// SV_WalkMove (players). The C version reads FL_WATERJUMP from `sv_player`;
/// here it reads it from `e` (same thing in single player).
pub fn walk_move<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    let oldonground = ents.flags(e) & FL_ONGROUND;
    ents.set_flags(e, ents.flags(e) & !FL_ONGROUND);
    let oldorg = ents.v(e, V::Origin);
    let oldvel = ents.v(e, V::Velocity);
    let ft = p.frametime as f32;
    let mut steptrace = Trace::new(Vec3::ZERO);
    let clip = fly_move(w, ents, e, ft, Some(&mut steptrace));
    if clip & 2 == 0 {
        return;
    }
    if oldonground == 0 && ents.f(e, F::WaterLevel) == 0.0 {
        return;
    }
    if ents.fi(e, F::MoveType) != MOVETYPE_WALK || p.nostep || ents.flags(e) & FL_WATERJUMP != 0 {
        return;
    }
    let nosteporg = ents.v(e, V::Origin);
    let nostepvel = ents.v(e, V::Velocity);
    ents.set_v(e, V::Origin, oldorg);
    let upmove = Vec3::new(0.0, 0.0, STEPSIZE);
    let downmove = Vec3::new(0.0, 0.0, (-STEPSIZE as f64 + oldvel.z as f64 * p.frametime) as f32);
    push_entity(w, ents, e, upmove);
    ents.set_v(e, V::Velocity, Vec3::new(oldvel.x, oldvel.y, 0.0));
    let mut clip = fly_move(w, ents, e, ft, Some(&mut steptrace));
    if clip != 0 {
        let o = ents.v(e, V::Origin);
        if (oldorg.y - o.y).abs() < 0.03125 && (oldorg.x - o.x).abs() < 0.03125 {
            clip = try_unstick(w, ents, e, oldvel);
        }
    }
    if clip & 2 != 0 {
        wall_friction(ents, e, &steptrace);
    }
    let downtrace = push_entity(w, ents, e, downmove);
    if downtrace.plane_normal.z > 0.7 {
        // C quirk: tests the mover's solid, not the ground's.
        if ents.fi(e, F::Solid) == SOLID_BSP {
            ents.set_flags(e, ents.flags(e) | FL_ONGROUND);
            ents.set_e(e, E::GroundEntity, downtrace.ent.unwrap_or(0));
        }
    } else {
        ents.set_v(e, V::Origin, nosteporg);
        ents.set_v(e, V::Velocity, nostepvel);
    }
}

/// Middle part of SV_Physics_Client: run after PlayerPreThink, before
/// PlayerPostThink. Returns false when the C code returns early (no link,
/// no PostThink).
pub fn physics_client_move<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) -> bool {
    check_velocity(ents, p, e);
    match ents.fi(e, F::MoveType) {
        MOVETYPE_NONE => {
            if !run_think(w, ents, p, e) {
                return false;
            }
        }
        MOVETYPE_WALK => {
            if !run_think(w, ents, p, e) {
                return false;
            }
            if !check_water(w, ents, e) && ents.flags(e) & FL_WATERJUMP == 0 {
                add_gravity(ents, p, e);
            }
            check_stuck(w, ents, e);
            walk_move(w, ents, p, e);
        }
        MOVETYPE_TOSS | MOVETYPE_BOUNCE | MOVETYPE_GIB => physics_toss(w, ents, p, e),
        MOVETYPE_FLY => {
            if !run_think(w, ents, p, e) {
                return false;
            }
            fly_move(w, ents, e, p.frametime as f32, None);
        }
        MOVETYPE_NOCLIP => {
            if !run_think(w, ents, p, e) {
                return false;
            }
            ents.set_v(e, V::Origin, ma(ents.v(e, V::Origin), p.frametime as f32, ents.v(e, V::Velocity)));
        }
        _ => return false,
    }
    w.link_edict(ents, e, true);
    true
}
