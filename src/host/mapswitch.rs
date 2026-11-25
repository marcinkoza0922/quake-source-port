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
        // `game` may come from a server or a mod, and the new process trusts its
        // command line with the shell (`Origin::User`).
        if let Some(bad) = [Some(info.dir.as_str()), info.base.dir(), map].into_iter().flatten().find(|v| !plain_launch_value(v)) {
            self.con.print(&format!("game: bad name \"{bad}\"\n"));
            return;
        }
        let mut args = relaunch_args(&self.cmdline);
        args.extend(info.launch_args());
        if let Some(m) = map {
            // the config doesn't hold the new game's add-ons and modifiers
            if self.menu.setup_playing() {
                args.extend(setup_args(&self.server.core.cvars));
            }
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
        self.r.set_max_texture_size(max_texture_size([self.cv("r_texturesize"), self.cv("gl_max_size")]));
        let local = self.server.active() && self.cl.model_precache.get(1) == Some(&self.server.core.sv.modelname);
        let sv = &mut self.server.core.sv;
        let reloaded = local && sv.reloaded;
        let reload_bsp = if local { sv.reload_bsp.take() } else { None };
        let external_entities = sv.external_entities.as_deref().filter(|_| local);
        let options = crate::assets::LevelLoad { colored, enhanced_models: enhanced, external_entities, reload_bsp };
        let fs = &self.server.core.fs;
        let con = &mut self.con;
        self.level = Level::load(fs, &mut self.r, self.snd.as_mut(), &mut self.cl, options, &mut |s| con.print(s));
        // Fog_ParseWorldspawn; an svc_fog parsed in the same batch wins.
        if self.cl.fog == qclient::Fog::default() {
            let w = self.level.world_fog;
            self.cl.fog = qclient::Fog { density: w.density, color: w.color, time: 0.0 };
        }
        self.view.reset();
        self.feedback = Default::default();
        self.load_csqc();
        self.debris.new_map(&mut self.cl);
        self.bboxes.clear_map();
        if reloaded {
            self.automap = Default::default();
        }
        self.blood.clear();
        self.impacts.clear();
        self.hitfx.clear();
        self.tracers.clear();
        self.con.print(&qcore::ctr!("loaded {} in {}s\n", self.level.mapname, format!("{:.2}", t0.elapsed().as_secs_f32())));
    }
}

/// `+cvar value` for what the new game has changed.
fn setup_args(cv: &Cvars) -> Vec<String> {
    qserver::addons::settings::per_game()
        .chain(["custom_enable"])
        .filter_map(|n| cv.by_name(n))
        .filter(|c| c.string != c.default)
        .flat_map(|c| [format!("+{}", c.name), qcore::cvar::format_float(c.value)])
        .collect()
}

/// A value that stays one argument of its command on a command line
/// (`Host::stuff_cmds`): it can't start a `+command` or a switch, or split a line.
pub(super) fn plain_launch_value(v: &str) -> bool {
    !v.is_empty() && !v.starts_with(['+', '-']) && !v.contains(|c: char| c.is_control() || matches!(c, ';' | '|' | '"'))
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

/// The smallest of the texture size limits that are on (above 0); 0 = none.
fn max_texture_size(limits: [f32; 2]) -> u32 {
    limits.into_iter().filter(|&l| l > 0.0).map(|l| (l as u32).max(1)).min().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{max_texture_size, plain_launch_value, setup_args};

    #[test]
    fn another_mods_game_gets_the_choices() {
        let mut cv = qcore::cvar::Cvars::new();
        for &(n, d, f) in qserver::addons::CVARS.iter().chain(qserver::modifiers::CVARS) {
            cv.register(n, d, f);
        }
        assert!(setup_args(&cv).is_empty());
        cv.set("addon_portalgun", "1");
        cv.set("modifier_damagetaken", "0.5");
        cv.set("custom_enable", "1");
        cv.set("addon_portalgun_slot", "3"); // archived
        assert_eq!(setup_args(&cv), ["+addon_portalgun", "1", "+modifier_damagetaken", "0.5", "+custom_enable", "1"]);
    }

    #[test]
    fn launch_values_cannot_smuggle_commands() {
        for v in ["e1m1", "ad_tears", "sub/dir map", "a!b", "jam-2+x"] {
            assert!(plain_launch_value(v), "{v}");
        }
        for v in ["", "+!touch x", "-game", "e1m1;!touch x", "x | touch y", "a\n!b", "a\"; !b"] {
            assert!(!plain_launch_value(v), "{v}");
        }
    }

    #[test]
    fn the_smaller_texture_limit_wins() {
        assert_eq!(max_texture_size([0.0, 0.0]), 0);
        assert_eq!(max_texture_size([256.0, 2048.0]), 256);
        assert_eq!(max_texture_size([0.0, 2048.0]), 2048);
        assert_eq!(max_texture_size([-1.0, f32::NAN]), 0);
        assert_eq!(max_texture_size([0.5, 0.0]), 1);
    }
}
