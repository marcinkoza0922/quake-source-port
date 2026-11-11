//! protocol.h constants (duplicated from qserver::msg; see note in `msg`).

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
/// Nehahra alpha (protocol 15 only; same bit as U_EXTEND1).
pub const U_TRANS: u32 = 1 << 15;
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

pub const SND_VOLUME: u32 = 1 << 0;
pub const SND_ATTENUATION: u32 = 1 << 1;
pub const SND_LARGEENTITY: u32 = 1 << 3;
pub const SND_LARGESOUND: u32 = 1 << 4;
pub const DEFAULT_SOUND_PACKET_VOLUME: i32 = 255;
pub const DEFAULT_SOUND_PACKET_ATTENUATION: f32 = 1.0;

pub const B_LARGEMODEL: u32 = 1 << 0;
pub const B_LARGEFRAME: u32 = 1 << 1;
pub const B_ALPHA: u32 = 1 << 2;
pub const B_SCALE: u32 = 1 << 3;

pub const ENTALPHA_DEFAULT: u8 = 0;
pub const ENTALPHA_ZERO: u8 = 1;
pub const ENTSCALE_DEFAULT: u8 = 16;

/// ENTALPHA_ENCODE.
pub fn entalpha_encode(a: f32) -> u8 {
    if a == 0.0 { ENTALPHA_DEFAULT } else { crate::q_rint((a * 254.0 + 1.0).clamp(1.0, 255.0)) as u8 }
}
/// ENTALPHA_DECODE (0 → 1.0).
pub fn entalpha_decode(a: u8) -> f32 {
    if a == ENTALPHA_DEFAULT { 1.0 } else { (a as f32 - 1.0) / 254.0 }
}

pub const DEFAULT_VIEWHEIGHT: f32 = 22.0;
pub const GAME_COOP: u8 = 0;
pub const GAME_DEATHMATCH: u8 = 1;

pub const SVC_BAD: u8 = 0;
pub const SVC_NOP: u8 = 1;
pub const SVC_DISCONNECT: u8 = 2;
pub const SVC_UPDATESTAT: u8 = 3;
pub const SVC_VERSION: u8 = 4;
pub const SVC_SETVIEW: u8 = 5;
pub const SVC_SOUND: u8 = 6;
pub const SVC_TIME: u8 = 7;
pub const SVC_PRINT: u8 = 8;
pub const SVC_STUFFTEXT: u8 = 9;
pub const SVC_SETANGLE: u8 = 10;
pub const SVC_SERVERINFO: u8 = 11;
pub const SVC_LIGHTSTYLE: u8 = 12;
pub const SVC_UPDATENAME: u8 = 13;
pub const SVC_UPDATEFRAGS: u8 = 14;
pub const SVC_CLIENTDATA: u8 = 15;
pub const SVC_STOPSOUND: u8 = 16;
pub const SVC_UPDATECOLORS: u8 = 17;
pub const SVC_PARTICLE: u8 = 18;
pub const SVC_DAMAGE: u8 = 19;
pub const SVC_SPAWNSTATIC: u8 = 20;
pub const SVC_SPAWNBINARY: u8 = 21;
pub const SVC_SPAWNBASELINE: u8 = 22;
pub const SVC_TEMP_ENTITY: u8 = 23;
pub const SVC_SETPAUSE: u8 = 24;
pub const SVC_SIGNONNUM: u8 = 25;
pub const SVC_CENTERPRINT: u8 = 26;
pub const SVC_KILLEDMONSTER: u8 = 27;
pub const SVC_FOUNDSECRET: u8 = 28;
pub const SVC_SPAWNSTATICSOUND: u8 = 29;
pub const SVC_INTERMISSION: u8 = 30;
pub const SVC_FINALE: u8 = 31;
pub const SVC_CDTRACK: u8 = 32;
pub const SVC_SELLSCREEN: u8 = 33;
pub const SVC_CUTSCENE: u8 = 34;
pub const SVC_SKYBOX: u8 = 37;
pub const SVC_BF: u8 = 40;
pub const SVC_FOG: u8 = 41;
pub const SVC_SPAWNBASELINE2: u8 = 42;
pub const SVC_SPAWNSTATIC2: u8 = 43;
pub const SVC_SPAWNSTATICSOUND2: u8 = 44;
pub const SVC_ACHIEVEMENT: u8 = 52;
pub const SVC_LOCALSOUND: u8 = 56;

pub const CLC_BAD: u8 = 0;
pub const CLC_NOP: u8 = 1;
pub const CLC_DISCONNECT: u8 = 2;
pub const CLC_MOVE: u8 = 3;
pub const CLC_STRINGCMD: u8 = 4;

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
pub const TE_EXPLOSION2: u8 = 12;
pub const TE_BEAM: u8 = 13;

pub const STAT_HEALTH: usize = 0;
pub const STAT_FRAGS: usize = 1;
pub const STAT_WEAPON: usize = 2;
pub const STAT_AMMO: usize = 3;
pub const STAT_ARMOR: usize = 4;
pub const STAT_WEAPONFRAME: usize = 5;
pub const STAT_SHELLS: usize = 6;
pub const STAT_NAILS: usize = 7;
pub const STAT_ROCKETS: usize = 8;
pub const STAT_CELLS: usize = 9;
pub const STAT_ACTIVEWEAPON: usize = 10;
pub const STAT_TOTALSECRETS: usize = 11;
pub const STAT_TOTALMONSTERS: usize = 12;
pub const STAT_SECRETS: usize = 13;
pub const STAT_MONSTERS: usize = 14;
pub const STAT_ITEMS: usize = 15;
pub const MAX_CL_STATS: usize = 256;

pub const SIGNONS: u32 = 4;
pub const MAX_MSGLEN: usize = 64000;
pub const MAX_DATAGRAM: usize = 64000;
pub const MAX_MODELS: usize = 4096;
pub const MAX_SOUNDS: usize = 2048;
pub const MAX_LIGHTSTYLES: usize = 64;
pub const MAX_SCOREBOARD: usize = 16;
