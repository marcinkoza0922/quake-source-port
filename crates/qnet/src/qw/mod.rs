//! QuakeWorld protocol 28 (QW/client protocol.h, net_chan.c, common.c,
//! crc.c, md4.c): constants, the sequenced netchan, delta usercmds,
//! the move checksum byte and the map checksum.

mod md4;
mod tables;

pub use md4::block_checksum;

use crate::{MsgReader, SizeBuf};

pub const PROTOCOL_VERSION: i32 = 28;
pub const PORT_SERVER: u16 = 27500;
/// Largest QW packet payload (MAX_MSGLEN).
pub const MAX_MSGLEN: usize = 1450;
pub const PACKET_HEADER: usize = 8;
pub const MAX_CLIENTS: usize = 32;
pub const UPDATE_BACKUP: usize = 64;
pub const UPDATE_MASK: u32 = UPDATE_BACKUP as u32 - 1;
pub const MAX_PACKET_ENTITIES: usize = 64;

// Connectionless (out-of-band) packets.
pub const S2C_CHALLENGE: u8 = b'c';
pub const S2C_CONNECTION: u8 = b'j';
pub const A2C_PRINT: u8 = b'n';
pub const A2A_PING: u8 = b'k';
pub const A2A_ACK: u8 = b'l';

pub const SVC_BAD: u8 = 0;
pub const SVC_NOP: u8 = 1;
pub const SVC_DISCONNECT: u8 = 2;
pub const SVC_UPDATESTAT: u8 = 3;
pub const SVC_SETVIEW: u8 = 5;
pub const SVC_SOUND: u8 = 6;
pub const SVC_PRINT: u8 = 8;
pub const SVC_STUFFTEXT: u8 = 9;
pub const SVC_SETANGLE: u8 = 10;
pub const SVC_SERVERDATA: u8 = 11;
pub const SVC_LIGHTSTYLE: u8 = 12;
pub const SVC_UPDATEFRAGS: u8 = 14;
pub const SVC_STOPSOUND: u8 = 16;
pub const SVC_DAMAGE: u8 = 19;
pub const SVC_SPAWNSTATIC: u8 = 20;
pub const SVC_SPAWNBASELINE: u8 = 22;
pub const SVC_TEMP_ENTITY: u8 = 23;
pub const SVC_SETPAUSE: u8 = 24;
pub const SVC_CENTERPRINT: u8 = 26;
pub const SVC_KILLEDMONSTER: u8 = 27;
pub const SVC_FOUNDSECRET: u8 = 28;
pub const SVC_SPAWNSTATICSOUND: u8 = 29;
pub const SVC_INTERMISSION: u8 = 30;
pub const SVC_FINALE: u8 = 31;
pub const SVC_CDTRACK: u8 = 32;
pub const SVC_SELLSCREEN: u8 = 33;
pub const SVC_SMALLKICK: u8 = 34;
pub const SVC_BIGKICK: u8 = 35;
pub const SVC_UPDATEPING: u8 = 36;
pub const SVC_UPDATEENTERTIME: u8 = 37;
pub const SVC_UPDATESTATLONG: u8 = 38;
pub const SVC_MUZZLEFLASH: u8 = 39;
pub const SVC_UPDATEUSERINFO: u8 = 40;
pub const SVC_DOWNLOAD: u8 = 41;
pub const SVC_PLAYERINFO: u8 = 42;
pub const SVC_NAILS: u8 = 43;
pub const SVC_CHOKECOUNT: u8 = 44;
pub const SVC_MODELLIST: u8 = 45;
pub const SVC_SOUNDLIST: u8 = 46;
pub const SVC_PACKETENTITIES: u8 = 47;
pub const SVC_DELTAPACKETENTITIES: u8 = 48;
pub const SVC_MAXSPEED: u8 = 49;
pub const SVC_ENTGRAVITY: u8 = 50;
pub const SVC_SETINFO: u8 = 51;
pub const SVC_SERVERINFO: u8 = 52;
pub const SVC_UPDATEPL: u8 = 53;

pub const CLC_NOP: u8 = 1;
pub const CLC_MOVE: u8 = 3;
pub const CLC_STRINGCMD: u8 = 4;
pub const CLC_DELTA: u8 = 5;

pub const PF_MSEC: u32 = 1 << 0;
pub const PF_COMMAND: u32 = 1 << 1;
pub const PF_VELOCITY1: u32 = 1 << 2;
pub const PF_MODEL: u32 = 1 << 5;
pub const PF_SKINNUM: u32 = 1 << 6;
pub const PF_EFFECTS: u32 = 1 << 7;
pub const PF_WEAPONFRAME: u32 = 1 << 8;
pub const PF_DEAD: u32 = 1 << 9;
pub const PF_GIB: u32 = 1 << 10;
pub const PF_NOGRAV: u32 = 1 << 11;

pub const CM_ANGLE1: u8 = 1 << 0;
pub const CM_ANGLE3: u8 = 1 << 1;
pub const CM_FORWARD: u8 = 1 << 2;
pub const CM_SIDE: u8 = 1 << 3;
pub const CM_UP: u8 = 1 << 4;
pub const CM_BUTTONS: u8 = 1 << 5;
pub const CM_IMPULSE: u8 = 1 << 6;
pub const CM_ANGLE2: u8 = 1 << 7;

/// Packet entity bits: the low 9 bits of the first word are the number.
pub const U_ORIGIN1: u32 = 1 << 9;
pub const U_ORIGIN2: u32 = 1 << 10;
pub const U_ORIGIN3: u32 = 1 << 11;
pub const U_ANGLE2: u32 = 1 << 12;
pub const U_FRAME: u32 = 1 << 13;
pub const U_REMOVE: u32 = 1 << 14;
pub const U_MOREBITS: u32 = 1 << 15;
pub const U_ANGLE1: u32 = 1 << 0;
pub const U_ANGLE3: u32 = 1 << 1;
pub const U_MODEL: u32 = 1 << 2;
pub const U_COLORMAP: u32 = 1 << 3;
pub const U_SKIN: u32 = 1 << 4;
pub const U_EFFECTS: u32 = 1 << 5;
pub const U_SOLID: u32 = 1 << 6;

pub const SND_VOLUME: i32 = 1 << 15;
pub const SND_ATTENUATION: i32 = 1 << 14;

pub const TE_SPIKE: u8 = 0;
pub const TE_SUPERSPIKE: u8 = 1;
pub const TE_GUNSHOT: u8 = 2;
pub const TE_EXPLOSION: u8 = 3;
pub const TE_TAREXPLOSION: u8 = 4;
pub const TE_LIGHTNING1: u8 = 5;
pub const TE_LIGHTNING2: u8 = 6;
pub const TE_WIZSPIKE: u8 = 7;
pub const TE_KNIGHTSPIKE: u8 = 8;
pub const TE_LIGHTNING3: u8 = 9;
pub const TE_LAVASPLASH: u8 = 10;
pub const TE_TELEPORT: u8 = 11;
pub const TE_BLOOD: u8 = 12;
pub const TE_LIGHTNINGBLOOD: u8 = 13;

pub const PRINT_CHAT: i32 = 3;

pub const EF_BRIGHTFIELD: i32 = 1;
pub const EF_MUZZLEFLASH: i32 = 2;
pub const EF_BRIGHTLIGHT: i32 = 4;
pub const EF_DIMLIGHT: i32 = 8;
pub const EF_FLAG1: i32 = 16;
pub const EF_FLAG2: i32 = 32;
pub const EF_BLUE: i32 = 64;
pub const EF_RED: i32 = 128;

pub const BUTTON_ATTACK: u8 = 1;
pub const BUTTON_JUMP: u8 = 2;

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
