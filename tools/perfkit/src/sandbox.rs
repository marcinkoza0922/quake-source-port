//! The base dir every benchmark run uses as `-basedir .`.
//!
//! `<root>/base/<game>/` is a real directory holding a symlink to each entry
//! of the game data's `<game>/`, except the engines' saved configs
//! (`config.cfg`, `vkQuake.cfg`, `ironwail.cfg`, `autoexec.cfg`). So:
//! - every engine starts from its defaults plus the command line, never from
//!   the user's own settings;
//! - a config an engine writes lands in the sandbox, never in the game data.
//!
//! Real files the engines left in the sandbox are deleted on every build, so
//! one session can't leak settings into the next. Demos are copied into
//! `<root>/base/<game>/perfkit/`.
//!
//! Our engine keeps its settings and saves in `$XDG_DATA_HOME/
//! quake-source-port/<game>`, not in the base dir, so the runner points
//! `XDG_DATA_HOME` at `<root>/userdata`, recreated on every build with an
//! empty `config.cfg` per game (which also keeps the first-run preset
//! pop-up away). Ours then starts from its defaults like the references and
//! never reads or writes the user's own configs and saves.

use std::path::{Path, PathBuf};

/// Engine config files that are never linked into the sandbox.
pub const ENGINE_CONFIGS: [&str; 4] = ["config.cfg", "vkquake.cfg", "ironwail.cfg", "autoexec.cfg"];

pub fn is_engine_config(name: &str) -> bool {
    ENGINE_CONFIGS.contains(&name.to_ascii_lowercase().as_str())
}

pub struct Sandbox {
    pub dir: PathBuf,
    /// `XDG_DATA_HOME` for our engine.
    pub userdata: PathBuf,
}

impl Sandbox {
    /// Builds (or refreshes) `<root>/base` and `<root>/userdata` for `id1`
    /// and `game`.
    pub fn build(basedir: &Path, root: &Path, game: &str) -> std::io::Result<Sandbox> {
        let src = crate::engine::abs(basedir);
        let dir = root.join("base");
        std::fs::create_dir_all(&dir)?;
        let mut games = vec!["id1"];
        if !game.eq_ignore_ascii_case("id1") {
            games.push(game);
        }
        let userdata = root.join("userdata");
        if userdata.is_dir() {
            std::fs::remove_dir_all(&userdata)?;
        }
        for &g in &games {
            let d = userdata.join("quake-source-port").join(g.to_ascii_lowercase());
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join("config.cfg"), "")?;
        }
        for g in games {
            let to = dir.join(g);
            if to.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
                std::fs::remove_file(&to)?; // a whole-dir link from an older layout
            }
            std::fs::create_dir_all(to.join("perfkit"))?;
            for e in std::fs::read_dir(&to)?.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let ty = e.file_type()?;
                if ty.is_file() && name.ends_with(".cfg") {
                    std::fs::remove_file(e.path())?; // written by an engine
                }
            }
            for e in std::fs::read_dir(src.join(g))?.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let link = to.join(&name);
                if is_engine_config(&name) || name == "perfkit" || link.symlink_metadata().is_ok() {
                    continue;
                }
                std::os::unix::fs::symlink(e.path(), link)?;
            }
        }
        Ok(Sandbox { dir: crate::engine::abs(&dir), userdata: crate::engine::abs(&userdata) })
    }

    /// Copies a demo in; returns the name to pass to `timedemo`.
    pub fn install_demo(&self, game: &str, name: &str, demo: &Path) -> std::io::Result<String> {
        std::fs::copy(demo, self.dir.join(game).join("perfkit").join(format!("{name}.dem")))?;
        Ok(format!("perfkit/{name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_engine_configs_only() {
        assert!(is_engine_config("vkQuake.cfg"));
        assert!(is_engine_config("CONFIG.CFG"));
        assert!(!is_engine_config("default.cfg"));
        assert!(!is_engine_config("quake.rc"));
    }

    #[test]
    fn builds_links_without_configs() {
        let t = std::env::temp_dir().join(format!("perfkit-sandbox-{}", std::process::id()));
        let data = t.join("data");
        for g in ["id1", "mod"] {
            std::fs::create_dir_all(data.join(g)).unwrap();
            std::fs::write(data.join(g).join("pak0.pak"), b"x").unwrap();
            std::fs::write(data.join(g).join("config.cfg"), b"user").unwrap();
        }
        let sb = Sandbox::build(&data, &t.join("out"), "mod").unwrap();
        for g in ["id1", "mod"] {
            assert!(sb.dir.join(g).join("pak0.pak").symlink_metadata().unwrap().file_type().is_symlink());
            assert!(!sb.dir.join(g).join("config.cfg").exists());
        }
        let ours = sb.userdata.join("quake-source-port").join("mod");
        assert_eq!(std::fs::read(ours.join("config.cfg")).unwrap(), b"");
        // A config an engine wrote into the sandbox is gone after a rebuild.
        std::fs::write(sb.dir.join("mod").join("ironwail.cfg"), b"engine").unwrap();
        std::fs::write(ours.join("config.cfg"), b"r_msaa 4").unwrap();
        std::fs::write(ours.join("autosave0.sav"), b"save").unwrap();
        let sb = Sandbox::build(&data, &t.join("out"), "mod").unwrap();
        assert!(!sb.dir.join("mod").join("ironwail.cfg").exists());
        assert_eq!(std::fs::read(ours.join("config.cfg")).unwrap(), b"");
        assert!(!ours.join("autosave0.sav").exists());
        assert!(sb.userdata.join("quake-source-port").join("id1").join("config.cfg").is_file());
        assert_eq!(std::fs::read(data.join("mod").join("config.cfg")).unwrap(), b"user");
        let _ = std::fs::remove_dir_all(&t);
    }
}
