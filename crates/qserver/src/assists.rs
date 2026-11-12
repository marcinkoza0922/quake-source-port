//! Gameplay assists (Options → Gameplay → Assists). Engine-side so they work
//! with any progs: damage scaling hooks QC `T_Damage`; ammo, regeneration
//! and extra jumps watch the client edicts once per server frame. All are
//! off in deathmatch.

use qcore::cvar::{CVAR_ARCHIVE, Cvars};
use qvm::{OFS_PARM0, Result, Vm};

use crate::server::{Core, Level};

/// (name, default, flags) of the assist cvars.
pub const CVARS: &[(&str, &str, u32)] = &[
    ("assist_damagetaken", "1", CVAR_ARCHIVE),
    ("assist_enemyhealth", "1", CVAR_ARCHIVE),
    ("assist_ammo", "1", CVAR_ARCHIVE),
    ("assist_infiniteammo", "0", CVAR_ARCHIVE),
    ("assist_nofalldamage", "0", CVAR_ARCHIVE),
    ("assist_nodrown", "0", CVAR_ARCHIVE),
    ("assist_noselfdamage", "0", CVAR_ARCHIVE),
    ("assist_regen", "0", CVAR_ARCHIVE),
    ("assist_jumps", "1", CVAR_ARCHIVE),
];

const FL_CLIENT: i32 = 8;
const FL_MONSTER: i32 = 32;
const FL_ONGROUND: i32 = 512;
const FL_JUMPRELEASED: i32 = 4096;
const CONTENTS_WATER: f32 = -3.0;
/// id1 bound_other_ammo caps (shells, nails, rockets, cells).
const AMMO_CAPS: [f32; 4] = [100.0, 200.0, 100.0, 100.0];
/// Regeneration: delay after the last hit, rate (hp/s) and ceiling.
const REGEN_DELAY: f64 = 5.0;
const REGEN_RATE: f32 = 5.0;
const REGEN_MAX: f32 = 100.0;
/// id1 PlayerJump vertical speed.
const JUMP_SPEED: f32 = 270.0;

/// Per-client tracking, reset with the level.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientAssist {
    seen: bool,
    ammo: [f32; 4],
    health: f32,
    last_hurt: f64,
    air_jumps: u32,
}

fn deathmatch(vm: &Vm, c: &Core) -> bool {
    vm.g_f(c.sv.globals.deathmatch) != 0.0
}

fn cv(c: &Cvars, n: &str) -> f32 {
    c.value_of(n)
}

/// T_Damage(targ, inflictor, attacker, damage) hook.
/// Returns `true` to skip the call (damage fully negated).
pub(crate) fn before_damage(vm: &mut Vm, c: &Core) -> Result<bool> {
    if deathmatch(vm, c) {
        return Ok(false);
    }
    let f = &c.sv.fields;
    let targ = vm.g_i(OFS_PARM0);
    let attacker = vm.g_i(OFS_PARM0 + 6);
    let dmg_ofs = OFS_PARM0 + 9;
    if targ <= 0 || targ as usize >= vm.num_edicts() {
        return Ok(false);
    }
    let targ = targ as u32;
    let flags = vm.ent_f(targ, f.flags) as i32;
    let mut dmg = vm.g_f(dmg_ofs);
    if dmg <= 0.0 {
        return Ok(false);
    }
    let cvars = &c.cvars;
    if flags & FL_CLIENT != 0 || (targ as usize) <= c.maxclients() {
        let caller = vm.current_function_name();
        let world = attacker == 0;
        let fall = cv(cvars, "assist_nofalldamage") != 0.0 && caller == "PlayerPostThink";
        let drown = cv(cvars, "assist_nodrown") != 0.0
            && caller == "WaterMove"
            && vm.ent_f(targ, f.watertype) == CONTENTS_WATER;
        if attacker as u32 == targ && cv(cvars, "assist_noselfdamage") != 0.0 {
            // keep rocket jumping: id1 T_Damage knockback without the damage
            let inflictor = vm.g_i(OFS_PARM0 + 3).max(0) as u32;
            let (mn, mx) = (vm.ent_v(inflictor, f.absmin), vm.ent_v(inflictor, f.absmax));
            let o = vm.ent_v(targ, f.origin);
            let d = [0, 1, 2].map(|k| o[k] - (mn[k] + mx[k]) * 0.5);
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if len > 0.0 {
                let mut v = vm.ent_v(targ, f.velocity);
                for k in 0..3 {
                    v[k] += d[k] / len * dmg * 8.0;
                }
                vm.set_ent_v(targ, f.velocity, v);
            }
            return Ok(true);
        }
        if world && (fall || drown) {
            return Ok(true);
        } else {
            dmg *= cv(cvars, "assist_damagetaken").clamp(0.0, 4.0);
        }
    } else if flags & FL_MONSTER != 0 {
        dmg /= cv(cvars, "assist_enemyhealth").clamp(0.25, 4.0);
    }
    vm.set_g_f(dmg_ofs, dmg);
    Ok(false)
}

/// Ammo, regeneration and extra jumps; runs after each physics frame.
pub(crate) fn frame(l: &mut Level, c: &mut Core) {
    let vm = &mut l.vm;
    let n = c.maxclients();
    if l.assist.len() != n {
        l.assist = vec![ClientAssist::default(); n];
    }
    if deathmatch(vm, c) {
        return;
    }
    let f = c.sv.fields.clone();
    let ammo_f = [f.ammo_shells, f.ammo_nails, f.ammo_rockets, f.ammo_cells];
    let mult = cv(&c.cvars, "assist_ammo").clamp(0.5, 4.0);
    let infinite = cv(&c.cvars, "assist_infiniteammo") != 0.0;
    let regen = cv(&c.cvars, "assist_regen") != 0.0;
    let jumps = cv(&c.cvars, "assist_jumps").clamp(1.0, 3.0) as u32;
    let time = vm.time;
    let ft = vm.g_f(c.sv.globals.frametime);
    for i in 0..n {
        let e = i as u32 + 1;
        let st = &mut l.assist[i];
        if !c.clients[i].spawned || vm.is_free(e) {
            *st = ClientAssist::default();
            continue;
        }
        let health = vm.ent_f(e, f.health);
        let alive = health > 0.0;
        let mut ammo = ammo_f.map(|o| vm.ent_f(e, o));
        if st.seen && alive && st.health > 0.0 {
            let cur = vm.ent_f(e, f.currentammo);
            for k in 0..4 {
                let (old, new) = (st.ammo[k], ammo[k]);
                let v = if new > old && mult != 1.0 {
                    (old + (new - old) * mult).min(AMMO_CAPS[k].max(new)).round()
                } else if new < old && infinite {
                    old
                } else {
                    new
                };
                if v != new {
                    vm.set_ent_f(e, ammo_f[k], v);
                    if cur == new {
                        vm.set_ent_f(e, f.currentammo, v);
                    }
                    ammo[k] = v;
                }
            }
        }
        // regeneration
        if health < st.health || !st.seen {
            st.last_hurt = time;
        }
        let mut health = health;
        if regen && alive && health < REGEN_MAX && time - st.last_hurt >= REGEN_DELAY {
            health = (health + REGEN_RATE * ft).min(REGEN_MAX);
            vm.set_ent_f(e, f.health, health);
        }
        // extra jumps: QC's FL_JUMPRELEASED is set while the button is up
        // and cleared by its own ground jump, so a press counts once
        let flags = vm.ent_f(e, f.flags) as i32;
        let held = vm.ent_f(e, f.button2) != 0.0;
        if flags & FL_ONGROUND != 0 || vm.ent_f(e, f.waterlevel) >= 2.0 {
            st.air_jumps = 0;
        } else if held && flags & FL_JUMPRELEASED != 0 && alive && st.air_jumps + 1 < jumps {
            st.air_jumps += 1;
            vm.set_ent_f(e, f.flags, (flags & !FL_JUMPRELEASED) as f32);
            let mut v = vm.ent_v(e, f.velocity);
            v[2] = JUMP_SPEED;
            vm.set_ent_v(e, f.velocity, v);
            crate::send::start_sound(vm, c, e, 4, b"player/plyrjmp8.wav", 255, 1.0);
        }
        st.ammo = ammo;
        st.health = health;
        st.seen = true;
    }
}
