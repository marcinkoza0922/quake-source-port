//! Demo playback (CL_PlayDemo_f / CL_GetMessage). A .dem file is a text
//! line holding a CD track number, then blocks of
//! `i32 length, 3 × f32 view angles, length bytes of server message`.

pub struct Demo {
    data: Vec<u8>,
    pos: usize,
}

/// One recorded server message with the view angles at the time.
pub struct DemoMessage<'a> {
    pub angles: [f32; 3],
    pub data: &'a [u8],
}

/// Largest message a demo block may hold (MAX_MSGLEN in ironwail).
const MAX_MSGLEN: usize = 96_000;

impl Demo {
    pub fn new(data: Vec<u8>) -> Self {
        // The header (cls.forcetrack, unused: svc_cdtrack follows) is an
        // optional '-' and digits ending in '\n'.
        let pos = data.iter().take(16).position(|&b| b == b'\n').map_or(0, |e| e + 1);
        Demo { data, pos }
    }

    /// The next message, or `None` at the end of the file or on a damaged block.
    pub fn next_message(&mut self) -> Option<DemoMessage<'_>> {
        let rest = self.data.get(self.pos..)?;
        if rest.len() < 16 {
            return None;
        }
        let word = |i: usize| <[u8; 4]>::try_from(&rest[i * 4..i * 4 + 4]).unwrap();
        let len = i32::from_le_bytes(word(0));
        if len < 0 || len as usize > MAX_MSGLEN || rest.len() < 16 + len as usize {
            return None;
        }
        let angles = [1, 2, 3].map(|i| f32::from_le_bytes(word(i)));
        let start = self.pos + 16;
        self.pos = start + len as usize;
        Some(DemoMessage { angles, data: &self.data[start..self.pos] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(angles: [f32; 3], msg: &[u8]) -> Vec<u8> {
        let mut v = (msg.len() as i32).to_le_bytes().to_vec();
        for a in angles {
            v.extend(a.to_le_bytes());
        }
        v.extend(msg);
        v
    }

    #[test]
    fn reads_header_and_blocks() {
        let mut d = b"-1\n".to_vec();
        d.extend(block([1.0, 2.0, 3.0], &[7, 8]));
        d.extend(block([0.0; 3], &[]));
        d.extend([1, 2, 3]); // truncated tail
        let mut demo = Demo::new(d);
        let m = demo.next_message().unwrap();
        assert_eq!((m.angles, m.data), ([1.0, 2.0, 3.0], &[7u8, 8][..]));
        assert!(demo.next_message().unwrap().data.is_empty());
        assert!(demo.next_message().is_none());
    }

    #[test]
    fn rejects_oversized_block() {
        let mut d = b"2\n".to_vec();
        d.extend(i32::MAX.to_le_bytes());
        d.extend([0; 12]);
        let mut demo = Demo::new(d);
        assert!(demo.next_message().is_none());
    }
}
