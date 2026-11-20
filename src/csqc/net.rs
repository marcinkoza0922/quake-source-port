//! CSQC `read*` network event reader (`CSQC_Parse_Event` / `svcfte_cgamepacket`).

use qnet::protocol::{PRFL_24BITCOORD, PRFL_FLOATCOORD, PRFL_INT32COORD, PRFL_SHORTANGLE};

/// Cursor over a `svcfte_cgamepacket` payload.
#[derive(Default, Clone)]
pub struct PacketReader {
    data: Vec<u8>,
    pos: usize,
    pub protocol_flags: u32,
}

impl PacketReader {
    pub fn reset(&mut self, data: &[u8], protocol_flags: u32) {
        self.data.clear();
        self.data.extend_from_slice(data);
        self.pos = 0;
        self.protocol_flags = protocol_flags;
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.pos = 0;
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    pub fn read_byte(&mut self) -> i32 {
        self.take(1).map_or(-1, |b| b[0] as i32)
    }

    pub fn read_char(&mut self) -> i32 {
        self.take(1).map_or(-1, |b| b[0] as i8 as i32)
    }

    pub fn read_short(&mut self) -> i32 {
        self.take(2)
            .map_or(-1, |b| i16::from_le_bytes([b[0], b[1]]) as i32)
    }

    pub fn read_long(&mut self) -> i32 {
        self.take(4)
            .map_or(-1, |b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_float(&mut self) -> f32 {
        self.take(4)
            .map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_coord(&mut self) -> f32 {
        if self.protocol_flags & PRFL_FLOATCOORD != 0 {
            self.read_float()
        } else if self.protocol_flags & PRFL_INT32COORD != 0 {
            self.read_long() as f32 / 16.0
        } else if self.protocol_flags & PRFL_24BITCOORD != 0 {
            let lo = self.read_short();
            let frac = self.read_byte();
            lo as f32 + frac as f32 / 255.0
        } else {
            self.read_short() as f32 * 0.125
        }
    }

    pub fn read_angle(&mut self) -> f32 {
        if self.protocol_flags & PRFL_SHORTANGLE != 0 {
            self.read_short() as f32 * (360.0 / 65536.0)
        } else {
            self.read_char() as f32 * (360.0 / 256.0)
        }
    }

    pub fn read_string(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(&b) = self.data.get(self.pos) {
            self.pos += 1;
            if b == 0 {
                break;
            }
            out.push(b);
        }
        out
    }

    pub fn read_entity_num(&mut self) -> i32 {
        let v = self.take(2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        v as i32
    }
}
