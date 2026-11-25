//! Gameplay modifiers (Customize → Gameplay Modifiers): easier and harder
//! rules for a customized game. Engine-side so they work with any progs:
//! damage scaling hooks QC `T_Damage`; ammo, health, regeneration and extra
//! jumps watch the client edicts once per server frame; extra monsters and
//! pistol starts act at map spawn and changelevel. They only apply in a
//! customized game (`Core::custom`, saved as `.qsa`), never in deathmatch,
//! so a vanilla game plays exactly like other ports.

use qcore::cvar::{CVAR_ARCHIVE, Cvars};
use qvm::{OFS_PARM0, Result, Vm};

use crate::server::{Core, Level};

/// (name, default, flags) of the modifier cvars. Their values are saved
/// in the `.qsa` header of a customized game ([`save_text`]).
pub const CVARS: &[(&str, &str, u32)] = &[
    ("modifier_damagetaken", "1", CVAR_ARCHIVE),
    ("modifier_enemyhealth", "1", CVAR_ARCHIVE),
    ("modifier_ammo", "1", CVAR_ARCHIVE),
    ("modifier_health", "1", CVAR_ARCHIVE),
    ("modifier_infiniteammo", "0", CVAR_ARCHIVE),
    ("modifier_nofalldamage", "0", CVAR_ARCHIVE),
    ("modifier_nodrown", "0", CVAR_ARCHIVE),
    ("modifier_noselfdamage", "0", CVAR_ARCHIVE),
    ("modifier_regen", "0", CVAR_ARCHIVE),
    ("modifier_jumps", "1", CVAR_ARCHIVE),
    // extra copies of every monster at map load (like `sv_horde`)
    ("modifier_horde", "0", CVAR_ARCHIVE),
    // weapon pickups: 0 the game decides, 1 better weapon, 2 never switch
    // (`weaponswitch`, by `cl_weaponpriority`)
    ("modifier_autoswitch", "0", CVAR_ARCHIVE),
    // changelevel resets each player's items (QC SetNewParms)
    ("modifier_pistolstart", "0", CVAR_ARCHIVE),
    // only autosaves; the host refuses `save`
    ("modifier_nosaves", "0", CVAR_ARCHIVE),
];

/// Modifiers apply: a customized game, not deathmatch.
pub fn active(c: &Core) -> bool {
    c.custom && c.cvars.value_of("deathmatch") == 0.0
}

/// Value of a modifier cvar when modifiers apply, else its default.
pub fn value(c: &Core, name: &str) -> f32 {
    if active(c) {
        return c.cvars.value_of(name);
    }
    CVARS.iter().find(|v| v.0 == name).and_then(|v| v.1.parse().ok()).unwrap_or(0.0)
}

/// Modifiers whose value differs from the default (name, value).
pub fn changed(cv: &Cvars) -> impl Iterator<Item = (&'static str, f32)> + '_ {
    CVARS.iter().filter_map(|&(n, d, _)| {
        let v = cv.value_of(n);
        (d.parse::<f32>().ok() != Some(v)).then_some((n, v))
    })
}

/// `.qsa` header lines: `modifier <name> <value>` for every modifier.
pub fn save_text(cv: &Cvars) -> String {
    CVARS.iter().map(|&(n, ..)| format!("modifier {n} {}\n", qcore::cvar::format_float(cv.value_of(n)))).collect()
}

/// Restores the modifiers from a `.qsa` header (defaults for missing lines).
pub fn load_text(cv: &mut Cvars, text: &str) {
    for &(n, d, _) in CVARS {
        cv.set(n, d);
    }
    for line in text.lines() {
        let mut w = line.split_whitespace();
        if w.next() != Some("modifier") {
            continue;
        }
        let (Some(n), Some(v)) = (w.next(), w.next()) else { continue };
        if CVARS.iter().any(|c| c.0 == n) && v.parse::<f32>().is_ok_and(f32::is_finite) {
            cv.set(n, v);
        }
    }
}

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

/// id1 health ceiling for ordinary pickups.
const HEALTH_MAX: f32 = 100.0;

/// Per-client tracking, reset with the level.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientModifiers {
    seen: bool,
    ammo: [f32; 4],
    health: f32,
    last_hurt: f64,
    air_jumps: u32,
}

fn deathmatch(vm: &Vm, c: &Core) -> bool {
    vm.g_f(c.sv.globals.deathmatch) != 0.0
}

/// T_Damage(targ, inflictor, attacker, damage) hook.
/// Returns `true` to skip the call (damage fully negated).
pub(crate) fn before_damage(vm: &mut Vm, c: &Core) -> Result<bool> {
    if !c.custom || deathmatch(vm, c) {
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
    let cv = |n: &str| c.cvars.value_of(n);
    if flags & FL_CLIENT != 0 || (targ as usize) <= c.maxclients() {
        let caller = vm.current_function_name();
        let world = attacker == 0;
        let fall = cv("modifier_nofalldamage") != 0.0 && caller == "PlayerPostThink";
        let drown = cv("modifier_nodrown") != 0.0
            && caller == "WaterMove"
            && vm.ent_f(targ, f.watertype) == CONTENTS_WATER;
        if attacker as u32 == targ && cv("modifier_noselfdamage") != 0.0 {
            // keep rocket jumping: id1 T_Damage knockback without the damage
            let inflictor = u32::try_from(vm.g_i(OFS_PARM0 + 3)).ok().filter(|&e| (e as usize) < vm.num_edicts()).unwrap_or(0);
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
            dmg *= cv("modifier_damagetaken").clamp(0.0, 4.0);
        }
    } else if flags & FL_MONSTER != 0 {
        dmg /= cv("modifier_enemyhealth").clamp(0.25, 4.0);
    }
    vm.set_g_f(dmg_ofs, dmg);
    Ok(false)
}

/// Ammo, health, regeneration and extra jumps; runs after each physics frame.
pub(crate) fn frame(l: &mut Level, c: &mut Core) {
    let vm = &mut l.vm;
    let n = c.maxclients();
    if l.modifiers.len() != n {
        l.modifiers = vec![ClientModifiers::default(); n];
    }
    if !c.custom || deathmatch(vm, c) {
        return;
    }
    let f = c.sv.fields.clone();
    let ammo_f = [f.ammo_shells, f.ammo_nails, f.ammo_rockets, f.ammo_cells];
    let cv = |n: &str| c.cvars.value_of(n);
    let mult = cv("modifier_ammo").clamp(0.5, 4.0);
    let health_mult = cv("modifier_health").clamp(0.5, 2.0);
    let infinite = cv("modifier_infiniteammo") != 0.0;
    let regen = cv("modifier_regen") != 0.0;
    let jumps = cv("modifier_jumps").clamp(1.0, 3.0) as u32;
    let time = vm.time;
    let ft = c.sv.phys.frametime as f32;
    for i in 0..n {
        let e = i as u32 + 1;
        let st = &mut l.modifiers[i];
        if !c.clients[i].spawned || vm.is_free(e) {
            *st = ClientModifiers::default();
            continue;
        }
        let mut health = vm.ent_f(e, f.health);
        let alive = health > 0.0;
        // health pickups: scale increases (not respawns or level starts)
        if st.seen && alive && st.health > 0.0 && health > st.health && health_mult != 1.0 {
            let v = (st.health + (health - st.health) * health_mult).min(HEALTH_MAX.max(health)).round();
            if v != health {
                vm.set_ent_f(e, f.health, v);
                health = v;
            }
        }
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
