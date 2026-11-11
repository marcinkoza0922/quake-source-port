//! Quake search-path filesystem: gamedir stack, PAK and PK3 archives.
//!
//! Order mirrors quakespasm/ironwail `COM_AddGameDirectory`: for each gamedir
//! the loose directory is pushed first, then `pak0.pak`, `pak1.pak`, ... each
//! on top (so paks override loose files in the same dir, higher paks win), then
//! `*.pk3` sorted by name (QSS-style). Later gamedirs override earlier ones.
use crate::reader::Reader;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
struct Entry {
    offset: u64,
    size: u32,
    csize: u32,
    /// 0 = stored, 8 = deflate; u16::MAX = PK3 local header not resolved yet.
    method: u16,
}

/// A PAK or PK3 archive index (file data is read on demand).
#[derive(Debug)]
pub struct Pack {
    pub path: PathBuf,
    entries: HashMap<String, Entry>,
    names: Vec<String>,
}

fn norm(name: &str) -> String {
    name.replace('\\', "/").trim_start_matches('/').to_ascii_lowercase()
}

impl Pack {
    pub fn load_pak(path: &Path) -> Option<Pack> {
        let mut f = File::open(path).ok()?;
        let mut hdr = [0u8; 12];
        f.read_exact(&mut hdr).ok()?;
        let mut r = Reader::new(&hdr);
        if r.bytes(4).ok()? != b"PACK" {
            return None;
        }
        let ofs = r.i32().ok()?;
        let len = r.i32().ok()?;
        if ofs < 0 || len < 0 || len % 64 != 0 {
            return None;
        }
        let mut dir = vec![0u8; len as usize];
        f.seek(SeekFrom::Start(ofs as u64)).ok()?;
        f.read_exact(&mut dir).ok()?;
        let mut r = Reader::new(&dir);
        let mut pack = Pack { path: path.into(), entries: HashMap::new(), names: Vec::new() };
        while r.remaining() >= 64 {
            let name = r.fixed_str(56).ok()?;
            let fo = r.i32().ok()?;
            let fl = r.i32().ok()?;
            if fo < 0 || fl < 0 {
                return None;
            }
            pack.insert(&name, Entry { offset: fo as u64, size: fl as u32, csize: fl as u32, method: 0 });
        }
        Some(pack)
    }

    pub fn load_pk3(path: &Path) -> Option<Pack> {
        let mut f = File::open(path).ok()?;
        let flen = f.seek(SeekFrom::End(0)).ok()?;
        let tail_len = flen.min(22 + 65535);
        let mut tail = vec![0u8; tail_len as usize];
        f.seek(SeekFrom::Start(flen - tail_len)).ok()?;
        f.read_exact(&mut tail).ok()?;
        let eocd = (0..=tail.len().checked_sub(22)?).rev().find(|&i| tail[i..i + 4] == *b"PK\x05\x06")?;
        let mut r = Reader::at(&tail, eocd + 10).ok()?;
        let count = r.u16().ok()? as usize;
        let cd_size = r.u32().ok()? as usize;
        let cd_ofs = r.u32().ok()? as u64;
        let mut cd = vec![0u8; cd_size];
        f.seek(SeekFrom::Start(cd_ofs)).ok()?;
        f.read_exact(&mut cd).ok()?;
        let mut r = Reader::new(&cd);
        let mut pack = Pack { path: path.into(), entries: HashMap::new(), names: Vec::new() };
        for _ in 0..count {
            if r.bytes(4).ok()? != b"PK\x01\x02" {
                return None;
            }
            r.skip(6).ok()?;
            let method = r.u16().ok()?;
            r.skip(8).ok()?;
            let csize = r.u32().ok()?;
            let size = r.u32().ok()?;
            let nlen = r.u16().ok()? as usize;
            let elen = r.u16().ok()? as usize;
            let clen = r.u16().ok()? as usize;
            r.skip(8).ok()?;
            let local = r.u32().ok()? as u64;
            let name = String::from_utf8_lossy(r.bytes(nlen).ok()?).into_owned();
            r.skip(elen + clen).ok()?;
            if name.ends_with('/') || (method != 0 && method != 8) {
                continue;
            }
            // offset points at local header; resolved lazily on read.
            pack.insert(&name, Entry { offset: local, size, csize, method: method | 0x8000 });
        }
        Some(pack)
    }

    fn insert(&mut self, name: &str, e: Entry) {
        let n = norm(name);
        if self.entries.insert(n.clone(), e).is_none() {
            self.names.push(n);
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&norm(name))
    }

    /// Lowercased file names in archive order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let e = *self.entries.get(&norm(name))?;
        let mut f = File::open(&self.path).ok()?;
        let mut offset = e.offset;
        let method = e.method & 0x7fff;
        if e.method & 0x8000 != 0 {
            let mut lh = [0u8; 30];
            f.seek(SeekFrom::Start(offset)).ok()?;
            f.read_exact(&mut lh).ok()?;
            if lh[..4] != *b"PK\x03\x04" {
                return None;
            }
            let n = u16::from_le_bytes([lh[26], lh[27]]) as u64;
            let x = u16::from_le_bytes([lh[28], lh[29]]) as u64;
            offset += 30 + n + x;
        }
        let mut raw = vec![0u8; e.csize as usize];
        f.seek(SeekFrom::Start(offset)).ok()?;
        f.read_exact(&mut raw).ok()?;
        match method {
            0 => Some(raw),
            8 => {
                let out = miniz_oxide::inflate::decompress_to_vec_with_limit(&raw, e.size as usize).ok()?;
                (out.len() == e.size as usize).then_some(out)
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
enum Source {
    Dir(PathBuf),
    Pack(Pack),
}

#[derive(Debug)]
pub struct SearchPath {
    /// Index into `Fs::game_dirs` (0 = first added, usually id1).
    pub game: usize,
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
    game_dirs: Vec<String>,
    /// Lowest priority first; searched in reverse.
    paths: Vec<SearchPath>,
}

impl Fs {
    pub fn new(basedir: impl Into<PathBuf>) -> Self {
        Self { basedir: basedir.into(), game_dirs: Vec::new(), paths: Vec::new() }
    }

    /// `QUAKE_BASEDIR` or the default `./vkQuake directory with maps`.
    pub fn default_basedir() -> PathBuf {
        std::env::var_os("QUAKE_BASEDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("vkQuake directory with maps"))
    }

    pub fn basedir(&self) -> &Path {
        &self.basedir
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

    /// COM_AddGameDirectory. Ignored if already added (case-insensitive).
    /// Returns false if the directory doesn't exist.
    pub fn add_game_dir(&mut self, name: &str) -> bool {
        if self.game_dirs.iter().any(|g| g.eq_ignore_ascii_case(name)) {
            return true;
        }
        let dir = self.basedir.join(name);
        if !dir.is_dir() {
            return false;
        }
        let game = self.game_dirs.len();
        self.game_dirs.push(name.to_string());
        self.paths.push(SearchPath { game, source: Source::Dir(dir.clone()) });
        for i in 0.. {
            // Case-insensitive like loose files (e.g. func_mapjam3 ships PAK0.PAK).
            let pak = resolve_loose(&dir, &format!("pak{i}.pak"));
            match pak.and_then(|p| Pack::load_pak(&p)) {
                Some(p) => self.paths.push(SearchPath { game, source: Source::Pack(p) }),
                None => break,
            }
        }
        let mut pk3s: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pk3")))
            .collect();
        pk3s.sort();
        for p in pk3s {
            if let Some(p) = Pack::load_pk3(&p) {
                self.paths.push(SearchPath { game, source: Source::Pack(p) });
            }
        }
        true
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

    /// COM_LoadFile: first match from the highest-priority search path.
    pub fn open(&self, path: &str) -> Option<Vec<u8>> {
        let path = path.replace('\\', "/");
        match self.find(&path)? {
            (sp, None) => sp.pack()?.read(&path),
            (_, Some(f)) => std::fs::read(f).ok(),
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
                        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
                            let rp = if rel.is_empty() { name } else { format!("{rel}/{name}") };
                            let Ok(ft) = e.file_type() else { continue };
                            if ft.is_dir() {
                                stack.push((e.path(), rp));
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
    if exact.is_file() {
        return Some(exact);
    }
    let mut cur = dir.to_path_buf();
    for comp in path.split('/').filter(|c| !c.is_empty()) {
        let direct = cur.join(comp);
        if direct.exists() {
            cur = direct;
            continue;
        }
        let found = std::fs::read_dir(&cur).ok()?.flatten().find(|e| {
            e.file_name().to_str().is_some_and(|n| n.eq_ignore_ascii_case(comp))
        })?;
        cur = found.path();
    }
    cur.is_file().then_some(cur)
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
