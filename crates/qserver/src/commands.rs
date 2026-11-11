//! Server console commands (host_cmd.c): map, changelevel, restart, god,
//! noclip, notarget, give, kill and the client signon commands.

use qcore::cmd::Args;
use qcore::cvar::q_atof;
use qworld::{MOVETYPE_NOCLIP, MOVETYPE_WALK};

use crate::client::Signon;
use crate::server::{Server, exec};

const FL_GODMODE: i32 = 64;
const FL_NOTARGET: i32 = 128;

const IT_SHOTGUN: i32 = 1;
const IT_ARMOR1: i32 = 8192;
const IT_ARMOR2: i32 = 16384;
const IT_ARMOR3: i32 = 32768;

pub(crate) fn register(s: &mut Server) {
    s.add_cmd("map", cmd_map);
    s.add_cmd("changelevel", cmd_changelevel);
    s.add_cmd("restart", cmd_restart);
    s.add_cmd("god", |s, a| toggle_flag(s, a, FL_GODMODE, "godmode"));
    s.add_cmd("notarget", |s, a| toggle_flag(s, a, FL_NOTARGET, "notarget"));
    s.add_cmd("noclip", cmd_noclip);
    s.add_cmd("setpos", cmd_setpos);
    s.add_cmd("give", cmd_give);
    s.add_cmd("kill", cmd_kill);
    s.add_cmd("prespawn", cmd_prespawn);
    s.add_cmd("spawn", cmd_spawn);
    s.add_cmd("begin", cmd_begin);
}

impl Server {
    /// Player edict of the client running the command (console = client 0).
    fn cmd_player(&self) -> Option<(usize, u32)> {
        let i = self.core.cmd_client.unwrap_or(0);
        let ok = self.level.is_some() && self.core.clients.get(i).is_some_and(|c| c.active && c.spawned);
        ok.then_some((i, i as u32 + 1))
    }

    fn client_print(&mut self, i: usize, s: &str) {
        let m = &mut self.core.clients[i].message;
        m.byte(crate::msg::SVC_PRINT);
        m.string(s.as_bytes());
        self.core.print(s);
    }

    fn deathmatch(&self) -> bool {
        self.level.as_ref().is_some_and(|l| l.vm.g_f(self.core.sv.globals.deathmatch) != 0.0)
    }
}

fn cmd_map(s: &mut Server, a: &Args) {
    if a.argc() < 2 {
        let m = if s.active() { format!("Current map: {}\n", s.core.sv.name) } else { "map <levelname>: start a new server\n".into() };
        s.core.print(&m);
        return;
    }
    if s.core.cmd_client.is_some() {
        return;
    }
    let had_local = s.core.clients.iter().any(|c| c.active && c.local);
    for i in 0..s.core.maxclients() {
        s.drop_client(i, false);
    }
    s.core.serverflags = 0.0;
    let name = a.argv(1).strip_suffix(".bsp").unwrap_or(a.argv(1)).to_string();
    if s.spawn_server(&name).is_ok() && had_local {
        s.connect_local();
    }
}

fn cmd_changelevel(s: &mut Server, a: &Args) {
    if a.argc() != 2 {
        s.core.print("changelevel <levelname> : continue game on a new level\n");
        return;
    }
    if !s.active() {
        s.core.print("Only the server may changelevel\n");
        return;
    }
    let level = a.argv(1).to_string();
    if !s.core.fs.exists(&format!("maps/{level}.bsp")) {
        s.core.host_error(qvm::VmError::new(format!("cannot find map maps/{level}.bsp")));
        return;
    }
    s.save_spawn_parms();
    let _ = s.spawn_server(&level);
}

fn cmd_restart(s: &mut Server, _: &Args) {
    if !s.active() || s.core.cmd_client.is_some() {
        return;
    }
    let name = s.core.sv.name.clone();
    let _ = s.spawn_server(&name);
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

fn cmd_noclip(s: &mut Server, a: &Args) {
    let Some((i, e)) = s.cmd_player() else { return };
    if s.deathmatch() {
        return;
    }
    let mo = s.core.sv.fields.movetype;
    let l = s.level.as_mut().unwrap();
    let on = match a.argc() {
        1 => l.vm.ent_f(e, mo) as i32 != MOVETYPE_NOCLIP,
        2 => q_atof(a.argv(1)) != 0.0,
        _ => {
            s.core.print("noclip [value] : toggle noclip mode. values: 0 = off, 1 = on\n");
            return;
        }
    };
    l.vm.set_ent_f(e, mo, if on { MOVETYPE_NOCLIP } else { MOVETYPE_WALK } as f32);
    s.client_print(i, &format!("noclip {}\n", if on { "ON" } else { "OFF" }));
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

fn cmd_kill(s: &mut Server, _: &Args) {
    let Some((i, e)) = s.cmd_player() else { return };
    let f = s.core.sv.fields.clone();
    let g = s.core.sv.globals.clone();
    let l = s.level.as_mut().unwrap();
    if l.vm.ent_f(e, f.health) <= 0.0 {
        s.client_print(i, "Can't suicide -- already dead!\n");
        return;
    }
    l.vm.set_g_f(g.time, l.vm.time as f32);
    l.vm.set_self(e);
    { let fnum = l.vm.g_i(g.f_client_kill); exec(&mut l.vm, &mut s.core, &mut l.world, fnum); }
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

fn cmd_prespawn(s: &mut Server, _: &Args) {
    let Some(i) = s.core.cmd_client else {
        s.core.print("prespawn is not valid from the console\n");
        return;
    };
    let cl = &mut s.core.clients[i];
    if cl.spawned {
        s.core.print("prespawn not valid -- already spawned\n");
        return;
    }
    cl.sendsignon = Signon::SignonBufs;
    cl.signonidx = 0;
}

fn cmd_spawn(s: &mut Server, _: &Args) {
    let Some(i) = s.core.cmd_client else {
        s.core.print("spawn is not valid from the console\n");
        return;
    };
    s.client_spawn(i);
}

fn cmd_begin(s: &mut Server, _: &Args) {
    let Some(i) = s.core.cmd_client else {
        s.core.print("begin is not valid from the console\n");
        return;
    };
    s.core.clients[i].spawned = true;
}
