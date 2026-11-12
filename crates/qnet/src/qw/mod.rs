//! QuakeWorld protocol 28 (QW/client protocol.h, net_chan.c, common.c,
//! crc.c, md4.c): constants, the sequenced netchan, delta usercmds,
//! the move checksum byte and the map checksum.

mod md4;
mod tables;

pub use md4::block_checksum;

use crate::{MsgReader, SizeBuf};
mod consts;
pub use consts::*;

/// MSG_ReadCoord (1/8 unit shorts).
pub fn read_coord(m: &mut MsgReader<'_>) -> f32 {
    m.short() as f32 * (1.0 / 8.0)
}
/// MSG_ReadAngle (signed byte).
pub fn read_angle(m: &mut MsgReader<'_>) -> f32 {
    m.char() as f32 * (360.0 / 256.0)
}
pub fn read_angle16(m: &mut MsgReader<'_>) -> f32 {
    m.short() as f32 * (360.0 / 65536.0)
}
pub fn write_coord(b: &mut SizeBuf, f: f32) {
    b.short((f * 8.0) as i32);
}
pub fn write_angle(b: &mut SizeBuf, f: f32) {
    b.byte((f * 256.0 / 360.0) as i32 & 255);
}
pub fn write_angle16(b: &mut SizeBuf, f: f32) {
    b.short((f * 65536.0 / 360.0) as i32 & 65535);
}

/// usercmd_t.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UserCmd {
    pub msec: u8,
    pub angles: [f32; 3],
    pub forwardmove: i16,
    pub sidemove: i16,
    pub upmove: i16,
    pub buttons: u8,
    pub impulse: u8,
}

/// MSG_WriteDeltaUsercmd.
pub fn write_delta_usercmd(b: &mut SizeBuf, from: &UserCmd, cmd: &UserCmd) {
    let mut bits = 0;
    for (i, bit) in [CM_ANGLE1, CM_ANGLE2, CM_ANGLE3].into_iter().enumerate() {
        if cmd.angles[i] != from.angles[i] {
            bits |= bit;
        }
    }
    if cmd.forwardmove != from.forwardmove {
        bits |= CM_FORWARD;
    }
    if cmd.sidemove != from.sidemove {
        bits |= CM_SIDE;
    }
    if cmd.upmove != from.upmove {
        bits |= CM_UP;
    }
    if cmd.buttons != from.buttons {
        bits |= CM_BUTTONS;
    }
    if cmd.impulse != from.impulse {
        bits |= CM_IMPULSE;
    }
    b.byte(bits as i32);
    for (i, bit) in [CM_ANGLE1, CM_ANGLE2, CM_ANGLE3].into_iter().enumerate() {
        if bits & bit != 0 {
            write_angle16(b, cmd.angles[i]);
        }
    }
    for (bit, v) in [(CM_FORWARD, cmd.forwardmove), (CM_SIDE, cmd.sidemove), (CM_UP, cmd.upmove)] {
        if bits & bit != 0 {
            b.short(v as i32);
        }
    }
    if bits & CM_BUTTONS != 0 {
        b.byte(cmd.buttons as i32);
    }
    if bits & CM_IMPULSE != 0 {
        b.byte(cmd.impulse as i32);
    }
    b.byte(cmd.msec as i32);
}

/// MSG_ReadDeltaUsercmd.
pub fn read_delta_usercmd(m: &mut MsgReader<'_>, from: &UserCmd) -> UserCmd {
    let mut c = *from;
    let bits = m.byte() as u8;
    for (i, bit) in [CM_ANGLE1, CM_ANGLE2, CM_ANGLE3].into_iter().enumerate() {
        if bits & bit != 0 {
            c.angles[i] = read_angle16(m);
        }
    }
    if bits & CM_FORWARD != 0 {
        c.forwardmove = m.short() as i16;
    }
    if bits & CM_SIDE != 0 {
        c.sidemove = m.short() as i16;
    }
    if bits & CM_UP != 0 {
        c.upmove = m.short() as i16;
    }
    if bits & CM_BUTTONS != 0 {
        c.buttons = m.byte() as u8;
    }
    if bits & CM_IMPULSE != 0 {
        c.impulse = m.byte() as u8;
    }
    c.msec = m.byte() as u8;
    c
}

/// CRC_Block (CRC-16/CCITT, init 0xffff).
pub fn crc_block(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for &b in data {
        crc = (crc << 8) ^ tables::CRCTABLE[((crc >> 8) as u8 ^ b) as usize];
    }
    crc
}

/// COM_BlockSequenceCRCByte: the clc_move checksum byte.
pub fn sequence_crc_byte(base: &[u8], sequence: i32) -> u8 {
    // sizeof(chktbl) - 8 = 1020 in the C code (1024 + 4 map checksum bytes).
    let o = sequence.rem_euclid(1020) as usize;
    let p = [tables::chk(o), tables::chk(o + 1), tables::chk(o + 2), tables::chk(o + 3)];
    let n = base.len().min(60);
    let mut chk = Vec::with_capacity(n + 4);
    chk.extend_from_slice(&base[..n]);
    chk.push((sequence & 0xff) as u8 ^ p[0]);
    chk.push(p[1]);
    chk.push(((sequence >> 8) & 0xff) as u8 ^ p[2]);
    chk.push(p[3]);
    (crc_block(&chk) & 0xff) as u8
}

/// The BSP's checksum2 (CM_LoadMap): XOR of the MD4 block checksums of every
/// lump except entities, visibility, leafs and nodes. `None` if not a BSP.
pub fn map_checksum2(bsp: &[u8]) -> Option<u32> {
    const LUMP_ENTITIES: usize = 0;
    const LUMP_VISIBILITY: usize = 4;
    const LUMP_NODES: usize = 5;
    const LUMP_LEAFS: usize = 10;
    let rd = |o: usize| bsp.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    rd(0)?;
    let mut sum = 0u32;
    for i in 0..15 {
        if matches!(i, LUMP_ENTITIES | LUMP_VISIBILITY | LUMP_LEAFS | LUMP_NODES) {
            continue;
        }
        let ofs = rd(4 + i * 8)? as usize;
        let len = rd(8 + i * 8)? as usize;
        sum ^= block_checksum(bsp.get(ofs..ofs + len)?);
    }
    Some(sum)
}

/// An out-of-band packet: -1 sequence, then the payload.
pub fn oob(payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xff; 4];
    v.extend_from_slice(payload);
    v
}

/// Payload of an out-of-band packet.
pub fn oob_payload(pkt: &[u8]) -> Option<&[u8]> {
    pkt.strip_prefix(&[0xff, 0xff, 0xff, 0xff])
}

/// The client side of a netchan_t: sequenced unreliable packets with one
/// reliable message in flight, resent until acknowledged.
#[derive(Debug, Clone)]
pub struct Netchan {
    pub qport: u16,
    pub incoming_sequence: u32,
    pub incoming_acknowledged: u32,
    pub incoming_reliable_acknowledged: u32,
    pub incoming_reliable_sequence: u32,
    pub outgoing_sequence: u32,
    pub reliable_sequence: u32,
    pub last_reliable_sequence: u32,
    /// Reliable bytes waiting for the in-flight message to be acked.
    pub message: SizeBuf,
    reliable_buf: Vec<u8>,
    pub last_received: f64,
    /// Packets lost before the newest one (net_drop).
    pub drop: u32,
}

impl Netchan {
    pub fn new(qport: u16, now: f64) -> Self {
        Self {
            qport,
            incoming_sequence: 0,
            incoming_acknowledged: 0,
            incoming_reliable_acknowledged: 0,
            incoming_reliable_sequence: 0,
            outgoing_sequence: 0,
            reliable_sequence: 0,
            last_reliable_sequence: 0,
            message: SizeBuf::new(MAX_MSGLEN),
            reliable_buf: Vec::new(),
            last_received: now,
            drop: 0,
        }
    }

    /// Netchan_CanReliable (no bandwidth choke on the client).
    pub fn can_reliable(&self) -> bool {
        self.reliable_buf.is_empty()
    }

    /// Netchan_Transmit: the packet to send, carrying `data` unreliably and
    /// the reliable message when it is new or was dropped.
    pub fn transmit(&mut self, data: &[u8]) -> Vec<u8> {
        let mut send_reliable = self.incoming_acknowledged > self.last_reliable_sequence
            && self.incoming_reliable_acknowledged != self.reliable_sequence;
        if self.reliable_buf.is_empty() && !self.message.is_empty() {
            self.reliable_buf = std::mem::take(&mut self.message.data);
            self.reliable_sequence ^= 1;
            send_reliable = true;
        }
        let mut b = SizeBuf::new(MAX_MSGLEN + PACKET_HEADER);
        b.long((self.outgoing_sequence | (send_reliable as u32) << 31) as i32);
        b.long((self.incoming_sequence | self.incoming_reliable_sequence << 31) as i32);
        b.short(self.qport as i32);
        self.outgoing_sequence += 1;
        if send_reliable {
            b.write(&self.reliable_buf);
            self.last_reliable_sequence = self.outgoing_sequence;
        }
        if b.maxsize - b.len() >= data.len() {
            b.write(data);
        }
        b.data
    }

    /// Netchan_Process: the payload of an in-order packet, or None for a
    /// stale or duplicated one.
    pub fn process<'a>(&mut self, pkt: &'a [u8], now: f64) -> Option<&'a [u8]> {
        if pkt.len() < 8 {
            return None;
        }
        let w = |o: usize| u32::from_le_bytes(pkt[o..o + 4].try_into().unwrap());
        let (seq, ack) = (w(0), w(4));
        let reliable_message = seq >> 31;
        let reliable_ack = ack >> 31;
        let seq = seq & !(1 << 31);
        let ack = ack & !(1 << 31);
        if seq <= self.incoming_sequence {
            return None;
        }
        self.drop = seq - (self.incoming_sequence + 1);
        if reliable_ack == self.reliable_sequence {
            self.reliable_buf.clear();
        }
        self.incoming_sequence = seq;
        self.incoming_acknowledged = ack;
        self.incoming_reliable_acknowledged = reliable_ack;
        if reliable_message != 0 {
            self.incoming_reliable_sequence ^= 1;
        }
        self.last_received = now;
        Some(&pkt[8..])
    }
}

/// The server side of a netchan_t (reads the client's qport, writes none).
/// Used by test servers.
#[derive(Debug, Clone, Default)]
pub struct ServerChan {
    pub incoming_sequence: u32,
    pub incoming_acknowledged: u32,
    incoming_reliable_acknowledged: u32,
    incoming_reliable_sequence: u32,
    pub outgoing_sequence: u32,
    reliable_sequence: u32,
    last_reliable_sequence: u32,
    /// Reliable bytes queued for the next free reliable slot.
    pub message: Vec<u8>,
    reliable_buf: Vec<u8>,
}

impl ServerChan {
    /// Netchan_Process: (qport, payload) of an in-order client packet.
    pub fn process<'a>(&mut self, p: &'a [u8]) -> Option<(u16, &'a [u8])> {
        if p.len() < 10 {
            return None;
        }
        let w = |o: usize| u32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        let (seq, ack) = (w(0), w(4));
        let (rel, relack) = (seq >> 31, ack >> 31);
        let (seq, ack) = (seq & !(1 << 31), ack & !(1 << 31));
        if seq <= self.incoming_sequence && !(seq == 0 && self.incoming_sequence == 0) {
            return None;
        }
        if relack == self.reliable_sequence {
            self.reliable_buf.clear();
        }
        self.incoming_sequence = seq;
        self.incoming_acknowledged = ack;
        self.incoming_reliable_acknowledged = relack;
        if rel != 0 {
            self.incoming_reliable_sequence ^= 1;
        }
        Some((u16::from_le_bytes([p[8], p[9]]), &p[10..]))
    }

    /// Netchan_Transmit.
    pub fn transmit(&mut self, data: &[u8]) -> Vec<u8> {
        let mut send_reliable = self.incoming_acknowledged > self.last_reliable_sequence
            && self.incoming_reliable_acknowledged != self.reliable_sequence;
        if self.reliable_buf.is_empty() && !self.message.is_empty() {
            // Callers keep queued reliable data under MAX_MSGLEN.
            self.reliable_buf = std::mem::take(&mut self.message);
            self.reliable_sequence ^= 1;
            send_reliable = true;
        }
        let mut v = Vec::with_capacity(PACKET_HEADER + self.reliable_buf.len() + data.len());
        v.extend_from_slice(&(self.outgoing_sequence | (send_reliable as u32) << 31).to_le_bytes());
        v.extend_from_slice(&(self.incoming_sequence | self.incoming_reliable_sequence << 31).to_le_bytes());
        self.outgoing_sequence += 1;
        if send_reliable {
            v.extend_from_slice(&self.reliable_buf);
            self.last_reliable_sequence = self.outgoing_sequence;
        }
        if v.len() + data.len() <= MAX_MSGLEN + PACKET_HEADER {
            v.extend_from_slice(data);
        }
        v
    }
}

/// Info strings (`\key\value\key\value`).
pub fn info_value<'a>(info: &'a str, key: &str) -> &'a str {
    let mut it = info.strip_prefix('\\').unwrap_or(info).split('\\');
    while let (Some(k), v) = (it.next(), it.next()) {
        if k == key {
            return v.unwrap_or("");
        }
    }
    ""
}

/// Info_SetValueForKey (empty value removes the key).
pub fn info_set(info: &mut String, key: &str, value: &str) {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut it = info.strip_prefix('\\').unwrap_or(info).split('\\');
    while let (Some(k), Some(v)) = (it.next(), it.next()) {
        if k != key && !k.is_empty() {
            pairs.push((k.into(), v.into()));
        }
    }
    if !value.is_empty() {
        pairs.push((key.into(), value.into()));
    }
    *info = pairs.iter().map(|(k, v)| format!("\\{k}\\{v}")).collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_ccitt() {
        // CRC-16/CCITT-FALSE check value.
        assert_eq!(crc_block(b"123456789"), 0x29b1);
    }

    #[test]
    fn usercmd_roundtrip() {
        let a = UserCmd { msec: 13, angles: [10.0, 270.0, 0.0], forwardmove: 400, sidemove: -350, upmove: 0, buttons: 3, impulse: 7 };
        let mut b = SizeBuf::new(64);
        write_delta_usercmd(&mut b, &UserCmd::default(), &a);
        let mut m = MsgReader::new(&b.data);
        let r = read_delta_usercmd(&mut m, &UserCmd::default());
        assert!(m.at_end() && !m.badread);
        assert_eq!((r.msec, r.forwardmove, r.sidemove, r.buttons, r.impulse), (13, 400, -350, 3, 7));
        assert!((r.angles[0] - 10.0).abs() < 0.01 && (r.angles[1] - 270.0 + 360.0).abs() < 0.01);
    }

    #[test]
    fn netchan_reliable_until_acked() {
        let mut c = Netchan::new(1234, 0.0);
        c.message.byte(CLC_STRINGCMD as i32);
        c.message.string(b"new");
        let p = c.transmit(&[CLC_NOP]);
        assert_eq!(u32::from_le_bytes(p[0..4].try_into().unwrap()), 1 << 31);
        assert_eq!(u16::from_le_bytes(p[8..10].try_into().unwrap()), 1234);
        assert_eq!(&p[10..], b"\x04new\x00\x01");
        // Packets after it go out without it until the server acks a later one.
        assert_eq!(c.transmit(&[]).len(), 10);
        assert_eq!(c.transmit(&[]).len(), 10);
        // Server packet 1 acks our 2 without the reliable bit: resend.
        let mut s = Vec::new();
        s.extend_from_slice(&1u32.to_le_bytes());
        s.extend_from_slice(&2u32.to_le_bytes());
        s.push(SVC_NOP);
        assert_eq!(c.process(&s, 0.1), Some(&[SVC_NOP][..]));
        let p = c.transmit(&[]);
        assert!(p.len() > 10 && u32::from_le_bytes(p[0..4].try_into().unwrap()) >> 31 == 1);
        // Acked with the reliable bit: buffer clears, no resend.
        let mut s = Vec::new();
        s.extend_from_slice(&2u32.to_le_bytes());
        s.extend_from_slice(&(3u32 | 1 << 31).to_le_bytes());
        assert!(c.process(&s, 0.2).is_some());
        assert!(c.can_reliable());
        let p = c.transmit(&[]);
        assert_eq!(p.len(), 10);
        // Duplicates are dropped.
        assert!(c.process(&s, 0.3).is_none());
    }

    #[test]
    fn info_strings() {
        let mut s = String::from("\\name\\bob\\topcolor\\4");
        assert_eq!(info_value(&s, "topcolor"), "4");
        info_set(&mut s, "name", "alice");
        assert_eq!(info_value(&s, "name"), "alice");
        assert_eq!(info_value(&s, "missing"), "");
        info_set(&mut s, "topcolor", "");
        assert_eq!(s, "\\name\\alice");
    }
}
