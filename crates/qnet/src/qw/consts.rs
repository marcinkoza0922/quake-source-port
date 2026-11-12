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
