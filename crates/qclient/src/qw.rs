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
mod level;
mod emit;
mod entities;
mod frame_out;
mod client_cmd;
mod parse;

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

    fn slot(&self, i: i32) -> R<usize> {
        if !(0..MAX_CLIENTS as i32).contains(&i) {
            return err(format!("CL_ParseServerMessage: bad player slot {i}"));
        }
        Ok(i as usize)
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
