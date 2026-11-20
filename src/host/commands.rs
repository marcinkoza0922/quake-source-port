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
            h.print(&qcore::ctr!("execing config.cfg (inherited from {})\n", g));
            if let Some(d) = h.fs().open(name).filter(|_| h.fs().origin(name).is_some_and(|o| o.game.eq_ignore_ascii_case(h.game_name()))) {
                h.cbuf.insert_text(&String::from_utf8_lossy(&d));
                h.print(&qcore::ctr!("execing config.cfg (mod defaults)\n"));
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
                h.print(&qcore::ctr!("execing {}\n", name));
            }
            None if name.eq_ignore_ascii_case("default.cfg") => {
                h.cbuf.insert_text(PAD_DEFAULTS);
                h.cbuf.insert_text(DEFAULT_CFG);
                h.print(&qcore::ctr!("execing default.cfg (built-in)\n"));
            }
            None => h.print(&qcore::ctr!("couldn't exec {}\n", name)),
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
        if h.cv("cl_addonslotdefaults") < 1.0 {
            for (key, cmd) in [('9' as i32, "weapon_slot 9"), ('0' as i32, "weapon_slot 0")] {
                if h.input.binding(key).is_none_or(|b| b == "impulse 0") {
                    h.input.set_binding(key, Some(cmd));
                }
            }
            h.server.core.cvars.set("cl_addonslotdefaults", "1");
        }
        if h.cv("cl_automapbind") < 1.0 {
            if h.input.binding(b'm' as i32).is_none() {
                h.input.set_binding(b'm' as i32, Some("automap"));
            }
            h.server.core.cvars.set("cl_automapbind", "1");
        }
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
        h.print(&qcore::ctr!("{} cvars\n", n));
    });
    c.add("cmdlist", |h, a| {
        let mut l: Vec<String> = h.cmds.complete(a.argv(1)).into_iter().map(str::to_string).collect();
        l.extend(h.server.complete_command(a.argv(1)).into_iter().map(str::to_string));
        l.sort_unstable();
        for s in &l {
            h.print(&format!("   {s}\n"));
        }
        h.print(&qcore::ctr!("{} commands\n", l.len()));
    });
    c.add("quit", |h, _| h.quit = true);
    super::render_commands::register(c);
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
    // scripted menu input for headless screenshots: key names as `bind`
    // takes them (`menu_keys DOWNARROW TAB ENTER`), or typed text
    c.add("menu_keys", |h, a| {
        for i in 1..a.argc() {
            let k = qinput::keys::string_to_keynum(a.argv(i));
            if k >= 0 && h.menu.active() {
                h.menu_key(k);
            }
        }
        h.sync_key_dest();
    });
    c.add("menu_text", |h, a| {
        for ch in a.args().trim_matches('"').chars() {
            h.menu.text(ch, &mut h.server.core.cvars);
        }
    });
    c.add("menu_help", |h, a| h.menu.open_help(a.argv(1)));
    c.add("menu_singleplayer", |h, _| h.menu.open(Screen::SinglePlayer));
    c.add("menu_newgame", |h, _| h.menu.open_new_game());
    c.add("menu_quit", |h, _| h.menu.open(Screen::Quit));
    c.add("menu_hudedit", |h, _| {
        h.menu.open(Screen::HudEditor);
        h.menu.hud_edit.open(&h.server.core.cvars);
    });
    c.add("hud_preset_save", |h, a| {
        let name = a.args().trim();
        if name.is_empty() {
            h.print("usage: hud_preset_save <name>\n");
            return;
        }
        let layout = h.cvs(crate::hud::editor::CVAR).trim().to_string();
        let layout = if matches!(layout.as_str(), "" | "default" | "classic") {
            h.hud.layout.to_text()
        } else {
            layout
        };
        if let Some(i) = crate::hud::presets::upsert(&mut h.menu.hud_edit.presets, name, &layout) {
            let saved = h.menu.hud_edit.presets[i].clone();
            if let Err(e) = crate::hud::presets::write_one(&saved) {
                h.print(&format!("couldn't save HUD preset: {e}\n"));
            }
            h.server.core.cvars.set(crate::hud::editor::PRESET, "2");
            h.server.core.cvars.set(crate::hud::presets::PRESET_NAME, &saved.name);
            h.menu.hud_edit.editing_preset = Some(saved.name.clone());
            h.print(&format!("Saved HUD preset \"{}\"\n", saved.name));
        } else {
            h.print("Invalid or reserved HUD preset name\n");
        }
    });
    c.add("hud_preset_load", |h, a| {
        let name = a.args().trim();
        if name.is_empty() {
            h.print("usage: hud_preset_load <name>\n");
            return;
        }
        if name.eq_ignore_ascii_case("modern") || name.eq_ignore_ascii_case("default") {
            h.server.core.cvars.set(crate::hud::editor::CVAR, "default");
            h.server.core.cvars.set(crate::hud::editor::PRESET, "0");
            h.server.core.cvars.set(crate::hud::presets::PRESET_NAME, "");
        } else if name.eq_ignore_ascii_case("classic") {
            h.server.core.cvars.set(crate::hud::editor::CVAR, "classic");
            h.server.core.cvars.set(crate::hud::editor::PRESET, "1");
            h.server.core.cvars.set(crate::hud::presets::PRESET_NAME, "");
        } else if let Some(p) = crate::hud::presets::find(&h.menu.hud_edit.presets, name).cloned() {
            h.server.core.cvars.set(crate::hud::editor::CVAR, &p.layout);
            h.server.core.cvars.set(crate::hud::editor::PRESET, "2");
            h.server.core.cvars.set(crate::hud::presets::PRESET_NAME, &p.name);
        } else {
            h.print(&format!("Unknown HUD preset \"{name}\"\n"));
        }
    });
    c.add("hud_preset_delete", |h, a| {
        let name = a.args().trim();
        if name.is_empty() {
            h.print("usage: hud_preset_delete <name>\n");
            return;
        }
        if crate::hud::presets::is_reserved(name) {
            h.print("Cannot delete built-in HUD preset\n");
            return;
        }
        if crate::hud::presets::remove(&mut h.menu.hud_edit.presets, name) {
            if let Err(e) = crate::hud::presets::delete_one(name) {
                h.print(&format!("couldn't delete HUD preset: {e}\n"));
            }
            if h.cvs(crate::hud::presets::PRESET_NAME).eq_ignore_ascii_case(name) {
                h.server.core.cvars.set(crate::hud::presets::PRESET_NAME, "");
            }
            if h.menu.hud_edit.editing_preset.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
                h.menu.hud_edit.editing_preset = None;
            }
            h.print(&format!("Deleted HUD preset \"{name}\"\n"));
        } else {
            h.print(&format!("Unknown HUD preset \"{name}\"\n"));
        }
    });
    c.add("hud_preset_list", |h, _| {
        h.print("Built-in: modern, classic\n");
        if h.menu.hud_edit.presets.is_empty() {
            h.print("Saved: (none)\n");
        } else {
            let names: Vec<&str> = h.menu.hud_edit.presets.iter().map(|p| p.name.as_str()).collect();
            h.print(&format!("Saved: {}\n", names.join(", ")));
        }
    });
    c.add("automap", |h, a| {
        if h.cl.signon != 4 || h.cl.intermission != 0 || h.cl.gametype == 1 || h.cv("deathmatch") != 0.0 {
            h.automap.close_fullscreen();
            return;
        }
        let pos = h.player_origin();
        let yaw = h.cl.viewangles[1];
        match a.argv(1) {
            "0" | "off" => h.automap.close_fullscreen(),
            "1" | "on" => h.automap.open_fullscreen(pos, yaw, &h.server.core.cvars),
            _ => h.automap.toggle_fullscreen(pos, yaw, &h.server.core.cvars),
        }
        h.sync_key_dest();
    });
    c.add("menu_options", |h, a| {
        use crate::menu::Page;
        let page = match a.argv(1).to_ascii_lowercase().as_str() {
            "gameplay" => Some(Page::Gameplay),
            "controls" => Some(Page::Controls),
            "input" => Some(Page::Input),
            "mouse" | "keyboard" => Some(Page::Mouse),
            "controller" | "gamepad" => Some(Page::Controller),
            "gyro" | "flick" => Some(Page::Gyro),
            "touch" | "touchscreen" => Some(Page::Touch),
            "autosave" => Some(Page::Autosave),
            "weaponpriority" | "weapons" => Some(Page::WeaponPriority),
            "automap" | "map" => Some(Page::Automap),
            "customize" => Some(Page::Customize),
            "newgamecustomize" => Some(Page::NewGameCustomize),
            "modifiers" => Some(Page::Modifiers),
            "autoaim" => Some(Page::AutoAim),
            "grenadearc" => Some(Page::GrenadeArc),
            "hud" => Some(Page::Hud),
            "crosshair" => Some(Page::Crosshair),
            "hitsound" => Some(Page::HitSound),
            "killsound" => Some(Page::KillSound),
            "accessibility" => Some(Page::Accessibility),
            "graphics" => Some(Page::Graphics),
            "enhancements" => Some(Page::Enhancements),
            "water" => Some(Page::Water),
            "bloom" => Some(Page::Bloom),
            "rtlights" => Some(Page::RtLights),
            "impacts" => Some(Page::Impacts),
            "blood" => Some(Page::Blood),
            "debris" => Some(Page::Debris),
            "shader" | "screenshader" => Some(Page::ScreenShader),
            "audio" => Some(Page::Audio),
            "audiomix" | "mix" => Some(Page::AudioMix),
            "demos" => Some(Page::Demos),
            "video" => Some(Page::Video),
            "addons" => Some(Page::Addons),
            "newgame" => Some(Page::NewGameAddons),
            "portalgun" => Some(Page::PortalGun),
            "railgun" => Some(Page::Railgun),
            "gel" => Some(Page::Gel),
            "washer" => Some(Page::Washer),
            "hookshot" => Some(Page::Hookshot),
            "cable" => Some(Page::Cable),
            "boomerang" => Some(Page::Boomerang),
            "goggles" => Some(Page::Goggles),
            "projectileshotguns" => Some(Page::ProjectileShotguns),
            "bullettime" => Some(Page::BulletTime),
            "hitstop" => Some(Page::Hitstop),
            "sticky" => Some(Page::Sticky),
            _ => None,
        };
        if page == Some(Page::Graphics) {
            h.vid_menu();
        }
        h.menu.open(page.map_or(Screen::Options, Screen::OptionsPage));
    });
    c.add("clear", |h, _| h.con.clear_scrollback());
    c.add("con_type", |h, a| {
        h.sync_console_catalog();
        for ch in a.args().chars() {
            h.con.text(ch);
        }
    });
    for sh_name in ["bash", "sh"] {
        c.add(sh_name, |h, a| {
            h.sync_shell_env();
            if a.argc() == 1 {
                h.con.start_pty();
            } else {
                let cmd = if a.argv(1) == "-c" && a.argc() >= 3 {
                    a.all()[2..].join(" ")
                } else {
                    a.args().to_string()
                };
                h.con.run_shell_cmd(&cmd, None);
            }
        });
    }
    c.add("bf", |h, _| h.view.bonus_flash());
    super::compat_cmds::register(c);
    super::freecam::register(c);
    c.add("+showscores", |h, _| h.show_scores = true);
    c.add("-showscores", |h, _| h.show_scores = false);
    super::demo::register(c);
    super::demo_play::register(c);
    super::autodemo::register(c);
    super::capture::register(c);
    for n in ["sizeup", "sizedown", "cd", "help", "scr_autoscale", "togglezoom", "messagemode", "messagemode2", "menu_multiplayer"] {
        c.add(n, |_, _| {});
    }
    c.add("cfg_unbindall", |_, _| {});
    c.add("fog", cmd_fog);
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
    // Debug/headless: hand split-screen player 2..6 a toy (index into
    // `Toy::ALL`; anything else puts it away).
    c.add("splittoy", |h, a| {
        let n = a.argv(1).parse::<usize>().unwrap_or(0);
        let toy = a.argv(2).parse::<usize>().ok().and_then(|k| qserver::addons::Toy::ALL.get(k).copied());
        let allowed = h.allowed_toys();
        match n.checked_sub(2).and_then(|k| h.guests.get_mut(k)) {
            Some(g) => g.toy.held = toy.filter(|t| allowed.contains(t)),
            None => h.print("splittoy <player 2..6> <toy 0..6>\n"),
        }
    });
    // Debug/headless (tools/qprof co-op): drive split-screen player 2..6's
    // input: `splitbtn <n> +attack|-dodge|...` or `splitbtn <n> impulse 7`.
    c.add("splitbtn", |h, a| {
        let n = a.argv(1).parse::<usize>().unwrap_or(0);
        let Some(g) = n.checked_sub(2).and_then(|k| h.guests.get_mut(k)) else {
            return h.print("splitbtn <player 2..6> <+button|-button|impulse N>\n");
        };
        let what = a.argv(2);
        if what == "impulse" {
            g.input.set_impulse(a.argv(3).parse().unwrap_or(0));
            return;
        }
        let (down, name) = match what.split_at_checked(1) {
            Some(("+", n)) => (true, n),
            Some(("-", n)) => (false, n),
            _ => return h.print("splitbtn: expected +button or -button\n"),
        };
        // A fixed fake key so press and release pair up.
        const KEY: i32 = 4242;
        match qinput::btn::NAMES.iter().position(|b| *b == name) {
            Some(i) if down => g.input.buttons[i].press(KEY),
            Some(i) => g.input.buttons[i].release(KEY),
            None => h.print(&format!("splitbtn: unknown button {name}\n")),
        }
    });
    // Debug/headless: hand player 1 a toy (index into `Toy::ALL`; anything
    // else puts it away).
    // tools/qprof: frame profiling (see host/prof.rs).
    c.add("prof_start", |h, _| {
        h.r.gpu_times_wanted = true;
        let _ = h.r.take_gpu_times();
        h.prof = Some(super::prof::Profiler::new());
    });
    c.add("prof_stop", |h, a| {
        let Some(p) = h.prof.take() else { return h.print("prof_stop: not recording\n") };
        let views = h.r.gpu_view_times();
        let destinations = h.r.gpu_destination_times();
        let json = p.report(h.r.take_gpu_times(), &views, &destinations);
        let path = a.argv(1);
        if path.is_empty() {
            h.print(&json);
        } else if let Err(e) = qcore::vfs::write(std::path::Path::new(path), json.as_bytes()) {
            h.print(&format!("prof_stop: {path}: {e}\n"));
        }
    });
    c.add("toy", |h, a| {
        let toy = a.argv(1).parse::<usize>().ok().and_then(|k| qserver::addons::Toy::ALL.get(k).copied());
        let allowed = h.allowed_toys();
        h.toy.held = toy.filter(|t| allowed.contains(t));
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
    super::saves::register(c);
    // Host_Pause_f: a playing demo pauses itself (ericw); otherwise the
    // server decides (`pausable`) and answers with svc_setpause.
    c.add("pause", |h, _| {
        if let Some(d) = &mut h.demo {
            d.paused = !d.paused;
        } else if h.remote() {
            h.cl.send_string_cmd("pause");
        } else if let Some(i) = h.local.filter(|_| h.server.active()) {
            h.server.client_command(i, "pause");
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
            h.print(&qcore::ctr!("Can't \"cmd\", not connected\n"));
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
                h.print(&qcore::ctr!("Looking for Quake servers...\n"));
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
            _ => h.print(&qcore::ctr!("Bad value, must be between 1 and 65534\n")),
        }
    });
    c.add("music", |h, a| {
        if let Some(s) = &mut h.snd
            && !s.play_music(a.argv(1), true)
        {
            h.con.print(&qcore::ctr!("Couldn't handle music file {}\n", a.argv(1)));
        }
    });
    c.add("music_track", |h, a| {
        let Ok(track) = a.argv(1).parse::<u8>() else {
            h.print("usage: music_track <track number>\n");
            return;
        };
        if !h.play_cd_track(track, true) {
            h.con.print(&qcore::ctr!("Couldn't handle music file {}\n", format!("track{track:02}")));
        }
    });
    c.add("soundtrack", |h, a| {
        h.refresh_soundtracks();
        if a.argc() < 2 {
            let cur = h.server.core.cvars.by_name("s_soundtrack").map_or("", |c| c.string.trim()).to_owned();
            let shown = if cur.is_empty() { "default" } else { cur.as_str() };
            let mut msg = format!("Soundtrack: {shown}\nAvailable: default");
            for p in h.audio_fx.soundtracks.packs() {
                msg.push_str(&format!(", {} ({})", p.id, p.label));
            }
            msg.push('\n');
            h.print(&msg);
            return;
        }
        let arg = a.argv(1);
        let val = if arg.eq_ignore_ascii_case("default") || arg.eq_ignore_ascii_case("auto") { "" } else { arg };
        h.server.core.cvars.set("s_soundtrack", val);
    });
    c.add("music_stop", |h, _| {
        h.audio_fx.soundtracks.clear_track();
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
    super::mapswitch::register(c);
    c.add("bugreport", |h, a| {
        let text = (1..a.argc()).map(|i| a.argv(i)).collect::<Vec<_>>().join(" ");
        if text.trim().is_empty() {
            h.print("bugreport \"description\" : save a report (json, save, config, screenshot) for developers\n");
            return;
        }
        h.bugreport(&text);
    });
}
