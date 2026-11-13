use super::*;

pub(super) fn register_commands(c: &mut Commands<Host>) {
    c.add("exec", |h, a| {
        if a.argc() != 2 {
            h.print("exec <filename> : execute a script file\n");
            return;
        }
        let name = a.argv(1);
        // A mod without its own user config starts from the base game's
        // (nearest pack below it first, then id1); runs after any config.cfg
        // the mod ships in its data, and is saved as the mod's own on quit.
        if name.eq_ignore_ascii_case("config.cfg")
            && let Some((g, t)) = h.inherited_config()
        {
            h.cbuf.insert_text("_paddefaults_merge\n");
            h.cbuf.insert_text(&t);
            h.print(&format!("execing config.cfg (inherited from {g})\n"));
            if let Some(d) = h.fs().open(name).filter(|_| h.fs().origin(name).is_some_and(|o| o.game.eq_ignore_ascii_case(h.game_name()))) {
                h.cbuf.insert_text(&String::from_utf8_lossy(&d));
                h.print("execing config.cfg (mod defaults)\n");
            }
            return;
        }
        match h.fs().open(name) {
            Some(d) => {
                let t = String::from_utf8_lossy(&d).into_owned();
                if name.eq_ignore_ascii_case("default.cfg") {
                    h.cbuf.insert_text(PAD_DEFAULTS);
                } else if name.eq_ignore_ascii_case("config.cfg") {
                    h.cbuf.insert_text("_paddefaults_merge\n");
                }
                h.cbuf.insert_text(&t);
                h.print(&format!("execing {name}\n"));
            }
            None if name.eq_ignore_ascii_case("default.cfg") => {
                h.cbuf.insert_text(PAD_DEFAULTS);
                h.cbuf.insert_text(DEFAULT_CFG);
                h.print("execing default.cfg (built-in)\n");
            }
            None => h.print(&format!("couldn't exec {name}\n")),
        }
    });
    c.add("hitfeedback_preview", |h, a| {
        match a.argv(1) {
            "hit" => h.play_feedback(false),
            "kill" => h.play_feedback(true),
            _ => h.print("hitfeedback_preview <hit|kill>\n"),
        }
    });
    c.add("hitmarker_test", |h, a| {
        h.feedback.trigger(h.realtime, a.argv(1) == "kill");
    });
    c.add("stuffcmds", |h, _| h.stuff_cmds());
    // Once per config (cl_paddefaults): fill in default pad binds whose key
    // is still unbound, so configs saved before the pad
    // defaults existed gain them without losing the player's own choices.
    c.add("_paddefaults_merge", |h, _| {
        if h.cv("cl_paddefaults") >= 1.0 {
            return;
        }
        for (key, cmd) in pad_default_binds() {
            if h.input.binding(key).is_none() {
                h.input.set_binding(key, Some(&cmd));
            }
        }
        h.server.core.cvars.set("cl_paddefaults", "1");
    });
    // Options > Controls > Reset: default.cfg's binds plus the pad defaults
    // (its cvars are left alone).
    c.add("_resetbinds", |h, _| {
        let text = h.fs().open("default.cfg").map_or_else(|| DEFAULT_CFG.to_string(), |d| String::from_utf8_lossy(&d).into_owned());
        let mut binds = String::from("unbindall\n");
        for l in text.lines() {
            let t = l.trim_start().to_ascii_lowercase();
            if t.starts_with("bind ") || t.starts_with("alias ") {
                binds.push_str(l);
                binds.push('\n');
            }
        }
        h.cbuf.insert_text(PAD_DEFAULTS);
        h.cbuf.insert_text(&binds);
    });
    c.add("cvarlist", |h, a| {
        let p = a.argv(1).to_ascii_lowercase();
        let mut l: Vec<String> = h
            .server
            .core
            .cvars
            .iter()
            .filter(|c| c.name.to_ascii_lowercase().starts_with(&p))
            .map(|c| format!("{}{} {} \"{}\"\n", if c.flags & CVAR_ARCHIVE != 0 { '*' } else { ' ' }, if c.flags & CVAR_SERVERINFO != 0 { 's' } else { ' ' }, c.name, c.string))
            .collect();
        l.sort_unstable_by_key(|s| s[3..].to_ascii_lowercase());
        let n = l.len();
        for s in l {
            h.print(&s);
        }
        h.print(&format!("{n} cvars\n"));
    });
    c.add("cmdlist", |h, a| {
        let mut l: Vec<String> = h.cmds.complete(a.argv(1)).into_iter().map(str::to_string).collect();
        l.extend(h.server.complete_command(a.argv(1)).into_iter().map(str::to_string));
        l.sort_unstable();
        for s in &l {
            h.print(&format!("   {s}\n"));
        }
        h.print(&format!("{} commands\n", l.len()));
    });
    c.add("quit", |h, _| h.quit = true);
    c.add("r_visibility", |h, a| {
        let message = h.r.debug_visibility(&a.all()[1..]);
        h.print(&message);
        if a.argv(1) == "isolate"
            && let Some(entity) = h.cl.entities.get(h.cl.viewentity)
        {
            let mut eye = entity.origin;
            eye[2] += h.cl.viewheight;
            h.print(&format!("Client eye {eye:?}, angles {:?}\n", h.cl.viewangles));
        }
    });
    c.add("screenshot", |h, a| h.screenshot = Some(a.argv(1).to_string()));
    c.add("toggleconsole", |h, _| h.toggle_console());
    c.add("togglemenu", |h, _| {
        if h.menu.active() {
            h.menu.screen = Screen::None;
        } else {
            h.menu.open(Screen::Main);
        }
    });
    c.add("menu_main", |h, _| h.menu.open(Screen::Main));
    c.add("menu_singleplayer", |h, _| h.menu.open(Screen::SinglePlayer));
    c.add("menu_quit", |h, _| h.menu.open(Screen::Quit));
    c.add("menu_options", |h, a| {
        use crate::menu::Page;
        let page = match a.argv(1).to_ascii_lowercase().as_str() {
            "gameplay" => Some(Page::Gameplay),
            "controls" => Some(Page::Controls),
            "input" => Some(Page::Input),
            "mouse" | "keyboard" => Some(Page::Mouse),
            "controller" | "gamepad" => Some(Page::Controller),
            "touch" | "touchscreen" => Some(Page::Touch),
            "autosave" => Some(Page::Autosave),
            "assists" => Some(Page::Assists),
            "autoaim" => Some(Page::AutoAim),
            "hud" => Some(Page::Hud),
            "hitsound" => Some(Page::HitSound),
            "killsound" => Some(Page::KillSound),
            "hudlayout" | "layout" => Some(Page::HudLayout),
            "messages" => Some(Page::Messages),
            "accessibility" => Some(Page::Accessibility),
            "graphics" => Some(Page::Graphics),
            "enhancements" => Some(Page::Enhancements),
            "impacts" => Some(Page::Impacts),
            "shader" | "screenshader" => Some(Page::ScreenShader),
            "audio" => Some(Page::Audio),
            "addons" => Some(Page::Addons),
            "newgame" => Some(Page::NewGameAddons),
            "portalgun" => Some(Page::PortalGun),
            "railgun" => Some(Page::Railgun),
            "hookshot" => Some(Page::Hookshot),
            _ => None,
        };
        if page == Some(Page::Graphics) {
            h.vid_menu();
        }
        h.menu.open(page.map_or(Screen::Options, Screen::OptionsPage));
    });
    c.add("clear", |h, _| h.con.lines.clear());
    c.add("bf", |h, _| h.view.bonus_flash());
    c.add("+showscores", |h, _| h.show_scores = true);
    c.add("-showscores", |h, _| h.show_scores = false);
    c.add("startdemos", |h, a| {
        h.demos = (1..a.argc()).map(|i| a.argv(i).to_string()).collect();
        h.print(&format!("{} demo(s) in loop\n", h.demos.len()));
        if !h.server.active() && !h.dedicated && !h.remote() && h.demonum != -1 && h.demo.is_none() {
            h.demonum = 0;
            if h.cv("cl_startdemos") == 0.0 {
                h.demonum = -1;
                return;
            }
            h.next_demo();
        } else {
            h.demonum = -1;
        }
    });
    c.add("demos", |h, _| {
        if h.demonum == -1 {
            h.demonum = 1;
        }
        h.stop_game();
        h.next_demo();
    });
    c.add("record", |h, a| h.record(a));
    c.add("stop", |h, _| {
        if h.recording.is_none() {
            h.print("Not recording a demo.\n");
        }
        h.stop_recording();
    });
    c.add("timedemo", |h, a| {
        if a.argc() != 2 {
            h.print("timedemo <demoname> : gets demo speeds\n");
            return;
        }
        h.play_demo(a.argv(1));
        if h.demo.is_some() {
            h.timedemo = Some((0, std::time::Instant::now()));
        }
    });
    c.add("stopdemo", |h, _| {
        if h.demo.is_some() {
            h.disconnect_client();
        }
    });
    c.add("playdemo", |h, a| {
        if a.argc() < 2 {
            h.print("playdemo <demoname> : plays a demo\n");
            return;
        }
        h.play_demo(a.argv(1));
    });
    for n in ["sizeup", "sizedown", "cd", "help", "scr_autoscale", "togglezoom", "messagemode", "messagemode2", "menu_multiplayer"] {
        c.add(n, |_, _| {});
    }
    c.add("cfg_unbindall", |_, _| {});
    c.add("fog", cmd_fog);
    c.add("r_partinfo", |h, _| {
        let s = h.fte.info(&h.cl);
        h.con.print(&s);
    });
    // Exercise the real impact path against the surface under the crosshair.
    c.add("impact_test", |h, a| {
        let kind = match a.argv(1) {
            "bullet" => 0, "nail" => 1, "explosion" => 2,
            _ => { h.con.print("impact_test bullet|nail|explosion\n"); return; }
        };
        let Some(bsp) = h.level.bsp.as_ref() else {return};
        let (forward, _, _) = qcore::math::angle_vectors(h.cl.viewangles.into());
        let eye = qcore::math::Vec3::from(h.cl.entities.get(h.cl.viewentity).map_or([0.0; 3], |e| e.origin))
            + qcore::math::Vec3::new(0.0, 0.0, h.cl.viewheight);
        let cm = qworld::CollisionModel::from_bsp(bsp);
        let tr = cm.hull(0, 0).trace(eye, eye + forward * 1024.0);
        if tr.fraction < 1.0 {
            h.cl.weapon_impacts.push((kind, (tr.endpos + tr.plane_normal * if kind==2 {8.0} else {0.0}).to_array()));
        }
    });
    // Cosmetic diagnostic: exercise the real blood path without a QC hit.
    c.add("blood_test", |h, a| {
        let distance = a.argv(1).parse::<f32>().unwrap_or(64.0).clamp(1.0, 1024.0);
        let (forward, _, _) = qcore::math::angle_vectors(h.cl.viewangles.into());
        let eye = qcore::math::Vec3::from(h.cl.entities.get(h.cl.viewentity).map_or([0.0; 3], |e| e.origin))
            + qcore::math::Vec3::new(0.0, 0.0, h.cl.viewheight);
        h.blood.debug_hits.push((eye + forward * distance, a.argv(2) == "burst"));
    });
    // Debug: run an FTE effect 128 units in front of the view.
    c.add("fte_spawn", |h, a| {
        if a.argc() < 2 {
            h.con.print("fte_spawn <effect> [count]\n");
            return;
        }
        let (f, _, _) = qcore::math::angle_vectors(qcore::math::Vec3::from(h.cl.viewangles));
        let e = &h.cl.entities.get(h.cl.viewentity).map_or([0.0; 3], |e| e.origin);
        let org = [e[0] + f.x * 128.0, e[1] + f.y * 128.0, e[2] + 22.0 + f.z * 128.0];
        let count = if a.argc() > 2 { qpart::num::atof(a.argv(2)) } else { 1.0 };
        h.cl.fte_queue.push(qclient::FteRequest::Effect { name: a.argv(1).to_string(), org, dir: None, count });
    });
    // Run a client command as split-screen player 2..6 (setpos, give, god...).
    c.add("splitcmd", |h, a| {
        let n = a.argv(1).parse::<usize>().unwrap_or(0);
        let Some(g) = n.checked_sub(2).and_then(|k| h.guests.get(k)) else {
            h.print("splitcmd <player 2..6> <command>\n");
            return;
        };
        let slot = g.slot;
        let rest = a.args().split_once(char::is_whitespace).map_or("", |(_, r)| r).trim().to_string();
        h.server.client_command(slot, &rest);
        h.drain_server_log();
    });
    // Host_Name_f / Host_Color_f: set the archived cvar, then tell the server.
    c.add("name", |h, a| {
        if a.argc() < 2 {
            let n = h.server.core.cvars.by_name("_cl_name").map(|c| c.string.clone()).unwrap_or_default();
            h.print(&format!("\"name\" is \"{n}\"\n"));
            return;
        }
        let n = if a.argc() == 2 { a.argv(1).to_string() } else { a.args().to_string() };
        h.server.core.cvars.set("_cl_name", &n);
        h.send_userinfo(&format!("name \"{n}\""));
    });
    c.add("color", |h, a| {
        if a.argc() < 2 {
            let c = h.cv("_cl_color") as i32;
            h.print(&format!("\"color\" is \"{} {}\"\ncolor <0-13> [0-13]\n", c >> 4, c & 15));
            return;
        }
        let p = |i: usize| a.argv(i).parse::<f32>().unwrap_or(0.0) as i32 & 15;
        let top = p(1).min(13);
        let bottom = if a.argc() == 2 { top } else { p(2).min(13) };
        h.server.core.cvars.set("_cl_color", &(top * 16 + bottom).to_string());
        h.send_userinfo(&format!("color {top} {bottom}"));
    });
    c.add("vid_apply", |h, _| h.vid_apply());
    c.add("vid_restart", |h, _| h.vid_apply());
    c.add("_vid_menu", |h, _| h.vid_menu());
    c.add("force_centerview", |h, _| h.cl.viewangles[0] = 0.0);
    c.add("centerview", |h, _| h.cl.viewangles[0] = 0.0);
    c.add("menu_load", |h, _| {
        h.menu.autosaves = false;
        h.menu.open(Screen::Load);
        h.scan_saves();
    });
    c.add("menu_autosaves", |h, _| {
        h.menu.autosaves = true;
        h.menu.open(Screen::Load);
        h.scan_saves();
    });
    c.add("menu_save", |h, _| {
        h.menu.autosaves = false;
        if h.server.active() {
            h.menu.open(Screen::Save);
            h.scan_saves();
        }
    });
    c.add("pause", |h, _| {
        if h.server.active() {
            let p = !h.server.core.sv.paused;
            h.server.core.sv.paused = p;
            h.cl.paused = p;
            h.print(if p { "paused\n" } else { "unpaused\n" });
        }
    });
    c.add("disconnect", |h, _| h.stop_game());
    c.add("connect", |h, a| {
        if a.argc() < 2 {
            h.print("usage: connect <server>\n");
            return;
        }
        // COM_Parse splits "host:port" at the colon.
        let addr: String = a.args().split_whitespace().collect();
        if addr.starts_with("qw://") {
            h.connect_qw(&addr);
        } else {
            h.connect_remote(&addr);
        }
    });
    // Cmd_ForwardToServer_f.
    c.add("cmd", |h, a| {
        if !h.remote() && h.local.is_none() {
            h.print("Can't \"cmd\", not connected\n");
            return;
        }
        if a.args().trim() == "pext" {
            // The server's extension query: reply with what we support.
            let p2 = if h.cv("cl_nopext") != 0.0 { 0 } else { qclient::delta::PEXT2_SUPPORTED };
            let s = format!(
                "pext {:#x} 0 {:#x} {:#x}",
                qclient::delta::PROTOCOL_FTE_PEXT1,
                qclient::delta::PROTOCOL_FTE_PEXT2,
                p2
            );
            h.cl.send_string_cmd(&s);
        } else if a.argc() > 1 {
            let s = a.args().to_string();
            h.cl.send_string_cmd(&s);
        }
    });
    c.add("qwconnect", |h, a| {
        if a.argc() < 2 {
            h.print("usage: qwconnect <server[:port]>\n");
            return;
        }
        let addr: String = a.args().split_whitespace().collect();
        h.connect_qw(&addr);
    });
    // CL_Reconnect_f: our client restarts the signon on the next serverinfo
    // by itself, so only a dropped connection needs action.
    c.add("reconnect", |h, _| {
        if !h.remote()
            && h.local.is_none()
            && let Some((a, qw)) = h.last_server
        {
            if qw {
                h.connect_qw(&a.to_string());
            } else {
                h.connect_remote(&a.to_string());
            }
        }
    });
    c.add("menu_multiplayer", |h, a| {
        let page = a.argv(1).to_ascii_lowercase();
        if let Some(Action::Command(c)) = h.menu.open_multiplayer(&page) {
            h.cbuf.add_text(&c);
        }
    });
    c.add("slist", |h, _| {
        if h.lan_search.is_some() {
            return;
        }
        match crate::net::LanSearch::start(h.net_port, h.realtime) {
            Ok(ls) => {
                h.print("Looking for Quake servers...\n");
                h.lan_search = Some(ls);
                h.menu.servers.clear();
                h.menu.searching = true;
            }
            Err(e) => h.print(&format!("slist: {e}\n")),
        }
    });
    // Maps for the Host Game page: the running game dir's own maps.
    c.add("_netmaps", |h, _| {
        let base = h.fs().basedir().to_path_buf();
        let game = h.game_name().to_string();
        h.menu.net_maps = qui::mods::maps(&base, &game);
    });
    c.add("port", |h, a| {
        if a.argc() < 2 {
            h.print(&format!("\"port\" is \"{}\"\n", h.net_port));
            return;
        }
        match a.argv(1).parse::<u16>() {
            Ok(p) if p > 0 => {
                h.net_port = p;
                if let Some(mut n) = h.net_server.take() {
                    n.shutdown(&mut h.server, h.realtime);
                }
            }
            _ => h.print("Bad value, must be between 1 and 65534\n"),
        }
    });
    c.add("music", |h, a| {
        if let Some(s) = &mut h.snd
            && !s.play_music(a.argv(1), true)
        {
            h.con.print(&format!("Couldn't handle music file {}\n", a.argv(1)));
        }
    });
    c.add("music_stop", |h, _| {
        if let Some(s) = &h.snd {
            s.stop_music();
        }
    });
    c.add("delay", |h, a| {
        if a.argc() < 3 {
            h.print("delay <seconds> <command...> : run a command later\n");
            return;
        }
        let t = h.realtime + a.argv(1).parse::<f64>().unwrap_or(0.0);
        let cmd = a.all()[2..].iter().map(|s| if s.contains(' ') { format!("\"{s}\"") } else { s.clone() }).collect::<Vec<_>>().join(" ");
        h.delayed.push((t, cmd));
    });
    c.add("version", |h, _| h.print("quake-source-port 0.1.0\n"));
    super::death::register(c);
    c.add("_scansaves", |h, _| h.scan_saves());
    c.add("_mods", |h, _| h.scan_mods());
    c.add("_modmaps", |h, a| h.scan_mod_maps(a.argv(1)));
    // menu_mods [dir]: the mod list, or one mod's maps
    c.add("menu_mods", |h, a| {
        h.scan_mods();
        if a.argc() > 1 && h.menu.mods.select(a.argv(1)) {
            h.scan_mod_maps(a.argv(1));
            h.menu.open(Screen::ModMaps);
        } else {
            h.menu.open(Screen::Mods);
        }
    });
    c.add("game", |h, a| {
        if a.argc() < 2 {
            let g = h.game_name().to_string();
            h.print(&format!("\"game\" is \"{g}\"\ngame <dir> [map] : switch to another mod\n"));
            return;
        }
        let map = (a.argc() > 2).then(|| a.argv(2).to_string());
        h.switch_game(a.argv(1), map.as_deref());
    });
    c.add("save", |h, a| {
        if a.argc() != 2 {
            h.print("save <savename> : save a game\n");
            return;
        }
        if !h.server.active() || h.cl.intermission != 0 || h.cl.stats[STAT_HEALTH] <= 0 {
            h.print("Can't save now.\n");
            return;
        }
        let text = h.server.save_game();
        let p = h.savegame_path(a.argv(1));
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        match std::fs::write(&p, qcore::text::encode(&text)) {
            Ok(()) => {
                // the slot is one save: drop a same-named file of the other format
                if let (Some(d), Some(stem)) = (p.parent(), p.file_stem().and_then(|s| s.to_str())) {
                    let _ = crate::autosave::remove_other_exts(d, stem, p.extension().and_then(|e| e.to_str()));
                }
                h.print(&format!("Saved {}\n", p.display()));
            }
            Err(e) => h.print(&format!("ERROR: couldn't write {}: {e}\n", p.display())),
        }
    });
    c.add("load", |h, a| {
        if a.argc() != 2 {
            h.print("load <savename> : load a game\n");
            return;
        }
        let Some(p) = h.find_save(a.argv(1)) else {
            h.print(&format!("ERROR: couldn't open {}\n", h.savegame_path(a.argv(1)).display()));
            return;
        };
        let Ok(bytes) = std::fs::read(&p) else {
            h.print(&format!("ERROR: couldn't open {}\n", p.display()));
            return;
        };
        let text = qcore::text::decode(&bytes);
        h.print(&format!("Loading game from {}...\n", p.display()));
        if let Some(i) = h.local.take() {
            h.server.drop_client(i, false);
        }
        h.disconnect_client();
        if let Err(e) = h.server.load_game(&text) {
            h.print(&format!("load failed: {e}\n"));
        }
        h.drain_server_log();
    });
}
