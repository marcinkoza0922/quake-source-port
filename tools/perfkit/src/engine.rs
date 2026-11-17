//! The engines a benchmark can run and where to find them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Ours,
    Ironwail,
    VkQuake,
}

impl Engine {
    pub const ALL: [Engine; 3] = [Engine::Ours, Engine::Ironwail, Engine::VkQuake];

    pub fn name(self) -> &'static str {
        match self {
            Engine::Ours => "ours",
            Engine::Ironwail => "ironwail",
            Engine::VkQuake => "vkquake",
        }
    }

    pub fn parse(s: &str) -> Option<Engine> {
        Engine::ALL.into_iter().find(|e| e.name() == s)
    }

    /// Parses a comma-separated engine list ("ours,ironwail").
    pub fn parse_list(s: &str) -> Result<Vec<Engine>, String> {
        s.split(',').filter(|p| !p.is_empty()).map(|p| Engine::parse(p).ok_or(format!("unknown engine {p}"))).collect()
    }

    /// Finds the binary: ours from `target/release`, ironwail extracted from
    /// the basedir's `ironwail-*-linux-x64.7z` into `refdir` on first use,
    /// vkQuake as `<basedir>/vkquake.AppImage`.
    pub fn locate(self, basedir: &Path, refdir: &Path) -> Option<PathBuf> {
        match self {
            Engine::Ours => Some(abs(Path::new("target/release/quake-source-port"))).filter(|p| p.is_file()),
            Engine::Ironwail => ironwail(basedir, refdir).map(|p| abs(&p)),
            Engine::VkQuake => Some(abs(&basedir.join("vkquake.AppImage"))).filter(|p| p.is_file()),
        }
    }

    /// Settings every run of this engine gets: no vsync, and for ours no
    /// autosaves (they would rotate the user's saves).
    pub fn base_args(self) -> Vec<String> {
        let mut v: Vec<String> = ["+vid_vsync", "0"].map(String::from).to_vec();
        if self == Engine::Ours {
            v.extend(["+sv_autosave", "0"].map(String::from));
        }
        v
    }
}

pub fn abs(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn ironwail(basedir: &Path, refdir: &Path) -> Option<PathBuf> {
    let find = || {
        let mut found = std::fs::read_dir(refdir).ok()?.flatten().map(|e| e.path().join("ironwail")).filter(|p| p.is_file());
        found.next()
    };
    if let Some(p) = find() {
        return Some(p);
    }
    let arc = std::fs::read_dir(basedir).ok()?.flatten().map(|e| e.path()).find(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy();
        n.starts_with("ironwail-") && n.ends_with("-linux-x64.7z")
    })?;
    std::fs::create_dir_all(refdir).ok()?;
    let ok = Command::new("7z")
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", refdir.display()))
        .arg(abs(&arc))
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if ok { find() } else { None }
}

/// "1500 frames   5.4 seconds 278.2 fps" -> (frames, secs, fps).
pub fn parse_fps(line: &str) -> Option<(u32, f64, f64)> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|s| *s == "frames")?;
    if i == 0 || w.get(i + 2) != Some(&"seconds") || w.get(i + 4) != Some(&"fps") {
        return None;
    }
    Some((w[i - 1].parse().ok()?, w[i + 1].parse().ok()?, w[i + 3].parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_line() {
        assert_eq!(parse_fps("1500 frames   5.4 seconds 278.2 fps\n"), Some((1500, 5.4, 278.2)));
        assert_eq!(parse_fps("Playing demo from qbench/s.dem."), None);
        assert_eq!(parse_fps("frames 1 seconds 2 fps"), None);
    }

    #[test]
    fn engine_names_round_trip() {
        for e in Engine::ALL {
            assert_eq!(Engine::parse(e.name()), Some(e));
        }
        assert_eq!(Engine::parse_list("ours,vkquake").unwrap(), vec![Engine::Ours, Engine::VkQuake]);
        assert!(Engine::parse_list("ours,quakespasm").is_err());
    }
}
