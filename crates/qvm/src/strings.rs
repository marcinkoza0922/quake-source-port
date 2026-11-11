//! QC string handles. Port of PR_GetString / PR_SetEngineString / PR_AllocString
//! plus QSS tempstrings and strzone bookkeeping.
//!
//! Convention (as in the references): `h >= 0` is an offset into the progs
//! string lump; `h < 0` is engine string slot `-1 - h`.

use std::collections::HashMap;

use crate::VmError;

/// Number of rotating temp strings (STRINGTEMP_BUFFERS).
pub const STRINGTEMP_BUFFERS: usize = 1024;
/// Max temp string length incl. NUL (STRINGTEMP_LENGTH).
pub const STRINGTEMP_LENGTH: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrKind {
    /// Rotating tempstring slot (overwritten after 1024 more temps).
    Temp,
    /// strzone'd; freed by strunzone.
    Zone,
    /// ED_NewString / engine allocations; live until the next progs load.
    Alloc,
    /// Interned engine constant (PR_SetEngineString on an engine-owned string).
    Engine,
}

#[derive(Debug, Clone)]
struct Slot {
    kind: StrKind,
    data: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct Strings {
    progs: Vec<u8>,
    known: Vec<Option<Slot>>,
    free: Vec<usize>,
    temp_index: usize,
    interned: HashMap<Vec<u8>, i32>,
}

impl Strings {
    /// `progs` is the NUL-terminated string lump.
    pub fn new(progs: Vec<u8>) -> Self {
        let mut s = Strings { progs, ..Default::default() };
        s.known = (0..STRINGTEMP_BUFFERS)
            .map(|_| Some(Slot { kind: StrKind::Temp, data: Vec::new() }))
            .collect();
        s
    }

    /// PR_GetString: bytes (no NUL) for a handle.
    pub fn get(&self, h: i32) -> Result<&[u8], VmError> {
        if h >= 0 && (h as usize) < self.progs.len() {
            let s = &self.progs[h as usize..];
            return Ok(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]);
        }
        if h < 0 && ((-1 - h as i64) as usize) < self.known.len() {
            return match &self.known[(-1 - h) as usize] {
                Some(s) => Ok(&s.data),
                None => Err(VmError::new(format!("PR_GetString: attempt to get a non-existant string {h}"))),
            };
        }
        Err(VmError::new(format!("PR_GetString: invalid string offset {h}")))
    }

    pub fn kind(&self, h: i32) -> Option<StrKind> {
        if h >= 0 {
            return None;
        }
        self.known.get((-1 - h as i64) as usize)?.as_ref().map(|s| s.kind)
    }

    fn alloc_slot(&mut self, kind: StrKind, data: Vec<u8>) -> i32 {
        let slot = Some(Slot { kind, data });
        let i = match self.free.pop() {
            Some(i) => {
                self.known[i] = slot;
                i
            }
            None => {
                self.known.push(slot);
                self.known.len() - 1
            }
        };
        -1 - i as i32
    }

    /// QSS PR_MakeTempString: copy into the next rotating temp buffer (truncated to 1023 bytes).
    pub fn temp(&mut self, s: &[u8]) -> i32 {
        self.temp_index = (self.temp_index + 1) & (STRINGTEMP_BUFFERS - 1);
        let n = s.len().min(STRINGTEMP_LENGTH - 1);
        let slot = self.known[self.temp_index].as_mut().unwrap();
        slot.data.clear();
        slot.data.extend_from_slice(&s[..n]);
        -1 - self.temp_index as i32
    }

    /// PR_AllocString: persistent (until progs reload) string.
    pub fn alloc(&mut self, s: &[u8]) -> i32 {
        self.alloc_slot(StrKind::Alloc, s.to_vec())
    }

    /// ED_NewString: like [`Self::alloc`] but turns `\n` escapes into newlines.
    pub fn new_string(&mut self, s: &[u8]) -> i32 {
        let mut out = Vec::with_capacity(s.len());
        let mut i = 0;
        while i < s.len() {
            if s[i] == b'\\' && i + 1 < s.len() {
                i += 1;
                out.push(if s[i] == b'n' { b'\n' } else { b'\\' });
            } else {
                out.push(s[i]);
            }
            i += 1;
        }
        self.alloc(&out)
    }

    /// PR_SetEngineString for engine-owned constants: interned, stable handle.
    pub fn engine(&mut self, s: &[u8]) -> i32 {
        if s.is_empty() {
            return 0;
        }
        if let Some(&h) = self.interned.get(s) {
            return h;
        }
        let h = self.alloc_slot(StrKind::Engine, s.to_vec());
        self.interned.insert(s.to_vec(), h);
        h
    }

    /// strzone: allocate a zoned copy.
    pub fn zone(&mut self, s: &[u8]) -> i32 {
        self.alloc_slot(StrKind::Zone, s.to_vec())
    }

    /// strunzone. Returns false (warning in the references) if `h` was not zoned.
    pub fn unzone(&mut self, h: i32) -> bool {
        if h == 0 {
            return true;
        }
        if self.kind(h) != Some(StrKind::Zone) {
            return false;
        }
        let i = (-1 - h) as usize;
        self.known[i] = None;
        self.free.push(i);
        true
    }

    /// ED_RezoneString: replace a zoned string in place (frees old zone string).
    pub fn rezone(&mut self, old: i32, s: &[u8]) -> i32 {
        if self.kind(old) == Some(StrKind::Zone) {
            self.unzone(old);
        }
        self.zone(s)
    }

    /// Number of live engine slots (excluding temps).
    pub fn live_count(&self) -> usize {
        self.known[STRINGTEMP_BUFFERS..].iter().filter(|s| s.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles() {
        let mut s = Strings::new(b"\0abc\0".to_vec());
        assert_eq!(s.get(1).unwrap(), b"abc");
        let t = s.temp(b"x");
        assert!(t < 0 && s.get(t).unwrap() == b"x");
        let z = s.zone(b"zz");
        assert!(s.unzone(z) && s.get(z).is_err() && !s.unzone(t));
        let n = s.new_string(b"a\\nb");
        assert_eq!(s.get(n).unwrap(), b"a\nb");
        assert_eq!(s.engine(b"m"), s.engine(b"m"));
        assert!(s.get(99).is_err());
    }
}
