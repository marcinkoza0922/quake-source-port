//! Monster stepping movement (sv_move.c).

use crate::world::MoveType;
use crate::{CONTENTS_EMPTY, CONTENTS_SOLID, E, Edicts, Ent, F, FL_FLY, FL_ONGROUND, FL_PARTIALGROUND, FL_SWIM, STEPSIZE, V, Vec3, World};
use qcore::math::angle_mod;

/// SV_CheckBottom.
pub fn check_bottom<D: Edicts + ?Sized>(w: &World, ents: &D, e: Ent) -> bool {
    let org = ents.v(e, V::Origin);
    let mins = org + ents.v(e, V::Mins);
    let maxs = org + ents.v(e, V::Maxs);
    let mut start = Vec3::new(0.0, 0.0, mins.z - 1.0);
    let mut easy = true;
    'o: for x in 0..2 {
        for y in 0..2 {
            start.x = if x != 0 { maxs.x } else { mins.x };
            start.y = if y != 0 { maxs.y } else { mins.y };
            if w.point_contents(start) != CONTENTS_SOLID {
                easy = false;
                break 'o;
            }
        }
    }
    if easy {
        return true;
    }
    start.z = mins.z;
    start.x = (mins.x + maxs.x) * 0.5;
    start.y = (mins.y + maxs.y) * 0.5;
    let mut stop = Vec3::new(start.x, start.y, start.z - 2.0 * STEPSIZE);
    let tr = w.trace(ents, start, Vec3::ZERO, Vec3::ZERO, stop, MoveType::NoMonsters, Some(e));
    if tr.fraction == 1.0 {
        return false;
    }
    let mid = tr.endpos.z;
    let mut bottom = mid;
    for x in 0..2 {
        for y in 0..2 {
            start.x = if x != 0 { maxs.x } else { mins.x };
            start.y = if y != 0 { maxs.y } else { mins.y };
            stop.x = start.x;
            stop.y = start.y;
            let tr = w.trace(ents, start, Vec3::ZERO, Vec3::ZERO, stop, MoveType::NoMonsters, Some(e));
            if tr.fraction != 1.0 && tr.endpos.z > bottom {
                bottom = tr.endpos.z;
            }
            if tr.fraction == 1.0 || mid - tr.endpos.z > STEPSIZE {
                return false;
            }
        }
    }
    true
}

/// SV_movestep. On failure nothing moves and false is returned.
pub fn movestep<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent, mv: Vec3, relink: bool) -> bool {
    let oldorg = ents.v(e, V::Origin);
    let (mins, maxs) = (ents.v(e, V::Mins), ents.v(e, V::Maxs));
    let flags = ents.flags(e);
    if flags & (FL_SWIM | FL_FLY) != 0 {
        let enemy = ents.e(e, E::Enemy);
        for i in 0..2 {
            let org = ents.v(e, V::Origin);
            let mut neworg = org + mv;
            if i == 0 && enemy != 0 {
                let dz = org.z - ents.v(enemy, V::Origin).z;
                if dz > 40.0 {
                    neworg.z -= 8.0;
                }
                if dz < 30.0 {
                    neworg.z += 8.0;
                }
            }
            let tr = w.trace(ents, org, mins, maxs, neworg, MoveType::Normal, Some(e));
            if tr.fraction == 1.0 {
                if flags & FL_SWIM != 0 && w.point_contents(tr.endpos) == CONTENTS_EMPTY {
                    return false;
                }
                ents.set_v(e, V::Origin, tr.endpos);
                if relink {
                    w.link_edict(ents, e, true);
                }
                return true;
            }
            if enemy == 0 {
                break;
            }
        }
        return false;
    }
    let mut neworg = oldorg + mv;
    neworg.z += STEPSIZE;
    let mut end = neworg;
    end.z -= STEPSIZE * 2.0;
    let mut tr = w.trace(ents, neworg, mins, maxs, end, MoveType::Normal, Some(e));
    if tr.allsolid {
        return false;
    }
    if tr.startsolid {
        neworg.z -= STEPSIZE;
        tr = w.trace(ents, neworg, mins, maxs, end, MoveType::Normal, Some(e));
        if tr.allsolid || tr.startsolid {
            return false;
        }
    }
    if tr.fraction == 1.0 {
        if flags & FL_PARTIALGROUND != 0 {
            ents.set_v(e, V::Origin, oldorg + mv);
            if relink {
                w.link_edict(ents, e, true);
            }
            ents.set_flags(e, ents.flags(e) & !FL_ONGROUND);
            return true;
        }
        return false;
    }
    ents.set_v(e, V::Origin, tr.endpos);
    if !check_bottom(w, ents, e) {
        if flags & FL_PARTIALGROUND != 0 {
            if relink {
                w.link_edict(ents, e, true);
            }
            return true;
        }
        ents.set_v(e, V::Origin, oldorg);
        return false;
    }
    if flags & FL_PARTIALGROUND != 0 {
        ents.set_flags(e, ents.flags(e) & !FL_PARTIALGROUND);
    }
    ents.set_e(e, E::GroundEntity, tr.ent.unwrap_or(0));
    if relink {
        w.link_edict(ents, e, true);
    }
    true
}

/// PF_changeyaw on `e`.
pub fn change_yaw<D: Edicts + ?Sized>(ents: &mut D, e: Ent) {
    let mut angles = ents.v(e, V::Angles);
    let current = angle_mod(angles.y);
    let ideal = ents.f(e, F::IdealYaw);
    let speed = ents.f(e, F::YawSpeed);
    if current == ideal {
        return;
    }
    let mut mv = ideal - current;
    if ideal > current {
        if mv >= 180.0 {
            mv -= 360.0;
        }
    } else if mv <= -180.0 {
        mv += 360.0;
    }
    if mv > 0.0 {
        if mv > speed {
            mv = speed;
        }
    } else if mv < -speed {
        mv = -speed;
    }
    angles.y = angle_mod(current + mv);
    ents.set_v(e, V::Angles, angles);
}

/// SV_StepDirection.
pub fn step_direction<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent, yaw: f32, dist: f32) -> bool {
    ents.set_f(e, F::IdealYaw, yaw);
    change_yaw(ents, e);
    let yaw = (yaw as f64 * std::f64::consts::PI * 2.0 / 360.0) as f32 as f64;
    let mv = Vec3::new((yaw.cos() * dist as f64) as f32, (yaw.sin() * dist as f64) as f32, 0.0);
    let oldorigin = ents.v(e, V::Origin);
    if movestep(w, ents, e, mv, false) {
        let delta = ents.v(e, V::Angles).y - ents.f(e, F::IdealYaw);
        if delta > 45.0 && delta < 315.0 {
            ents.set_v(e, V::Origin, oldorigin);
        }
        w.link_edict(ents, e, true);
        return true;
    }
    w.link_edict(ents, e, true);
    false
}

const DI_NODIR: f32 = -1.0;

/// SV_NewChaseDir.
pub fn new_chase_dir<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, actor: Ent, enemy: Ent, dist: f32) {
    let olddir = angle_mod(((ents.f(actor, F::IdealYaw) / 45.0) as i32 * 45) as f32);
    let turnaround = angle_mod(olddir - 180.0);
    let (ao, eo) = (ents.v(actor, V::Origin), ents.v(enemy, V::Origin));
    let deltax = eo.x - ao.x;
    let deltay = eo.y - ao.y;
    let mut d = [0.0f32; 3];
    d[1] = if deltax > 10.0 {
        0.0
    } else if deltax < -10.0 {
        180.0
    } else {
        DI_NODIR
    };
    d[2] = if deltay < -10.0 {
        270.0
    } else if deltay > 10.0 {
        90.0
    } else {
        DI_NODIR
    };
    if d[1] != DI_NODIR && d[2] != DI_NODIR {
        let tdir = if d[1] == 0.0 {
            if d[2] == 90.0 { 45.0 } else { 315.0 }
        } else if d[2] == 90.0 {
            135.0
        } else {
            215.0
        };
        if tdir != turnaround && step_direction(w, ents, actor, tdir, dist) {
            return;
        }
    }
    if (ents.rand() & 3) & 1 != 0 || (deltay as i32).abs() > (deltax as i32).abs() {
        d.swap(1, 2);
    }
    if d[1] != DI_NODIR && d[1] != turnaround && step_direction(w, ents, actor, d[1], dist) {
        return;
    }
    if d[2] != DI_NODIR && d[2] != turnaround && step_direction(w, ents, actor, d[2], dist) {
        return;
    }
    if olddir != DI_NODIR && step_direction(w, ents, actor, olddir, dist) {
        return;
    }
    if ents.rand() & 1 != 0 {
        let mut tdir = 0.0;
        while tdir <= 315.0 {
            if tdir != turnaround && step_direction(w, ents, actor, tdir, dist) {
                return;
            }
            tdir += 45.0;
        }
    } else {
        let mut tdir = 315.0;
        while tdir >= 0.0 {
            if tdir != turnaround && step_direction(w, ents, actor, tdir, dist) {
                return;
            }
            tdir -= 45.0;
        }
    }
    if turnaround != DI_NODIR && step_direction(w, ents, actor, turnaround, dist) {
        return;
    }
    ents.set_f(actor, F::IdealYaw, olddir);
    if !check_bottom(w, ents, actor) {
        ents.set_flags(actor, ents.flags(actor) | FL_PARTIALGROUND);
    }
}

/// SV_CloseEnough.
pub fn close_enough<D: Edicts + ?Sized>(ents: &D, e: Ent, goal: Ent, dist: f32) -> bool {
    let (gmin, gmax) = (ents.v(goal, V::AbsMin), ents.v(goal, V::AbsMax));
    let (emin, emax) = (ents.v(e, V::AbsMin), ents.v(e, V::AbsMax));
    (0..3).all(|i| !(gmin[i] > emax[i] + dist || gmax[i] < emin[i] - dist))
}

/// SV_MoveToGoal (the movetogoal builtin) for `e` = self.
pub fn move_to_goal<D: Edicts + ?Sized>(w: &mut World, ents: &mut D, e: Ent, dist: f32) {
    let goal = ents.e(e, E::GoalEntity);
    if ents.flags(e) & (FL_ONGROUND | FL_FLY | FL_SWIM) == 0 {
        return;
    }
    if ents.e(e, E::Enemy) != 0 && close_enough(ents, e, goal, dist) {
        return;
    }
    if (ents.rand() & 3) == 1 || !step_direction(w, ents, e, ents.f(e, F::IdealYaw), dist) {
        new_chase_dir(w, ents, e, goal, dist);
    }
}
