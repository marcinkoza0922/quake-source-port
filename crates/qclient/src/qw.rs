//! QuakeWorld client state (QW/client cl_parse.c, cl_ents.c, cl_pred.c,
//! cl_input.c CL_SendCmd).
//!
//! Server messages are parsed here and *translated* into NetQuake
//! protocol 999 messages (float coords and angles) for [`ClientState`], so
//! rendering, sound, effects, the HUD and demo recording work unchanged.
//! Each QW packet that carries entities becomes one NQ frame: `svc_time`,
//! the translated non-entity messages, `svc_clientdata` from the QW stats
//! and one `U_SIGNAL` update per visible player and packet entity.
//!
//! What QW does differently and stays here: the signon handshake
//! (soundlist / modellist / prespawn / spawn / begin, driven by the host
//! through [`QwAction`]s), delta-compressed packet entities kept in
//! [`UPDATE_BACKUP`] frames, players as `svc_playerinfo`, nails as
//! `svc_nails`, the move commands sent each frame ([`QwState::build_cmd`])
//! and local player prediction ([`QwState::predict`]).

use qnet::protocol as nq;
use qnet::qw::*;
use qnet::{MsgReader, SizeBuf};
use qworld::Vec3;
use qworld::pmove::{self, MoveCmd, MoveVars, PhysEnt, PhysShape, PlayerState};

use crate::ParseError;

type R<T = ()> = Result<T, ParseError>;

fn err<T>(s: impl Into<String>) -> R<T> {
    Err(ParseError(s.into()))
}

/// Flags of the NQ stream the translator writes.
pub const NQ_FLAGS: u32 = nq::PRFL_FLOATCOORD | nq::PRFL_FLOATANGLE;
const MAX_EDICTS: usize = 512;
/// NQ entity numbers used for nails (after the 512 QW edicts).
const NAIL_BASE: usize = 512;
const MAX_PROJECTILES: usize = 32;

/// player_info_t.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QwPlayer {
    pub userid: i32,
    pub userinfo: String,
    pub name: String,
    pub topcolor: i32,
    pub bottomcolor: i32,
    pub spectator: bool,
    pub frags: i32,
    pub ping: i32,
    pub pl: i32,
}

/// entity_state_t.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct QwEntity {
    pub number: usize,
    pub flags: u32,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub modelindex: usize,
    pub frame: i32,
    pub colormap: i32,
    pub skinnum: i32,
    pub effects: i32,
}

/// player_state_t.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct QwPlayerState {
    /// Frame sequence this state arrived in (cl.parsecount).
    pub messagenum: u32,
    pub present: bool,
    pub state_time: f64,
    pub command: UserCmd,
    pub origin: [f32; 3],
    pub viewangles: [f32; 3],
    pub velocity: [f32; 3],
    pub flags: u32,
    pub modelindex: usize,
    pub frame: i32,
    pub skinnum: i32,
    pub effects: i32,
    pub weaponframe: i32,
    /// Prediction carry-over.
    pub oldbuttons: u8,
    pub waterjumptime: f32,
    pub onground: bool,
}

/// frame_t.
#[derive(Debug, Clone)]
pub struct QwFrame {
    pub cmd: UserCmd,
    pub senttime: f64,
    pub delta_sequence: i32,
    pub receivedtime: f64,
    pub playerstate: Vec<QwPlayerState>,
    /// Packet entities, sorted by number.
    pub entities: Vec<QwEntity>,
    /// Incoming sequence the entities belong to; `None` if invalid.
    pub entities_seq: Option<u32>,
}

impl Default for QwFrame {
    fn default() -> Self {
        Self {
            cmd: UserCmd::default(),
            senttime: 0.0,
            delta_sequence: -1,
            receivedtime: -1.0,
            playerstate: vec![QwPlayerState::default(); MAX_CLIENTS],
            entities: Vec::new(),
            entities_seq: None,
        }
    }
}

/// Something the host has to do for the connection.
#[derive(Debug, Clone, PartialEq)]
pub enum QwAction {
    /// Send a reliable clc_stringcmd.
    Cmd(String),
    /// Model and sound lists are complete: check/download files, then call
    /// [`QwState::finish_precache`].
    Precache,
    /// One svc_download block (size -1: not found; percent 100: done).
    Download { size: i32, percent: u8, data: Vec<u8> },
    /// `packet <addr> "<text>"`: an out-of-band packet.
    Packet { addr: String, data: String },
    /// `reconnect`: start the signon over with `new`.
    Reconnect,
    Disconnect(String),
}

/// Result of parsing one QW message.
#[derive(Debug, Default)]
pub struct Parsed {
    /// NQ messages for [`ClientState::parse_server_message`](crate::ClientState).
    pub nq: Vec<Vec<u8>>,
    pub actions: Vec<QwAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Waiting for svc_serverdata.
    Connected,
    /// Reading sound and model lists / downloading.
    Precache,
    /// Prespawn / spawn signon.
    Signon,
    /// Got the first entity frame after `begin`.
    Active,
}

pub struct QwState {
    pub stage: Stage,
    pub servercount: i32,
    pub gamedir: String,
    pub playernum: usize,
    pub spectator: bool,
    pub levelname: String,
    pub movevars: MoveVars,
    /// Index 0 unused.
    pub sound_names: Vec<String>,
    pub model_names: Vec<String>,
    pub playerindex: usize,
    pub spikeindex: usize,
    pub players: Vec<QwPlayer>,
    pub serverinfo: String,
    pub stats: [i32; nq::MAX_CL_STATS],
    baselines: Vec<QwEntity>,
    pub frames: Vec<QwFrame>,
    /// cl.validsequence (0: no valid entity frame to delta from).
    pub validsequence: u32,
    /// cl.parsecount: the newest acknowledged outgoing sequence.
    pub parsecount: u32,
    /// Outgoing sequence of the next command (mirrors the netchan).
    pub outgoing_sequence: u32,
    /// cls.latency.
    pub latency: f64,
    pub paused: bool,
    pub intermission: Option<([f32; 3], [f32; 3])>,
    pub cdtrack: u8,
    /// NQ signon stage emitted so far.
    nq_signon: u32,
    nq_ready: bool,
    nails: Vec<([f32; 3], [f32; 3])>,
    punch: f32,
    muzzle: Vec<usize>,
    stuffbuf: String,
    /// Predicted local player (cl.simorg / simvel / onground).
    pub simorg: [f32; 3],
    pub simvel: [f32; 3],
    pub sim_onground: bool,
    pub predicted: bool,
}

impl Default for QwState {
    fn default() -> Self {
        Self::new()
    }
}

fn coord3(m: &mut MsgReader<'_>) -> [f32; 3] {
    [read_coord(m), read_coord(m), read_coord(m)]
}

/// NQ message writer (protocol 999 with [`NQ_FLAGS`]).
struct Nq(SizeBuf);

impl Nq {
    fn new() -> Self {
        Nq(SizeBuf::new(nq::MAX_MSGLEN))
    }
    fn byte(&mut self, b: u8) {
        self.0.byte(b as i32);
    }
    fn vec(&mut self, v: [f32; 3]) {
        for x in v {
            self.0.coord(x, NQ_FLAGS);
        }
    }
    fn angles(&mut self, v: [f32; 3]) {
        for x in v {
            self.0.angle(x, NQ_FLAGS);
        }
    }
    fn string(&mut self, s: &str) {
        self.0.string(s.as_bytes());
    }
}

impl QwState {
    pub fn new() -> Self {
        Self {
            stage: Stage::Connected,
            servercount: 0,
            gamedir: String::new(),
            playernum: 0,
            spectator: false,
            levelname: String::new(),
            movevars: MoveVars::default(),
            sound_names: vec![String::new()],
            model_names: vec![String::new()],
            playerindex: 0,
            spikeindex: 0,
            players: vec![QwPlayer::default(); MAX_CLIENTS],
            serverinfo: String::new(),
            stats: [0; nq::MAX_CL_STATS],
            baselines: vec![QwEntity::default(); MAX_EDICTS],
            frames: vec![QwFrame::default(); UPDATE_BACKUP],
            validsequence: 0,
            parsecount: 0,
            outgoing_sequence: 0,
            latency: 0.0,
            paused: false,
            intermission: None,
            cdtrack: 0,
            nq_signon: 0,
            nq_ready: false,
            nails: Vec::new(),
            punch: 0.0,
            muzzle: Vec::new(),
            stuffbuf: String::new(),
            simorg: [0.0; 3],
            simvel: [0.0; 3],
            sim_onground: false,
            predicted: false,
        }
    }

    /// CL_ClearState for a new map (keeps the connection's sequences).
    fn clear_level(&mut self) {
        let (out, parse) = (self.outgoing_sequence, self.parsecount);
        let players = std::mem::take(&mut self.players);
        let serverinfo = std::mem::take(&mut self.serverinfo);
        *self = Self::new();
        self.outgoing_sequence = out;
        self.parsecount = parse;
        // Userinfo persists across maps; frags and pings are resent.
        self.players = players;
        self.serverinfo = serverinfo;
    }

    pub fn deathmatch(&self) -> bool {
        info_value(&self.serverinfo, "deathmatch").parse::<i32>().unwrap_or(0) != 0
    }

    fn frame_index(seq: u32) -> usize {
        (seq & UPDATE_MASK) as usize
    }

    /// Parse one QW server message (the netchan payload). `ack` is the
    /// netchan's incoming_acknowledged, `inseq` its incoming_sequence.
    pub fn parse(&mut self, data: &[u8], ack: u32, inseq: u32, realtime: f64) -> R<Parsed> {
        let mut out = Parsed::default();
        let mut body = Nq::new();
        // CL_ParseClientdata: the frame this packet answers.
        self.parsecount = ack;
        let fi = Self::frame_index(ack);
        self.frames[fi].receivedtime = realtime;
        let latency = realtime - self.frames[fi].senttime;
        if (0.0..=1.0).contains(&latency) {
            self.latency = if latency < self.latency { latency } else { self.latency + 0.001 };
        }
        self.nails.clear();
        let mut got_entities = false;
        let mut m = MsgReader::new(data);
        loop {
            if m.badread {
                return err("CL_ParseServerMessage: Bad server message");
            }
            if m.at_end() {
                break;
            }
            let cmd = m.byte() as u8;
            match cmd {
                SVC_NOP => {}
                SVC_DISCONNECT => {
                    out.actions.push(QwAction::Disconnect("Server disconnected\n".into()));
                    return Ok(out);
                }
                SVC_PRINT => {
                    let level = m.byte();
                    let s = m.string_lossy();
                    body.byte(nq::SVC_PRINT);
                    // \x01 marks chat (talk sound, coloured) as in NQ.
                    let s = if level == PRINT_CHAT && !s.starts_with('\x01') { format!("\x01{s}") } else { s };
                    body.string(&s);
                }
                SVC_CENTERPRINT => {
                    let s = m.string_lossy();
                    body.byte(nq::SVC_CENTERPRINT);
                    body.string(&s);
                }
                SVC_STUFFTEXT => {
                    let s = m.string_lossy();
                    self.stuff_text(&s, &mut body, &mut out);
                }
                SVC_DAMAGE => {
                    let armor = m.byte();
                    let blood = m.byte();
                    let from = coord3(&mut m);
                    body.byte(nq::SVC_DAMAGE);
                    body.byte(armor as u8);
                    body.byte(blood as u8);
                    body.vec(from);
                }
                SVC_SERVERDATA => self.parse_serverdata(&mut m, &mut out)?,
                SVC_SETANGLE => {
                    let a = [read_angle(&mut m), read_angle(&mut m), read_angle(&mut m)];
                    body.byte(nq::SVC_SETANGLE);
                    body.angles(a);
                }
                SVC_LIGHTSTYLE => {
                    let i = m.byte();
                    let s = m.string_lossy();
                    body.byte(nq::SVC_LIGHTSTYLE);
                    body.byte(i as u8);
                    body.string(&s);
                }
                SVC_SOUND => {
                    let channel = m.short();
                    let volume = if channel & SND_VOLUME != 0 { m.byte() } else { nq::DEFAULT_SOUND_PACKET_VOLUME };
                    let atten = if channel & SND_ATTENUATION != 0 { m.byte() } else { 64 };
                    let sound = m.byte();
                    let pos = coord3(&mut m);
                    let ent = (channel >> 3) & 1023;
                    body.byte(nq::SVC_SOUND);
                    body.byte((nq::SND_VOLUME | nq::SND_ATTENUATION) as u8);
                    body.byte(volume as u8);
                    body.byte(atten as u8);
                    body.0.short(ent << 3 | (channel & 7));
                    body.byte(sound as u8);
                    body.vec(pos);
                }
                SVC_STOPSOUND => {
                    let i = m.short();
                    body.byte(nq::SVC_STOPSOUND);
                    body.0.short(i);
                }
                SVC_UPDATEFRAGS => {
                    let i = self.slot(m.byte())?;
                    self.players[i].frags = m.short();
                    self.emit_frags(&mut body, i);
                }
                SVC_UPDATEPING => {
                    let i = self.slot(m.byte())?;
                    self.players[i].ping = m.short();
                }
                SVC_UPDATEPL => {
                    let i = self.slot(m.byte())?;
                    self.players[i].pl = m.byte();
                }
                SVC_UPDATEENTERTIME => {
                    self.slot(m.byte())?;
                    m.float();
                }
                SVC_SPAWNBASELINE => {
                    let i = m.short();
                    let b = Self::parse_baseline(&mut m, i.max(0) as usize);
                    if let Some(s) = self.baselines.get_mut(i.max(0) as usize) {
                        *s = b;
                    }
                }
                SVC_SPAWNSTATIC => {
                    let b = Self::parse_baseline(&mut m, 0);
                    body.byte(nq::SVC_SPAWNSTATIC);
                    body.byte(b.modelindex as u8);
                    body.byte(b.frame as u8);
                    body.byte(b.colormap as u8);
                    body.byte(b.skinnum as u8);
                    for i in 0..3 {
                        body.0.coord(b.origin[i], NQ_FLAGS);
                        body.0.angle(b.angles[i], NQ_FLAGS);
                    }
                }
                SVC_TEMP_ENTITY => self.parse_tent(&mut m, &mut body)?,
                SVC_KILLEDMONSTER => body.byte(nq::SVC_KILLEDMONSTER),
                SVC_FOUNDSECRET => body.byte(nq::SVC_FOUNDSECRET),
                SVC_UPDATESTAT | SVC_UPDATESTATLONG => {
                    let i = m.byte();
                    let v = if cmd == SVC_UPDATESTAT { m.byte() } else { m.long() };
                    if !(0..nq::MAX_CL_STATS as i32).contains(&i) {
                        return err(format!("CL_SetStat: {i} is invalid"));
                    }
                    self.stats[i as usize] = v;
                    // Stats 0..=10 and items ride in svc_clientdata.
                    if i as usize > nq::STAT_ACTIVEWEAPON && i as usize != nq::STAT_ITEMS {
                        body.byte(nq::SVC_UPDATESTAT);
                        body.byte(i as u8);
                        body.0.long(v);
                    }
                }
                SVC_SPAWNSTATICSOUND => {
                    let org = coord3(&mut m);
                    let (s, v, a) = (m.byte(), m.byte(), m.byte());
                    body.byte(nq::SVC_SPAWNSTATICSOUND);
                    body.vec(org);
                    body.byte(s as u8);
                    body.byte(v as u8);
                    body.byte(a as u8);
                }
                SVC_CDTRACK => {
                    self.cdtrack = m.byte() as u8;
                    if self.nq_ready {
                        self.emit_cdtrack(&mut body);
                    }
                }
                SVC_INTERMISSION => {
                    let org = coord3(&mut m);
                    let ang = [read_angle(&mut m), read_angle(&mut m), read_angle(&mut m)];
                    self.intermission = Some((org, ang));
                    body.byte(nq::SVC_SETANGLE);
                    body.angles(ang);
                    body.byte(nq::SVC_INTERMISSION);
                }
                SVC_FINALE => {
                    let s = m.string_lossy();
                    body.byte(nq::SVC_FINALE);
                    body.string(&s);
                }
                SVC_SELLSCREEN => body.byte(nq::SVC_SELLSCREEN),
                SVC_SMALLKICK => self.punch = -2.0,
                SVC_BIGKICK => self.punch = -4.0,
                SVC_MUZZLEFLASH => {
                    let i = m.short();
                    if (1..=MAX_CLIENTS as i32).contains(&i) {
                        self.muzzle.push(i as usize - 1);
                    }
                }
                SVC_UPDATEUSERINFO => {
                    let i = self.slot(m.byte())?;
                    self.players[i].userid = m.long();
                    self.players[i].userinfo = m.string_lossy();
                    self.process_userinfo(i, &mut body);
                }
                SVC_SETINFO => {
                    let i = self.slot(m.byte())?;
                    let k = m.string_lossy();
                    let v = m.string_lossy();
                    info_set(&mut self.players[i].userinfo, &k, &v);
                    self.process_userinfo(i, &mut body);
                }
                SVC_SERVERINFO => {
                    let k = m.string_lossy();
                    let v = m.string_lossy();
                    info_set(&mut self.serverinfo, &k, &v);
                }
                SVC_DOWNLOAD => {
                    let size = m.short();
                    let percent = m.byte() as u8;
                    let mut data = Vec::new();
                    for _ in 0..size.max(0) {
                        data.push(m.byte() as u8);
                    }
                    out.actions.push(QwAction::Download { size, percent, data });
                }
                SVC_PLAYERINFO => self.parse_playerinfo(&mut m)?,
                SVC_NAILS => self.parse_nails(&mut m),
                SVC_CHOKECOUNT => {
                    m.byte();
                }
                SVC_MODELLIST | SVC_SOUNDLIST => self.parse_list(&mut m, cmd == SVC_MODELLIST, &mut out)?,
                SVC_PACKETENTITIES | SVC_DELTAPACKETENTITIES => {
                    self.parse_packet_entities(&mut m, cmd == SVC_DELTAPACKETENTITIES, inseq)?;
                    got_entities = true;
                }
                SVC_MAXSPEED => self.movevars.maxspeed = m.float(),
                SVC_ENTGRAVITY => self.movevars.entgravity = m.float(),
                SVC_SETPAUSE => {
                    self.paused = m.byte() != 0;
                    body.byte(nq::SVC_SETPAUSE);
                    body.byte(self.paused as u8);
                }
                _ => return err(format!("CL_ParseServerMessage: Illegible server message {cmd}")),
            }
        }
        if m.badread {
            return err("CL_ParseServerMessage: Bad server message");
        }
        let frame = got_entities && self.nq_ready && self.nq_signon >= 3;
        if frame && self.stage == Stage::Signon {
            self.stage = Stage::Active;
        }
        if frame || !body.0.is_empty() {
            let mut msg = Nq::new();
            if frame {
                msg.byte(nq::SVC_TIME);
                msg.0.float(realtime as f32);
            }
            msg.0.write(&body.0.data);
            if frame {
                self.emit_frame(&mut msg);
            }
            out.nq.push(msg.0.data);
        }
        Ok(out)
    }

    fn slot(&self, i: i32) -> R<usize> {
        if !(0..MAX_CLIENTS as i32).contains(&i) {
            return err(format!("CL_ParseServerMessage: bad player slot {i}"));
        }
        Ok(i as usize)
    }

    /// CL_ParseServerData.
    fn parse_serverdata(&mut self, m: &mut MsgReader<'_>, out: &mut Parsed) -> R {
        self.clear_level();
        // FTE/MVD extension headers come before the version; we ask for none.
        let protover = m.long();
        if protover != PROTOCOL_VERSION {
            return err(format!("Server returned version {protover}, not {PROTOCOL_VERSION}"));
        }
        self.servercount = m.long();
        self.gamedir = m.string_lossy();
        let pn = m.byte();
        self.spectator = pn & 128 != 0;
        self.playernum = (pn & 127) as usize;
        if self.playernum >= MAX_CLIENTS {
            return err(format!("bad playernum {}", self.playernum));
        }
        self.levelname = m.string_lossy();
        let mv = &mut self.movevars;
        for f in [
            &mut mv.gravity,
            &mut mv.stopspeed,
            &mut mv.maxspeed,
            &mut mv.spectatormaxspeed,
            &mut mv.accelerate,
            &mut mv.airaccelerate,
            &mut mv.wateraccelerate,
            &mut mv.friction,
            &mut mv.waterfriction,
            &mut mv.entgravity,
        ] {
            *f = m.float();
        }
        self.stage = Stage::Precache;
        out.actions.push(QwAction::Cmd(format!("soundlist {} 0", self.servercount)));
        Ok(())
    }

    /// CL_ParseSoundlist / CL_ParseModellist.
    fn parse_list(&mut self, m: &mut MsgReader<'_>, models: bool, out: &mut Parsed) -> R {
        let mut n = m.byte().max(0) as usize;
        let list = if models { &mut self.model_names } else { &mut self.sound_names };
        loop {
            let s = m.string_lossy();
            if s.is_empty() || m.badread {
                break;
            }
            n += 1;
            if n >= 256 {
                return err("Server sent too many precaches");
            }
            if list.len() <= n {
                list.resize(n + 1, String::new());
            }
            list[n] = s;
        }
        let next = m.byte();
        if next > 0 {
            let what = if models { "modellist" } else { "soundlist" };
            out.actions.push(QwAction::Cmd(format!("{what} {} {next}", self.servercount)));
            return Ok(());
        }
        if !models {
            out.actions.push(QwAction::Cmd(format!("modellist {} 0", self.servercount)));
            return Ok(());
        }
        let find = |name: &str| self.model_names.iter().position(|n| n == name).unwrap_or(0);
        self.playerindex = find("progs/player.mdl");
        self.spikeindex = find("progs/spike.mdl");
        out.actions.push(QwAction::Precache);
        Ok(())
    }

    /// Files the host must have before [`QwState::finish_precache`]:
    /// models (not inline `*N` ones) then sounds.
    pub fn needed_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self.model_names.iter().skip(1).filter(|n| !n.is_empty() && !n.starts_with('*')).cloned().collect();
        v.extend(self.sound_names.iter().skip(1).filter(|n| !n.is_empty()).map(|n| format!("sound/{n}")));
        v
    }

    /// Lists and downloads are done: the NQ serverinfo (signon 1) for the
    /// client, and the `prespawn` command with the map's checksum2.
    pub fn finish_precache(&mut self, map_checksum2: u32) -> (Vec<u8>, String) {
        let mut b = Nq::new();
        b.byte(nq::SVC_SERVERINFO);
        b.0.long(nq::PROTOCOL_RMQ as i32);
        b.0.long(NQ_FLAGS as i32);
        b.byte(MAX_CLIENTS as u8);
        b.byte(if self.deathmatch() { nq::GAME_DEATHMATCH } else { nq::GAME_COOP });
        b.string(&self.levelname);
        for n in self.model_names.iter().skip(1) {
            b.string(n);
        }
        b.string("");
        for n in self.sound_names.iter().skip(1) {
            b.string(n);
        }
        b.string("");
        self.nq_ready = true;
        self.emit_cdtrack(&mut b);
        b.byte(nq::SVC_SETVIEW);
        b.0.short(self.playernum as i32 + 1);
        for i in 0..MAX_CLIENTS {
            self.emit_player(&mut b, i);
            self.emit_frags(&mut b, i);
        }
        self.nq_signon = 1;
        b.byte(nq::SVC_SIGNONNUM);
        b.byte(1);
        self.stage = Stage::Signon;
        (b.0.data, format!("prespawn {} 0 {}", self.servercount, map_checksum2 as i32))
    }

    fn emit_cdtrack(&self, b: &mut Nq) {
        b.byte(nq::SVC_CDTRACK);
        b.byte(self.cdtrack);
        b.byte(self.cdtrack);
    }

    fn emit_frags(&self, b: &mut Nq, i: usize) {
        if self.nq_ready {
            b.byte(nq::SVC_UPDATEFRAGS);
            b.byte(i as u8);
            b.0.short(self.players[i].frags);
        }
    }

    fn emit_player(&self, b: &mut Nq, i: usize) {
        if !self.nq_ready {
            return;
        }
        let p = &self.players[i];
        b.byte(nq::SVC_UPDATENAME);
        b.byte(i as u8);
        b.string(&p.name);
        b.byte(nq::SVC_UPDATECOLORS);
        b.byte(i as u8);
        b.byte(((p.topcolor.clamp(0, 15) << 4) | p.bottomcolor.clamp(0, 15)) as u8);
    }

    /// CL_ProcessUserInfo.
    fn process_userinfo(&mut self, i: usize, b: &mut Nq) {
        let p = &mut self.players[i];
        p.name = info_value(&p.userinfo, "name").to_string();
        p.topcolor = info_value(&p.userinfo, "topcolor").parse().unwrap_or(0);
        p.bottomcolor = info_value(&p.userinfo, "bottomcolor").parse().unwrap_or(0);
        p.spectator = !info_value(&p.userinfo, "*spectator").is_empty();
        self.emit_player(b, i);
    }

    /// Stuffed command lines: the signon commands are handled here, the
    /// rest go to the console as NQ stufftext.
    fn stuff_text(&mut self, s: &str, body: &mut Nq, out: &mut Parsed) {
        self.stuffbuf.push_str(s);
        while let Some(nl) = self.stuffbuf.find('\n') {
            let line: String = self.stuffbuf.drain(..=nl).collect();
            let t = line.trim();
            let (word, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
            let rest = rest.trim();
            match word {
                "cmd" => {
                    if rest.starts_with("spawn") && self.nq_signon == 1 {
                        self.nq_signon = 2;
                        body.byte(nq::SVC_SIGNONNUM);
                        body.byte(2);
                    }
                    out.actions.push(QwAction::Cmd(rest.to_string()));
                }
                "skins" => {
                    if self.nq_signon == 2 {
                        self.nq_signon = 3;
                        body.byte(nq::SVC_SIGNONNUM);
                        body.byte(3);
                    }
                    out.actions.push(QwAction::Cmd(format!("begin {}", self.servercount)));
                }
                "reconnect" => out.actions.push(QwAction::Reconnect),
                "changing" | "nextul" | "stopul" => {}
                "fullserverinfo" => self.serverinfo = rest.trim_matches('"').to_string(),
                "packet" => {
                    let (addr, data) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                    let data = data.trim().trim_matches('"').replace("\\n", "\n");
                    out.actions.push(QwAction::Packet { addr: addr.to_string(), data });
                }
                _ => {
                    body.byte(nq::SVC_STUFFTEXT);
                    body.string(&line);
                }
            }
        }
    }

    /// CL_ParseBaseline.
    fn parse_baseline(m: &mut MsgReader<'_>, number: usize) -> QwEntity {
        let mut e = QwEntity { number, ..Default::default() };
        e.modelindex = m.byte().max(0) as usize;
        e.frame = m.byte();
        e.colormap = m.byte();
        e.skinnum = m.byte();
        for i in 0..3 {
            e.origin[i] = read_coord(m);
            e.angles[i] = read_angle(m);
        }
        e
    }

    /// CL_ParseTEnt, rewritten as NQ temp entities or particles.
    fn parse_tent(&mut self, m: &mut MsgReader<'_>, b: &mut Nq) -> R {
        let ty = m.byte() as u8;
        let particles = |b: &mut Nq, org: [f32; 3], color: u8, count: i32| {
            b.byte(nq::SVC_PARTICLE);
            b.vec(org);
            for _ in 0..3 {
                b.byte(0);
            }
            b.byte(count.clamp(1, 254) as u8);
            b.byte(color);
        };
        match ty {
            TE_GUNSHOT => {
                let count = m.byte();
                let pos = coord3(m);
                particles(b, pos, 0, 20 * count);
            }
            TE_BLOOD => {
                let count = m.byte();
                let pos = coord3(m);
                particles(b, pos, 73, 20 * count);
            }
            TE_LIGHTNINGBLOOD => {
                let pos = coord3(m);
                particles(b, pos, 225, 50);
            }
            TE_LIGHTNING1 | TE_LIGHTNING2 | TE_LIGHTNING3 => {
                let ent = m.short();
                let (s, e) = (coord3(m), coord3(m));
                b.byte(nq::SVC_TEMP_ENTITY);
                b.byte(ty);
                b.0.short(ent);
                b.vec(s);
                b.vec(e);
            }
            TE_SPIKE | TE_SUPERSPIKE | TE_EXPLOSION | TE_TAREXPLOSION | TE_WIZSPIKE | TE_KNIGHTSPIKE | TE_LAVASPLASH | TE_TELEPORT => {
                let pos = coord3(m);
                b.byte(nq::SVC_TEMP_ENTITY);
                b.byte(ty);
                b.vec(pos);
            }
            _ => return err(format!("CL_ParseTEnt: bad type {ty}")),
        }
        Ok(())
    }

    /// CL_ParsePlayerinfo.
    fn parse_playerinfo(&mut self, m: &mut MsgReader<'_>) -> R {
        let num = m.byte();
        if !(0..MAX_CLIENTS as i32).contains(&num) {
            return err("CL_ParsePlayerinfo: bad num");
        }
        let fi = Self::frame_index(self.parsecount);
        let senttime = self.frames[fi].senttime;
        let playerindex = self.playerindex;
        let st = &mut self.frames[fi].playerstate[num as usize];
        let flags = m.short() as u16 as u32;
        st.flags = flags;
        st.messagenum = self.parsecount;
        st.present = true;
        st.origin = coord3(m);
        st.frame = m.byte();
        st.state_time = if flags & PF_MSEC != 0 { senttime - m.byte() as f64 * 0.001 } else { senttime };
        if flags & PF_COMMAND != 0 {
            st.command = read_delta_usercmd(m, &UserCmd::default());
        }
        for i in 0..3 {
            st.velocity[i] = if flags & (PF_VELOCITY1 << i) != 0 { m.short() as f32 } else { 0.0 };
        }
        st.modelindex = if flags & PF_MODEL != 0 { m.byte().max(0) as usize } else { playerindex };
        st.skinnum = if flags & PF_SKINNUM != 0 { m.byte() } else { 0 };
        st.effects = if flags & PF_EFFECTS != 0 { m.byte() } else { 0 };
        st.weaponframe = if flags & PF_WEAPONFRAME != 0 { m.byte() } else { 0 };
        st.viewangles = st.command.angles;
        Ok(())
    }

    /// CL_ParseProjectiles.
    fn parse_nails(&mut self, m: &mut MsgReader<'_>) {
        let c = m.byte().max(0);
        for _ in 0..c {
            let mut b = [0i32; 6];
            for x in &mut b {
                *x = m.byte() & 255;
            }
            if self.nails.len() == MAX_PROJECTILES {
                continue;
            }
            let o = [
                (((b[0] + ((b[1] & 15) << 8)) << 1) - 4096) as f32,
                ((((b[1] >> 4) + (b[2] << 4)) << 1) - 4096) as f32,
                (((b[3] + ((b[4] & 15) << 8)) << 1) - 4096) as f32,
            ];
            let a = [(360 * (b[4] >> 4) / 16) as f32, (360 * b[5] / 256) as f32, 0.0];
            self.nails.push((o, a));
        }
    }

    /// CL_ParseDelta.
    fn parse_delta(m: &mut MsgReader<'_>, from: &QwEntity, word: u32) -> QwEntity {
        let mut to = *from;
        to.number = (word & 511) as usize;
        let mut bits = word & !511;
        if bits & U_MOREBITS != 0 {
            bits |= m.byte() as u32 & 255;
        }
        to.flags = bits;
        if bits & U_MODEL != 0 {
            to.modelindex = m.byte().max(0) as usize;
        }
        if bits & U_FRAME != 0 {
            to.frame = m.byte();
        }
        if bits & U_COLORMAP != 0 {
            to.colormap = m.byte();
        }
        if bits & U_SKIN != 0 {
            to.skinnum = m.byte();
        }
        if bits & U_EFFECTS != 0 {
            to.effects = m.byte();
        }
        for (i, (ob, ab)) in [(U_ORIGIN1, U_ANGLE1), (U_ORIGIN2, U_ANGLE2), (U_ORIGIN3, U_ANGLE3)].into_iter().enumerate() {
            if bits & ob != 0 {
                to.origin[i] = read_coord(m);
            }
            if bits & ab != 0 {
                to.angles[i] = read_angle(m);
            }
        }
        to
    }

    /// CL_ParsePacketEntities. Frames are keyed by the acknowledged
    /// outgoing sequence (the server keys its copies by the sequence it
    /// received), and `from` names the frame the server deltas against.
    fn parse_packet_entities(&mut self, m: &mut MsgReader<'_>, delta: bool, _inseq: u32) -> R {
        let newseq = self.parsecount;
        let newi = Self::frame_index(newseq);
        let mut old: Vec<QwEntity> = Vec::new();
        let mut full = true;
        let mut valid = true;
        if delta {
            let from = m.byte() as u32;
            full = false;
            let oldi = Self::frame_index(from);
            let f = &self.frames[oldi];
            match f.entities_seq {
                Some(s) if s & 255 == from && self.outgoing_sequence.wrapping_sub(s) < UPDATE_BACKUP as u32 - 1 => {
                    old = f.entities.clone();
                }
                _ => valid = false,
            }
        }
        let mut new: Vec<QwEntity> = Vec::with_capacity(old.len());
        let mut oi = 0;
        loop {
            let word = m.short() as u16 as u32;
            if m.badread {
                return err("msg_badread in packetentities");
            }
            if word == 0 {
                new.extend_from_slice(&old[oi.min(old.len())..]);
                break;
            }
            if !valid {
                // FlushEntityPacket: read and ignore.
                Self::parse_delta(m, &QwEntity::default(), word);
                continue;
            }
            let newnum = (word & 511) as usize;
            while oi < old.len() && old[oi].number < newnum {
                if full {
                    valid = false;
                    break;
                }
                new.push(old[oi]);
                oi += 1;
            }
            if !valid {
                Self::parse_delta(m, &QwEntity::default(), word);
                continue;
            }
            if oi < old.len() && old[oi].number == newnum {
                if word & U_REMOVE != 0 {
                    oi += 1;
                    continue;
                }
                let e = Self::parse_delta(m, &old[oi], word);
                new.push(e);
                oi += 1;
            } else {
                // New from the baseline.
                if word & U_REMOVE != 0 {
                    if full {
                        valid = false;
                    }
                    continue;
                }
                let base = self.baselines.get(newnum).copied().unwrap_or_default();
                let e = Self::parse_delta(m, &base, word);
                new.push(e);
            }
            if new.len() > MAX_PACKET_ENTITIES * 8 {
                return err("CL_ParsePacketEntities: too many entities");
            }
        }
        let f = &mut self.frames[newi];
        if valid {
            f.entities = new;
            f.entities_seq = Some(newseq);
            self.validsequence = newseq;
        } else {
            f.entities_seq = None;
            self.validsequence = 0;
        }
        Ok(())
    }

    /// The NQ entity frame: svc_clientdata, players, packet entities, nails.
    fn emit_frame(&mut self, b: &mut Nq) {
        let fi = Self::frame_index(self.parsecount);
        let me = self.frames[fi].playerstate[self.playernum];
        self.emit_clientdata(b, &me);
        let muzzle = std::mem::take(&mut self.muzzle);
        for j in 0..MAX_CLIENTS {
            let st = self.frames[fi].playerstate[j];
            if st.messagenum != self.parsecount || !st.present {
                continue;
            }
            let local = j == self.playernum;
            if st.modelindex == 0 && !local {
                continue;
            }
            let mut origin = st.origin;
            if local && let Some((o, _)) = self.intermission {
                origin = o;
            }
            let pitch = -st.viewangles[0] / 3.0;
            let angles = [pitch, st.viewangles[1], 0.0];
            let mut effects = map_effects(st.effects);
            if muzzle.contains(&j) {
                effects |= crate::EF_MUZZLEFLASH;
            }
            let model = if local && self.intermission.is_some() { 0 } else { st.modelindex };
            let colormap = if st.modelindex == self.playerindex { j as i32 + 1 } else { 0 };
            emit_update(b, j + 1, model, st.frame, colormap, st.skinnum, effects, origin, angles);
        }
        if self.validsequence != 0 {
            let ents = &self.frames[fi].entities;
            for e in ents {
                if e.modelindex == 0 && map_effects(e.effects) == 0 {
                    continue;
                }
                emit_update(b, e.number, e.modelindex, e.frame, e.colormap, e.skinnum, map_effects(e.effects), e.origin, e.angles);
            }
        } else if let Some(prev) = self.last_valid_frame() {
            // No usable delta this packet: keep showing the last valid entities.
            for e in prev {
                emit_update(b, e.number, e.modelindex, e.frame, e.colormap, e.skinnum, map_effects(e.effects), e.origin, e.angles);
            }
        }
        for (k, (o, a)) in self.nails.iter().enumerate() {
            emit_update(b, NAIL_BASE + k, self.spikeindex, 0, 0, 0, 0, *o, *a);
        }
    }

    fn last_valid_frame(&self) -> Option<Vec<QwEntity>> {
        self.frames.iter().filter_map(|f| f.entities_seq.map(|s| (s, &f.entities))).max_by_key(|(s, _)| *s).map(|(_, e)| e.clone())
    }

    /// svc_clientdata from the QW stats and the local player state.
    fn emit_clientdata(&mut self, b: &mut Nq, me: &QwPlayerState) {
        use nq::*;
        let s = &self.stats;
        let viewheight: i32 = if me.flags & PF_GIB != 0 {
            8
        } else if me.flags & PF_DEAD != 0 {
            -16
        } else {
            DEFAULT_VIEWHEIGHT as i32
        };
        let mut bits = SU_ITEMS | SU_WEAPONFRAME | SU_ARMOR | SU_WEAPON;
        if viewheight != DEFAULT_VIEWHEIGHT as i32 {
            bits |= SU_VIEWHEIGHT;
        }
        if self.punch != 0.0 {
            bits |= SU_PUNCH1;
        }
        let vel: Vec<i32> = me.velocity.iter().map(|v| (*v / 16.0).clamp(-127.0, 127.0) as i32).collect();
        for (i, v) in vel.iter().enumerate() {
            if *v != 0 {
                bits |= SU_VELOCITY1 << i;
            }
        }
        let hi = |v: i32| (v >> 8) & 0xff != 0;
        for (bit, v) in [
            (SU_WEAPON2, s[STAT_WEAPON]),
            (SU_ARMOR2, s[STAT_ARMOR]),
            (SU_AMMO2, s[STAT_AMMO]),
            (SU_SHELLS2, s[STAT_SHELLS]),
            (SU_NAILS2, s[STAT_NAILS]),
            (SU_ROCKETS2, s[STAT_ROCKETS]),
            (SU_CELLS2, s[STAT_CELLS]),
            (SU_WEAPONFRAME2, me.weaponframe),
        ] {
            if hi(v) {
                bits |= bit;
            }
        }
        if bits & 0xff00_0000 != 0 {
            bits |= SU_EXTEND2;
        }
        if bits & 0x00ff_0000 != 0 {
            bits |= SU_EXTEND1;
        }
        b.byte(SVC_CLIENTDATA);
        b.0.short(bits as i32);
        if bits & SU_EXTEND1 != 0 {
            b.byte((bits >> 16) as u8);
        }
        if bits & SU_EXTEND2 != 0 {
            b.byte((bits >> 24) as u8);
        }
        if bits & SU_VIEWHEIGHT != 0 {
            b.byte(viewheight as i8 as u8);
        }
        for (i, v) in vel.iter().enumerate() {
            if i == 0 && bits & SU_PUNCH1 != 0 {
                b.byte(self.punch as i8 as u8);
            }
            if bits & (SU_VELOCITY1 << i) != 0 {
                b.byte(*v as i8 as u8);
            }
        }
        self.punch = 0.0;
        b.0.long(s[STAT_ITEMS]);
        b.byte(me.weaponframe as u8);
        b.byte(s[STAT_ARMOR] as u8);
        b.byte(s[STAT_WEAPON] as u8);
        b.0.short(s[STAT_HEALTH]);
        b.byte(s[STAT_AMMO] as u8);
        for i in 0..4 {
            b.byte(s[STAT_SHELLS + i] as u8);
        }
        b.byte(s[STAT_ACTIVEWEAPON] as u8);
        for (bit, v) in [
            (SU_WEAPON2, s[STAT_WEAPON]),
            (SU_ARMOR2, s[STAT_ARMOR]),
            (SU_AMMO2, s[STAT_AMMO]),
            (SU_SHELLS2, s[STAT_SHELLS]),
            (SU_NAILS2, s[STAT_NAILS]),
            (SU_ROCKETS2, s[STAT_ROCKETS]),
            (SU_CELLS2, s[STAT_CELLS]),
            (SU_WEAPONFRAME2, me.weaponframe),
        ] {
            if bits & bit != 0 {
                b.byte((v >> 8) as u8);
            }
        }
    }

    /// CL_SendCmd: store `cmd` as the command for the next outgoing
    /// sequence and build the clc_move (+ clc_delta) payload. The caller
    /// passes the payload to the netchan, whose outgoing_sequence must
    /// equal `self.outgoing_sequence` before the call.
    pub fn build_cmd(&mut self, cmd: UserCmd, lost: u8, realtime: f64) -> Vec<u8> {
        let seq = self.outgoing_sequence;
        let i = Self::frame_index(seq);
        self.frames[i].cmd = cmd;
        self.frames[i].senttime = realtime;
        self.frames[i].receivedtime = -1.0;
        let mut b = SizeBuf::new(128);
        b.byte(CLC_MOVE as i32);
        let ck = b.len();
        b.byte(0);
        b.byte(lost as i32);
        let c2 = self.frames[Self::frame_index(seq.wrapping_sub(2))].cmd;
        let c1 = self.frames[Self::frame_index(seq.wrapping_sub(1))].cmd;
        write_delta_usercmd(&mut b, &UserCmd::default(), &c2);
        write_delta_usercmd(&mut b, &c2, &c1);
        write_delta_usercmd(&mut b, &c1, &cmd);
        b.data[ck] = sequence_crc_byte(&b.data[ck + 1..], seq as i32);
        if seq.wrapping_sub(self.validsequence) >= UPDATE_BACKUP as u32 - 1 {
            self.validsequence = 0;
        }
        if self.validsequence != 0 && self.stage == Stage::Active {
            self.frames[i].delta_sequence = self.validsequence as i32;
            b.byte(CLC_DELTA as i32);
            b.byte((self.validsequence & 255) as i32);
        } else {
            self.frames[i].delta_sequence = -1;
        }
        self.outgoing_sequence = seq.wrapping_add(1);
        b.data
    }

    /// Physents for prediction: the world, inline brush models in the
    /// current frame, and (except `skip`) solid players.
    pub fn physents<'a>(&self, world: &'a qworld::CollisionModel, skip: Option<usize>) -> Vec<PhysEnt<'a>> {
        let mut v = vec![PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(world, 0) }];
        let fi = Self::frame_index(self.parsecount);
        for e in &self.frames[fi].entities {
            let Some(name) = self.model_names.get(e.modelindex) else { continue };
            let Some(sub) = name.strip_prefix('*').and_then(|n| n.parse::<usize>().ok()) else { continue };
            if world.submodels.get(sub).is_some_and(|s| s.headnode[1] != 0) {
                v.push(PhysEnt { origin: Vec3::from_array(e.origin), shape: PhysShape::Model(world, sub) });
            }
        }
        for (j, st) in self.frames[fi].playerstate.iter().enumerate() {
            if Some(j) == skip || j == self.playernum || st.messagenum != self.parsecount || !st.present || st.modelindex == 0 {
                continue;
            }
            if st.flags & PF_DEAD != 0 {
                continue;
            }
            v.push(PhysEnt {
                origin: Vec3::from_array(st.origin),
                shape: PhysShape::Box { mins: pmove::PLAYER_MINS, maxs: pmove::PLAYER_MAXS },
            });
        }
        v
    }

    /// CL_PredictMove: run the commands the server hasn't acknowledged yet
    /// from the last received local player state; sets `simorg`/`simvel`.
    pub fn predict(&mut self, world: &qworld::CollisionModel, realtime: f64, nopred: bool) {
        self.predicted = false;
        if self.paused || self.intermission.is_some() || self.validsequence == 0 || self.stage != Stage::Active {
            return;
        }
        let ack = self.parsecount;
        if self.outgoing_sequence.wrapping_sub(ack) >= UPDATE_BACKUP as u32 - 1 {
            return;
        }
        let me = self.playernum;
        let base = self.frames[Self::frame_index(ack)].playerstate[me];
        if nopred {
            self.simorg = base.origin;
            self.simvel = base.velocity;
            self.predicted = true;
            return;
        }
        let ents = self.physents(world, None);
        let time = (realtime - self.latency).min(realtime);
        let dead = self.stats[nq::STAT_HEALTH] <= 0;
        let mut from = base;
        let mut fromtime = self.frames[Self::frame_index(ack)].senttime;
        let mut to = base;
        let mut totime = fromtime;
        let mut i = 1u32;
        while i < UPDATE_BACKUP as u32 - 1 && ack.wrapping_add(i) != self.outgoing_sequence {
            let f = &self.frames[Self::frame_index(ack.wrapping_add(i))];
            let c = f.cmd;
            let mc = MoveCmd {
                msec: c.msec,
                angles: c.angles,
                forwardmove: c.forwardmove as f32,
                sidemove: c.sidemove as f32,
                upmove: c.upmove as f32,
                buttons: c.buttons,
            };
            let ps = PlayerState {
                origin: Vec3::from_array(from.origin),
                velocity: Vec3::from_array(from.velocity),
                oldbuttons: from.oldbuttons,
                waterjumptime: from.waterjumptime,
                onground: from.onground,
            };
            let r = pmove::predict(&ents, &self.movevars, &ps, &mc, dead, self.spectator);
            to = QwPlayerState {
                origin: r.origin.to_array(),
                velocity: r.velocity.to_array(),
                oldbuttons: r.oldbuttons,
                waterjumptime: r.waterjumptime,
                onground: r.onground,
                ..from
            };
            totime = f.senttime;
            if totime >= time {
                break;
            }
            from = to;
            fromtime = totime;
            i += 1;
        }
        let f = if totime == fromtime { 0.0 } else { ((time - fromtime) / (totime - fromtime)).clamp(0.0, 1.0) as f32 };
        let teleported = (0..3).any(|k| (from.origin[k] - to.origin[k]).abs() > 128.0);
        for k in 0..3 {
            if teleported {
                self.simorg[k] = to.origin[k];
                self.simvel[k] = to.velocity[k];
            } else {
                self.simorg[k] = from.origin[k] + f * (to.origin[k] - from.origin[k]);
                self.simvel[k] = from.velocity[k] + f * (to.velocity[k] - from.velocity[k]);
            }
        }
        self.sim_onground = to.onground;
        self.predicted = true;
    }

    /// Ping of player slot `i` (svc_updateping).
    pub fn ping(&self, i: usize) -> i32 {
        self.players.get(i).map_or(0, |p| p.ping)
    }

    /// Prepare for `reconnect` (a new map): stop delta compression and
    /// start the signon over.
    pub fn reconnect(&mut self) {
        self.validsequence = 0;
        self.stage = Stage::Connected;
        self.nq_signon = 0;
        self.nq_ready = false;
    }
}

/// QW EF_* → NQ effects (blue/red glows become the quad/pentagram lights).
fn map_effects(e: i32) -> i32 {
    let mut o = e & (EF_BRIGHTFIELD | EF_MUZZLEFLASH | EF_BRIGHTLIGHT | EF_DIMLIGHT);
    if e & EF_BLUE != 0 {
        o |= crate::EF_QEX_QUADLIGHT;
    }
    if e & EF_RED != 0 {
        o |= crate::EF_QEX_PENTALIGHT;
    }
    o
}

/// One NQ fast update with every field present.
#[allow(clippy::too_many_arguments)]
fn emit_update(b: &mut Nq, num: usize, model: usize, frame: i32, colormap: i32, skin: i32, effects: i32, origin: [f32; 3], angles: [f32; 3]) {
    use nq::*;
    let mut bits = U_SIGNAL | U_MOREBITS | U_MODEL | U_FRAME | U_COLORMAP | U_SKIN | U_EFFECTS;
    bits |= U_ORIGIN1 | U_ORIGIN2 | U_ORIGIN3 | U_ANGLE1 | U_ANGLE2 | U_ANGLE3;
    if num > 255 {
        bits |= U_LONGENTITY;
    }
    b.byte((bits & 0xff) as u8);
    b.byte((bits >> 8) as u8);
    if num > 255 {
        b.0.short(num as i32);
    } else {
        b.byte(num as u8);
    }
    b.byte(model as u8);
    b.byte(frame as u8);
    b.byte(colormap as u8);
    b.byte(skin as u8);
    b.byte(effects as u8);
    for i in 0..3 {
        b.0.coord(origin[i], NQ_FLAGS);
        b.0.angle(angles[i], NQ_FLAGS);
    }
}

#[cfg(test)]
mod tests;
