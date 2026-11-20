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
        let mut f = crate::vfs::open_read(path).ok()?;
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
        let mut f = crate::vfs::open_read(path).ok()?;
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
            let mut csize = r.u32().ok()? as u64;
            let mut size = r.u32().ok()? as u64;
            let nlen = r.u16().ok()? as usize;
            let elen = r.u16().ok()? as usize;
            let clen = r.u16().ok()? as usize;
            r.skip(8).ok()?;
            let mut local = r.u32().ok()? as u64;
            let name = String::from_utf8_lossy(r.bytes(nlen).ok()?).into_owned();
            // Zip64 (some archivers write it even for small files): sizes and
            // offset of 0xFFFFFFFF are in the 0x0001 extra field, in this order.
            let mut extra = Reader::new(r.bytes(elen).ok()?);
            while let (Ok(id), Ok(len)) = (extra.u16(), extra.u16()) {
                let Ok(body) = extra.bytes(len as usize) else { break };
                if id == 1 {
                    let mut z = Reader::new(body);
                    for v in [&mut size, &mut csize, &mut local] {
                        if *v == 0xFFFF_FFFF {
                            let (Ok(lo), Ok(hi)) = (z.u32(), z.u32()) else { break };
                            *v = u64::from(lo) | (u64::from(hi) << 32);
                        }
                    }
                }
            }
            r.skip(clen).ok()?;
            let (Ok(size), Ok(csize)) = (u32::try_from(size), u32::try_from(csize)) else { continue };
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

    /// Uncompressed length of an archive member, without opening it.
    pub fn entry_size(&self, name: &str) -> Option<u32> {
        self.entries.get(&norm(name)).map(|e| e.size)
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let e = *self.entries.get(&norm(name))?;
        let mut f = crate::vfs::open_read(&self.path).ok()?;
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

#[cfg(test)]
mod tests {
    use super::Pack;

    /// One stored entry whose central record keeps its sizes in a Zip64
    /// extra field (0xFFFFFFFF in the 32-bit fields), as dtintin_fixed.zip does.
    fn zip64_entry(name: &str, data: &[u8]) -> Vec<u8> {
        let mut crc = 0xffff_ffffu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        let crc = !crc;
        let (n, len) = (name.len() as u16, data.len() as u64);
        let mut v = Vec::new();
        let w16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
        let w32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        w32(&mut v, 0x0403_4b50);
        for x in [45u16, 0, 0, 0, 0] {
            w16(&mut v, x);
        }
        w32(&mut v, crc);
        w32(&mut v, len as u32);
        w32(&mut v, len as u32);
        w16(&mut v, n);
        w16(&mut v, 0);
        v.extend_from_slice(name.as_bytes());
        v.extend_from_slice(data);
        let cd = v.len() as u32;
        w32(&mut v, 0x0201_4b50);
        for x in [45u16, 45, 0, 0, 0, 0] {
            w16(&mut v, x);
        }
        w32(&mut v, crc);
        w32(&mut v, 0xFFFF_FFFF);
        w32(&mut v, 0xFFFF_FFFF);
        for x in [n, 20, 0, 0, 0] {
            w16(&mut v, x);
        }
        w32(&mut v, 0);
        w32(&mut v, 0);
        v.extend_from_slice(name.as_bytes());
        w16(&mut v, 1);
        w16(&mut v, 16);
        v.extend_from_slice(&len.to_le_bytes());
        v.extend_from_slice(&len.to_le_bytes());
        let cd_len = v.len() as u32 - cd;
        w32(&mut v, 0x0605_4b50);
        for x in [0u16, 0, 1, 1] {
            w16(&mut v, x);
        }
        w32(&mut v, cd_len);
        w32(&mut v, cd);
        w16(&mut v, 0);
        v
    }

    #[test]
    fn reads_zip64_sizes() {
        let p = std::env::temp_dir().join(format!("qcore-zip64-{}.zip", std::process::id()));
        std::fs::write(&p, zip64_entry("gfx/poster.png", b"hello zip64")).unwrap();
        let pack = Pack::load_pk3(&p).expect("zip64 central directory");
        assert_eq!(pack.read("gfx/poster.png").as_deref(), Some(&b"hello zip64"[..]));
        let _ = std::fs::remove_file(p);
    }
}
