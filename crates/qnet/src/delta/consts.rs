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
