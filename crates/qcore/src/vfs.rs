//! Virtual filesystem for game data and user files. Natively a thin wrapper
//! over `std::fs`; on wasm32 an in-memory file tree that the web front-end
//! fills with `mount_file` and persists by polling `take_dirty`.
use crate::time::SystemTime;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};

mod mem;

/// A readable, seekable file handle (PAK/PK3 access).
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub len: u64,
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use std::fs;
    use std::io::Write;

    pub fn read(path: &Path) -> io::Result<Vec<u8>> { fs::read(path) }
    pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)
    }
    /// Replaces `path` whole or not at all: a crash, a full disk or a power
    /// cut leaves the old file.
    pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
        // through a symlink: a linked config stays a link
        let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let Some(name) = path.file_name() else { return Err(io::ErrorKind::InvalidInput.into()) };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut tmp = name.to_os_string();
        tmp.push(format!(".{}.tmp", std::process::id()));
        let tmp = path.with_file_name(tmp);
        let put = || {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(data)?;
            if let Ok(md) = fs::metadata(path) {
                let _ = f.set_permissions(md.permissions());
            }
            f.sync_all()?;
            drop(f);
            fs::rename(&tmp, path)
        };
        let r = put();
        if r.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        r
    }
    pub fn append(path: &Path, data: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(data)
    }
    pub fn exists(path: &Path) -> bool { path.exists() }
    pub fn is_dir(path: &Path) -> bool { path.is_dir() }
    pub fn is_file(path: &Path) -> bool { path.is_file() }
    pub fn create_dir_all(path: &Path) -> io::Result<()> { fs::create_dir_all(path) }
    pub fn remove_file(path: &Path) -> io::Result<()> { fs::remove_file(path) }
    pub fn rename(from: &Path, to: &Path) -> io::Result<()> { fs::rename(from, to) }
    pub fn read_dir(path: &Path) -> io::Result<Vec<DirEntry>> {
        let mut out = Vec::new();
        for e in fs::read_dir(path)?.flatten() {
            let Ok(md) = fs::metadata(e.path()) else { continue };
            out.push(DirEntry { path: e.path(), is_dir: md.is_dir(), len: md.len() });
        }
        Ok(out)
    }
    pub fn modified(path: &Path) -> io::Result<SystemTime> { fs::metadata(path)?.modified() }
    pub fn open_read(path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(fs::File::open(path)?))
    }
    pub fn mount_file(path: &Path, data: Vec<u8>, _modified: Option<SystemTime>) { let _ = write(path, &data); }
    pub fn take_dirty() -> Vec<PathBuf> { Vec::new() }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use mem::GLOBAL;

    pub fn read(path: &Path) -> io::Result<Vec<u8>> { GLOBAL.lock().unwrap().read(path) }
    pub fn write(path: &Path, data: &[u8]) -> io::Result<()> { GLOBAL.lock().unwrap().write(path, data, false) }
    pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> { write(path, data) }
    pub fn append(path: &Path, data: &[u8]) -> io::Result<()> { GLOBAL.lock().unwrap().write(path, data, true) }
    pub fn exists(path: &Path) -> bool { GLOBAL.lock().unwrap().exists(path) }
    pub fn is_dir(path: &Path) -> bool { GLOBAL.lock().unwrap().is_dir(path) }
    pub fn is_file(path: &Path) -> bool { GLOBAL.lock().unwrap().is_file(path) }
    pub fn create_dir_all(path: &Path) -> io::Result<()> { GLOBAL.lock().unwrap().create_dir_all(path); Ok(()) }
    pub fn remove_file(path: &Path) -> io::Result<()> { GLOBAL.lock().unwrap().remove_file(path) }
    pub fn rename(from: &Path, to: &Path) -> io::Result<()> { GLOBAL.lock().unwrap().rename(from, to) }
    pub fn read_dir(path: &Path) -> io::Result<Vec<DirEntry>> { GLOBAL.lock().unwrap().read_dir(path) }
    pub fn modified(path: &Path) -> io::Result<SystemTime> { GLOBAL.lock().unwrap().modified(path) }
    pub fn open_read(path: &Path) -> io::Result<Box<dyn ReadSeek>> { GLOBAL.lock().unwrap().open_read(path) }
    /// `modified`: when it was written, for a file kept from an earlier visit.
    pub fn mount_file(path: &Path, data: Vec<u8>, modified: Option<SystemTime>) { GLOBAL.lock().unwrap().mount(path, data, modified) }
    pub fn take_dirty() -> Vec<PathBuf> { GLOBAL.lock().unwrap().take_dirty() }
}

pub use imp::*;

/// `read` as UTF-8 text (lossy).
pub fn read_to_string(path: &Path) -> io::Result<String> {
    read(path).map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod atomic_tests {
    use super::*;

    #[test]
    fn replaces_whole_or_not_at_all() {
        let dir = std::env::temp_dir().join(format!("qsp-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("sub/config.cfg");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(read(&p).unwrap(), b"two");
        // the temporary file can't be made: the old file stays
        let tmp = dir.join(format!("sub/config.cfg.{}.tmp", std::process::id()));
        std::fs::create_dir(&tmp).unwrap();
        assert!(write_atomic(&p, b"three").is_err());
        assert_eq!(read(&p).unwrap(), b"two");
        std::fs::remove_dir(&tmp).unwrap();
        #[cfg(unix)]
        {
            let link = dir.join("link.cfg");
            std::os::unix::fs::symlink(&p, &link).unwrap();
            write_atomic(&link, b"four").unwrap();
            assert!(link.is_symlink());
            assert_eq!(read(&p).unwrap(), b"four");
        }
        assert_eq!(read_dir(&dir.join("sub")).unwrap().len(), 1, "no temporary files left");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
