//! Connection-control packets (net_dgrm.c CCREQ_*/CCREP_*). Wire format:
//! big-endian `NETFLAG_CTL | length` header, then the command byte and
//! little-endian MSG_* fields.

use crate::dgram::{NETFLAG_CTL, NETFLAG_LENGTH_MASK};
use crate::msg::{MsgReader, SizeBuf};

pub const NET_PROTOCOL_VERSION: u8 = 3;
pub const CCREQ_CONNECT: u8 = 0x01;
pub const CCREQ_SERVER_INFO: u8 = 0x02;
pub const CCREQ_PLAYER_INFO: u8 = 0x03;
pub const CCREQ_RULE_INFO: u8 = 0x04;
pub const CCREP_ACCEPT: u8 = 0x81;
pub const CCREP_REJECT: u8 = 0x82;
pub const CCREP_SERVER_INFO: u8 = 0x83;
pub const CCREP_PLAYER_INFO: u8 = 0x84;
pub const CCREP_RULE_INFO: u8 = 0x85;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ccreq {
    Connect { game: String, version: u8 },
    ServerInfo { game: String, version: u8 },
    PlayerInfo { player: u8 },
    RuleInfo { prev: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ccrep {
    /// Server's private port for this connection.
    Accept { port: i32 },
    Reject { reason: String },
    ServerInfo { address: String, hostname: String, level: String, players: u8, max_players: u8, version: u8 },
    PlayerInfo { player: u8, name: String, colors: i32, frags: i32, connect_time: i32, address: String },
    RuleInfo { rule: String, value: String },
}

fn finish(b: SizeBuf) -> Vec<u8> {
    let len = (b.data.len() + 4) as u32;
    let mut p = (NETFLAG_CTL | len).to_be_bytes().to_vec();
    p.extend_from_slice(&b.data);
    p
}

/// Command byte and payload reader for a control packet, if `pkt` is one.
fn open(pkt: &[u8]) -> Option<(u8, MsgReader<'_>)> {
    if pkt.len() < 5 {
        return None;
    }
    let lf = u32::from_be_bytes(pkt[..4].try_into().unwrap());
    if lf & !NETFLAG_LENGTH_MASK != NETFLAG_CTL || (lf & NETFLAG_LENGTH_MASK) as usize != pkt.len() {
        return None;
    }
    Some((pkt[4], MsgReader::new(&pkt[5..])))
}

impl Ccreq {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = SizeBuf::new(1024);
        match self {
            Ccreq::Connect { game, version } | Ccreq::ServerInfo { game, version } => {
                b.byte(if matches!(self, Ccreq::Connect { .. }) { CCREQ_CONNECT } else { CCREQ_SERVER_INFO } as i32);
                b.string(game.as_bytes());
                b.byte(*version as i32);
            }
            Ccreq::PlayerInfo { player } => {
                b.byte(CCREQ_PLAYER_INFO as i32);
                b.byte(*player as i32);
            }
            Ccreq::RuleInfo { prev } => {
                b.byte(CCREQ_RULE_INFO as i32);
                b.string(prev.as_bytes());
            }
        }
        finish(b)
    }
    pub fn decode(pkt: &[u8]) -> Option<Self> {
        let (cmd, mut r) = open(pkt)?;
        let v = match cmd {
            CCREQ_CONNECT => Ccreq::Connect { game: r.string_lossy(), version: r.byte() as u8 },
            CCREQ_SERVER_INFO => Ccreq::ServerInfo { game: r.string_lossy(), version: r.byte() as u8 },
            CCREQ_PLAYER_INFO => Ccreq::PlayerInfo { player: r.byte() as u8 },
            // An empty first query may omit the string entirely.
            CCREQ_RULE_INFO => Ccreq::RuleInfo { prev: if r.at_end() { String::new() } else { r.string_lossy() } },
            _ => return None,
        };
        (!r.badread).then_some(v)
    }
}

impl Ccrep {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = SizeBuf::new(1024);
        match self {
            Ccrep::Accept { port } => {
                b.byte(CCREP_ACCEPT as i32);
                b.long(*port);
            }
            Ccrep::Reject { reason } => {
                b.byte(CCREP_REJECT as i32);
                b.string(reason.as_bytes());
            }
            Ccrep::ServerInfo { address, hostname, level, players, max_players, version } => {
                b.byte(CCREP_SERVER_INFO as i32);
                b.string(address.as_bytes());
                b.string(hostname.as_bytes());
                b.string(level.as_bytes());
                b.byte(*players as i32);
                b.byte(*max_players as i32);
                b.byte(*version as i32);
            }
            Ccrep::PlayerInfo { player, name, colors, frags, connect_time, address } => {
                b.byte(CCREP_PLAYER_INFO as i32);
                b.byte(*player as i32);
                b.string(name.as_bytes());
                b.long(*colors);
                b.long(*frags);
                b.long(*connect_time);
                b.string(address.as_bytes());
            }
            Ccrep::RuleInfo { rule, value } => {
                b.byte(CCREP_RULE_INFO as i32);
                if !rule.is_empty() {
                    b.string(rule.as_bytes());
                    b.string(value.as_bytes());
                }
            }
        }
        finish(b)
    }
    pub fn decode(pkt: &[u8]) -> Option<Self> {
        let (cmd, mut r) = open(pkt)?;
        let v = match cmd {
            CCREP_ACCEPT => Ccrep::Accept { port: r.long() },
            CCREP_REJECT => Ccrep::Reject { reason: r.string_lossy() },
            CCREP_SERVER_INFO => Ccrep::ServerInfo {
                address: r.string_lossy(),
                hostname: r.string_lossy(),
                level: r.string_lossy(),
                players: r.byte() as u8,
                max_players: r.byte() as u8,
                version: r.byte() as u8,
            },
            CCREP_PLAYER_INFO => Ccrep::PlayerInfo {
                player: r.byte() as u8,
                name: r.string_lossy(),
                colors: r.long(),
                frags: r.long(),
                connect_time: r.long(),
                address: r.string_lossy(),
            },
            CCREP_RULE_INFO if r.at_end() => Ccrep::RuleInfo { rule: String::new(), value: String::new() },
            CCREP_RULE_INFO => Ccrep::RuleInfo { rule: r.string_lossy(), value: r.string_lossy() },
            _ => return None,
        };
        (!r.badread).then_some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let reqs = [
            Ccreq::Connect { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION },
            Ccreq::ServerInfo { game: "QUAKE".into(), version: 3 },
            Ccreq::PlayerInfo { player: 2 },
            Ccreq::RuleInfo { prev: "sv_gravity".into() },
        ];
        for q in reqs {
            assert_eq!(Ccreq::decode(&q.encode()), Some(q));
        }
        let reps = [
            Ccrep::Accept { port: 26001 },
            Ccrep::Reject { reason: "full".into() },
            Ccrep::ServerInfo { address: "1.2.3.4:26000".into(), hostname: "h".into(), level: "e1m1".into(), players: 1, max_players: 8, version: 3 },
            Ccrep::PlayerInfo { player: 0, name: "p".into(), colors: 4, frags: -1, connect_time: 9, address: "x".into() },
            Ccrep::RuleInfo { rule: "a".into(), value: "b".into() },
            Ccrep::RuleInfo { rule: String::new(), value: String::new() },
        ];
        for p in reps {
            assert_eq!(Ccrep::decode(&p.encode()), Some(p));
        }
        let p = Ccrep::Accept { port: 1 }.encode();
        assert_eq!(&p[..4], &[0x80, 0, 0, 9]);
        assert!(Ccreq::decode(&p).is_none());
    }
}
