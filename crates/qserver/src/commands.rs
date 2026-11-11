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
    s.add_cmd("color", cmd_color);
    s.add_cmd("name", cmd_name);
    s.add_cmd("prespawn", cmd_prespawn);
    s.add_cmd("spawn", cmd_spawn);
    s.add_cmd("begin", cmd_begin);
    s.add_cmd("maxplayers", cmd_maxplayers);
    s.add_cmd("say", |s, a| cmd_say(s, a, false));
    s.add_cmd("say_team", |s, a| cmd_say(s, a, true));
    s.add_cmd("tell", cmd_tell);
    s.add_cmd("status", cmd_status);
    s.add_cmd("kick", cmd_kick);
    s.add_cmd("ping", cmd_ping);
}

/// Host_MaxPlayers_f: slot count for the next server (1..=16); more than
/// one turns deathmatch on, one turns it off.
fn cmd_maxplayers(s: &mut Server, a: &Args) {
    if s.core.cmd_client.is_some() {
        return;
    }
    if a.argc() != 2 {
        s.core.print(&format!("\"maxplayers\" is \"{}\"\n", s.core.maxclients()));
        return;
    }
    let n = (q_atof(a.argv(1)) as usize).clamp(1, crate::server::MAX_SCOREBOARD);
    if n == s.core.maxclients() {
        return;
    }
    if s.active() {
        s.core.print("maxplayers can not be changed while a server is running.\n");
        return;
    }
    s.set_maxclients(n);
    s.core.cvars.set("deathmatch", if n == 1 { "0" } else { "1" });
}

/// Text after the command, without surrounding quotes (Host_Say_f).
fn say_text(a: &Args) -> String {
    let t = a.args();
    let t = t.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(t);
    t.to_string()
}

/// Name of the sender: the client, the listen server's local player, or
/// `<hostname>` from a dedicated console.
fn sender(s: &Server) -> (Option<usize>, String) {
    let i = s.core.cmd_client.or_else(|| s.core.clients.iter().position(|c| c.active && c.local));
    match i {
        Some(i) => (Some(i), s.core.clients[i].name.clone()),
        None => (None, format!("<{}>", s.core.cvars.by_name("hostname").map_or("", |c| c.string.as_str()))),
    }
}

/// Host_Say_f.
fn cmd_say(s: &mut Server, a: &Args, team: bool) {
    if a.argc() < 2 || !s.active() {
        return;
    }
    let (from, name) = sender(s);
    let text = say_text(a);
    let msg = if from.is_some() { format!("\x01{name}: {text}\n") } else { format!("\x01{name} {text}\n") };
    let teamplay = team && s.core.cvars.value_of("teamplay") != 0.0;
    let my_team = from.and_then(|i| s.level.as_ref().map(|l| l.vm.ent_f(i as u32 + 1, s.core.sv.fields.team)));
    for j in 0..s.core.maxclients() {
        let c = &s.core.clients[j];
        if !c.active || !c.spawned {
            continue;
        }
        if teamplay
            && let (Some(t), Some(l)) = (my_team, s.level.as_ref())
            && l.vm.ent_f(j as u32 + 1, s.core.sv.fields.team) != t
        {
            continue;
        }
        let m = &mut s.core.clients[j].message;
        m.byte(crate::msg::SVC_PRINT);
        m.string(msg.as_bytes());
    }
    if !s.core.clients.iter().any(|c| c.active && c.local) {
        s.core.print(&msg.replace('\x01', ""));
    }
}

/// Host_Tell_f: `tell <name> <text>`.
fn cmd_tell(s: &mut Server, a: &Args) {
    if a.argc() < 3 || !s.active() {
        return;
    }
    let (_, name) = sender(s);
    let rest = a.args();
    let text = rest.split_once(char::is_whitespace).map_or("", |(_, t)| t).trim();
    let text = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(text);
    let msg = format!("{name}: {text}\n");
    let to = a.argv(1).to_string();
    for j in 0..s.core.maxclients() {
        let c = &s.core.clients[j];
        if c.active && c.spawned && c.name.eq_ignore_ascii_case(&to) {
            let m = &mut s.core.clients[j].message;
            m.byte(crate::msg::SVC_PRINT);
            m.string(msg.as_bytes());
            break;
        }
    }
}

/// Print to whoever ran the command: a client's console or the server's.
fn reply(s: &mut Server, text: &str) {
    // The listen server's console is the local player's.
    let to = s.core.cmd_client.or_else(|| s.core.clients.iter().position(|c| c.active && c.local));
    match to {
        Some(i) => {
            let m = &mut s.core.clients[i].message;
            m.byte(crate::msg::SVC_PRINT);
            m.string(text.as_bytes());
        }
        None => s.core.print(text),
    }
}

/// Host_Status_f.
fn cmd_status(s: &mut Server, _a: &Args) {
    if !s.active() {
        return;
    }
    let host = s.core.cvars.by_name("hostname").map_or(String::new(), |c| c.string.clone());
    let active = s.core.clients.iter().filter(|c| c.active).count();
    let mut out = format!("host:    {host}\nmap:     {}\nplayers: {active} active ({} max)\n\n", s.core.sv.name, s.core.maxclients());
    for j in 0..s.core.maxclients() {
        let c = &s.core.clients[j];
        if !c.active {
            continue;
        }
        let frags = s.level.as_ref().map_or(0, |l| l.vm.ent_f(j as u32 + 1, s.core.sv.fields.frags) as i32);
        let ms = (c.ping() * 1000.0).round() as i32;
        out.push_str(&format!("#{:<2} {:<16.16}  {frags:3}  {ms:4} ms\n", j + 1, c.name));
    }
    reply(s, &out);
}

/// Host_Kick_f: `kick <name>` or `kick # <slot>`, optional message. Console only.
fn cmd_kick(s: &mut Server, a: &Args) {
    if s.core.cmd_client.is_some_and(|i| !s.core.clients[i].local) || !s.active() || a.argc() < 2 {
        return;
    }
    let (target, msg_at) = if a.argv(1) == "#" {
        let n = q_atof(a.argv(2)) as usize;
        (n.checked_sub(1).filter(|&i| i < s.core.maxclients() && s.core.clients[i].active), 3)
    } else {
        (s.core.clients.iter().position(|c| c.active && c.name.eq_ignore_ascii_case(a.argv(1))), 2)
    };
    let Some(i) = target else { return };
    if s.core.clients[i].local {
        s.core.print("You can't kick yourself!\n");
        return;
    }
    let (_, who) = sender(s);
    let mut text = format!("Kicked by {who}");
    if a.argc() > msg_at {
        let rest: Vec<&str> = (msg_at..a.argc()).map(|k| a.argv(k)).collect();
        text.push_str(": ");
        text.push_str(&rest.join(" "));
    }
    text.push('\n');
    let m = &mut s.core.clients[i].message;
    m.byte(crate::msg::SVC_PRINT);
    m.string(text.as_bytes());
    s.drop_client(i, false);
}

/// Host_Ping_f: average round trip per player, in ms.
fn cmd_ping(s: &mut Server, _a: &Args) {
    let mut out = String::from("Client ping times:\n");
    for c in s.core.clients.iter().filter(|c| c.active) {
        out.push_str(&format!("{:4} {}\n", (c.ping() * 1000.0).round() as i32, c.name));
    }
    reply(s, &out);
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

/// Host_Color_f, server side: `color <top> [bottom]` from a client.
fn cmd_color(s: &mut Server, a: &Args) {
    if a.argc() < 2 {
        return;
    }
    let i = s.core.cmd_client.unwrap_or(0);
    if i >= s.core.clients.len() {
        return;
    }
    let p = |k: usize| (q_atof(a.argv(k)) as i32 & 15).min(13);
    let top = p(1);
    let bottom = if a.argc() == 2 { top } else { p(2) };
    let colors = top * 16 + bottom;
    s.core.clients[i].colors = colors;
    if let Some(l) = s.level.as_mut()
        && s.core.clients[i].spawned
    {
        l.vm.set_ent_f(i as u32 + 1, s.core.sv.fields.team, (bottom + 1) as f32);
    }
    let m = &mut s.core.sv.reliable_datagram;
    m.byte(crate::msg::SVC_UPDATECOLORS);
    m.byte(i as i32);
    m.byte(colors);
}

/// Host_Name_f, server side.
fn cmd_name(s: &mut Server, a: &Args) {
    if a.argc() < 2 {
        return;
    }
    let i = s.core.cmd_client.unwrap_or(0);
    if i >= s.core.clients.len() {
        return;
    }
    let raw = if a.argc() == 2 { a.argv(1) } else { a.args() };
    let name: String = raw.chars().take(15).collect();
    let old = std::mem::replace(&mut s.core.clients[i].name, name.clone());
    if !old.is_empty() && old != name {
        s.core.print(&format!("{old} renamed to {name}\n"));
    }
    if let Some(l) = s.level.as_mut()
        && s.core.clients[i].spawned
    {
        let h = l.vm.strings.engine(name.as_bytes());
        l.vm.set_ent_i(i as u32 + 1, s.core.sv.fields.netname, h);
    }
    let m = &mut s.core.sv.reliable_datagram;
    m.byte(crate::msg::SVC_UPDATENAME);
    m.byte(i as i32);
    m.string(name.as_bytes());
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
