//! Message buffers and NetQuake wire encoding (common.c MSG_Write*, protocol.h).

/// sizebuf_t. Overflow clears the buffer and sets `overflowed` (allowoverflow semantics).
#[derive(Debug, Clone, Default)]
pub struct SizeBuf {
    pub data: Vec<u8>,
    pub maxsize: usize,
    pub overflowed: bool,
}

/// C `Q_rint`.
#[inline]
pub fn q_rint(x: f32) -> i32 {
    if x > 0.0 { (x + 0.5) as i32 } else { (x - 0.5) as i32 }
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

pub const PROTOCOL_NETQUAKE: u32 = 15;
pub const PROTOCOL_FITZQUAKE: u32 = 666;
pub const PROTOCOL_RMQ: u32 = 999;

pub const PRFL_SHORTANGLE: u32 = 1 << 1;
pub const PRFL_FLOATANGLE: u32 = 1 << 2;
pub const PRFL_24BITCOORD: u32 = 1 << 3;
pub const PRFL_FLOATCOORD: u32 = 1 << 4;
pub const PRFL_EDICTSCALE: u32 = 1 << 5;
pub const PRFL_ALPHASANITY: u32 = 1 << 6;
pub const PRFL_INT32COORD: u32 = 1 << 7;

pub const U_MOREBITS: u32 = 1 << 0;
pub const U_ORIGIN1: u32 = 1 << 1;
pub const U_ORIGIN2: u32 = 1 << 2;
pub const U_ORIGIN3: u32 = 1 << 3;
pub const U_ANGLE2: u32 = 1 << 4;
pub const U_STEP: u32 = 1 << 5;
pub const U_FRAME: u32 = 1 << 6;
pub const U_SIGNAL: u32 = 1 << 7;
pub const U_ANGLE1: u32 = 1 << 8;
pub const U_ANGLE3: u32 = 1 << 9;
pub const U_MODEL: u32 = 1 << 10;
pub const U_COLORMAP: u32 = 1 << 11;
pub const U_SKIN: u32 = 1 << 12;
pub const U_EFFECTS: u32 = 1 << 13;
pub const U_LONGENTITY: u32 = 1 << 14;
pub const U_EXTEND1: u32 = 1 << 15;
pub const U_ALPHA: u32 = 1 << 16;
pub const U_FRAME2: u32 = 1 << 17;
pub const U_MODEL2: u32 = 1 << 18;
pub const U_LERPFINISH: u32 = 1 << 19;
pub const U_SCALE: u32 = 1 << 20;
pub const U_EXTEND2: u32 = 1 << 23;

pub const SU_VIEWHEIGHT: u32 = 1 << 0;
pub const SU_IDEALPITCH: u32 = 1 << 1;
pub const SU_PUNCH1: u32 = 1 << 2;
pub const SU_VELOCITY1: u32 = 1 << 5;
pub const SU_ITEMS: u32 = 1 << 9;
pub const SU_ONGROUND: u32 = 1 << 10;
pub const SU_INWATER: u32 = 1 << 11;
pub const SU_WEAPONFRAME: u32 = 1 << 12;
pub const SU_ARMOR: u32 = 1 << 13;
pub const SU_WEAPON: u32 = 1 << 14;
pub const SU_EXTEND1: u32 = 1 << 15;
pub const SU_WEAPON2: u32 = 1 << 16;
pub const SU_ARMOR2: u32 = 1 << 17;
pub const SU_AMMO2: u32 = 1 << 18;
pub const SU_SHELLS2: u32 = 1 << 19;
pub const SU_NAILS2: u32 = 1 << 20;
pub const SU_ROCKETS2: u32 = 1 << 21;
pub const SU_CELLS2: u32 = 1 << 22;
pub const SU_EXTEND2: u32 = 1 << 23;
pub const SU_WEAPONFRAME2: u32 = 1 << 24;
pub const SU_WEAPONALPHA: u32 = 1 << 25;

pub const SND_VOLUME: i32 = 1 << 0;
pub const SND_ATTENUATION: i32 = 1 << 1;
pub const SND_LARGEENTITY: i32 = 1 << 3;
pub const SND_LARGESOUND: i32 = 1 << 4;

pub const B_LARGEMODEL: i32 = 1 << 0;
pub const B_LARGEFRAME: i32 = 1 << 1;
pub const B_ALPHA: i32 = 1 << 2;
pub const B_SCALE: i32 = 1 << 3;

pub const ENTALPHA_DEFAULT: u8 = 0;
pub const ENTALPHA_ZERO: u8 = 1;
pub const ENTSCALE_DEFAULT: u8 = 16;

/// ENTALPHA_ENCODE.
pub fn entalpha_encode(a: f32) -> u8 {
    if a == 0.0 { ENTALPHA_DEFAULT } else { q_rint((a * 254.0 + 1.0).clamp(1.0, 255.0)) as u8 }
}
/// ENTSCALE_ENCODE.
pub fn entscale_encode(a: f32) -> u8 {
    if a != 0.0 { (a * ENTSCALE_DEFAULT as f32) as i32 as u8 } else { ENTSCALE_DEFAULT }
}

pub const DEFAULT_VIEWHEIGHT: f32 = 22.0;
pub const GAME_COOP: i32 = 0;
pub const GAME_DEATHMATCH: i32 = 1;

pub const SVC_NOP: i32 = 1;
pub const SVC_DISCONNECT: i32 = 2;
pub const SVC_UPDATESTAT: i32 = 3;
pub const SVC_SETVIEW: i32 = 5;
pub const SVC_SOUND: i32 = 6;
pub const SVC_TIME: i32 = 7;
pub const SVC_PRINT: i32 = 8;
pub const SVC_STUFFTEXT: i32 = 9;
pub const SVC_SETANGLE: i32 = 10;
pub const SVC_SERVERINFO: i32 = 11;
pub const SVC_LIGHTSTYLE: i32 = 12;
pub const SVC_UPDATENAME: i32 = 13;
pub const SVC_UPDATEFRAGS: i32 = 14;
pub const SVC_CLIENTDATA: i32 = 15;
pub const SVC_UPDATECOLORS: i32 = 17;
pub const SVC_PARTICLE: i32 = 18;
pub const SVC_DAMAGE: i32 = 19;
pub const SVC_SPAWNSTATIC: i32 = 20;
pub const SVC_SPAWNBASELINE: i32 = 22;
pub const SVC_SIGNONNUM: i32 = 25;
pub const SVC_CENTERPRINT: i32 = 26;
pub const SVC_SPAWNSTATICSOUND: i32 = 29;
pub const SVC_CDTRACK: i32 = 32;
pub const SVC_SPAWNBASELINE2: i32 = 42;
pub const SVC_SPAWNSTATIC2: i32 = 43;
pub const SVC_SPAWNSTATICSOUND2: i32 = 44;
pub const SVC_LOCALSOUND: i32 = 56;

pub const STAT_ACTIVEWEAPON: i32 = 10;
pub const STAT_TOTALSECRETS: i32 = 11;
pub const STAT_TOTALMONSTERS: i32 = 12;
pub const STAT_SECRETS: i32 = 13;
pub const STAT_MONSTERS: i32 = 14;

pub const MAX_MSGLEN: usize = 64000;
pub const MAX_DATAGRAM: usize = 64000;
pub const DATAGRAM_MTU: usize = 1400;
pub const SIGNON_SIZE: usize = 31500;
pub const MAX_SIGNON_BUFFERS: usize = 256;
pub const MAX_MODELS: usize = 4096;
pub const MAX_SOUNDS: usize = 2048;
pub const MAX_LIGHTSTYLES: usize = 64;
pub const NUM_SPAWN_PARMS: usize = 16;
pub const EF_MUZZLEFLASH: i32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        let mut b = SizeBuf::new(64);
        b.coord(1.0, 0);
        b.coord(-1.0, PRFL_INT32COORD);
        b.angle(90.0, 0);
        b.angle(90.0, PRFL_SHORTANGLE);
        b.string(b"hi");
        assert_eq!(b.data, [8, 0, 0xf0, 0xff, 0xff, 0xff, 64, 0, 0x40, b'h', b'i', 0]);
        let mut s = SizeBuf::new(2);
        s.long(1);
        assert!(s.overflowed && s.is_empty());
        assert_eq!(entalpha_encode(1.0), 255);
        assert_eq!(entalpha_encode(0.0), 0);
    }
}
