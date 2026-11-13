//! Single-player autosave event tracking and bounded, crash-safe slot rotation.
use qserver::LevelStats;
use std::{fs, io, path::Path};

pub const MAX: usize = 10;

/// Save file extensions: plain v5 saves, and saves of games with add-ons
/// (which no other port can load).
pub const SAVE_EXT: &str = "sav";
pub const ADDON_EXT: &str = "qsa";

pub fn ext(addons: bool) -> &'static str {
    if addons { ADDON_EXT } else { SAVE_EXT }
}

/// Removes `name` in every save extension except `keep`.
pub fn remove_other_exts(dir: &Path, name: &str, keep: Option<&str>) -> io::Result<()> {
    for e in [SAVE_EXT, ADDON_EXT].into_iter().filter(|&e| Some(e) != keep) {
        match fs::remove_file(dir.join(format!("{name}.{e}"))) {
            Ok(()) => {}
            Err(x) if x.kind() == io::ErrorKind::NotFound => {}
            Err(x) => return Err(x),
        }
    }
    Ok(())
}

/// The existing save called `name` (no extension): the newer of its `.sav`
/// and `.qsa` files.
pub fn find(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    [SAVE_EXT, ADDON_EXT].into_iter().map(|e| dir.join(format!("{name}.{e}")))
        .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p))).max().map(|x| x.1)
}

#[derive(Default)]
pub struct Tracker {
    previous: Option<(f64, LevelStats)>,
    last_save: f64,
}

impl Tracker {
    pub fn reset(&mut self) { *self = Self::default(); }

    /// Flags: level entry, secret, all monsters, all secrets. Zero disables timer.
    pub fn update(&mut self, time: f64, stats: LevelStats, loaded: bool, flags: [bool; 4], interval: f64) -> Option<&'static str> {
        let previous = self.previous.replace((time, stats));
        let Some((_, old)) = previous.filter(|(t, _)| *t <= time) else {
            self.last_save = time;
            return (!loaded && flags[0]).then_some("Level entry");
        };
        let reason = if flags[1] && stats.found_secrets > old.found_secrets {
            Some("Secret found")
        } else if flags[2] && stats.total_monsters > 0 && stats.killed_monsters >= stats.total_monsters
            && old.killed_monsters < stats.total_monsters {
            Some("All enemies killed")
        } else if flags[3] && stats.total_secrets > 0 && stats.found_secrets >= stats.total_secrets
            && old.found_secrets < stats.total_secrets {
            Some("All secrets found")
        } else if interval > 0.0 && time - self.last_save >= interval {
            Some("Timer")
        } else { None };
        if reason.is_some() { self.last_save = time; }
        reason
    }
}

/// Newest is autosave0. Stage the complete save before touching older slots.
/// `ext` is the new save's extension; older slots keep theirs.
pub fn rotate(dir: &Path, keep: usize, text: &str, ext: &str) -> io::Result<()> {
    let keep = keep.clamp(1, MAX);
    fs::create_dir_all(dir)?;
    let staged = dir.join("autosave.tmp");
    let mut file = fs::File::create(&staged)?;
    use io::Write;
    file.write_all(&qcore::text::encode(text))?;
    file.sync_all()?;
    for i in (1..keep).rev() {
        remove_other_exts(dir, &format!("autosave{i}"), Some(""))?;
        for e in [SAVE_EXT, ADDON_EXT] {
            match fs::rename(dir.join(format!("autosave{}.{e}", i - 1)), dir.join(format!("autosave{i}.{e}"))) {
                Ok(()) => {}
                Err(x) if x.kind() == io::ErrorKind::NotFound => {}
                Err(x) => return Err(x),
            }
        }
    }
    remove_other_exts(dir, "autosave0", Some(""))?;
    fs::rename(staged, dir.join(format!("autosave0.{ext}")))?;
    for i in keep..MAX {
        remove_other_exts(dir, &format!("autosave{i}"), Some(""))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triggers_and_load_baseline() {
        let mut t = Tracker::default();
        let s = LevelStats { total_monsters: 2, total_secrets: 2, ..Default::default() };
        assert!(t.update(1.0, s, false, [true; 4], 60.0).is_some());
        assert!(t.update(2.0, s, false, [true; 4], 60.0).is_none());
        let s = LevelStats { found_secrets: 1, ..s };
        assert!(t.update(3.0, s, false, [true; 4], 60.0).is_some());
        let s = LevelStats { killed_monsters: 2, ..s };
        assert!(t.update(4.0, s, false, [true; 4], 60.0).is_some());
        assert!(t.update(5.0, s, false, [true; 4], 60.0).is_none());
        assert!(t.update(64.0, s, false, [true; 4], 60.0).is_some());
        t.reset();
        assert!(t.update(64.0, s, true, [true; 4], 60.0).is_none());
        assert!(t.update(65.0, s, true, [true; 4], 60.0).is_none());
        assert!(t.update(66.0, LevelStats { found_secrets: 2, ..s }, true, [true; 4], 60.0).is_some());
    }
    #[test]
    fn disabled_events_and_new_level_baselines() {
        let mut tracker = Tracker::default();
        let empty = LevelStats::default();
        assert!(tracker.update(1.0, empty, false, [false; 4], 0.0).is_none());
        let complete = LevelStats { total_secrets: 1, found_secrets: 1, total_monsters: 2, killed_monsters: 2 };
        assert!(tracker.update(2.0, complete, false, [false; 4], 0.0).is_none());
        // Enabling a completion trigger after the event must not save repeatedly.
        assert!(tracker.update(3.0, complete, false, [false, false, true, true], 0.0).is_none());
        tracker.reset();
        assert_eq!(tracker.update(1.0, empty, false, [true; 4], 0.0), Some("Level entry"));
        assert!(tracker.update(2.0, empty, false, [true; 4], 0.0).is_none());
        assert_eq!(tracker.update(3.0, complete, false, [false, false, true, false], 0.0), Some("All enemies killed"));
    }

    #[test]
    fn rotation_survives_restart_and_reduces_retention() {
        let dir = std::env::temp_dir().join(format!("qsp-autosave-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("s0.sav"), "manual").unwrap();
        for n in 0..5 { rotate(&dir, 3, &n.to_string(), "sav").unwrap(); }
        for (i, expected) in ["4", "3", "2"].iter().enumerate() {
            assert_eq!(fs::read_to_string(dir.join(format!("autosave{i}.sav"))).unwrap(), *expected);
        }
        let oldest_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1234);
        fs::File::options().write(true).open(dir.join("autosave0.sav")).unwrap().set_modified(oldest_time).unwrap();
        rotate(&dir, 3, "next", "sav").unwrap();
        assert_eq!(fs::metadata(dir.join("autosave1.sav")).unwrap().modified().unwrap(), oldest_time);
        // A staging failure cannot change the currently loadable history.
        fs::create_dir(dir.join("autosave.tmp")).unwrap();
        assert!(rotate(&dir, 3, "failed", "sav").is_err());
        assert_eq!(fs::read_to_string(dir.join("autosave0.sav")).unwrap(), "next");
        fs::remove_dir(dir.join("autosave.tmp")).unwrap();
        rotate(&dir, 1, "new", "sav").unwrap();
        assert!(!dir.join("autosave1.sav").exists());
        assert_eq!(fs::read_to_string(dir.join("s0.sav")).unwrap(), "manual");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rotation_carries_each_saves_extension() {
        let dir = std::env::temp_dir().join(format!("qsp-autosave-ext-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        rotate(&dir, 3, "plain", SAVE_EXT).unwrap();
        rotate(&dir, 3, "addon", ADDON_EXT).unwrap();
        assert_eq!(fs::read_to_string(dir.join("autosave0.qsa")).unwrap(), "addon");
        assert_eq!(fs::read_to_string(dir.join("autosave1.sav")).unwrap(), "plain");
        rotate(&dir, 3, "addon2", ADDON_EXT).unwrap();
        rotate(&dir, 3, "plain2", SAVE_EXT).unwrap();
        // the oldest ("plain") fell off; no slot has both extensions
        assert_eq!(fs::read_to_string(dir.join("autosave0.sav")).unwrap(), "plain2");
        assert_eq!(fs::read_to_string(dir.join("autosave1.qsa")).unwrap(), "addon2");
        assert_eq!(fs::read_to_string(dir.join("autosave2.qsa")).unwrap(), "addon");
        assert!(!dir.join("autosave0.qsa").exists() && !dir.join("autosave1.sav").exists() && !dir.join("autosave2.sav").exists());
        assert_eq!(find(&dir, "autosave1"), Some(dir.join("autosave1.qsa")));
        assert_eq!(find(&dir, "nope"), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
