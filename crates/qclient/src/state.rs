//! client_state_t / entity_t equivalents.

use qecs::{ParticlePool, Pool};
use qnet::SizeBuf;
use qnet::protocol::*;

use crate::effects::{Beam, Dlight, QRand, TempEntity};

pub const MAX_EDICTS: usize = 32000;
pub const MAX_STATIC_ENTITIES: usize = 4096;
pub const MAX_BEAMS: usize = 32;
pub const MAX_DLIGHTS: usize = 64;
pub const MAX_PARTICLES: usize = 16384;
pub const MAX_TEMP_ENTITIES: usize = 256;

pub const LERP_MOVESTEP: u32 = 1 << 0;
pub const LERP_RESETANIM: u32 = 1 << 1;
pub const LERP_RESETANIM2: u32 = 1 << 2;
pub const LERP_RESETMOVE: u32 = 1 << 3;
pub const LERP_FINISH: u32 = 1 << 4;

/// Entity `effects` bits (server.h).
pub const EF_BRIGHTFIELD: i32 = 1;
pub const EF_MUZZLEFLASH: i32 = 2;
pub const EF_BRIGHTLIGHT: i32 = 4;
pub const EF_DIMLIGHT: i32 = 8;
pub const EF_QEX_QUADLIGHT: i32 = 16;
pub const EF_QEX_PENTALIGHT: i32 = 32;

/// Model flags (MDL header `flags`), set by the host via [`ClientState::model_flags`].
pub const MF_ROCKET: u32 = 1;
pub const MF_GRENADE: u32 = 2;
pub const MF_GIB: u32 = 4;
pub const MF_ROTATE: u32 = 8;
pub const MF_TRACER: u32 = 16;
pub const MF_ZOMGIB: u32 = 32;
pub const MF_TRACER2: u32 = 64;
pub const MF_TRACER3: u32 = 128;

/// entity_state_t (baseline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityState {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub modelindex: usize,
    pub frame: i32,
    pub colormap: i32,
    pub skin: i32,
    pub effects: i32,
    pub alpha: u8,
    pub scale: u8,
    /// Particle precache indices (DP_ENT_TRAILEFFECTNUM; baselines from
    /// svcfte_spawnbaseline2 / spawnstatic2 only).
    pub traileffect: u16,
    pub emiteffect: u16,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            angles: [0.0; 3],
            modelindex: 0,
            frame: 0,
            colormap: 0,
            skin: 0,
            effects: 0,
            alpha: ENTALPHA_DEFAULT,
            scale: ENTSCALE_DEFAULT,
            traileffect: 0,
            emiteffect: 0,
        }
    }
}

/// entity_t (client side). `model == 0` means no model (empty slot).
#[derive(Debug, Clone, Default)]
pub struct ClientEntity {
    pub baseline: EntityState,
    pub msgtime: f64,
    /// [0] = last received, [1] = previous (interpolation endpoints).
    pub msg_origins: [[f32; 3]; 2],
    pub msg_angles: [[f32; 3]; 2],
    /// Interpolated (CL_RelinkEntities output).
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub model: usize,
    pub frame: i32,
    pub skin: i32,
    pub colormap: i32,
    pub effects: i32,
    pub alpha: u8,
    pub scale: u8,
    pub lerpflags: u32,
    pub lerpfinish: f64,
    pub forcelink: bool,
    pub syncbase: f32,
    pub trailorg: [f32; 3],
    pub traildelay: f32,
    /// Particle precache index of the entity's trail / per-frame emitter
    /// (0 = none; replacement deltas only).
    pub traileffect: u16,
    pub emiteffect: u16,
    /// Replacement deltas: last received state, and whether the entity is
    /// in the server's snapshot.
    pub netstate: qnet::delta::EntState,
    pub delta_live: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Score {
    pub name: String,
    pub frags: i32,
    pub colors: i32,
    /// Round trip in ms, filled by the host (local server or `ping` replies).
    pub ping: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StaticSound {
    pub origin: [f32; 3],
    pub sound: usize,
    pub volume: i32,
    pub attenuation: i32,
}

/// svc_fog state (Fog_ParseServerMessage).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fog {
    pub density: f32,
    pub color: [f32; 3],
    /// Fade time in seconds.
    pub time: f32,
}

/// Side effects for the host (sound, console, command buffer).
#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    Print(String),
    CenterPrint(String),
    /// One complete stufftext line (with trailing '\n'), for the command buffer.
    StuffText(String),
    Sound { ent: i32, channel: i32, sound: usize, origin: [f32; 3], volume: f32, attenuation: f32 },
    /// Engine sound by name from a temp entity (ent -1, channel 0).
    TempSound { name: &'static str, origin: [f32; 3] },
    LocalSound { sound: usize },
    StopSound { ent: i32, channel: i32 },
    StaticSound(StaticSound),
    CdTrack { track: u8, loop_track: u8 },
    Damage { armor: i32, blood: i32, from: [f32; 3] },
    Pause(bool),
    Skybox(String),
    Fog(Fog),
    /// `bf` (bonus flash).
    BonusFlash,
    SellScreen,
    Achievement(String),
    LightStyle(usize),
    /// Signon stage reached (CL_SignonReply); 4 = fully connected.
    Signon(u32),
    NewMap,
    Disconnect,
}

pub struct ClientState {
    pub protocol: u32,
    pub protocolflags: u32,
    /// cls.signon (0..=SIGNONS).
    pub signon: u32,
    pub maxclients: usize,
    pub gametype: u8,
    pub levelname: String,
    /// Bare map name ("start").
    pub mapname: String,
    /// Index 0 is always "" (no model).
    pub model_precache: Vec<String>,
    pub sound_precache: Vec<String>,
    /// MF_* flags per model index; fill after loading models.
    pub model_flags: Vec<u32>,
    pub entities: Vec<ClientEntity>,
    pub static_entities: Vec<ClientEntity>,
    pub static_sounds: Vec<StaticSound>,
    /// Entity indices to draw this frame (CL_RelinkEntities).
    pub visedicts: Vec<usize>,
    /// Entities the host's debris physics draws itself (ragdolls, gibs):
    /// skipped when drawing and get no model trail. Empty when unused.
    pub debris_owned: Vec<bool>,
    pub temp_entities: Vec<TempEntity>,
    pub viewentity: usize,
    /// Weapon model state (cl.viewent).
    pub viewent: ClientEntity,

    pub mtime: [f64; 2],
    /// cls.demoplayback: view angles come from the demo (`mviewangles`).
    pub demoplayback: bool,
    /// Demo view angles for the last two messages (cl.mviewangles).
    pub mviewangles: [[f32; 3]; 2],
    pub time: f64,
    pub oldtime: f64,
    /// Skip interpolation (local server without netinterval, timedemo, cl_nolerp).
    pub nolerp: bool,
    pub paused: bool,

    pub stats: [i32; MAX_CL_STATS],
    pub items: i32,
    pub item_gettime: [f64; 32],
    /// standard_quake: false for hipnotic/rogue (STAT_ACTIVEWEAPON is a bit index).
    pub standard_quake: bool,
    pub scores: Vec<Score>,
    pub lightstyles: Vec<String>,

    pub viewangles: [f32; 3],
    pub fixangle: bool,
    pub mvelocity: [[f32; 3]; 2],
    pub velocity: [f32; 3],
    pub punchangle: [f32; 3],
    pub punchangles: [[f32; 3]; 2],
    pub punchtime: f64,
    pub viewheight: f32,
    pub idealpitch: f32,
    pub onground: bool,
    pub inwater: bool,

    /// 0 none, 1 intermission, 2 finale, 3 cutscene.
    pub intermission: u8,
    pub completed_time: f64,
    pub centerprint: String,
    pub centerprint_time: f64,
    pub cdtrack: u8,
    pub looptrack: u8,
    pub skybox: String,
    pub fog: Fog,

    pub beams: Pool<Beam>,
    /// Bullet/nail/explosion impact points since the renderer last took
    /// them (water splashes where the shot crossed a liquid surface); the
    /// flag marks explosions.
    pub shot_impacts: Vec<([f32; 3], bool)>,
    /// Where shots drew blood this frame (`svc_particle` in blood colour:
    /// QC's SpawnBlood), so client effects know a body was hit.
    pub blood_hits: Vec<[f32; 3]>,
    pub dlights: Pool<Dlight>,
    pub particles: ParticlePool,
    pub(crate) avelocities: Vec<[f32; 3]>,
    pub(crate) tracercount: i32,
    pub rng: QRand,
    pub sv_gravity: f32,
    /// Effects handled by FTE scripted particles (set by the host).
    pub fte_names: crate::fte::FteNames,
    /// Scripted effects to run this frame (drained by the host).
    pub fte_queue: Vec<crate::fte::FteRequest>,
    /// svcdp_precache particle effect names (index 0 unused).
    pub particle_precache: Vec<String>,
    /// Active FTE protocol extensions (PEXT2_*) from svc_serverinfo.
    pub pext2: u32,
    /// Sequence of the datagram being parsed (network play), for acks.
    pub net_sequence: Option<u32>,
    /// clcdp_ackframe values to send with the next move (-1 = resend all).
    pub ack_frames: Vec<i32>,

    pub events: Vec<ClientEvent>,
    pub(crate) stuffcmdbuf: Vec<u8>,
    /// Reliable clc_* bytes to send (signon replies, string commands).
    pub out_reliable: SizeBuf,
    pub movemessages: u32,
    /// Player name/colors for the signon reply.
    pub name: String,
    pub color: i32,
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            protocol: PROTOCOL_NETQUAKE,
            protocolflags: 0,
            signon: 0,
            maxclients: 1,
            gametype: 0,
            levelname: String::new(),
            mapname: String::new(),
            model_precache: vec![String::new()],
            sound_precache: vec![String::new()],
            model_flags: vec![0],
            entities: vec![ClientEntity::default()],
            static_entities: Vec::new(),
            static_sounds: Vec::new(),
            visedicts: Vec::new(),
            debris_owned: Vec::new(),
            temp_entities: Vec::new(),
            viewentity: 0,
            viewent: ClientEntity::default(),
            mtime: [0.0; 2],
            demoplayback: false,
            mviewangles: [[0.0; 3]; 2],
            time: 0.0,
            oldtime: 0.0,
            nolerp: false,
            paused: false,
            stats: [0; MAX_CL_STATS],
            items: 0,
            item_gettime: [0.0; 32],
            standard_quake: true,
            scores: vec![Score::default()],
            lightstyles: vec![String::new(); MAX_LIGHTSTYLES],
            viewangles: [0.0; 3],
            fixangle: false,
            mvelocity: [[0.0; 3]; 2],
            velocity: [0.0; 3],
            punchangle: [0.0; 3],
            punchangles: [[0.0; 3]; 2],
            punchtime: 0.0,
            viewheight: DEFAULT_VIEWHEIGHT,
            idealpitch: 0.0,
            onground: false,
            inwater: false,
            intermission: 0,
            completed_time: 0.0,
            centerprint: String::new(),
            centerprint_time: 0.0,
            cdtrack: 0,
            looptrack: 0,
            skybox: String::new(),
            fog: Fog::default(),
            beams: Pool::with_max(MAX_BEAMS),
            shot_impacts: Vec::new(),
            blood_hits: Vec::new(),
            dlights: Pool::with_max(MAX_DLIGHTS),
            particles: ParticlePool::new(MAX_PARTICLES),
            avelocities: Vec::new(),
            tracercount: 0,
            rng: QRand::default(),
            sv_gravity: 800.0,
            fte_names: Default::default(),
            fte_queue: Vec::new(),
            particle_precache: Vec::new(),
            pext2: 0,
            net_sequence: None,
            ack_frames: Vec::new(),
            events: Vec::new(),
            stuffcmdbuf: Vec::new(),
            out_reliable: SizeBuf::new(MAX_MSGLEN),
            movemessages: 0,
            name: "player".into(),
            color: 0,
        }
    }

    /// CL_ClearState (keeps connection-level settings).
    pub fn clear_state(&mut self) {
        let keep = (self.standard_quake, self.nolerp, self.name.clone(), self.color, self.sv_gravity, self.signon, self.demoplayback);
        let out = std::mem::take(&mut self.out_reliable);
        let events = std::mem::take(&mut self.events);
        let fte_names = std::mem::take(&mut self.fte_names);
        *self = Self::new();
        self.fte_names = fte_names;
        (self.standard_quake, self.nolerp, self.name, self.color, self.sv_gravity, self.signon, self.demoplayback) = keep;
        self.out_reliable = out;
        self.events = events;
    }

    /// Number of entity slots allocated (cl.num_entities).
    pub fn num_entities(&self) -> usize {
        self.entities.len()
    }

    /// Entities with a model that were in the last update.
    pub fn active_entities(&self) -> impl Iterator<Item = (usize, &ClientEntity)> {
        let t = self.mtime[0];
        self.entities.iter().enumerate().skip(1).filter(move |(_, e)| e.model != 0 && e.msgtime == t)
    }

    pub fn model_name(&self, i: usize) -> Option<&str> {
        self.model_precache.get(i).map(String::as_str).filter(|s| !s.is_empty())
    }

    pub fn take_events(&mut self) -> Vec<ClientEvent> {
        std::mem::take(&mut self.events)
    }

    /// Drain pending reliable clc_* bytes.
    pub fn take_reliable_out(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out_reliable.data)
    }

    /// Queue a clc_stringcmd.
    pub fn send_string_cmd(&mut self, s: &str) {
        self.out_reliable.byte(CLC_STRINGCMD as i32);
        self.out_reliable.string(s.as_bytes());
    }

    /// Light style brightness (CL_LightStyles: 'a' = 0, 'm' = 264 ≈ normal; 256 when empty).
    pub fn lightstyle_value(&self, i: usize) -> i32 {
        let Some(s) = self.lightstyles.get(i).map(String::as_bytes).filter(|s| !s.is_empty()) else { return 256 };
        let k = (self.time * 10.0) as usize % s.len();
        (s[k] as i32 - b'a' as i32) * 22
    }

    /// CL_SignonReply.
    pub(crate) fn signon_reply(&mut self) {
        match self.signon {
            1 => self.send_string_cmd("prespawn"),
            2 => {
                let name = format!("name \"{}\"\n", self.name);
                self.send_string_cmd(&name);
                let c = format!("color {} {}\n", self.color >> 4, self.color & 15);
                self.send_string_cmd(&c);
                self.send_string_cmd("spawn ");
            }
            3 => self.send_string_cmd("begin"),
            _ => {}
        }
        self.events.push(ClientEvent::Signon(self.signon));
    }
}
