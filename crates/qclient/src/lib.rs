//! qclient — NetQuake client state (port of cl_parse.c, cl_main.c,
//! cl_tent.c, cl_input.c and the particle code of r_part.c). Headless:
//! no rendering or sound, those consume [`ClientState`] and [`ClientEvent`]s.
//!
//! Per frame: feed every received message to
//! [`ClientState::parse_server_message`], then call [`ClientState::frame`]
//! (time advance, CL_RelinkEntities, beams, dlights, particles), then
//! build input with [`ClientState::build_move`].

mod anorms;
mod effects;
mod input;
mod parse;
pub mod qw;
mod relink;
mod state;

pub use effects::{Beam, Dlight, PT_BLOB, PT_BLOB2, PT_EXPLODE, PT_EXPLODE2, PT_FIRE, PT_GRAV, PT_SLOWGRAV, PT_STATIC, QRand, TempEntity};
pub use input::{InputState, MoveSpeeds, UserCmd};
pub use parse::ParseError;
pub use state::*;
