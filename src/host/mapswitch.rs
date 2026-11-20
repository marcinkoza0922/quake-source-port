use super::*;

impl Host {
    /// The base game's user config text, when the running mod has none of
    /// its own yet: (game dir it came from, text).
    pub(super) fn inherited_config(&self) -> Option<(String, String)> {
        if qcore::vfs::is_file(&self.user_dir().join("config.cfg")) {
            return None;
        }
        let dirs = &self.fs_snapshot.1;
        dirs.iter().rev().skip(1).find_map(|g| {
            let t = qcore::vfs::read(&crate::user_dir(g).join("config.cfg")).ok()?;
            Some((g.clone(), String::from_utf8_lossy(&t).into_owned()))
        })
    }

    /// Name of the top gamedir ("id1" when no mod is mounted).
    pub fn game_name(&self) -> &str {
        self.fs_snapshot.1.last().map_or("id1", String::as_str)
    }

    /// `game <dir> [map]`: switch to another mod. The renderer, sound and
    /// file system are built for one gamedir at startup, so this saves the
    /// config and re-executes the engine with the mod's launch flags (and
    /// `+map`); the current mod only needs the `map` command.
    pub(super) fn switch_game(&mut self, dir: &str, map: Option<&str>) {
        let basedir = self.fs_snapshot.0.clone();
        if dir.eq_ignore_ascii_case(self.game_name()) {
            if let Some(m) = map {
                self.cbuf.add_text(&format!("maxplayers 1\ndeathmatch 0\ncoop 0\nmap {m}\n"));
            }
            return;
        }
        let mods_dir = crate::user_mods_dir();
        let Some(info) = qui::mods::scan_roots(&[&basedir, &mods_dir]).into_iter().find(|m| m.dir.eq_ignore_ascii_case(dir)) else {
            self.con.print(&qcore::ctr!("game: no game dir \"{}\" in {}\n", dir, basedir.display()));
            return;
        };
        let mut args = relaunch_args(&self.cmdline);
        args.extend(info.launch_args());
        if let Some(m) = map {
            args.extend(["+map".to_string(), m.to_string()]);
        }
        self.write_config();
        let Ok(exe) = std::env::current_exe() else {
            self.con.print(&qcore::ctr!("game: can't find the engine executable\n"));
            return;
        };
        self.con.print(&qcore::ctr!("Switching to {}...\n", info.title));
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // drop the window and audio device before the new image opens its own
            self.window = None;
            self.snd = None;
            let e = cmd.exec();
            self.con.print(&qcore::ctr!("game: exec failed: {}\n", e));
        }
        #[cfg(not(unix))]
        match cmd.spawn() {
            Ok(_) => self.quit = true,
            Err(e) => self.con.print(&qcore::ctr!("game: launch failed: {}\n", e)),
        }
    }

    /// Fills the mod menu's list (`_mods`).
    pub(super) fn scan_mods(&mut self) {
        let mods_dir = crate::user_mods_dir();
        let mods = qui::mods::scan_roots(&[&self.fs_snapshot.0, &mods_dir]);
        let times: Vec<u64> = mods
            .iter()
            .map(|m| {
                qcore::vfs::modified(&crate::user_dir(&m.dir).join(qui::stats::FILE_NAME))
                    .ok()
                    .and_then(|t| t.duration_since(qcore::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs())
            })
            .collect();
        let cur = self.game_name().to_string();
        self.menu.set_mods(mods, &cur);
        self.menu.mods.set_last_played(times);
    }

    /// Starts loading the Quaddicted index for Map Packs > Get more maps.
    pub(super) fn open_quaddicted(&mut self, force: bool) {
        let basedir = self.fs_snapshot.0.clone();
        let install_root = crate::user_mods_dir();
        let cache = crate::quaddicted_cache_dir();
        self.menu.mods.online.data.open(&basedir, &install_root, &cache, force);
    }

    /// Fills the mod menu's map page for one mod (`_modmaps <dir>`).
    pub(super) fn scan_mod_maps(&mut self, dir: &str) {
        let mods_dir = crate::user_mods_dir();
        let maps = qui::mods::maps_roots(&[&self.fs_snapshot.0, &mods_dir], dir);
        let stats = qui::stats::Stats::load(&crate::user_dir(dir));
        self.menu.set_mod_maps(maps, stats);
    }

    pub(super) fn new_map(&mut self) {
        self.death_handled = false;
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopAll);
            s.event(SoundEvent::ClearStatic);
        }
        let t0 = qcore::time::Instant::now();
        let colored = self.cv("r_coloredlight") != 0.0;
        let enhanced = self.cv("r_enhancedmodels") != 0.0;
        let fs = &self.server.core.fs;
        let con = &mut self.con;
        self.level = Level::load(fs, &mut self.r, self.snd.as_mut(), &mut self.cl, colored, enhanced, &mut |s| con.print(s));
        // Fog_ParseWorldspawn; an svc_fog parsed in the same batch wins.
        if self.cl.fog == qclient::Fog::default() {
            let w = self.level.world_fog;
            self.cl.fog = qclient::Fog { density: w.density, color: w.color, time: 0.0 };
        }
        self.view.reset();
        self.feedback = Default::default();
        self.load_csqc();
        self.debris.new_map(&mut self.cl);
        self.bboxes.pointfile.clear();
        self.blood.clear();
        self.impacts.clear();
        self.hitfx.clear();
        self.tracers.clear();
        self.con.print(&qcore::ctr!("loaded {} in {}s\n", self.level.mapname, format!("{:.2}", t0.elapsed().as_secs_f32())));
    }
}

/// The original command line without the mod selection (`-game x`,
/// `-hipnotic`/`-rogue`/`-quoth`) and without `+command` sequences, which
/// belong to the first launch.
pub(super) fn relaunch_args(cmdline: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = cmdline.iter();
    let mut in_cmd = false;
    while let Some(a) = it.next() {
        let l = a.to_ascii_lowercase();
        if l == "-game" {
            it.next();
            in_cmd = false;
        } else if matches!(l.as_str(), "-hipnotic" | "-rogue" | "-quoth") {
            in_cmd = false;
        } else if a.starts_with('+') {
            in_cmd = true;
        } else if a.starts_with('-') && a.len() > 1 && a.parse::<f64>().is_err() {
            in_cmd = false;
            out.push(a.clone());
        } else if !in_cmd {
            out.push(a.clone());
        }
    }
    out
}

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("_mods", |h, _| h.scan_mods());
    c.add("_modmaps", |h, a| h.scan_mod_maps(a.argv(1)));
    // Quaddicted browser: load (or with `force`, refetch) the archive's index
    c.add("_quaddicted", |h, a| h.open_quaddicted(a.argv(1) == "force"));
    c.add("menu_quaddicted", |h, _| {
        h.scan_mods();
        h.open_quaddicted(false);
        h.menu.open(Screen::ModOnline);
    });
    c.add("menu_mods", |h, a| {
        h.scan_mods();
        if a.argc() > 1 && h.menu.mods.select(a.argv(1)) {
            h.scan_mod_maps(a.argv(1));
            h.menu.open(Screen::ModMaps);
        } else {
            h.menu.open(Screen::Mods);
        }
    });
    c.add("menu_levels", |h, _| {
        let g = h.game_name().to_string();
        h.scan_mods();
        if h.menu.mods.select(&g) {
            h.scan_mod_maps(&g);
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
}
