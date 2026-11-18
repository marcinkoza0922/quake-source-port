//! Snapshots of the configs a benchmark must never change.
//!
//! Windowed runs are SIGKILLed so no engine writes its config, and the
//! sandbox keeps the references away from the game data's configs. The
//! guard checks both: it records each file's bytes before a session and
//! reports any that differ afterwards.

use std::path::{Path, PathBuf};

pub struct ConfigGuard {
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

/// Our engine's per-user settings dir (`$XDG_DATA_HOME/quake-source-port`).
pub fn user_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("quake-source-port")
}

impl ConfigGuard {
    /// Our engine's `global.cfg` and per-game `config.cfg` for `id1` and
    /// `game`, and every engine config in the game data's `id1` and `game`.
    pub fn for_game(basedir: &Path, game: &str) -> ConfigGuard {
        let user = user_dir();
        let mut paths = vec![user.join("global.cfg")];
        for g in ["id1", game] {
            paths.push(user.join(g.to_ascii_lowercase()).join("config.cfg"));
            for name in ["config.cfg", "vkQuake.cfg", "ironwail.cfg", "autoexec.cfg"] {
                paths.push(basedir.join(g).join(name));
            }
        }
        paths.sort();
        paths.dedup();
        ConfigGuard::of(paths)
    }

    pub fn of(paths: Vec<PathBuf>) -> ConfigGuard {
        ConfigGuard { files: paths.into_iter().map(|p| { let b = std::fs::read(&p).ok(); (p, b) }).collect() }
    }

    /// Files whose contents (or existence) changed since the snapshot.
    pub fn changed(&self) -> Vec<PathBuf> {
        self.files.iter().filter(|(p, b)| std::fs::read(p).ok() != *b).map(|(p, _)| p.clone()).collect()
    }

    /// Restores every changed file to its snapshot and returns what it fixed.
    pub fn restore(&self) -> Vec<PathBuf> {
        let mut fixed = Vec::new();
        for (p, b) in &self.files {
            if std::fs::read(p).ok() == *b {
                continue;
            }
            let ok = match b {
                Some(bytes) => std::fs::write(p, bytes).is_ok(),
                None => std::fs::remove_file(p).is_ok(),
            };
            if ok {
                fixed.push(p.clone());
            }
        }
        fixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_restores_changes() {
        let t = std::env::temp_dir().join(format!("perfkit-guard-{}", std::process::id()));
        std::fs::create_dir_all(&t).unwrap();
        let (a, b) = (t.join("a.cfg"), t.join("b.cfg"));
        std::fs::write(&a, b"one").unwrap();
        let g = ConfigGuard::of(vec![a.clone(), b.clone()]);
        assert!(g.changed().is_empty());
        std::fs::write(&a, b"two").unwrap();
        std::fs::write(&b, b"new").unwrap();
        assert_eq!(g.changed(), vec![a.clone(), b.clone()]);
        assert_eq!(g.restore().len(), 2);
        assert_eq!(std::fs::read(&a).unwrap(), b"one");
        assert!(!b.exists());
        let _ = std::fs::remove_dir_all(&t);
    }
}
