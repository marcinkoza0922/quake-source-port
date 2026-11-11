//! Message buffers and NetQuake wire encoding.
//!
//! NOTE: `SizeBuf` duplicates `qserver::msg::SizeBuf` (same semantics) so
//! qserver can switch to this crate later without behavior changes.

use crate::protocol::*;

/// C `Q_rint`.
#[inline]
pub fn q_rint(x: f32) -> i32 {
    if x > 0.0 { (x + 0.5) as i32 } else { (x - 0.5) as i32 }
}

/// sizebuf_t. Overflow clears the buffer and sets `overflowed` (allowoverflow semantics).
#[derive(Debug, Clone, Default)]
pub struct SizeBuf {
    pub data: Vec<u8>,
    pub maxsize: usize,
    pub overflowed: bool,
}

impl SizeBuf {
    pub fn new(maxsize: usize) -> Self {
        Self { data: Vec::new(), maxsize, overflowed: false }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }
    /// SZ_Write.
    pub fn write(&mut self, b: &[u8]) {
        if self.data.len() + b.len() > self.maxsize {
            self.overflowed = true;
            self.data.clear();
            if b.len() > self.maxsize {
                return;
            }
        }
        self.data.extend_from_slice(b);
    }
    pub fn byte(&mut self, c: i32) {
        self.write(&[c as u8]);
    }
    pub fn char(&mut self, c: i32) {
        self.write(&[c as u8]);
    }
    pub fn short(&mut self, c: i32) {
        self.write(&(c as u16).to_le_bytes());
    }
    pub fn long(&mut self, c: i32) {
        self.write(&c.to_le_bytes());
    }
    pub fn float(&mut self, f: f32) {
        self.write(&f.to_le_bytes());
    }
    /// MSG_WriteString (bytes up to the first NUL, then NUL).
    pub fn string(&mut self, s: &[u8]) {
        let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        let mut v = Vec::with_capacity(n + 1);
        v.extend_from_slice(&s[..n]);
        v.push(0);
        self.write(&v);
    }
    pub fn coord(&mut self, f: f32, flags: u32) {
        if flags & PRFL_FLOATCOORD != 0 {
            self.float(f)
        } else if flags & PRFL_INT32COORD != 0 {
            self.long(q_rint(f * 16.0))
        } else if flags & PRFL_24BITCOORD != 0 {
            self.short(f as i32);
            self.byte(((f * 255.0) as i32) % 255);
        } else {
            self.short(q_rint(f * 8.0))
        }
    }
    pub fn angle(&mut self, f: f32, flags: u32) {
        if flags & PRFL_FLOATANGLE != 0 {
            self.float(f)
        } else if flags & PRFL_SHORTANGLE != 0 {
            self.short(q_rint((f as f64 * 65536.0 / 360.0) as f32) & 65535)
        } else {
            self.byte(q_rint((f as f64 * 256.0 / 360.0) as f32) & 255)
        }
    }
    pub fn angle16(&mut self, f: f32, flags: u32) {
        if flags & PRFL_FLOATANGLE != 0 {
            self.float(f)
        } else {
            self.short(q_rint((f as f64 * 65536.0 / 360.0) as f32) & 65535)
        }
    }
}

/// MSG_Read* cursor. Reading past the end sets `badread` and returns -1
/// (bytes) / 0-ish values like the C code; never panics.
#[derive(Debug, Clone)]
pub struct MsgReader<'a> {
    data: &'a [u8],
    pub pos: usize,
    pub badread: bool,
}

impl<'a> MsgReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, badread: false }
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn at_end(&self) -> bool {
        self.pos >= self.data.len()
    }
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.pos + N > self.data.len() {
            self.badread = true;
            self.pos = self.data.len();
            return None;
        }
        let mut a = [0; N];
        a.copy_from_slice(&self.data[self.pos..self.pos + N]);
        self.pos += N;
        Some(a)
    }
    /// Unsigned byte, or -1 on overrun.
    pub fn byte(&mut self) -> i32 {
        self.take::<1>().map_or(-1, |b| b[0] as i32)
    }
    pub fn char(&mut self) -> i32 {
        self.take::<1>().map_or(-1, |b| b[0] as i8 as i32)
    }
    /// Signed short.
    pub fn short(&mut self) -> i32 {
        self.take::<2>().map_or(-1, |b| i16::from_le_bytes(b) as i32)
    }
    pub fn long(&mut self) -> i32 {
        self.take::<4>().map_or(-1, i32::from_le_bytes)
    }
    pub fn float(&mut self) -> f32 {
        self.take::<4>().map_or(-1.0, f32::from_le_bytes)
    }
    /// MSG_ReadString: bytes up to NUL (or end), NUL consumed.
    pub fn string(&mut self) -> &'a [u8] {
        let rest = &self.data[self.pos.min(self.data.len())..];
        match rest.iter().position(|&c| c == 0) {
            Some(n) => {
                self.pos += n + 1;
                &rest[..n]
            }
            None => {
                self.badread = true;
                self.pos = self.data.len();
                rest
            }
        }
    }
    /// String as lossy UTF-8 (Quake text is byte-oriented).
    pub fn string_lossy(&mut self) -> String {
        String::from_utf8_lossy(self.string()).into_owned()
    }
    pub fn coord(&mut self, flags: u32) -> f32 {
        if flags & PRFL_FLOATCOORD != 0 {
            self.float()
        } else if flags & PRFL_INT32COORD != 0 {
            self.long() as f32 * (1.0 / 16.0)
        } else if flags & PRFL_24BITCOORD != 0 {
            self.short() as f32 + self.byte() as f32 * (1.0 / 255.0)
        } else {
            self.short() as f32 * (1.0 / 8.0)
        }
    }
    pub fn angle(&mut self, flags: u32) -> f32 {
        if flags & PRFL_FLOATANGLE != 0 {
            self.float()
        } else if flags & PRFL_SHORTANGLE != 0 {
            self.short() as f32 * (360.0 / 65536.0)
        } else {
            self.char() as f32 * (360.0 / 256.0)
        }
    }
    pub fn angle16(&mut self, flags: u32) -> f32 {
        if flags & PRFL_FLOATANGLE != 0 {
            self.float()
        } else {
            self.short() as f32 * (360.0 / 65536.0)
        }
    }
    pub fn vec3_coord(&mut self, flags: u32) -> [f32; 3] {
        [self.coord(flags), self.coord(flags), self.coord(flags)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for flags in [0, PRFL_INT32COORD | PRFL_SHORTANGLE, PRFL_FLOATCOORD | PRFL_FLOATANGLE, PRFL_24BITCOORD] {
            let mut b = SizeBuf::new(256);
            b.coord(123.5, flags);
            b.angle(90.0, flags);
            b.angle16(-45.0, flags);
            b.string(b"hi");
            b.short(-2);
            let mut r = MsgReader::new(&b.data);
            assert!((r.coord(flags) - 123.5).abs() < 0.13, "{flags}");
            assert!((r.angle(flags) - 90.0).abs() < 0.01);
            assert!((r.angle16(flags) + 45.0).abs() < 0.01);
            assert_eq!(r.string(), b"hi");
            assert_eq!(r.short(), -2);
            assert!(!r.badread);
            assert_eq!(r.byte(), -1);
            assert!(r.badread);
        }
        let mut b = SizeBuf::new(64);
        b.coord(1.0, 0);
        b.angle(90.0, 0);
        assert_eq!(b.data, [8, 0, 64]);
        let mut s = SizeBuf::new(2);
        s.long(1);
        assert!(s.overflowed && s.is_empty());
    }
}
