//! Quake search-path filesystem: gamedir stack, PAK and PK3 archives.
//!
//! Order mirrors quakespasm/ironwail `COM_AddGameDirectory`: for each gamedir
//! the loose directory is pushed first, then `pak0.pak`, `pak1.pak`, ... each
//! on top (so paks override loose files in the same dir, higher paks win), then
//! `*.pk3` sorted by name (QSS-style). Later gamedirs override earlier ones.
use crate::vfs;
use crate::reader::Reader;
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
mod discovery;
pub use discovery::kex_save_dirs;
mod pack;
pub use pack::*;

#[derive(Debug)]
pub struct SearchPath {
    /// Index into `Fs::game_dirs` (0 = first added, usually id1).
    pub game: usize,
    is_user_dir: bool,
    source: Source,
}

impl SearchPath {
    pub fn pack(&self) -> Option<&Pack> {
        match &self.source {
            Source::Pack(p) => Some(p),
            Source::Dir(_) => None,
        }
    }
    pub fn dir(&self) -> Option<&Path> {
        match &self.source {
            Source::Dir(d) => Some(d),
            Source::Pack(_) => None,
        }
    }
}

/// Where a file was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOrigin {
    /// Gamedir name (e.g. "id1").
    pub game: String,
    /// Archive path, or None for a loose file.
    pub pack: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Fs {
    basedir: PathBuf,
    /// Extra roots searched after `basedir` by `add_game_dir` (e.g. user `mods/`).
    mod_roots: Vec<PathBuf>,
    game_dirs: Vec<String>,
    /// Lowest priority first; searched in reverse.
    paths: Vec<SearchPath>,
}

/// App-specific external storage on Android (`Android/data/<package>/files`), where the
/// Quake data (`id1/`) goes. The package name is set in `android/AndroidManifest.xml`.
pub const ANDROID_DATA_DIR: &str = "/sdcard/Android/data/org.quakesourceport.app/files";

/// Where the web front-end mounts the user's game data (`/quake/id1/pak0.pak`).
pub const WEB_BASEDIR: &str = "/quake";
/// Root of the per-game user dirs on the web (`/quake-user/<game>`).
pub const WEB_USER_ROOT: &str = "/quake-user";

impl Fs {
    pub fn new(basedir: impl Into<PathBuf>) -> Self {
        Self { basedir: basedir.into(), mod_roots: Vec::new(), game_dirs: Vec::new(), paths: Vec::new() }
    }

    /// `QUAKE_BASEDIR`, local data (two child levels), or an installed Quake.
    pub fn default_basedir() -> PathBuf {
        std::env::var_os("QUAKE_BASEDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Android: the app's own external folder, which the user can fill over USB or adb.
                if cfg!(target_arch = "wasm32") {
                    return PathBuf::from(WEB_BASEDIR);
                }
                if cfg!(target_os = "android") {
                    return PathBuf::from(ANDROID_DATA_DIR);
                }
                discovery::discover()
            })
    }

    pub fn basedir(&self) -> &Path {
        &self.basedir
    }

    /// Registers an additional root directory searched by `add_game_dir`
    /// after `basedir` (e.g. `<user data>/mods` for Quaddicted map packs).
    pub fn add_mod_root(&mut self, dir: impl Into<PathBuf>) {
        let dir = dir.into();
        if dir != self.basedir && !self.mod_roots.contains(&dir) {
            self.mod_roots.push(dir);
        }
    }

    pub fn set_mod_roots(&mut self, roots: Vec<PathBuf>) {
        self.mod_roots.clear();
        for r in roots {
            self.add_mod_root(r);
        }
    }

    pub fn mod_roots(&self) -> &[PathBuf] {
        &self.mod_roots
    }

    /// Gamedir names in add order (lowest priority first).
    pub fn game_dirs(&self) -> &[String] {
        &self.game_dirs
    }

    /// Top-priority gamedir name (`com_gamedir`).
    pub fn game_dir(&self) -> Option<&str> {
        self.game_dirs.last().map(String::as_str)
    }

    pub fn search_paths(&self) -> impl Iterator<Item = &SearchPath> {
        self.paths.iter().rev()
    }

    /// Adds a loose directory at top priority, attributed to the current top
    /// gamedir (ironwail `-userdir` style: user configs/saves override data).
    pub fn add_user_dir(&mut self, dir: impl Into<PathBuf>) {
        let game = self.game_dirs.len().saturating_sub(1);
        self.paths.push(SearchPath { game, is_user_dir: true, source: Source::Dir(dir.into()) });
    }

    /// Replaces any previously added user dirs with `dir` at top priority.
    pub fn replace_user_dir(&mut self, dir: impl Into<PathBuf>) {
        self.paths.retain(|p| !p.is_user_dir);
        self.add_user_dir(dir);
    }

    /// COM_AddGameDirectory. Ignored if already added (case-insensitive).
    /// Searches `basedir` and any `mod_roots`; mounts every existing copy in
    /// order so user-installed overlays (e.g. map packs for `ad`) layer cleanly
    /// on top of `basedir`. Returns false if none exist.
    pub fn add_game_dir(&mut self, name: &str) -> bool {
        if self.game_dirs.iter().any(|g| g.eq_ignore_ascii_case(name)) {
            return true;
        }
        let base_dir = if name.eq_ignore_ascii_case("id1")
            && !vfs::is_dir(&self.basedir.join(name))
            && discovery::has_pak(&self.basedir)
        {
            self.basedir.clone()
        } else {
            self.basedir.join(name)
        };
        let mut dirs = Vec::new();
        if vfs::is_dir(&base_dir) {
            dirs.push(base_dir);
        }
        for root in &self.mod_roots {
            let d = root.join(name);
            if vfs::is_dir(&d) && !dirs.contains(&d) {
                dirs.push(d);
            }
        }
        if dirs.is_empty() {
            return false;
        }
        let game = self.game_dirs.len();
        self.game_dirs.push(name.to_string());
        for dir in dirs {
            self.mount_dir(game, dir);
        }
        true
    }

    fn mount_dir(&mut self, game: usize, dir: PathBuf) {
        self.paths.push(SearchPath { game, is_user_dir: false, source: Source::Dir(dir.clone()) });
        for i in 0.. {
            // Case-insensitive like loose files (e.g. func_mapjam3 ships PAK0.PAK).
            let pak = resolve_loose(&dir, &format!("pak{i}.pak"));
            match pak.and_then(|p| Pack::load_pak(&p)) {
                Some(p) => self.paths.push(SearchPath { game, is_user_dir: false, source: Source::Pack(p) }),
                None => break,
            }
        }
        let mut pk3s: Vec<PathBuf> = vfs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter(|e| !e.is_dir)
            .map(|e| e.path)
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pk3")))
            .collect();
        pk3s.sort();
        for p in pk3s {
            if let Some(p) = Pack::load_pk3(&p) {
                self.paths.push(SearchPath { game, is_user_dir: false, source: Source::Pack(p) });
            }
        }
    }

    /// Drop all gamedirs above the first `keep` (COM_ResetGameDirectories).
    pub fn truncate_game_dirs(&mut self, keep: usize) {
        self.game_dirs.truncate(keep);
        self.paths.retain(|p| p.game < keep);
    }

    /// Finds the search path holding `path` and, for loose files, the real
    /// on-disk path. Loose lookups fall back to a case-insensitive match when
    /// the exact name is missing (an extension over ironwail/vkQuake).
    fn find(&self, path: &str) -> Option<(&SearchPath, Option<PathBuf>)> {
        let path = path.replace('\\', "/");
        if path.contains("..") || path.starts_with('/') {
            return None;
        }
        self.search_paths().find_map(|sp| match &sp.source {
            Source::Pack(p) => p.contains(&path).then_some((sp, None)),
            Source::Dir(d) => resolve_loose(d, &path).map(|f| (sp, Some(f))),
        })
    }

    fn find_path(&self, path: &str) -> Option<&SearchPath> {
        self.find(path).map(|(sp, _)| sp)
    }

    pub fn exists(&self, path: &str) -> bool {
        self.find_path(path).is_some()
    }

    pub fn origin(&self, path: &str) -> Option<FileOrigin> {
        let sp = self.find_path(path)?;
        Some(FileOrigin {
            game: self.game_dirs[sp.game].clone(),
            pack: sp.pack().map(|p| p.path.clone()),
        })
    }

    /// Open `companion` (e.g. a `.lit` or `.ent` next to a map) only if it
    /// comes from the same or a higher-priority game dir than `main`, like
    /// ironwail's `path_id` check.
    pub fn open_companion(&self, main: &str, companion: &str) -> Option<Vec<u8>> {
        let m = self.find_path(main)?.game;
        let c = self.find_path(companion)?.game;
        if c >= m { self.open(companion) } else { None }
    }

    /// Game dir index `path` would be read from (higher = higher priority,
    /// like vkQuake's `path_id`), or None when it doesn't exist.
    pub fn priority(&self, path: &str) -> Option<usize> {
        self.find_path(&path.replace('\\', "/")).map(|sp| sp.game)
    }

    /// COM_LoadFile: first match from the highest-priority search path.
    pub fn open(&self, path: &str) -> Option<Vec<u8>> {
        let path = path.replace('\\', "/");
        match self.find(&path)? {
            (sp, None) => sp.pack()?.read(&path),
            (_, Some(f)) => vfs::read(&f).ok(),
        }
    }

    /// Sorted unique paths (lowercase for archives) starting with `prefix`
    /// (e.g. "maps/") and ending with `ext` (e.g. ".bsp"; "" for any),
    /// compared case-insensitively. Loose files are scanned recursively.
    pub fn list(&self, prefix: &str, ext: &str) -> Vec<String> {
        let prefix = norm(prefix);
        let ext = ext.to_ascii_lowercase();
        let ok = |n: &str| {
            let l = n.to_ascii_lowercase();
            l.starts_with(&prefix) && l.ends_with(&ext)
        };
        let mut out = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for sp in &self.paths {
            match &sp.source {
                Source::Pack(p) => out.extend(p.names().iter().filter(|n| ok(n)).cloned()),
                Source::Dir(d) => {
                    // Start from the deepest directory part of the prefix.
                    let sub = prefix.rfind('/').map_or("", |i| &prefix[..i]);
                    let mut stack = vec![(d.join(sub), sub.to_string())];
                    while let Some((dir, rel)) = stack.pop() {
                        for e in vfs::read_dir(&dir).into_iter().flatten() {
                            let Some(name) = e.path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
                            let rp = if rel.is_empty() { name } else { format!("{rel}/{name}") };
                            if e.is_dir {
                                stack.push((e.path, rp));
                            } else if ok(&rp) && seen.insert(rp.to_ascii_lowercase()) {
                                out.insert(rp);
                            }
                        }
                    }
                }
            }
        }
        // Prefer one spelling per case-insensitive name.
        let mut uniq = BTreeSet::new();
        out.into_iter().filter(|n| uniq.insert(n.to_ascii_lowercase())).collect()
    }
}

/// Exact match first; on a miss, walk each component case-insensitively.
fn resolve_loose(dir: &Path, path: &str) -> Option<PathBuf> {
    let exact = dir.join(path);
    if vfs::is_file(&exact) {
        return Some(exact);
    }
    let mut cur = dir.to_path_buf();
    for comp in path.split('/').filter(|c| !c.is_empty()) {
        let direct = cur.join(comp);
        if vfs::exists(&direct) {
            cur = direct;
            continue;
        }
        let found = vfs::read_dir(&cur).ok()?.into_iter().find(|e| {
            e.path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(comp))
        })?;
        cur = found.path;
    }
    vfs::is_file(&cur).then_some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qcore-fs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_pak(path: &Path, files: &[(&str, &[u8])]) {
        let mut data = Vec::new();
        let mut dir = Vec::new();
        let mut ofs = 12;
        for (n, b) in files {
            data.extend_from_slice(b);
            let mut name = [0u8; 56];
            name[..n.len()].copy_from_slice(n.as_bytes());
            dir.extend_from_slice(&name);
            dir.extend_from_slice(&(ofs as i32).to_le_bytes());
            dir.extend_from_slice(&(b.len() as i32).to_le_bytes());
            ofs += b.len();
        }
        let mut out = b"PACK".to_vec();
        out.extend_from_slice(&(ofs as i32).to_le_bytes());
        out.extend_from_slice(&(dir.len() as i32).to_le_bytes());
        out.extend(data);
        out.extend(dir);
        std::fs::write(path, out).unwrap();
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8], bool)]) {
        let (mut out, mut cd) = (Vec::new(), Vec::new());
        for (n, b, deflate) in files {
            let comp = if *deflate { miniz_oxide::deflate::compress_to_vec(b, 6) } else { b.to_vec() };
            let method: u16 = if *deflate { 8 } else { 0 };
            let local = out.len() as u32;
            let common = |v: &mut Vec<u8>| {
                v.extend_from_slice(&[20, 0, 0, 0]);
                v.extend_from_slice(&method.to_le_bytes());
                v.extend_from_slice(&[0; 8]); // time, date, crc (unchecked)
                v.extend_from_slice(&(comp.len() as u32).to_le_bytes());
                v.extend_from_slice(&(b.len() as u32).to_le_bytes());
                v.extend_from_slice(&(n.len() as u16).to_le_bytes());
                v.extend_from_slice(&[0, 0]);
            };
            out.extend_from_slice(b"PK\x03\x04");
            common(&mut out);
            out.extend_from_slice(n.as_bytes());
            out.extend_from_slice(&comp);
            cd.extend_from_slice(b"PK\x01\x02\x14\x00");
            common(&mut cd);
            cd.extend_from_slice(&[0; 10]);
            cd.extend_from_slice(&local.to_le_bytes());
            cd.extend_from_slice(n.as_bytes());
        }
        let cd_ofs = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_ofs.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn loose_case_insensitive_fallback() {
        let b = tmpdir("case");
        std::fs::create_dir_all(b.join("id1/progs")).unwrap();
        std::fs::write(b.join("id1/progs/demonplush.mdl"), "m").unwrap();
        let mut fs = Fs::new(&b);
        assert!(fs.add_game_dir("id1"));
        assert_eq!(fs.open("progs/DEMONPLUSH.mdl").as_deref(), Some(&b"m"[..]));
        assert_eq!(fs.open("PROGS/demonplush.MDL").as_deref(), Some(&b"m"[..]));
        assert!(fs.open("progs/missing.mdl").is_none());
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn companion_needs_same_or_higher_priority() {
        let b = tmpdir("companion");
        std::fs::create_dir_all(b.join("id1/maps")).unwrap();
        std::fs::create_dir_all(b.join("q64/maps")).unwrap();
        std::fs::create_dir_all(b.join("lit/maps")).unwrap();
        std::fs::write(b.join("id1/maps/start.bsp"), "id").unwrap();
        std::fs::write(b.join("id1/maps/start.lit"), "idlit").unwrap();
        std::fs::write(b.join("q64/maps/start.bsp"), "q").unwrap();
        let mut fs = Fs::new(&b);
        fs.add_game_dir("id1");
        assert_eq!(fs.open_companion("maps/start.bsp", "maps/start.lit").as_deref(), Some(&b"idlit"[..]));
        fs.add_game_dir("q64");
        assert!(fs.open_companion("maps/start.bsp", "maps/start.lit").is_none());
        std::fs::write(b.join("lit/maps/start.lit"), "modlit").unwrap();
        fs.add_game_dir("lit");
        assert_eq!(fs.open_companion("maps/start.bsp", "maps/start.lit").as_deref(), Some(&b"modlit"[..]));
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn search_order() {
        let b = tmpdir("order");
        std::fs::create_dir_all(b.join("id1/maps")).unwrap();
        std::fs::create_dir_all(b.join("mod")).unwrap();
        std::fs::write(b.join("id1/a.txt"), "loose").unwrap();
        std::fs::write(b.join("id1/maps/x.bsp"), "x").unwrap();
        write_pak(&b.join("id1/pak0.pak"), &[("a.txt", b"pak0"), ("b.txt", b"pak0"), ("maps/start.bsp", b"s")]);
        write_pak(&b.join("id1/pak1.pak"), &[("b.txt", b"pak1")]);
        write_pak(&b.join("id1/pak3.pak"), &[("b.txt", b"pak3")]); // gap: not loaded
        let big: Vec<u8> = (0..5000u32).map(|i| (i % 7) as u8).collect();
        write_zip(&b.join("mod/z.pk3"), &[("c.txt", b"zip", false), ("Big.dat", &big, true), ("dir/", b"", false)]);
        std::fs::write(b.join("mod/b.txt"), "modloose").unwrap();

        let mut fs = Fs::new(&b);
        assert!(fs.add_game_dir("id1"));
        assert!(!fs.add_game_dir("nope"));
        assert_eq!(fs.open("a.txt").unwrap(), b"pak0"); // pak beats loose in same dir
        assert_eq!(fs.open("b.txt").unwrap(), b"pak1");
        assert!(fs.open("../x").is_none());
        assert!(fs.add_game_dir("mod"));
        assert_eq!(fs.open("b.txt").unwrap(), b"modloose"); // later gamedir wins
        assert_eq!(fs.open("c.txt").unwrap(), b"zip");
        assert_eq!(fs.open("big.dat").unwrap(), big);
        assert_eq!(fs.origin("a.txt").unwrap().game, "id1");
        assert_eq!(fs.list("maps/", ".bsp"), vec!["maps/start.bsp", "maps/x.bsp"]);
        fs.truncate_game_dirs(1);
        assert_eq!(fs.open("b.txt").unwrap(), b"pak1");
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn flat_paks_mount_as_id1() {
        let b = tmpdir("flat");
        write_pak(&b.join("pak0.pak"), &[("progs.dat", b"flat")]);
        let mut fs = Fs::new(&b);
        assert!(fs.add_game_dir("id1"));
        assert_eq!(fs.game_dir(), Some("id1"));
        assert_eq!(fs.open("progs.dat").as_deref(), Some(&b"flat"[..]));
        let _ = std::fs::remove_dir_all(b);
    }

    #[test]
    fn mod_roots_and_replace_user_dir() {
        let b = tmpdir("modroots");
        let (base, mods, u1, u2) = (b.join("base"), b.join("mods"), b.join("u1"), b.join("u2"));
        for d in [base.join("id1"), base.join("ad/maps"), mods.join("ad/maps"), mods.join("pack/maps"), u1.clone(), u2.clone()] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(base.join("ad/maps/base.bsp"), "b").unwrap();
        std::fs::write(mods.join("ad/maps/extra.bsp"), "e").unwrap();
        std::fs::write(mods.join("pack/maps/p.bsp"), "p").unwrap();
        std::fs::write(u1.join("config.cfg"), "u1").unwrap();
        std::fs::write(u2.join("config.cfg"), "u2").unwrap();
        let mut fs = Fs::new(&base);
        fs.add_mod_root(&mods);
        assert!(fs.add_game_dir("id1") && fs.add_game_dir("ad") && fs.add_game_dir("pack"));
        assert_eq!(fs.open("maps/base.bsp").as_deref(), Some(&b"b"[..]));
        assert_eq!(fs.open("maps/extra.bsp").as_deref(), Some(&b"e"[..]));
        assert_eq!(fs.open("maps/p.bsp").as_deref(), Some(&b"p"[..]));
        fs.add_user_dir(&u1);
        assert_eq!(fs.open("config.cfg").as_deref(), Some(&b"u1"[..]));
        fs.replace_user_dir(&u2);
        assert_eq!(fs.open("config.cfg").as_deref(), Some(&b"u2"[..]));
        let _ = std::fs::remove_dir_all(b);
    }

    #[test]
    fn real_data() {
        let mut base = Fs::default_basedir();
        if base.is_relative() && !base.exists() {
            base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(base);
        }
        if !base.join("id1").is_dir() {
            eprintln!("skip: no game data");
            return;
        }
        let mut fs = Fs::new(&base);
        assert!(fs.add_game_dir("id1"));
        let progs = fs.open("progs.dat").unwrap();
        assert_eq!(i32::from_le_bytes(progs[..4].try_into().unwrap()), 6);
        let bsp = fs.open("maps/start.bsp").unwrap();
        assert_eq!(i32::from_le_bytes(bsp[..4].try_into().unwrap()), 29);
        assert!(fs.list("maps/", ".bsp").iter().any(|m| m == "maps/e1m1.bsp"));
        if fs.add_game_dir("ad") {
            assert!(fs.list("maps/", ".bsp").len() > 10);
        }
    }
}
