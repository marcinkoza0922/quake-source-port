//! The demos on disk: saved ones (`record`, kept auto-demos) in the user
//! dir, auto-demos in its `autodemos/` folder named `<stamp>_<map>.dem`.
//! Listing for the Demos menu, slot pruning and keeping.

use std::path::{Path, PathBuf};

pub const AUTO_DIR: &str = "autodemos";

#[derive(Clone, Debug, PartialEq)]
pub struct DemoFile {
    pub path: PathBuf,
    /// File stem.
    pub name: String,
    pub auto: bool,
    pub bytes: u64,
    /// Seconds since the Unix epoch.
    pub modified: u64,
    /// Filled in by [`describe`] (background).
    pub map: Option<String>,
    pub duration: Option<f64>,
}

impl DemoFile {
    /// What `playdemo` takes: relative to the user dir.
    /// A saved demo converted for other ports keeps its full version
    /// beside it (`.qsd`, `demo::convert`); that one plays here.
    pub fn play_name(&self) -> String {
        let qsd = self.path.with_extension(super::convert::QSD_EXT);
        let path = if !self.auto && qcore::vfs::exists(&qsd) { &qsd } else { &self.path };
        let file = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if self.auto { format!("{AUTO_DIR}/{file}") } else { file }
    }

    /// The map an auto-demo's name records.
    pub fn auto_map(&self) -> Option<&str> {
        self.auto.then(|| self.name.split_once('_').map(|(_, m)| m)).flatten()
    }
}

fn dem_files(dir: &Path, auto: bool) -> Vec<DemoFile> {
    let Ok(entries) = qcore::vfs::read_dir(dir) else { return Vec::new() };
    entries
        .into_iter()
        .filter(|e| !e.is_dir && e.path.extension().is_some_and(|x| x.eq_ignore_ascii_case("dem")))
        .map(|e| DemoFile {
            name: e.path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
            modified: qcore::vfs::modified(&e.path).ok().and_then(|t| t.duration_since(qcore::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
            path: e.path,
            auto,
            bytes: e.len,
            map: None,
            duration: None,
        })
        .collect()
}

/// Saved demos (newest first) and auto-demos (newest first).
pub fn list(user_dir: &Path) -> (Vec<DemoFile>, Vec<DemoFile>) {
    let mut saved = dem_files(user_dir, false);
    let mut auto = dem_files(&user_dir.join(AUTO_DIR), true);
    saved.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    // stamps sort by time
    auto.sort_by(|a, b| b.name.cmp(&a.name));
    for d in auto.iter_mut() {
        d.map = d.auto_map().map(str::to_string);
    }
    (saved, auto)
}

/// Deletes the oldest auto-demos so at most `slots` remain (0 keeps all).
pub fn prune(auto_dir: &Path, slots: usize) -> usize {
    if slots == 0 {
        return 0;
    }
    let mut files = dem_files(auto_dir, true);
    files.sort_by(|a, b| b.name.cmp(&a.name));
    let mut n = 0;
    for f in files.iter().skip(slots) {
        if qcore::vfs::remove_file(&f.path).is_ok() {
            n += 1;
        }
    }
    n
}

/// Default kept name for an auto-demo `<stamp>_<map>`: `<map>_<stamp>`.
pub fn kept_name(auto_stem: &str) -> String {
    match auto_stem.split_once('_') {
        Some((stamp, map)) if !map.is_empty() => format!("{map}_{stamp}"),
        _ => auto_stem.to_string(),
    }
}

/// Moves an auto-demo into the saved demos as `name.dem` (made unique).
pub fn keep(auto: &Path, user_dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    let name = name.strip_suffix(".dem").unwrap_or(name);
    let mut dest = user_dir.join(format!("{name}.dem"));
    let mut i = 2;
    while qcore::vfs::exists(&dest) {
        dest = user_dir.join(format!("{name}_{i}.dem"));
        i += 1;
    }
    match qcore::vfs::rename(auto, &dest) {
        Ok(()) => Ok(dest),
        // another file system: copy, then remove
        Err(_) => {
            qcore::vfs::write(&dest, &qcore::vfs::read(auto)?)?;
            qcore::vfs::remove_file(auto)?;
            Ok(dest)
        }
    }
}

/// Map name and length of a demo file (runs the index scan).
pub fn describe(path: &Path) -> Option<(String, f64)> {
    let data = qcore::vfs::read(path).ok()?;
    let idx = super::index::scan(&data);
    Some((idx.maps.first().map(|m| m.name.clone()).unwrap_or_default(), idx.duration))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qsp-demolib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(AUTO_DIR)).unwrap();
        d
    }

    #[test]
    fn prune_keep_and_list() {
        let d = dir("a");
        let auto = d.join(AUTO_DIR);
        for s in ["20240101-000000_e1m1", "20240102-000000_e1m2", "20240103-000000_e1m3"] {
            std::fs::write(auto.join(format!("{s}.dem")), b"-1\n").unwrap();
        }
        std::fs::write(d.join("mine.dem"), b"-1\n").unwrap();
        assert_eq!(prune(&auto, 2), 1);
        let (saved, autos) = list(&d);
        assert_eq!(saved.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["mine"]);
        assert_eq!(autos.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["20240103-000000_e1m3", "20240102-000000_e1m2"]);
        assert_eq!(autos[0].map.as_deref(), Some("e1m3"));
        assert_eq!(autos[0].play_name(), "autodemos/20240103-000000_e1m3.dem");
        assert_eq!(prune(&auto, 0), 0);
        let kept = keep(&autos[1].path, &d, &kept_name(&autos[1].name)).unwrap();
        assert_eq!(kept, d.join("e1m2_20240102-000000.dem"));
        let again = keep(&autos[0].path, &d, "e1m2_20240102-000000").unwrap();
        assert_eq!(again, d.join("e1m2_20240102-000000_2.dem"));
        let (saved, autos) = list(&d);
        assert_eq!(saved.len(), 3);
        assert!(autos.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn kept_names() {
        assert_eq!(kept_name("20240101-120000_e1m1"), "e1m1_20240101-120000");
        assert_eq!(kept_name("odd"), "odd");
    }
}
