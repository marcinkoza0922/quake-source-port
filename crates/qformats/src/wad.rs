//! WAD2 archives (gfx.wad).

use crate::{Error, Reader, Result};

pub const TYP_PALETTE: u8 = b'@';
pub const TYP_QTEX: u8 = b'A';
pub const TYP_QPIC: u8 = b'B';
pub const TYP_SOUND: u8 = b'C';
pub const TYP_MIPTEX: u8 = b'D';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WadLump {
    /// Lowercased name.
    pub name: String,
    pub kind: u8,
    pub compression: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wad {
    pub lumps: Vec<WadLump>,
}

impl Wad {
    pub fn parse(data: &[u8]) -> Result<Wad> {
        let mut r = Reader::new(data, "wad");
        let magic = r.arr::<4>()?;
        if &magic != b"WAD2" && &magic != b"WAD3" {
            return Err(r.err("bad magic"));
        }
        let n = r.count("lump", 32)?;
        let ofs = r.i32()?;
        if ofs < 0 {
            return Err(r.err("negative info table offset"));
        }
        r.seek(ofs as usize)?;
        let mut lumps = Vec::with_capacity(n);
        for _ in 0..n {
            let pos = r.u32()? as usize;
            let disk = r.u32()? as usize;
            let _size = r.u32()?;
            let kind = r.u8()?;
            let compression = r.u8()?;
            r.bytes(2)?;
            let name = r.name::<16>()?.to_ascii_lowercase();
            let d = data
                .get(pos..pos.saturating_add(disk))
                .ok_or_else(|| Error::new("wad", format!("lump {name} out of bounds")))?;
            lumps.push(WadLump {
                name,
                kind,
                compression,
                data: d.to_vec(),
            });
        }
        Ok(Wad { lumps })
    }

    /// Case-insensitive lookup.
    pub fn get(&self, name: &str) -> Option<&WadLump> {
        self.lumps
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
    }
}
