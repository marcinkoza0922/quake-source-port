//! qnet — NetQuake message encoding and transport (common.c MSG_*,
//! protocol.h, net_dgrm.c, net_udp.c, net_loop.c). Headless, no deps.
//!
//! - [`msg`]: [`SizeBuf`] writer and [`MsgReader`] with protocol 15/666/999
//!   coord/angle encodings.
//! - [`protocol`]: svc_*/clc_*/TE_*/U_*/SU_* constants.
//! - [`delta`]: FTE replacement deltas (PEXT2_REPLACEMENTDELTAS).
//! - [`dgram`]: [`Connection`], the reliable/unreliable datagram state
//!   machine (sequence, ack, EOM fragmentation, resend).
//! - [`control`]: CCREQ/CCREP connection-control packets.
//! - [`transport`]: [`Transport`] trait with [`UdpTransport`] and
//!   [`LoopbackTransport`].

pub mod control;
pub mod delta;
pub mod dgram;
pub mod msg;
pub mod protocol;
pub mod qw;
pub mod transport;

pub use control::{Ccrep, Ccreq};
pub use dgram::{Connection, NetSocket, Received};
pub use msg::{MsgReader, SizeBuf, q_rint};
pub use transport::{LoopbackTransport, Transport, UdpTransport};
