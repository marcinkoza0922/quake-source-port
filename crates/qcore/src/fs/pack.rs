use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) offset: u64,
    pub(super) size: u32,
    pub(super) csize: u32,
    /// 0 = stored, 8 = deflate; u16::MAX = PK3 local header not resolved yet.
    pub(super) method: u16,
}

/// A PAK or PK3 archive index (file data is read on demand).
#[derive(Debug)]
pub struct Pack {
    pub path: PathBuf,
    pub(super) entries: HashMap<String, Entry>,
    pub(super) names: Vec<String>,
}

pub(super) fn norm(name: &str) -> String {
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

    pub(super) fn insert(&mut self, name: &str, e: Entry) {
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
pub(super) enum Source {
    Dir(PathBuf),
    Pack(Pack),
}
