//! Little-endian bounds-checked cursor over a byte slice.
use crate::math::Vec3;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadError {
    pub pos: usize,
    pub need: usize,
    pub len: usize,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "read of {} bytes at {} past end ({})", self.need, self.pos, self.len)
    }
}
impl std::error::Error for ReadError {}

pub type ReadResult<T> = Result<T, ReadError>;

#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

macro_rules! rd {
    ($name:ident, $t:ty) => {
        pub fn $name(&mut self) -> ReadResult<$t> {
            Ok(<$t>::from_le_bytes(self.array()?))
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    /// Reader positioned at `pos` (error if past end).
    pub fn at(data: &'a [u8], pos: usize) -> ReadResult<Self> {
        let mut r = Self::new(data);
        r.seek(pos)?;
        Ok(r)
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    fn err(&self, need: usize) -> ReadError {
        ReadError { pos: self.pos, need, len: self.data.len() }
    }
    pub fn seek(&mut self, pos: usize) -> ReadResult<()> {
        if pos > self.data.len() {
            return Err(ReadError { pos, need: 0, len: self.data.len() });
        }
        self.pos = pos;
        Ok(())
    }
    pub fn skip(&mut self, n: usize) -> ReadResult<()> {
        self.bytes(n).map(|_| ())
    }
    pub fn bytes(&mut self, n: usize) -> ReadResult<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(|| self.err(n))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn array<const N: usize>(&mut self) -> ReadResult<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }
    rd!(u16, u16);
    rd!(i16, i16);
    rd!(u32, u32);
    rd!(i32, i32);
    rd!(f32, f32);
    pub fn u8(&mut self) -> ReadResult<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn i8(&mut self) -> ReadResult<i8> {
        Ok(self.u8()? as i8)
    }
    pub fn vec3(&mut self) -> ReadResult<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    /// Fixed-size NUL-padded string field (lossy UTF-8, truncated at first NUL).
    pub fn fixed_str(&mut self, n: usize) -> ReadResult<String> {
        let b = self.bytes(n)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(n);
        Ok(String::from_utf8_lossy(&b[..end]).into_owned())
    }
    /// NUL-terminated string (terminator consumed; end of data also terminates).
    pub fn cstr(&mut self) -> ReadResult<&'a [u8]> {
        let rest = &self.data[self.pos..];
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        self.pos += (end + 1).min(rest.len());
        Ok(&rest[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads() {
        let d = [1u8, 0, 0xff, 0xff, 0, 0, 0x80, 0x3f, b'h', b'i', 0, 9];
        let mut r = Reader::new(&d);
        assert_eq!(r.u16().unwrap(), 1);
        assert_eq!(r.i16().unwrap(), -1);
        assert_eq!(r.f32().unwrap(), 1.0);
        assert_eq!(r.cstr().unwrap(), b"hi");
        assert_eq!(r.u8().unwrap(), 9);
        assert_eq!(r.u8(), Err(ReadError { pos: 12, need: 1, len: 12 }));
        r.seek(8).unwrap();
        assert_eq!(r.fixed_str(4).unwrap(), "hi");
        assert!(r.seek(13).is_err());
        assert!(r.bytes(usize::MAX).is_err());
    }
}
