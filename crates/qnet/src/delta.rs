//! FTE replacement deltas (PEXT2_REPLACEMENTDELTAS): the entity state both
//! sides track, and its wire encoding (vkQuake sv_main.c
//! MSGFTE_DeltaCalcBits / MSGFTE_WriteEntityUpdate, cl_parse.c
//! CLFTE_ReadDelta).
//!
//! An `svcfte_updateentities` message is `[short ack]` (PEXT2_PREDINFO only),
//! `float time`, then entity records until a zero short. A record starts
//! with the entity number (`0x8000` remove, `0x4000` plus a byte for bits
//! 14..21), followed for updates by the [`write_update`] fields. Removing
//! entity 0 means "forget every entity" (full resend).

use crate::msg::{MsgReader, SizeBuf};
use crate::protocol::{ENTALPHA_DEFAULT, ENTSCALE_DEFAULT};

/// 'FTEX': pext1 extensions follow in svc_serverinfo / the `pext` reply.
pub const PROTOCOL_FTE_PEXT1: u32 = u32::from_le_bytes(*b"FTEX");
/// 'FTE2': pext2 extensions follow.
pub const PROTOCOL_FTE_PEXT2: u32 = u32::from_le_bytes(*b"FTE2");
pub const PEXT2_REPLACEMENTDELTAS: u32 = 0x0000_0008;
pub const PEXT2_PREDINFO: u32 = 0x0000_0020;
/// pext2 bits this engine implements (both client and server).
pub const PEXT2_SUPPORTED: u32 = PEXT2_REPLACEMENTDELTAS;

pub const SVCFTE_SPAWNSTATIC2: u8 = 21;
pub const SVCDP_UPDATESTATBYTE: u8 = 51;
pub const SVCFTE_SPAWNBASELINE2: u8 = 66;
pub const SVCFTE_UPDATESTATSTRING: u8 = 78;
pub const SVCFTE_UPDATESTATFLOAT: u8 = 79;
pub const SVCFTE_UPDATEENTITIES: u8 = 86;
/// [long] sequence of a received svcfte_updateentities datagram (-1: resend all).
pub const CLCDP_ACKFRAME: u8 = 50;

pub const UF_FRAME: u32 = 1 << 0;
pub const UF_ORIGINXY: u32 = 1 << 1;
pub const UF_ORIGINZ: u32 = 1 << 2;
pub const UF_ANGLESXZ: u32 = 1 << 3;
pub const UF_ANGLESY: u32 = 1 << 4;
pub const UF_EFFECTS: u32 = 1 << 5;
pub const UF_PREDINFO: u32 = 1 << 6;
pub const UF_EXTEND1: u32 = 1 << 7;
pub const UF_RESET: u32 = 1 << 8;
pub const UF_16BIT: u32 = 1 << 9;
pub const UF_MODEL: u32 = 1 << 10;
pub const UF_SKIN: u32 = 1 << 11;
pub const UF_COLORMAP: u32 = 1 << 12;
pub const UF_SOLID: u32 = 1 << 13;
pub const UF_FLAGS: u32 = 1 << 14;
pub const UF_EXTEND2: u32 = 1 << 15;
pub const UF_ALPHA: u32 = 1 << 16;
pub const UF_SCALE: u32 = 1 << 17;
pub const UF_BONEDATA: u32 = 1 << 18;
pub const UF_DRAWFLAGS: u32 = 1 << 19;
pub const UF_TAGINFO: u32 = 1 << 20;
pub const UF_LIGHT: u32 = 1 << 21;
pub const UF_TRAILEFFECT: u32 = 1 << 22;
pub const UF_EXTEND3: u32 = 1 << 23;
pub const UF_COLORMOD: u32 = 1 << 24;
pub const UF_GLOW: u32 = 1 << 25;
pub const UF_FATNESS: u32 = 1 << 26;
pub const UF_MODELINDEX2: u32 = 1 << 27;
pub const UF_GRAVITYDIR: u32 = 1 << 28;
pub const UF_EFFECTS2: u32 = 1 << 29;
pub const UF_UNUSED2: u32 = 1 << 30;
pub const UF_UNUSED1: u32 = 1 << 31;

pub const UFP_FORWARD: u32 = 1 << 0;
pub const UFP_SIDE: u32 = 1 << 1;
pub const UFP_UP: u32 = 1 << 2;
pub const UFP_MOVETYPE: u32 = 1 << 3;
pub const UFP_VELOCITYXY: u32 = 1 << 4;
pub const UFP_VELOCITYZ: u32 = 1 << 5;
pub const UFP_MSEC: u32 = 1 << 6;
/// UFP_WEAPONFRAME_OLD without PEXT2_PREDINFO, UFP_VIEWANGLE with it.
pub const UFP_BIT7: u32 = 1 << 7;

/// eflags: MOVETYPE_STEP lerping.
pub const EFLAGS_STEP: u8 = 1;

/// Effect numbers are particle precache indices (14 bits).
pub const MAX_PARTICLETYPES: u16 = 2048;

/// entity_state_t fields carried by replacement deltas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntState {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub modelindex: u32,
    pub frame: u32,
    pub skin: u32,
    pub colormap: u8,
    pub effects: u32,
    pub eflags: u8,
    /// ENTALPHA encoding (0 = default).
    pub alpha: u8,
    /// ENTSCALE encoding (16 = 1.0).
    pub scale: u8,
    /// Particle precache index of the trail (0 = none).
    pub traileffect: u16,
    /// Particle precache index emitted every frame (0 = none).
    pub emiteffect: u16,
    pub colormod: [u8; 3],
}

impl Default for EntState {
    /// nullentitystate.
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            angles: [0.0; 3],
            modelindex: 0,
            frame: 0,
            skin: 0,
            colormap: 0,
            effects: 0,
            eflags: 0,
            alpha: ENTALPHA_DEFAULT,
            scale: ENTSCALE_DEFAULT,
            traileffect: 0,
            emiteffect: 0,
            colormod: [32; 3],
        }
    }
}

/// MSGFTE_DeltaCalcBits without prediction info (this server sends none).
pub fn calc_bits(from: &EntState, to: &EntState) -> u32 {
    let mut bits = 0;
    if to.origin[0] != from.origin[0] || to.origin[1] != from.origin[1] {
        bits |= UF_ORIGINXY;
    }
    if to.origin[2] != from.origin[2] {
        bits |= UF_ORIGINZ;
    }
    if to.angles[0] != from.angles[0] || to.angles[2] != from.angles[2] {
        bits |= UF_ANGLESXZ;
    }
    if to.angles[1] != from.angles[1] {
        bits |= UF_ANGLESY;
    }
    if to.modelindex != from.modelindex {
        bits |= UF_MODEL;
    }
    if to.frame != from.frame {
        bits |= UF_FRAME;
    }
    if to.skin != from.skin {
        bits |= UF_SKIN;
    }
    if to.colormap != from.colormap {
        bits |= UF_COLORMAP;
    }
    if to.effects != from.effects {
        bits |= UF_EFFECTS;
    }
    if to.eflags != from.eflags {
        bits |= UF_FLAGS;
    }
    if to.scale != from.scale {
        bits |= UF_SCALE;
    }
    if to.alpha != from.alpha {
        bits |= UF_ALPHA;
    }
    if to.colormod != from.colormod {
        bits |= UF_COLORMOD;
    }
    if to.traileffect != from.traileffect || to.emiteffect != from.emiteffect {
        bits |= UF_TRAILEFFECT;
    }
    bits
}

/// MSGFTE_WriteEntityUpdate for `bits` from [`calc_bits`] (plus UF_RESET).
pub fn write_update(mut bits: u32, s: &EntState, msg: &mut SizeBuf, pf: u32) {
    if (bits & UF_MODEL != 0 && s.modelindex > 255) || (bits & UF_FRAME != 0 && s.frame > 255) {
        bits |= UF_16BIT;
    }
    if bits & UF_SKIN != 0 && s.skin > 255 {
        bits |= UF_16BIT;
    }
    if bits & UF_EFFECTS != 0 {
        if s.effects & 0xffff_0000 != 0 {
            bits |= UF_EFFECTS | UF_EFFECTS2;
        } else if s.effects & 0x0000_ff00 != 0 {
            bits = (bits & !UF_EFFECTS) | UF_EFFECTS2;
        }
    }
    if bits & 0xff00_0000 != 0 {
        bits |= UF_EXTEND3;
    }
    if bits & 0x00ff_0000 != 0 {
        bits |= UF_EXTEND2;
    }
    if bits & 0x0000_ff00 != 0 {
        bits |= UF_EXTEND1;
    }
    msg.byte((bits & 0xff) as i32);
    for (ext, shift) in [(UF_EXTEND1, 8), (UF_EXTEND2, 16), (UF_EXTEND3, 24)] {
        if bits & ext != 0 {
            msg.byte(((bits >> shift) & 0xff) as i32);
        }
    }
    let wide = |msg: &mut SizeBuf, v: u32| if bits & UF_16BIT != 0 { msg.short(v as i32) } else { msg.byte(v as i32) };
    if bits & UF_FRAME != 0 {
        wide(msg, s.frame);
    }
    if bits & UF_ORIGINXY != 0 {
        msg.coord(s.origin[0], pf);
        msg.coord(s.origin[1], pf);
    }
    if bits & UF_ORIGINZ != 0 {
        msg.coord(s.origin[2], pf);
    }
    if bits & UF_ANGLESXZ != 0 {
        msg.angle(s.angles[0], pf);
        msg.angle(s.angles[2], pf);
    }
    if bits & UF_ANGLESY != 0 {
        msg.angle(s.angles[1], pf);
    }
    if bits & (UF_EFFECTS | UF_EFFECTS2) == UF_EFFECTS | UF_EFFECTS2 {
        msg.long(s.effects as i32);
    } else if bits & UF_EFFECTS2 != 0 {
        msg.short(s.effects as i32);
    } else if bits & UF_EFFECTS != 0 {
        msg.byte(s.effects as i32);
    }
    if bits & UF_MODEL != 0 {
        wide(msg, s.modelindex);
    }
    if bits & UF_SKIN != 0 {
        wide(msg, s.skin);
    }
    if bits & UF_COLORMAP != 0 {
        msg.byte(s.colormap as i32);
    }
    if bits & UF_FLAGS != 0 {
        msg.byte(s.eflags as i32);
    }
    if bits & UF_ALPHA != 0 {
        msg.byte(s.alpha.wrapping_sub(1) as i32);
    }
    if bits & UF_SCALE != 0 {
        msg.byte(s.scale as i32);
    }
    if bits & UF_TRAILEFFECT != 0 {
        let trail = (s.traileffect & 0x3fff) as i32;
        if s.emiteffect != 0 {
            msg.short(trail | 0x8000);
            msg.short((s.emiteffect & 0x3fff) as i32);
        } else {
            msg.short(trail);
        }
    }
    if bits & UF_COLORMOD != 0 {
        for c in s.colormod {
            msg.byte(c as i32);
        }
    }
}

/// CLFTE_ReadDelta: reads one entity's fields into `s`, which holds the
/// previous state (or the baseline when the update has UF_RESET; see the
/// returned bits). `pext2` decides how prediction info is laid out.
/// Returns the bits, or `None` for fields this client can't skip.
pub fn read_delta(m: &mut MsgReader<'_>, s: &mut EntState, baseline: &EntState, pf: u32, pext2: u32) -> Option<u32> {
    let mut bits = m.byte() as u32 & 0xff;
    for (ext, shift) in [(UF_EXTEND1, 8), (UF_EXTEND2, 16), (UF_EXTEND3, 24)] {
        if bits & ext != 0 {
            bits |= (m.byte() as u32 & 0xff) << shift;
        }
    }
    if bits & UF_RESET != 0 {
        *s = *baseline;
    }
    let wide = |m: &mut MsgReader<'_>| if bits & UF_16BIT != 0 { m.short() as u16 as u32 } else { m.byte() as u8 as u32 };
    if bits & UF_FRAME != 0 {
        s.frame = wide(m);
    }
    if bits & UF_ORIGINXY != 0 {
        s.origin[0] = m.coord(pf);
        s.origin[1] = m.coord(pf);
    }
    if bits & UF_ORIGINZ != 0 {
        s.origin[2] = m.coord(pf);
    }
    // Predicted entities get 16-bit angles (unless PREDINFO reworks them).
    let ang16 = bits & UF_PREDINFO != 0 && pext2 & PEXT2_PREDINFO == 0;
    let angle = |m: &mut MsgReader<'_>| if ang16 { m.angle16(pf) } else { m.angle(pf) };
    if bits & UF_ANGLESXZ != 0 {
        s.angles[0] = angle(m);
        s.angles[2] = angle(m);
    }
    if bits & UF_ANGLESY != 0 {
        s.angles[1] = angle(m);
    }
    if bits & (UF_EFFECTS | UF_EFFECTS2) == UF_EFFECTS | UF_EFFECTS2 {
        s.effects = m.long() as u32;
    } else if bits & UF_EFFECTS2 != 0 {
        s.effects = m.short() as u16 as u32;
    } else if bits & UF_EFFECTS != 0 {
        s.effects = m.byte() as u8 as u32;
    }
    if bits & UF_PREDINFO != 0 {
        // movement, movetype, velocity, msec: prediction isn't implemented.
        let pb = m.byte() as u32;
        for f in [UFP_FORWARD, UFP_SIDE, UFP_UP] {
            if pb & f != 0 {
                m.short();
            }
        }
        if pb & UFP_MOVETYPE != 0 {
            m.byte();
        }
        if pb & UFP_VELOCITYXY != 0 {
            m.short();
            m.short();
        }
        if pb & UFP_VELOCITYZ != 0 {
            m.short();
        }
        if pb & UFP_MSEC != 0 {
            m.byte();
        }
        if pb & UFP_BIT7 != 0 {
            if pext2 & PEXT2_PREDINFO != 0 {
                // view angles
                let n = (bits & UF_ANGLESXZ != 0) as usize * 2 + (bits & UF_ANGLESY != 0) as usize;
                for _ in 0..n {
                    m.short();
                }
            } else if m.byte() & 0x80 != 0 {
                // old weapon frame
                m.byte();
            }
        }
    }
    if bits & UF_MODEL != 0 {
        s.modelindex = wide(m);
    }
    if bits & UF_SKIN != 0 {
        s.skin = wide(m);
    }
    if bits & UF_COLORMAP != 0 {
        s.colormap = m.byte() as u8;
    }
    if bits & UF_SOLID != 0 {
        // MSG_ReadSize16
        if m.short() as u16 == 0x8000 {
            m.float();
            m.float();
            m.float();
            m.float();
            m.float();
            m.float();
        }
    }
    if bits & UF_FLAGS != 0 {
        s.eflags = m.byte() as u8;
    }
    if bits & UF_ALPHA != 0 {
        s.alpha = (m.byte() as u8).wrapping_add(1);
    }
    if bits & UF_SCALE != 0 {
        s.scale = m.byte() as u8;
    }
    if bits & UF_BONEDATA != 0 {
        let fl = m.byte() as u8;
        if fl & 0x80 != 0 {
            let n = m.byte() as u8 as usize;
            for _ in 0..n * 7 {
                m.short();
            }
        }
        if fl & 0x40 != 0 {
            m.byte();
            m.short();
        }
        if fl & 0x3f != 0 {
            return None;
        }
    }
    if bits & UF_DRAWFLAGS != 0 && m.byte() & 7 == 7 {
        m.byte();
    }
    if bits & UF_TAGINFO != 0 {
        read_entity(m, pext2);
        m.byte();
    }
    if bits & UF_LIGHT != 0 {
        for _ in 0..4 {
            m.short();
        }
        m.byte();
        m.byte();
    }
    if bits & UF_TRAILEFFECT != 0 {
        let v = m.short() as u16;
        s.traileffect = v & 0x3fff;
        s.emiteffect = if v & 0x8000 != 0 { m.short() as u16 & 0x3fff } else { 0 };
        if s.traileffect >= MAX_PARTICLETYPES {
            s.traileffect = 0;
        }
        if s.emiteffect >= MAX_PARTICLETYPES {
            s.emiteffect = 0;
        }
    }
    if bits & UF_COLORMOD != 0 {
        for c in &mut s.colormod {
            *c = m.byte() as u8;
        }
    }
    if bits & UF_GLOW != 0 {
        for _ in 0..5 {
            m.byte();
        }
    }
    if bits & UF_FATNESS != 0 {
        m.byte();
    }
    if bits & UF_MODELINDEX2 != 0 {
        wide(m);
    }
    if bits & UF_GRAVITYDIR != 0 {
        m.byte();
        m.byte();
    }
    if bits & (UF_UNUSED1 | UF_UNUSED2) != 0 {
        return None;
    }
    Some(bits)
}

/// Entity number in an updateentities record: `(num, remove)`; `None` ends
/// the list.
pub fn read_record_num(m: &mut MsgReader<'_>) -> Option<(u32, bool)> {
    let n = m.short() as u16 as u32;
    let remove = n & 0x8000 != 0;
    let num = if n & 0x4000 != 0 { (n & 0x3fff) | ((m.byte() as u32 & 0xff) << 14) } else { n & !0x8000 };
    (num != 0 || remove).then_some((num, remove))
}

/// Entity number header of an update (`remove` = false) or removal.
pub fn write_record_num(msg: &mut SizeBuf, num: u32, remove: bool) {
    let flag = if remove { 0x8000 } else { 0 };
    if num >= 0x4000 {
        msg.short((flag | 0x4000 | (num & 0x3fff)) as i32);
        msg.byte(((num >> 14) & 0xff) as i32);
    } else {
        msg.short((flag | num) as i32);
    }
}

/// MSG_ReadEntity: with replacement deltas, `0x8000` marks a 23-bit number.
pub fn read_entity(m: &mut MsgReader<'_>, pext2: u32) -> u32 {
    let e = m.short() as u16 as u32;
    if pext2 & PEXT2_REPLACEMENTDELTAS != 0 && e & 0x8000 != 0 { ((e & 0x7fff) << 8) | (m.byte() as u32 & 0xff) } else { e }
}

/// MSG_WriteEntity.
pub fn write_entity(msg: &mut SizeBuf, e: u32, pext2: u32) {
    if e > 0x7fff && pext2 & PEXT2_REPLACEMENTDELTAS != 0 {
        msg.short((0x8000 | (e >> 8)) as i32);
        msg.byte((e & 0xff) as i32);
    } else {
        msg.short(e as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(from: &EntState, to: &EntState, extra: u32) -> (EntState, u32) {
        let mut b = SizeBuf::new(1024);
        write_update(calc_bits(from, to) | extra, to, &mut b, 0);
        let mut m = MsgReader::new(&b.data);
        let mut s = *from;
        let bits = read_delta(&mut m, &mut s, &EntState::default(), 0, PEXT2_REPLACEMENTDELTAS).unwrap();
        assert!(m.at_end() && !m.badread);
        (s, bits)
    }

    #[test]
    fn delta_roundtrip() {
        let a = EntState::default();
        let b = EntState {
            origin: [16.0, -32.0, 8.0],
            angles: [0.0, 90.0, 0.0],
            modelindex: 300,
            frame: 7,
            skin: 1,
            colormap: 3,
            effects: 0x1_0004,
            eflags: EFLAGS_STEP,
            alpha: 128,
            scale: 32,
            traileffect: 5,
            emiteffect: 9,
            colormod: [64, 32, 0],
        };
        let (s, bits) = roundtrip(&a, &b, 0);
        assert_eq!(s, b);
        assert!(bits & UF_16BIT != 0 && bits & UF_TRAILEFFECT != 0);
        // Only what changed is sent; trail without emit is one short.
        let c = EntState { traileffect: 6, emiteffect: 0, ..b };
        let (s, bits) = roundtrip(&b, &c, 0);
        assert_eq!(s, c);
        assert_eq!(bits & !(UF_EXTEND1 | UF_EXTEND2 | UF_EXTEND3), UF_TRAILEFFECT);
        // Reset starts from the baseline.
        let (s, _) = roundtrip(&b, &a, UF_RESET);
        assert_eq!(s, a);
    }

    #[test]
    fn record_numbers() {
        let mut b = SizeBuf::new(64);
        for (n, rm) in [(5, false), (0x4001, true), (0x12345, false), (0, true)] {
            write_record_num(&mut b, n, rm);
        }
        b.short(0);
        let mut m = MsgReader::new(&b.data);
        assert_eq!(read_record_num(&mut m), Some((5, false)));
        assert_eq!(read_record_num(&mut m), Some((0x4001, true)));
        assert_eq!(read_record_num(&mut m), Some((0x12345, false)));
        assert_eq!(read_record_num(&mut m), Some((0, true)));
        assert_eq!(read_record_num(&mut m), None);
        let mut b = SizeBuf::new(8);
        write_entity(&mut b, 0x9000, PEXT2_REPLACEMENTDELTAS);
        assert_eq!(read_entity(&mut MsgReader::new(&b.data), PEXT2_REPLACEMENTDELTAS), 0x9000);
    }
}
