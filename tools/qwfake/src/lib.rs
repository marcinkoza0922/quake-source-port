//! A scripted QuakeWorld (protocol 28) server for testing the QW client:
//! the challenge/connect handshake, sound and model lists, an optional
//! fake download, prespawn with the map checksum, spawn, then one reply
//! per client packet carrying player info and packet entities.
//!
//! The connecting player is moved with the client's own commands through
//! `qworld::pmove` against the real map, so a client can walk around. A
//! second player ("bot") stands in front of the spawn point.

use std::net::SocketAddr;
use std::sync::Arc;

use qnet::protocol as nq;
use qnet::qw::*;
use qnet::{MsgReader, SizeBuf, Transport, UdpTransport};
use qworld::pmove::{self, MoveCmd, MoveVars, PhysEnt, PhysShape, PlayerState};
use qworld::{CollisionModel, Vec3};

pub struct FakeServer {
    pub sock: UdpTransport,
    pub client: Option<SocketAddr>,
    pub chan: ServerChan,
    pub map: String,
    pub checksum2: u32,
    world: Arc<CollisionModel>,
    /// A file offered for download (name, contents), listed as a model.
    pub download: Option<(String, Vec<u8>)>,
    dl_pos: usize,
    pub begun: bool,
    pub moves: u32,
    pub bad_checksums: u32,
    pub last_cmd: UserCmd,
    pub player: PlayerState,
    pub spawn_angle: f32,
    pub spawn: [f32; 3],
    /// Freeze the client's player (tests check prediction against it).
    pub frozen: bool,
    /// Commands received, in order.
    pub log: Vec<String>,
    frames: u32,
}

const MODELS: [&str; 3] = ["progs/player.mdl", "*1", "progs/s_bubble.spr"];
const SOUNDS: [&str; 2] = ["weapons/r_exp3.wav", "player/plyrjmp8.wav"];

impl FakeServer {
    /// `map` like "maps/e1m1.bsp"; `bsp` its contents.
    pub fn new(sock: UdpTransport, map: &str, bsp: &[u8]) -> Option<Self> {
        let b = qformats::Bsp::parse(bsp).ok()?;
        let world = Arc::new(CollisionModel::from_bsp(&b));
        let mut origin = Vec3::ZERO;
        let mut angle = 0.0;
        if let Ok(ents) = qformats::parse_entities(&b.entities) {
            let start = ents.iter().find(|e| e.classname() == Some("info_player_start"));
            if let Some(e) = start {
                let v: Vec<f32> = e.get("origin").unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    origin = Vec3::new(v[0], v[1], v[2] + 1.0);
                }
                angle = e.get("angle").and_then(|a| a.parse().ok()).unwrap_or(0.0);
            }
        }
        Some(Self {
            sock,
            client: None,
            chan: ServerChan::default(),
            map: map.to_string(),
            checksum2: map_checksum2(bsp)?,
            world,
            download: None,
            dl_pos: 0,
            begun: false,
            moves: 0,
            bad_checksums: 0,
            last_cmd: UserCmd::default(),
            player: PlayerState { origin, ..Default::default() },
            spawn_angle: angle,
            spawn: origin.to_array(),
            frozen: false,
            log: Vec::new(),
            frames: 0,
        })
    }

    pub fn port(&self) -> u16 {
        self.sock.local_addr().port()
    }

    fn reliable(&mut self, f: impl FnOnce(&mut SizeBuf)) {
        let mut b = SizeBuf::new(MAX_MSGLEN);
        f(&mut b);
        self.chan.message.extend_from_slice(&b.data);
    }

    fn stuff(b: &mut SizeBuf, s: &str) {
        b.byte(SVC_STUFFTEXT as i32);
        b.string(s.as_bytes());
    }

    fn userinfo(b: &mut SizeBuf, slot: u8, info: &str) {
        b.byte(SVC_UPDATEUSERINFO as i32);
        b.byte(slot as i32);
        b.long(slot as i32 + 1);
        b.string(info.as_bytes());
    }

    fn command(&mut self, s: &str, client_info: &str) {
        self.log.push(s.to_string());
        let args: Vec<&str> = s.split_whitespace().collect();
        match args.first().copied().unwrap_or("") {
            "new" => self.reliable(|b| {
                b.byte(SVC_SERVERDATA as i32);
                b.long(PROTOCOL_VERSION);
                b.long(1);
                b.string(b"qw");
                b.byte(0);
                b.string(b"qwfake");
                for f in [800.0, 100.0, 320.0, 500.0, 10.0, 0.7, 10.0, 4.0, 1.0, 1.0] {
                    b.float(f);
                }
                b.byte(SVC_CDTRACK as i32);
                b.byte(6);
                Self::stuff(b, "fullserverinfo \"\\maxclients\\8\\deathmatch\\1\\hostname\\qwfake\"\n");
            }),
            "soundlist" => self.reliable(|b| {
                b.byte(SVC_SOUNDLIST as i32);
                b.byte(0);
                for n in SOUNDS {
                    b.string(n.as_bytes());
                }
                b.string(b"");
                b.byte(0);
            }),
            "modellist" => {
                let map = self.map.clone();
                let dl = self.download.as_ref().map(|(n, _)| n.clone());
                self.reliable(|b| {
                    b.byte(SVC_MODELLIST as i32);
                    b.byte(0);
                    b.string(map.as_bytes());
                    for n in MODELS {
                        b.string(n.as_bytes());
                    }
                    if let Some(n) = dl {
                        b.string(n.as_bytes());
                    }
                    b.string(b"");
                    b.byte(0);
                });
            }
            "download" | "nextdl" => {
                if args[0] == "download" {
                    self.dl_pos = 0;
                }
                let Some((_, data)) = &self.download else {
                    return self.reliable(|b| {
                        b.byte(SVC_DOWNLOAD as i32);
                        b.short(-1);
                        b.byte(0);
                    });
                };
                let chunk: Vec<u8> = data[self.dl_pos..].iter().take(4).copied().collect();
                self.dl_pos += chunk.len();
                let pct = if self.dl_pos >= data.len() { 100 } else { (self.dl_pos * 100 / data.len()).min(99) as i32 };
                self.reliable(|b| {
                    b.byte(SVC_DOWNLOAD as i32);
                    b.short(chunk.len() as i32);
                    b.byte(pct);
                    b.write(&chunk);
                });
            }
            "prespawn" => {
                let sum = args.get(3).and_then(|v| v.parse::<i32>().ok());
                if sum != Some(self.checksum2 as i32) {
                    self.log.push(format!("BAD CHECKSUM {sum:?}"));
                }
                self.reliable(|b| {
                    b.byte(SVC_SPAWNBASELINE as i32);
                    b.short(40);
                    for v in [2, 0, 0, 0] {
                        b.byte(v);
                    }
                    for _ in 0..3 {
                        write_coord(b, 0.0);
                        write_angle(b, 0.0);
                    }
                    Self::stuff(b, "cmd spawn 1 0\n");
                });
            }
            "spawn" => {
                let info = client_info.to_string();
                let angle = self.spawn_angle;
                self.reliable(|b| {
                    Self::userinfo(b, 0, &info);
                    Self::userinfo(b, 1, "\\name\\bot\\topcolor\\4\\bottomcolor\\12");
                    for (stat, v) in [(nq::STAT_HEALTH, 100), (nq::STAT_ARMOR, 50), (nq::STAT_SHELLS, 25), (nq::STAT_AMMO, 25)] {
                        b.byte(SVC_UPDATESTAT as i32);
                        b.byte(stat as i32);
                        b.byte(v);
                    }
                    b.byte(SVC_UPDATESTATLONG as i32);
                    b.byte(nq::STAT_ITEMS as i32);
                    b.long(1 | 4096); // shotgun, shells
                    b.byte(SVC_UPDATESTAT as i32);
                    b.byte(nq::STAT_ACTIVEWEAPON as i32);
                    b.byte(1);
                    for i in 0..64 {
                        b.byte(SVC_LIGHTSTYLE as i32);
                        b.byte(i);
                        b.string(b"m");
                    }
                    b.byte(SVC_SETANGLE as i32);
                    write_angle(b, 0.0);
                    write_angle(b, angle);
                    write_angle(b, 0.0);
                    Self::stuff(b, "skins\n");
                });
            }
            "begin" => {
                self.begun = true;
                self.reliable(|b| {
                    b.byte(SVC_PRINT as i32);
                    b.byte(PRINT_CHAT);
                    b.string(b"console: welcome to qwfake\n");
                });
            }
            "say" => {
                let text = s[3..].trim().trim_matches('"').to_string();
                self.reliable(|b| {
                    b.byte(SVC_PRINT as i32);
                    b.byte(PRINT_CHAT);
                    b.string(format!("player: {text}\n").as_bytes());
                });
            }
            _ => {}
        }
    }

    fn run_move(&mut self, c: &UserCmd) {
        if self.frozen || !self.begun {
            return;
        }
        let ents = [PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(&self.world, 0) }];
        let mc = MoveCmd {
            msec: c.msec,
            angles: c.angles,
            forwardmove: c.forwardmove as f32,
            sidemove: c.sidemove as f32,
            upmove: c.upmove as f32,
            buttons: c.buttons,
        };
        self.player = pmove::predict(&ents, &MoveVars::default(), &self.player, &mc, false, false);
    }

    /// Handle everything pending and answer each client packet.
    pub fn frame(&mut self, client_info: &mut String) {
        let mut buf = vec![0u8; 8192];
        while let Ok(Some((n, from))) = self.sock.recv_from(&mut buf) {
            let pkt = buf[..n].to_vec();
            if let Some(p) = oob_payload(&pkt) {
                let s = String::from_utf8_lossy(p).into_owned();
                if s.starts_with("getchallenge") {
                    let _ = self.sock.send_to(&oob(b"c4242"), from);
                } else if let Some(rest) = s.strip_prefix("connect ") {
                    let a: Vec<&str> = rest.splitn(4, ' ').collect();
                    if a.len() == 4 && a[0] == "28" && a[2] == "4242" {
                        *client_info = a[3].trim().trim_matches('"').to_string();
                        self.client = Some(from);
                        self.chan = ServerChan::default();
                        self.begun = false;
                        let _ = self.sock.send_to(&oob(b"j"), from);
                    } else {
                        let _ = self.sock.send_to(&oob(b"nBad connect\n"), from);
                    }
                }
                continue;
            }
            if Some(from) != self.client {
                continue;
            }
            let Some((_, payload)) = self.chan.process(&pkt).map(|(q, p)| (q, p.to_vec())) else { continue };
            let mut m = MsgReader::new(&payload);
            while !m.at_end() && !m.badread {
                match m.byte() as u8 {
                    CLC_NOP => {}
                    CLC_STRINGCMD => {
                        let s = m.string_lossy();
                        if s == "drop" {
                            self.client = None;
                            self.begun = false;
                            break;
                        }
                        let info = client_info.clone();
                        self.command(&s, &info);
                    }
                    CLC_MOVE => {
                        let start = m.pos;
                        let ck = m.byte() as u8;
                        m.byte();
                        let c0 = read_delta_usercmd(&mut m, &UserCmd::default());
                        let c1 = read_delta_usercmd(&mut m, &c0);
                        let c2 = read_delta_usercmd(&mut m, &c1);
                        let calc = sequence_crc_byte(&payload[start + 1..m.pos], self.chan.incoming_sequence as i32);
                        if calc != ck {
                            self.bad_checksums += 1;
                            continue;
                        }
                        self.moves += 1;
                        self.last_cmd = c2;
                        self.run_move(&c2);
                    }
                    CLC_DELTA => {
                        m.byte();
                    }
                    c => {
                        self.log.push(format!("BAD CLC {c}"));
                        break;
                    }
                }
            }
            let Some(c) = self.client else { continue };
            let data = self.snapshot();
            let p = self.chan.transmit(&data);
            let _ = self.sock.send_to(&p, c);
        }
    }

    /// Player info for both players and a full packet entities update.
    fn snapshot(&mut self) -> Vec<u8> {
        let mut b = SizeBuf::new(MAX_MSGLEN);
        if !self.begun {
            return b.data;
        }
        self.frames += 1;
        let p = self.player;
        let me_cmd = UserCmd { angles: self.last_cmd.angles, ..Default::default() };
        b.byte(SVC_PLAYERINFO as i32);
        b.byte(0);
        b.short((PF_COMMAND | PF_VELOCITY1 | PF_VELOCITY1 << 1 | PF_VELOCITY1 << 2 | PF_WEAPONFRAME) as i32);
        for x in p.origin.to_array() {
            write_coord(&mut b, x);
        }
        b.byte(0);
        write_delta_usercmd(&mut b, &UserCmd::default(), &me_cmd);
        for v in p.velocity.to_array() {
            b.short(v as i32);
        }
        b.byte(0);
        // The bot: 128 units ahead of the spawn, facing back, running frames.
        let yaw = self.spawn_angle.to_radians();
        let bot = [128.0 * yaw.cos(), 128.0 * yaw.sin(), 0.0];
        let base = self.spawn;
        b.byte(SVC_PLAYERINFO as i32);
        b.byte(1);
        b.short(PF_COMMAND as i32);
        for k in 0..3 {
            write_coord(&mut b, base[k] + bot[k]);
        }
        b.byte(((self.frames / 8) % 6) as i32); // run frames
        write_delta_usercmd(&mut b, &UserCmd::default(), &UserCmd { angles: [0.0, self.spawn_angle + 180.0, 0.0], ..Default::default() });
        // Entity 40: a bubble sprite circling the spawn point.
        let t = self.frames as f32 / 72.0;
        b.byte(SVC_PACKETENTITIES as i32);
        b.short((40 | U_MOREBITS | U_ORIGIN1 | U_ORIGIN2 | U_ORIGIN3) as i32);
        b.byte(U_MODEL as i32);
        b.byte(4);
        write_coord(&mut b, base[0] + 64.0 * t.cos());
        write_coord(&mut b, base[1] + 64.0 * t.sin());
        write_coord(&mut b, base[2] + 16.0);
        b.short(0);
        b.data
    }
}
