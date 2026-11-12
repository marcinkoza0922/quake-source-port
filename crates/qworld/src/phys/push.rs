use super::*;

/// SV_PushEntity (does not touch velocity).
pub fn push_entity<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent, push: Vec3) -> Trace {
    let org = ents.v(e, V::Origin);
    let end = org + push;
    let solid = ents.fi(e, F::Solid);
    let ty = if ents.fi(e, F::MoveType) == MOVETYPE_FLYMISSILE {
        MoveType::Missile
    } else if solid == SOLID_TRIGGER || solid == SOLID_NOT {
        MoveType::NoMonsters
    } else {
        MoveType::Normal
    };
    let tr = w.trace(ents, org, ents.v(e, V::Mins), ents.v(e, V::Maxs), end, ty, Some(e));
    ents.set_v(e, V::Origin, tr.endpos);
    w.link_edict(ents, e, true);
    if let Some(t) = tr.ent {
        impact(w, ents, e, t);
    }
    tr
}

/// SV_PushMove.
pub fn push_move<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, pusher: Ent, movetime: f32) {
    let vel = ents.v(pusher, V::Velocity);
    if vel == Vec3::ZERO {
        ents.set_f(pusher, F::Ltime, ents.f(pusher, F::Ltime) + movetime);
        return;
    }
    let mv = vel * movetime;
    let mins = ents.v(pusher, V::AbsMin) + mv;
    let maxs = ents.v(pusher, V::AbsMax) + mv;
    let pushorig = ents.v(pusher, V::Origin);
    ents.set_v(pusher, V::Origin, pushorig + mv);
    ents.set_f(pusher, F::Ltime, ents.f(pusher, F::Ltime) + movetime);
    w.link_edict(ents, pusher, false);

    let mut moved = std::mem::take(&mut w.push_scratch);
    moved.clear();
    for check in 1..ents.num_edicts() {
        if ents.is_free(check) {
            continue;
        }
        let mt = ents.fi(check, F::MoveType);
        if mt == MOVETYPE_PUSH || mt == MOVETYPE_NONE || mt == MOVETYPE_NOCLIP {
            continue;
        }
        let riding = ents.flags(check) & FL_ONGROUND != 0 && ents.e(check, E::GroundEntity) == pusher;
        if !riding {
            let (amin, amax) = (ents.v(check, V::AbsMin), ents.v(check, V::AbsMax));
            if amin.x >= maxs.x
                || amin.y >= maxs.y
                || amin.z >= maxs.z
                || amax.x <= mins.x
                || amax.y <= mins.y
                || amax.z <= mins.z
            {
                continue;
            }
            if w.test_entity_position(ents, check).is_none() {
                continue;
            }
        }
        if mt != MOVETYPE_WALK {
            ents.set_flags(check, ents.flags(check) & !FL_ONGROUND);
        }
        let entorig = ents.v(check, V::Origin);
        moved.push((check, entorig));
        let solid_backup = ents.f(pusher, F::Solid);
        let sb = solid_backup as i32;
        if sb == SOLID_BSP || sb == SOLID_BBOX || sb == SOLID_SLIDEBOX {
            ents.set_f(pusher, F::Solid, SOLID_NOT as f32);
            push_entity(w, ents, check, mv);
            ents.set_f(pusher, F::Solid, solid_backup);
        }
        if let Some(block) = w.test_entity_position(ents, check) {
            let (cmins, cmaxs) = (ents.v(check, V::Mins), ents.v(check, V::Maxs));
            if cmins.x == cmaxs.x {
                continue;
            }
            let cs = ents.fi(check, F::Solid);
            if cs == SOLID_NOT || cs == SOLID_TRIGGER {
                let m = Vec3::new(0.0, 0.0, cmins.z);
                ents.set_v(check, V::Mins, m);
                ents.set_v(check, V::Maxs, m);
                continue;
            }
            if riding
                && block == pusher
                && (p.gameplayfix_elevators >= 2.0
                    || (p.gameplayfix_elevators != 0.0 && check <= p.maxclients))
            {
                let mut o = ents.v(check, V::Origin);
                o.z += DIST_EPSILON;
                ents.set_v(check, V::Origin, o);
                if w.test_entity_position(ents, check).is_none() {
                    continue;
                }
            }
            ents.set_v(check, V::Origin, entorig);
            w.link_edict(ents, check, true);
            ents.set_v(pusher, V::Origin, pushorig);
            w.link_edict(ents, pusher, false);
            ents.set_f(pusher, F::Ltime, ents.f(pusher, F::Ltime) - movetime);
            ents.blocked(w, pusher, check);
            for &(m, from) in &moved {
                ents.set_v(m, V::Origin, from);
                w.link_edict(ents, m, false);
            }
            break;
        }
    }
    w.push_scratch = moved;
}

/// SV_Physics_Pusher.
pub fn physics_pusher<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    let oldltime = ents.f(e, F::Ltime);
    let thinktime = ents.f(e, F::NextThink);
    let movetime = if (thinktime as f64) < oldltime as f64 + p.frametime {
        (thinktime - oldltime).max(0.0)
    } else {
        p.frametime as f32
    };
    if movetime != 0.0 {
        push_move(w, ents, p, e, movetime);
    }
    if thinktime > oldltime && thinktime <= ents.f(e, F::Ltime) {
        ents.set_f(e, F::NextThink, 0.0);
        ents.think(w, e, p.time);
    }
}

/// SV_Physics_None.
pub fn physics_none<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    run_think(w, ents, p, e);
}

/// SV_Physics_Noclip.
pub fn physics_noclip<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, p: &PhysParams, e: Ent) {
    if !run_think(w, ents, p, e) {
        return;
    }
    let ft = p.frametime as f32;
    ents.set_v(e, V::Angles, ma(ents.v(e, V::Angles), ft, ents.v(e, V::AVelocity)));
    ents.set_v(e, V::Origin, ma(ents.v(e, V::Origin), ft, ents.v(e, V::Velocity)));
    w.link_edict(ents, e, false);
}
