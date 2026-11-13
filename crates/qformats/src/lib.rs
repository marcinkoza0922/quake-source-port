//! qformats — parsers for Quake data formats into plain Rust structs (see docs/DESIGN.md).
//!
//! All parsers take `&[u8]` and return [`Result`]; malformed input yields an
//! [`Error`], never a panic.

pub mod bsp;
pub mod bspx;
pub mod entities;
pub mod image;
pub mod mdl;
pub mod spr;
pub mod wad;
pub mod wav;

pub use bsp::{Bsp, BspVersion};
pub use entities::{Entity, parse_entities};
pub use mdl::Mdl;
pub use spr::Spr;
pub use wad::Wad;
pub use wav::Wav;

/// 3-component vector.
pub type Vec3 = [f32; 3];

/// Parse error with format and context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Format being parsed ("bsp", "mdl", ...).
    pub format: &'static str,
    /// Human readable description of what failed.
    pub msg: String,
}

impl Error {
    pub fn new(format: &'static str, msg: impl Into<String>) -> Self {
        Self {
            format,
            msg: msg.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.format, self.msg)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Bounds-checked little-endian cursor.
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub fmt: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], fmt: &'static str) -> Self {
        Self { data, pos: 0, fmt }
    }
    pub fn err(&self, msg: impl Into<String>) -> Error {
        Error::new(self.fmt, format!("{} (at offset {})", msg.into(), self.pos))
    }
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(self.err(format!("seek to {pos} past end {}", self.data.len())));
        }
        self.pos = pos;
        Ok(())
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len());
        match end {
            Some(e) => {
                let s = &self.data[self.pos..e];
                self.pos = e;
                Ok(s)
            }
            None => Err(self.err(format!("unexpected end of data reading {n} bytes"))),
        }
    }
    pub fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.bytes(N)?);
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.arr::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.arr()?))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.arr()?))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.arr()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.arr()?))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.arr()?))
    }
    pub fn vec3(&mut self) -> Result<Vec3> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn vec3_i16(&mut self) -> Result<Vec3> {
        Ok([self.i16()? as f32, self.i16()? as f32, self.i16()? as f32])
    }
    /// Non-negative count, with a sanity bound on total bytes it implies.
    pub fn count(&mut self, what: &str, elem_size: usize) -> Result<usize> {
        let n = self.i32()?;
        check_count(self, n, what, elem_size)
    }
    pub fn name<const N: usize>(&mut self) -> Result<String> {
        Ok(cstr(&self.arr::<N>()?))
    }
}

pub(crate) fn check_count(r: &Reader, n: i32, what: &str, elem_size: usize) -> Result<usize> {
    if n < 0 {
        return Err(r.err(format!("negative {what} count {n}")));
    }
    let n = n as usize;
    if n.saturating_mul(elem_size.max(1)) > r.data.len() {
        return Err(r.err(format!("{what} count {n} exceeds data size")));
    }
    Ok(n)
}

/// NUL-terminated byte string to `String` (lossy).
pub fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}
