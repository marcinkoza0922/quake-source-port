//! `FRIK_FILE` file I/O builtins (`fopen`, `fclose`, `fgets`, `fputs`, `fseek`).
//!
//! Reads from the mounted Quake filesystem (`Fs`) or the gamedir's `data/`
//! directory; writes are sandboxed under `<basedir>/<gamedir>/data/` (or
//! in-memory when the filesystem root is unavailable) and never allow path
//! traversal (`..`, absolute paths, drive prefixes).

use std::path::{Component, Path, PathBuf};

use qcore::fs::Fs;

pub const FILE_READ: i32 = 0;
pub const FILE_APPEND: i32 = 1;
pub const FILE_WRITE: i32 = 2;

const MAX_FILES: usize = 64;

#[derive(Debug)]
struct OpenFile {
    writable: bool,
    path: Option<PathBuf>,
    buf: Vec<u8>,
    pos: usize,
    dirty: bool,
}

#[derive(Debug, Default)]
pub struct QcFiles {
    slots: Vec<Option<OpenFile>>,
}

fn sanitize_rel(name: &str) -> Option<String> {
    let trimmed = name.trim().replace('\\', "/");
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains(':') {
        return None;
    }
    let mut parts = Vec::new();
    for c in Path::new(&trimmed).components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

impl QcFiles {
    pub fn close_all(&mut self) {
        for slot in &mut self.slots {
            if let Some(f) = slot.take() {
                Self::flush(&f);
            }
        }
        self.slots.clear();
    }

    fn flush(f: &OpenFile) {
        if f.writable && f.dirty
            && let Some(p) = &f.path
        {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(p, &f.buf);
        }
    }

    fn write_path(fs: &Fs, rel: &str) -> Option<PathBuf> {
        let base = fs.basedir();
        let game = fs.game_dir().unwrap_or("id1");
        let rel = rel.strip_prefix("data/").unwrap_or(rel);
        Some(base.join(game).join("data").join(rel))
    }

    pub fn open(&mut self, fs: &Fs, name: &str, mode: i32) -> i32 {
        let Some(rel) = sanitize_rel(name) else { return -1 };
        let slot_idx = match self.slots.iter().position(Option::is_none) {
            Some(i) => i,
            None if self.slots.len() < MAX_FILES => {
                self.slots.push(None);
                self.slots.len() - 1
            }
            None => return -1,
        };
        let entry = match mode {
            FILE_READ => {
                let disk = Self::write_path(fs, &rel).and_then(|p| std::fs::read(p).ok());
                let data = disk
                    .or_else(|| fs.open(&rel))
                    .or_else(|| fs.open(&format!("data/{rel}")));
                let Some(buf) = data else { return -1 };
                OpenFile { writable: false, path: None, buf, pos: 0, dirty: false }
            }
            FILE_APPEND => {
                let path = Self::write_path(fs, &rel);
                let mut buf = path.as_ref().and_then(|p| std::fs::read(p).ok()).unwrap_or_default();
                let pos = buf.len();
                buf.reserve(64);
                OpenFile { writable: true, path, buf, pos, dirty: false }
            }
            FILE_WRITE => {
                let path = Self::write_path(fs, &rel);
                OpenFile { writable: true, path, buf: Vec::new(), pos: 0, dirty: true }
            }
            _ => return -1,
        };
        self.slots[slot_idx] = Some(entry);
        slot_idx as i32
    }

    pub fn close(&mut self, handle: i32) {
        if handle >= 0
            && let Some(slot) = self.slots.get_mut(handle as usize)
            && let Some(f) = slot.take()
        {
            Self::flush(&f);
        }
    }

    /// Reads one line without trailing `\r`/`\n`. Returns `None` on EOF or bad handle.
    pub fn gets(&mut self, handle: i32) -> Option<Vec<u8>> {
        if handle < 0 {
            return None;
        }
        let f = self.slots.get_mut(handle as usize)?.as_mut()?;
        if f.pos >= f.buf.len() {
            return None;
        }
        let rest = &f.buf[f.pos..];
        let (line, advance) = match rest.iter().position(|&b| b == b'\n') {
            Some(nl) => (&rest[..nl], nl + 1),
            None => (rest, rest.len()),
        };
        f.pos += advance;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        Some(line.to_vec())
    }

    pub fn puts(&mut self, handle: i32, data: &[u8]) {
        if handle < 0 {
            return;
        }
        let Some(Some(f)) = self.slots.get_mut(handle as usize) else { return };
        if !f.writable {
            return;
        }
        if f.pos == f.buf.len() {
            f.buf.extend_from_slice(data);
        } else {
            let end = f.pos + data.len();
            if end > f.buf.len() {
                f.buf.resize(end, 0);
            }
            f.buf[f.pos..end].copy_from_slice(data);
        }
        f.pos += data.len();
        f.dirty = true;
    }

    pub fn seek(&mut self, handle: i32, new_pos: Option<usize>) -> i32 {
        if handle < 0 {
            return -1;
        }
        let Some(Some(f)) = self.slots.get_mut(handle as usize) else { return -1 };
        let prev = f.pos as i32;
        if let Some(p) = new_pos {
            f.pos = p.min(f.buf.len());
        }
        prev
    }
}

impl Drop for QcFiles {
    fn drop(&mut self) {
        self.close_all();
    }
}
