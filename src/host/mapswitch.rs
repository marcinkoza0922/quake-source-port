use super::*;

impl Host {
    /// The base game's user config text, when the running mod has none of
    /// its own yet: (game dir it came from, text).
    pub(super) fn inherited_config(&self) -> Option<(String, String)> {
        if self.user_dir().join("config.cfg").is_file() {
            return None;
        }
        let dirs = &self.fs_snapshot.1;
        dirs.iter().rev().skip(1).find_map(|g| {
            let t = std::fs::read(crate::user_dir(g).join("config.cfg")).ok()?;
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
        let Some(info) = qui::mods::scan(&basedir).into_iter().find(|m| m.dir.eq_ignore_ascii_case(dir)) else {
            self.con.print(&format!("game: no game dir \"{dir}\" in {}\n", basedir.display()));
            return;
        };
        let mut args = relaunch_args(&self.cmdline);
        args.extend(info.launch_args());
        if let Some(m) = map {
            args.extend(["+map".to_string(), m.to_string()]);
        }
        self.write_config();
        let Ok(exe) = std::env::current_exe() else {
            self.con.print("game: can't find the engine executable\n");
            return;
        };
        self.con.print(&format!("Switching to {}...\n", info.title));
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // drop the window and audio device before the new image opens its own
            self.window = None;
            self.snd = None;
            let e = cmd.exec();
            self.con.print(&format!("game: exec failed: {e}\n"));
        }
        #[cfg(not(unix))]
        match cmd.spawn() {
            Ok(_) => self.quit = true,
            Err(e) => self.con.print(&format!("game: launch failed: {e}\n")),
        }
    }

    /// Fills the mod menu's list (`_mods`).
    pub(super) fn scan_mods(&mut self) {
        let mods = qui::mods::scan(&self.fs_snapshot.0);
        let cur = self.game_name().to_string();
        self.menu.set_mods(mods, &cur);
        self.menu.mods.level_select = self.cv("ui_levelselect") != 0.0;
    }

    /// Fills the mod menu's map page for one mod (`_modmaps <dir>`).
    pub(super) fn scan_mod_maps(&mut self, dir: &str) {
        let maps = qui::mods::maps(&self.fs_snapshot.0, dir);
        let stats = qui::stats::Stats::load(&crate::user_dir(dir));
        self.menu.set_mod_maps(maps, stats);
    }

    pub(super) fn new_map(&mut self) {
        self.death_handled = false;
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopAll);
            s.event(SoundEvent::ClearStatic);
        }
        let t0 = std::time::Instant::now();
        let colored = self.cv("r_coloredlight") != 0.0;
        let fs = &self.server.core.fs;
        let con = &mut self.con;
        self.level = Level::load(fs, &mut self.r, self.snd.as_mut(), &mut self.cl, colored, &mut |s| con.print(s));
        // Fog_ParseWorldspawn; an svc_fog parsed in the same batch wins.
        if self.cl.fog == qclient::Fog::default() {
            let w = self.level.world_fog;
            self.cl.fog = qclient::Fog { density: w.density, color: w.color, time: 0.0 };
        }
        self.view.reset();
        self.feedback = Default::default();
        self.debris.new_map(&mut self.cl);
        self.blood.clear();
        self.impacts.clear();
        self.con.print(&format!("loaded {} in {:.2}s\n", self.level.mapname, t0.elapsed().as_secs_f32()));
    }
}
