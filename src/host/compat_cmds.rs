//! Console commands other engines (ironwail, vkQuake) have that mods use:
//! QC `stuffcmd`s them (AD, Alkaline, sewerjam2, qbj3, Mjolnir, the jams)
//! or their configs set them, plus the references' mapper/debug commands
//! (`mapname`, `maps`, `path`, `find`/`apropos`, `entities`, `pointfile`).

use super::*;

pub(super) fn register(c: &mut Commands<Host>) {
    super::quaddicted_cli::register(c);
    super::ref_cmds::register(c);
    // Sky_SkyCommand_f: QC stuffs `sky <name>` to change the skybox (AD,
    // Alkaline, sewerjam2); lasts until the next map loads its own.
    c.add("sky", |h, a| match a.argc() {
        1 => h.print(&format!("\"sky\" is \"{}\"\n", h.level.sky)),
        2 => {
            let name = a.argv(1).to_string();
            let fs = &h.server.core.fs;
            h.level.set_sky(fs, &mut h.r, &name);
            if h.level.sky.is_empty() && !name.is_empty() {
                h.print(&format!("Couldn't load gfx/env/{name}\n"));
            }
        }
        _ => h.print("usage: sky <skyname>\n"),
    });
    register_skywind(c);
    // ironwail's HUD style cvar is `hudstyle` (0 classic, 1/2 modern
    // variants, 3 QuakeWorld); qbj3's QC stuffs it from an impulse. It
    // picks our classic or modern preset; a custom HUD only gives way to
    // classic.
    c.add("hudstyle", |h, a| {
        let preset = h.cv("hud_preset") as i32;
        if a.argc() < 2 {
            h.print(&format!("\"hudstyle\" is \"{}\" (hud_preset)\n", i32::from(preset != 1)));
            return;
        }
        let v = qcore::cvar::q_atof(a.argv(1)) as i32;
        if v == 0 {
            h.exec_line("hud_preset 1");
        } else if preset == 1 {
            h.exec_line("hud_preset 0");
        }
    });
    // S_Play / S_PlayVol / S_StopAllSoundsC: QC stuffs these too
    // (func_mapjam3, Mjolnir). A missing extension means ".wav".
    c.add("play", |h, a| {
        for i in 1..a.argc() {
            h.play_console_sound(a.argv(i), 1.0);
        }
    });
    c.add("playvol", |h, a| {
        for i in (1..a.argc()).step_by(2) {
            h.play_console_sound(a.argv(i), qcore::cvar::q_atof(a.argv(i + 1)));
        }
    });
    c.add("stopsound", |h, _| {
        if let Some(s) = &h.snd {
            s.event(qsound::SoundEvent::StopAll);
        }
    });
    c.add("mapname", |h, _| {
        let name = if h.server.active() { h.server.core.sv.name.clone() } else { h.cl.mapname.clone() };
        if name.is_empty() {
            h.print("no map loaded\n");
        } else {
            h.print(&format!("\"mapname\" is \"{name}\"\n"));
        }
    });
    // Host_Maps_f: every map in the search path, optionally filtered.
    c.add("maps", |h, a| {
        let filter = a.argv(1).to_ascii_lowercase();
        let mut names: Vec<String> = h
            .server
            .core
            .fs
            .list("maps/", ".bsp")
            .iter()
            .filter_map(|p| p.get(5..p.len() - 4).map(str::to_ascii_lowercase))
            .filter(|n| !n.contains('/') && n.contains(&filter))
            .collect();
        names.sort();
        names.dedup();
        let mut t: String = names.iter().map(|n| format!("   {n}\n")).collect();
        t += &match (names.len(), filter.is_empty()) {
            (0, false) => format!("no maps contain '{filter}'\n"),
            (n, false) => format!("{n} map{} containing '{filter}'\n", if n == 1 { "" } else { "s" }),
            (n, true) => format!("{n} map{}\n", if n == 1 { "" } else { "s" }),
        };
        h.print(&t);
    });
    // COM_Path_f
    c.add("path", |h, _| {
        let mut t = String::from("Current search path:\n");
        for sp in h.server.core.fs.search_paths() {
            if let Some(p) = sp.pack() {
                t += &format!("{} ({} files)\n", p.path.display(), p.names().len());
            } else if let Some(d) = sp.dir() {
                t += &format!("{}\n", d.display());
            }
        }
        h.print(&t);
    });
    // Cmd_Apropos_f
    for name in ["find", "apropos"] {
        c.add(name, |h, a| {
            let sub = a.argv(1).to_ascii_lowercase();
            if sub.is_empty() {
                h.print(&format!("{} <substring> : search through commands and cvars for the given substring\n", a.argv(0)));
                return;
            }
            let mut cmds: Vec<String> = h.cmds.complete("").into_iter().map(str::to_string).collect();
            cmds.extend(h.server.complete_command("").into_iter().map(str::to_string));
            cmds.retain(|c| c.to_ascii_lowercase().contains(&sub) && !c.starts_with('_'));
            cmds.sort_unstable();
            cmds.dedup();
            let mut t: String = cmds.iter().map(|c| format!("   {c}\n")).collect();
            let mut hits = cmds.len();
            let mut vars: Vec<(String, String)> = h
                .server
                .core
                .cvars
                .iter()
                .filter(|c| c.name.to_ascii_lowercase().contains(&sub))
                .map(|c| (c.name.clone(), c.string.clone()))
                .collect();
            vars.sort_unstable();
            for (n, v) in &vars {
                t += &format!("   {n} (current value: \"{v}\")\n");
            }
            hits += vars.len();
            let pl = if hits == 1 { "" } else { "s" };
            t += &if hits == 0 {
                format!("no cvars/commands contain '{sub}'\n")
            } else {
                format!("{hits} cvar{pl}/command{pl} containing '{sub}'\n")
            };
            h.print(&t);
        });
    }
    // CL_PrintEntities_f
    c.add("entities", |h, _| {
        if h.cl.signon == 0 {
            return;
        }
        let mut t = String::new();
        for (i, e) in h.cl.entities.iter().enumerate() {
            match h.cl.model_precache.get(e.model).filter(|m| e.model != 0 && !m.is_empty()) {
                None => t += &format!("{i:3}:EMPTY\n"),
                Some(m) => {
                    let (o, a) = (e.origin, e.angles);
                    t += &format!(
                        "{i:3}:{m}:{:2}  ({:5.1},{:5.1},{:5.1}) [{:5.1} {:5.1} {:5.1}]\n",
                        e.frame, o[0], o[1], o[2], a[0], a[1], a[2]
                    );
                }
            }
        }
        h.print(&t);
    });
    // pr_edict.c dumps of the local server's edicts
    c.add("edict", |h, a| {
        if let Some(t) = h.server.edict_text(qcore::cvar::q_atof(a.argv(1)) as i32) {
            h.print(&t);
        }
    });
    c.add("edicts", |h, _| {
        if let Some(t) = h.server.edicts_text() {
            h.print(&t);
        }
    });
    c.add("edictcount", |h, _| {
        if let Some(t) = h.server.edictcount_text() {
            h.print(&t);
        }
    });
    // R_ReadPointFile_f: `pointfile [leak]` shows maps/<map>.pts.
    c.add("pointfile", |h, a| {
        h.bboxes.pointfile.clear();
        if h.cl.signon == 0 || h.cl.mapname.is_empty() {
            return;
        }
        let name = format!("maps/{}.pts", h.cl.mapname);
        let leak = a.argv(1) == "leak";
        let Some(data) = h.server.core.fs.open(&name) else {
            h.print(&format!("couldn't open {name}\n"));
            return;
        };
        if !leak {
            h.print(&format!("Reading {name}...\n"));
        }
        let (read, path) = crate::bboxes::parse_pointfile(&String::from_utf8_lossy(&data));
        let n = path.len();
        h.bboxes.pointfile = path;
        if leak {
            h.print("map appears to have leaks!\n");
        } else {
            h.print(&format!("{read} points read ({n} significant)\n"));
        }
    });
    c.add("v_cshift", |h, a| {
        let n = |i| qcore::cvar::q_atof(a.argv(i)) as i32;
        h.view.set_empty_shift(n(1), n(2), n(3), n(4));
    });
    // SCR_SizeUp_f / SCR_SizeDown_f (ironwail gl_screen.c): adjust viewsize by 10, clamped 30..=130.
    c.add("sizeup", |h, _| adjust_viewsize(h, 10.0));
    c.add("sizedown", |h, _| adjust_viewsize(h, -10.0));
    // SCR_ToggleZoom_f / SCR_ZoomDown_f / SCR_ZoomUp_f (ironwail gl_screen.c).
    // (there is one zoom: player 1's)
    c.add("togglezoom", |h, _| {
        if h.input_target.is_some() {
            return;
        }
        h.zoom.1 = if h.zoom.1 != 0.0 {
            -h.zoom.1
        } else if h.zoom.0 > 0.5 {
            -1.0
        } else {
            1.0
        };
    });
    c.add("+zoom", |h, _| zoom(h, 1.0));
    c.add("-zoom", |h, _| zoom(h, -1.0));
    // Con_MessageMode_f / Con_MessageMode2_f (ironwail console.c).
    c.add("messagemode", |h, _| start_message_mode(h, false));
    c.add("messagemode2", |h, _| start_message_mode(h, true));
    // CD_f (vkQuake cd_sdl.c): routes to soundtrack/CD music playback.
    c.add("cd", cmd_cd);
}

fn zoom(h: &mut Host, dir: f32) {
    if h.input_target.is_none() {
        h.zoom.1 = dir;
    }
}

fn adjust_viewsize(h: &mut Host, delta: f32) {
    let next = (h.cv("viewsize") + delta).clamp(30.0, 130.0);
    h.server.core.cvars.set("viewsize", &format!("{}", next.round() as i32));
}

fn start_message_mode(h: &mut Host, team: bool) {
    if h.cl.signon == 0 || h.demo.is_some() {
        return;
    }
    if !h.con.forced {
        h.con.open = false;
    }
    h.con.start_chat(team);
    h.sync_key_dest();
}

/// CD_f (vkQuake cd_sdl.c).
fn cmd_cd(h: &mut Host, a: &Args) {
    if a.argc() < 2 {
        h.print("commands: on, off, reset, remap, play, stop, loop, pause, resume, eject, info, next, prev\n");
        return;
    }
    let cmd = a.argv(1).to_ascii_lowercase();
    let stop_music = |h: &mut Host| {
        h.audio_fx.soundtracks.clear_track();
        h.cd.playing = None;
        h.cd.paused = false;
        if let Some(s) = &h.snd {
            s.stop_music();
        }
    };
    match cmd.as_str() {
        "on" => h.cd.enabled = true,
        "off" => {
            stop_music(h);
            h.cd.enabled = false;
        }
        "reset" => {
            h.cd.enabled = true;
            stop_music(h);
            h.cd.reset_remap();
        }
        "remap" => {
            let count = a.argc() - 2;
            if count == 0 {
                for n in 1..h.cd.remap.len() {
                    if h.cd.remap[n] as usize != n {
                        h.print(&format!("  {n} -> {}\n", h.cd.remap[n]));
                    }
                }
            } else {
                for n in 1..=count.min(h.cd.remap.len() - 1) {
                    h.cd.remap[n] = (qcore::cvar::q_atof(a.argv(n + 1)) as i32).clamp(0, 99) as u8;
                }
            }
        }
        "play" => {
            let mut n = qcore::cvar::q_atof(a.argv(2)) as i32;
            if n <= 0 {
                n = 1;
            }
            h.play_cd_track(n.min(255) as u8, false);
        }
        "loop" => {
            let n = (qcore::cvar::q_atof(a.argv(2)) as i32).clamp(0, 255) as u8;
            h.play_cd_track(n, true);
        }
        "stop" | "eject" => stop_music(h),
        "pause" => {
            if h.cd.playing.is_some() && !h.cd.paused {
                h.cd.paused = true;
                if let Some(s) = &h.snd {
                    s.pause_music(true);
                }
            }
        }
        "resume" => {
            if h.cd.playing.is_some() && h.cd.paused {
                h.cd.paused = false;
                if let Some(s) = &h.snd {
                    s.pause_music(false);
                }
            }
        }
        "next" | "prev" => {
            if let Some((track, looping)) = h.cd.playing {
                let next = if cmd == "next" { track.saturating_add(1) } else { track.saturating_sub(1).max(1) };
                h.play_cd_track(next, looping);
            }
        }
        "info" => {
            let msg = match (h.cd.enabled, h.cd.playing, h.cd.paused) {
                (false, _, _) => "CD audio is disabled\n".to_string(),
                (true, Some((t, l)), true) => format!("Currently paused track {t} ({})\n", if l { "looping" } else { "once" }),
                (true, Some((t, l)), false) => format!("Currently playing track {t} ({})\n", if l { "looping" } else { "once" }),
                (true, None, _) => "Not playing\n".to_string(),
            };
            h.print(&msg);
        }
        _ => h.print(&format!("cd: unknown command \"{cmd}\"\n")),
    }
}

/// gl_sky.c's sky wind commands; they act on the loaded skybox's wind.
fn register_skywind(c: &mut Commands<Host>) {
    fn ready(h: &mut Host, need_map: bool) -> bool {
        if need_map && h.cl.signon == 0 {
            return false;
        }
        if h.level.sky.is_empty() {
            h.print("No skybox loaded\n");
            return false;
        }
        true
    }
    fn f(a: &qcore::cmd::Args, i: usize) -> f64 {
        f64::from(qcore::cvar::q_atof(a.argv(i)))
    }
    c.add("skywind", |h, a| {
        if !ready(h, true) {
            return;
        }
        let w = &mut h.level.sky_wind;
        if a.argc() < 2 {
            let t = format!(
                "usage:\n   {} [distance] [yaw] [period] [pitch]\ncurrent values:\n   \"distance\" is \"{}\"\n   \"yaw\"      is \"{}\"\n   \"period\"   is \"{}\"\n   \"pitch\"    is \"{}\"\n",
                a.argv(0), w.dist, w.yaw, w.period, w.pitch
            );
            h.print(&t);
            return;
        }
        w.set_dist(f(a, 1));
        if a.argc() >= 3 {
            w.set_yaw(f(a, 2));
        }
        if a.argc() >= 4 {
            w.period = f(a, 3) as f32;
        }
        if a.argc() >= 5 {
            w.set_pitch(f(a, 4));
        }
    });
    c.add("skywind_load", |h, _| {
        if ready(h, false) {
            let fs = &h.server.core.fs;
            h.level.sky_wind = h.level.load_sky_wind(fs, &h.level.sky.clone());
        }
    });
    // written to the user dir, which overrides the game data
    c.add("skywind_save", |h, _| {
        if !ready(h, false) {
            return;
        }
        let rel = crate::skywind::cfg_path(&h.level.sky);
        let path = h.user_dir().join(&rel);
        let r = path.parent().map_or(Ok(()), qcore::vfs::create_dir_all).and_then(|_| qcore::vfs::write(&path, h.level.sky_wind.save_text().as_bytes()));
        match r {
            Ok(()) => h.print(&format!("Wrote {}\n", path.display())),
            Err(_) => h.print(&format!("Couldn't write '{rel}'.\n")),
        }
    });
    // clouds move towards the player: the opposite of the view direction
    c.add("skywind_lookdir", |h, a| {
        if !ready(h, true) {
            return;
        }
        let (yaw, pitch) = (h.cl.viewangles[1], h.cl.viewangles[0]);
        let w = &mut h.level.sky_wind;
        w.yaw = ((f64::from(yaw) + 180.0) % 360.0) as f32;
        w.pitch = -pitch;
        if a.argc() >= 2 {
            w.period = f(a, 1) as f32;
        } else if w.period == 0.0 {
            w.period = 30.0;
        }
        if a.argc() >= 3 {
            w.set_dist(f(a, 2));
        } else if w.dist == 0.0 {
            w.dist = 1.0;
        }
    });
    c.add("skywind_rotate", |h, a| {
        if !ready(h, true) {
            return;
        }
        if a.argc() < 2 {
            h.print(&format!("usage:\n   {} <yawdelta> [pitchdelta]\n", a.argv(0)));
            return;
        }
        let w = &mut h.level.sky_wind;
        w.set_yaw(f64::from(w.yaw) + f(a, 1));
        if a.argc() >= 3 {
            w.set_pitch(f64::from(w.pitch) + f(a, 2));
        }
    });
}
