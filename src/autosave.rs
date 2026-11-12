//! Single-player autosave event tracking and bounded, crash-safe slot rotation.
use qserver::LevelStats;
use std::{fs, io, path::Path};

pub const MAX: usize = 10;

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
pub fn rotate(dir: &Path, keep: usize, text: &str) -> io::Result<()> {
    let keep = keep.clamp(1, MAX);
    fs::create_dir_all(dir)?;
    let staged = dir.join("autosave.tmp");
    let mut file = fs::File::create(&staged)?;
    use io::Write;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    for i in (1..keep).rev() {
        let from = dir.join(format!("autosave{}.sav", i - 1));
        match fs::rename(from, dir.join(format!("autosave{i}.sav"))) {
            Ok(()) => {},
            Err(e) if e.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
    }
    fs::rename(staged, dir.join("autosave0.sav"))?;
    for i in keep..MAX {
        match fs::remove_file(dir.join(format!("autosave{i}.sav"))) {
            Ok(()) => {},
            Err(e) if e.kind() == io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
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
        for n in 0..5 { rotate(&dir, 3, &n.to_string()).unwrap(); }
        for (i, expected) in ["4", "3", "2"].iter().enumerate() {
            assert_eq!(fs::read_to_string(dir.join(format!("autosave{i}.sav"))).unwrap(), *expected);
        }
        let oldest_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1234);
        fs::File::options().write(true).open(dir.join("autosave0.sav")).unwrap().set_modified(oldest_time).unwrap();
        rotate(&dir, 3, "next").unwrap();
        assert_eq!(fs::metadata(dir.join("autosave1.sav")).unwrap().modified().unwrap(), oldest_time);
        // A staging failure cannot change the currently loadable history.
        fs::create_dir(dir.join("autosave.tmp")).unwrap();
        assert!(rotate(&dir, 3, "failed").is_err());
        assert_eq!(fs::read_to_string(dir.join("autosave0.sav")).unwrap(), "next");
        fs::remove_dir(dir.join("autosave.tmp")).unwrap();
        rotate(&dir, 1, "new").unwrap();
        assert!(!dir.join("autosave1.sav").exists());
        assert_eq!(fs::read_to_string(dir.join("s0.sav")).unwrap(), "manual");
        fs::remove_dir_all(dir).unwrap();
    }
}
