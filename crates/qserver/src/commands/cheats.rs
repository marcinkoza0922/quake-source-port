//! Cheats and debugging commands (host_cmd.c, pr_edict.c): god, notarget,
//! noclip, fly, give, setpos/viewpos, portal_set and the edict dumps.

use qcore::cmd::Args;
use qcore::cvar::q_atof;
use qworld::{MOVETYPE_FLY, MOVETYPE_NOCLIP, MOVETYPE_WALK};

use crate::server::Server;

const FL_GODMODE: i32 = 64;
const FL_NOTARGET: i32 = 128;

const IT_SHOTGUN: i32 = 1;
const IT_ARMOR1: i32 = 8192;
const IT_ARMOR2: i32 = 16384;
const IT_ARMOR3: i32 = 32768;

pub(super) fn register(s: &mut Server) {
    s.add_cmd("god", |s, a| toggle_flag(s, a, FL_GODMODE, "godmode"));
    s.add_cmd("notarget", |s, a| toggle_flag(s, a, FL_NOTARGET, "notarget"));
    s.add_cmd("noclip", |s, a| toggle_movetype(s, a, MOVETYPE_NOCLIP, "noclip", "noclip"));
    s.add_cmd("fly", |s, a| toggle_movetype(s, a, MOVETYPE_FLY, "fly", "flymode"));
    s.add_cmd("setpos", cmd_setpos);
    s.add_cmd("viewpos", cmd_viewpos);
    s.add_cmd("portal_set", cmd_portal_set);
    s.add_cmd("give", cmd_give);
}

fn toggle_flag(s: &mut Server, a: &Args, flag: i32, label: &str) {
    let Some((i, e)) = s.cmd_player() else { return };
    if s.deathmatch() {
        return;
    }
    let fo = s.core.sv.fields.flags;
    let l = s.level.as_mut().unwrap();
    let fl = l.vm.ent_f(e, fo) as i32;
    let on = match a.argc() {
        1 => fl & flag == 0,
        2 => q_atof(a.argv(1)) != 0.0,
        _ => {
            s.core.print(&format!("{label} [value] : toggle {label}. values: 0 = off, 1 = on\n"));
            return;
        }
    };
    let fl = if on { fl | flag } else { fl & !flag };
    l.vm.set_ent_f(e, fo, fl as f32);
    s.client_print(i, &format!("{label} {}\n", if on { "ON" } else { "OFF" }));
}

/// Host_Noclip_f / Host_Fly_f: toggle (or set) a movetype against walking.
fn toggle_movetype(s: &mut Server, a: &Args, movetype: i32, cmd: &str, label: &str) {
    let Some((i, e)) = s.cmd_player() else { return };
    if s.deathmatch() {
        return;
    }
    let mo = s.core.sv.fields.movetype;
    let l = s.level.as_mut().unwrap();
    let on = match a.argc() {
        1 => l.vm.ent_f(e, mo) as i32 != movetype,
        2 => q_atof(a.argv(1)) != 0.0,
        _ => {
            s.core.print(&format!("{cmd} [value] : toggle {label}. values: 0 = off, 1 = on\n"));
            return;
        }
    };
    l.vm.set_ent_f(e, mo, if on { movetype } else { MOVETYPE_WALK } as f32);
    s.client_print(i, &format!("{label} {}\n", if on { "ON" } else { "OFF" }));
}

/// setpos x y z [pitch yaw roll]: teleport the player (debug).
fn cmd_setpos(s: &mut Server, a: &Args) {
    let Some((_, e)) = s.cmd_player() else { return };
    if s.deathmatch() {
        return;
    }
    if a.argc() != 4 && a.argc() != 7 {
        s.core.print("setpos x y z [pitch yaw roll]\n");
        return;
    }
    let f = s.core.sv.fields.clone();
    let v = |o: usize| [q_atof(a.argv(o)), q_atof(a.argv(o + 1)), q_atof(a.argv(o + 2))];
    let l = s.level.as_mut().unwrap();
    l.vm.set_ent_v(e, f.origin, v(1));
    l.vm.set_ent_v(e, f.velocity, [0.0; 3]);
    if a.argc() == 7 {
        l.vm.set_ent_v(e, f.angles, v(4));
        l.vm.set_ent_f(e, f.fixangle, 1.0);
    }
    l.world.link_edict(&mut crate::ents::Ents { vm: &mut l.vm, c: &mut s.core }, e, false);
}

/// `viewpos`: print the player's position and view angles in the form
/// `setpos` takes (vkQuake's CL_Viewpos_f), truncated to integers.
fn cmd_viewpos(s: &mut Server, _a: &Args) {
    let Some((i, e)) = s.cmd_player() else { return };
    let f = s.core.sv.fields.clone();
    let Some(l) = s.level.as_ref() else { return };
    let (o, a) = (l.vm.ent_v(e, f.origin), l.vm.ent_v(e, f.v_angle));
    let msg = format!(
        "Viewpos: ({} {} {}) {} {} {}\n",
        o[0] as i32, o[1] as i32, o[2] as i32, a[0] as i32, a[1] as i32, a[2] as i32
    );
    s.client_print(i, &msg);
}

/// `portal_set slot ox oy oz nx ny nz ux uy uz` (or `portal_set slot` to
/// remove): place the local player's portal exactly, without the gun's
/// surface checks. For screenshots and tests; single player only.
fn cmd_portal_set(s: &mut Server, a: &Args) {
    let Some((i, _)) = s.cmd_player() else { return };
    let slot = a.argv(1).parse::<usize>().ok().filter(|&n| n < 2);
    let (Some(slot), false, 2 | 11) = (slot, s.deathmatch(), a.argc()) else {
        s.core.print("portal_set <0|1> [ox oy oz nx ny nz ux uy uz]\n");
        return;
    };
    let v = |o: usize| {
        let f = |k: usize| a.argv(o + k).parse::<f32>().unwrap_or(0.0);
        qcore::math::Vec3::new(f(0), f(1), f(2))
    };
    let p = (a.argc() == 11).then(|| crate::addons::portal::Portal::new(v(2), v(5).normalize(), v(8).normalize()));
    if let Some(l) = s.level.as_mut() {
        l.portals.set(i, slot, p);
        l.portals.sync_links(&mut s.core.linked_portals);
    }
}

/// Host_Give_f (id1 items; mission-pack special cases not handled).
fn cmd_give(s: &mut Server, a: &Args) {
    let Some((_, e)) = s.cmd_player() else { return };
    if s.deathmatch() {
        return;
    }
    let f = s.core.sv.fields.clone();
    let l = s.level.as_mut().unwrap();
    let vm = &mut l.vm;
    let t = a.argv(1).as_bytes().first().copied().unwrap_or(0);
    let v = q_atof(a.argv(2));
    let items = vm.ent_f(e, f.items) as i32;
    match t {
        b'2'..=b'9' => {
            let bit = IT_SHOTGUN << (t - b'2');
            vm.set_ent_f(e, f.items, (items | bit) as f32);
        }
        b's' => vm.set_ent_f(e, f.ammo_shells, v),
        b'n' => vm.set_ent_f(e, f.ammo_nails, v),
        b'l' | b'r' => vm.set_ent_f(e, f.ammo_rockets, v),
        b'c' => vm.set_ent_f(e, f.ammo_cells, v),
        b'h' => vm.set_ent_f(e, f.health, v),
        b'a' => {
            let (ty, bit) = if v > 150.0 {
                (0.8, IT_ARMOR3)
            } else if v > 100.0 {
                (0.6, IT_ARMOR2)
            } else {
                (0.3, IT_ARMOR1)
            };
            vm.set_ent_f(e, f.armortype, ty);
            vm.set_ent_f(e, f.armorvalue, v);
            let it = (items & !(IT_ARMOR1 | IT_ARMOR2 | IT_ARMOR3)) | bit;
            vm.set_ent_f(e, f.items, it as f32);
        }
        _ => {}
    }
}

/// The edict dumps (pr_edict.c); the host prints them, since a listen
/// server's log only echoes diagnostics and a full dump doesn't fit a
/// client message. `None` without an active server.
impl Server {
    /// ED_PrintEdict_f: one edict's non-zero fields.
    pub fn edict_text(&self, n: i32) -> Option<String> {
        let l = self.level.as_ref().filter(|_| self.active())?;
        if n < 0 || n as usize >= l.vm.num_edicts() {
            return Some("Bad edict number\n".into());
        }
        Some(l.vm.ed_print(n as u32))
    }

    /// ED_PrintEdicts: every edict.
    pub fn edicts_text(&self) -> Option<String> {
        let l = self.level.as_ref().filter(|_| self.active())?;
        let mut t = format!("{} entities\n", l.vm.num_edicts());
        for e in 0..l.vm.num_edicts() as u32 {
            t += &l.vm.ed_print(e);
        }
        Some(t)
    }

    /// ED_Count: edict totals.
    pub fn edictcount_text(&self) -> Option<String> {
        let l = self.level.as_ref().filter(|_| self.active())?;
        let f = &self.core.sv.fields;
        let (mut active, mut models, mut solid, mut step) = (0, 0, 0, 0);
        for e in (0..l.vm.num_edicts() as u32).filter(|&e| !l.vm.is_free(e)) {
            active += 1;
            solid += i32::from(l.vm.ent_f(e, f.solid) != 0.0);
            models += i32::from(l.vm.ent_i(e, f.model) != 0);
            step += i32::from(l.vm.ent_f(e, f.movetype) as i32 == qworld::MOVETYPE_STEP);
        }
        Some(format!(
            "num_edicts:{:3}\nactive    :{active:3}\nview      :{models:3}\ntouch     :{solid:3}\nstep      :{step:3}\n",
            l.vm.num_edicts()
        ))
    }
}
