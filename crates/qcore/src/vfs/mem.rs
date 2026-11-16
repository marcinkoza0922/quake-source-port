//! In-memory file tree backing `qcore::vfs` on wasm32 (compiled everywhere so
//! it is unit-tested natively).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
use super::{DirEntry, ReadSeek};
use crate::time::SystemTime;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
pub(super) static GLOBAL: Mutex<MemFs> = Mutex::new(MemFs::new());

struct File {
    data: Arc<[u8]>,
    modified: SystemTime,
}

#[derive(Default)]
pub(super) struct MemFs {
    files: BTreeMap<PathBuf, File>,
    dirs: BTreeSet<PathBuf>,
    dirty: BTreeSet<PathBuf>,
}

/// Absolute-ish normal form: no `.`, `..` resolved, `/` root kept.
fn norm(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => { out.pop(); }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{}: not found", path.display()))
}

impl MemFs {
    pub(super) const fn new() -> Self {
        Self { files: BTreeMap::new(), dirs: BTreeSet::new(), dirty: BTreeSet::new() }
    }

    fn add_parents(&mut self, path: &Path) {
        let mut p = path.parent();
        while let Some(d) = p.filter(|d| !d.as_os_str().is_empty()) {
            if !self.dirs.insert(d.to_path_buf()) { break; }
            p = d.parent();
        }
    }

    pub(super) fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.files.get(&norm(path)).map(|f| f.data.to_vec()).ok_or_else(|| not_found(path))
    }

    pub(super) fn write(&mut self, path: &Path, data: &[u8], append: bool) -> io::Result<()> {
        let p = norm(path);
        if self.dirs.contains(&p) {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "is a directory"));
        }
        self.add_parents(&p);
        let modified = SystemTime::now();
        let entry = self.files.entry(p.clone()).or_insert_with(|| File { data: Arc::from([]), modified });
        let mut bytes = if append { entry.data.to_vec() } else { Vec::new() };
        bytes.extend_from_slice(data);
        *entry = File { data: Arc::from(bytes), modified };
        self.dirty.insert(p);
        Ok(())
    }

    pub(super) fn mount(&mut self, path: &Path, data: Vec<u8>) {
        let p = norm(path);
        self.add_parents(&p);
        self.files.insert(p, File { data: Arc::from(data), modified: SystemTime::now() });
    }

    pub(super) fn exists(&self, path: &Path) -> bool { self.is_file(path) || self.is_dir(path) }
    pub(super) fn is_file(&self, path: &Path) -> bool { self.files.contains_key(&norm(path)) }
    pub(super) fn is_dir(&self, path: &Path) -> bool {
        let p = norm(path);
        p == Path::new("/") || self.dirs.contains(&p)
    }

    pub(super) fn create_dir_all(&mut self, path: &Path) {
        let p = norm(path);
        self.add_parents(&p);
        self.dirs.insert(p);
    }

    pub(super) fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        let p = norm(path);
        self.files.remove(&p).ok_or_else(|| not_found(path))?;
        self.dirty.insert(p);
        Ok(())
    }

    pub(super) fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        let (f, t) = (norm(from), norm(to));
        let file = self.files.remove(&f).ok_or_else(|| not_found(from))?;
        self.add_parents(&t);
        self.files.insert(t.clone(), file);
        self.dirty.insert(f);
        self.dirty.insert(t);
        Ok(())
    }

    pub(super) fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        let p = norm(path);
        if !self.is_dir(&p) { return Err(not_found(path)); }
        let mut out: Vec<DirEntry> = self.dirs.iter()
            .filter(|d| d.parent() == Some(p.as_path()))
            .map(|d| DirEntry { path: d.clone(), is_dir: true, len: 0 })
            .collect();
        out.extend(self.files.iter()
            .filter(|(f, _)| f.parent() == Some(p.as_path()))
            .map(|(f, file)| DirEntry { path: f.clone(), is_dir: false, len: file.data.len() as u64 }));
        Ok(out)
    }

    pub(super) fn modified(&self, path: &Path) -> io::Result<SystemTime> {
        self.files.get(&norm(path)).map(|f| f.modified).ok_or_else(|| not_found(path))
    }

    pub(super) fn open_read(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        let f = self.files.get(&norm(path)).ok_or_else(|| not_found(path))?;
        Ok(Box::new(Cursor::new(f.data.clone())))
    }

    /// Written or removed paths since the last call (removed ones no longer exist).
    pub(super) fn take_dirty(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn tree_and_dirty_tracking() {
        let mut fs = MemFs::new();
        fs.mount(Path::new("/quake/id1/pak0.pak"), b"PACK".to_vec());
        assert!(fs.is_dir(Path::new("/quake")) && fs.is_dir(Path::new("/quake/id1")));
        assert!(fs.take_dirty().is_empty(), "mounts are not dirty");
        fs.write(Path::new("/quake-user/id1/./config.cfg"), b"bind", false).unwrap();
        fs.write(Path::new("/quake-user/id1/config.cfg"), b" x", true).unwrap();
        assert_eq!(fs.read(Path::new("/quake-user/id1/config.cfg")).unwrap(), b"bind x");
        assert_eq!(fs.take_dirty(), vec![PathBuf::from("/quake-user/id1/config.cfg")]);
        let list = fs.read_dir(Path::new("/quake")).unwrap();
        assert_eq!(list, vec![DirEntry { path: "/quake/id1".into(), is_dir: true, len: 0 }]);
        let mut r = fs.open_read(Path::new("/quake/id1/pak0.pak")).unwrap();
        r.seek(SeekFrom::Start(2)).unwrap();
        let mut s = String::new();
        r.read_to_string(&mut s).unwrap();
        assert_eq!(s, "CK");
        fs.rename(Path::new("/quake-user/id1/config.cfg"), Path::new("/quake-user/id1/b.cfg")).unwrap();
        assert!(!fs.is_file(Path::new("/quake-user/id1/config.cfg")));
        assert_eq!(fs.take_dirty().len(), 2);
        assert!(fs.remove_file(Path::new("/nope")).is_err());
    }
}
