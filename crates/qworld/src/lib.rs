//! qworld: collision hulls, traces, the entity area tree and server physics
//! (port of world.c, sv_move.c, sv_phys.c, sv_user.c). See docs/DESIGN.md.

pub mod ents;
pub mod hull;
pub mod monster;
pub mod phys;
pub mod pmove;
pub mod user;
pub mod world;

pub use ents::{E, Edicts, Ent, F, V};
pub use hull::{CollisionModel, Hull, Trace};
pub use phys::PhysParams;
pub use qcore::Vec3;
pub use world::{MoveType, World};

pub const DIST_EPSILON: f32 = 0.031_25;
pub const STEPSIZE: f32 = 18.0;

pub use qformats::bsp::{
    CONTENTS_EMPTY, CONTENTS_LAVA, CONTENTS_SKY, CONTENTS_SLIME, CONTENTS_SOLID, CONTENTS_WATER,
};
pub const CONTENTS_CURRENT_0: i32 = -9;
pub const CONTENTS_CURRENT_DOWN: i32 = -14;

pub const SOLID_NOT: i32 = 0;
pub const SOLID_TRIGGER: i32 = 1;
pub const SOLID_BBOX: i32 = 2;
pub const SOLID_SLIDEBOX: i32 = 3;
pub const SOLID_BSP: i32 = 4;

pub const MOVETYPE_NONE: i32 = 0;
pub const MOVETYPE_WALK: i32 = 3;
pub const MOVETYPE_STEP: i32 = 4;
pub const MOVETYPE_FLY: i32 = 5;
pub const MOVETYPE_TOSS: i32 = 6;
pub const MOVETYPE_PUSH: i32 = 7;
pub const MOVETYPE_NOCLIP: i32 = 8;
pub const MOVETYPE_FLYMISSILE: i32 = 9;
pub const MOVETYPE_BOUNCE: i32 = 10;
pub const MOVETYPE_GIB: i32 = 11;

pub const FL_FLY: i32 = 1;
pub const FL_SWIM: i32 = 2;
pub const FL_CLIENT: i32 = 8;
pub const FL_INWATER: i32 = 16;
pub const FL_MONSTER: i32 = 32;
pub const FL_ITEM: i32 = 256;
pub const FL_ONGROUND: i32 = 512;
pub const FL_PARTIALGROUND: i32 = 1024;
pub const FL_WATERJUMP: i32 = 2048;
