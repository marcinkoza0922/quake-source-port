//! qserver — headless NetQuake server (port of sv_main.c, sv_phys.c,
//! sv_user.c, pr_cmds.c and the server parts of host.c/host_cmd.c).
//! See docs/DESIGN.md.
//!
//! - [`Server`]: owns [`Core`] (filesystem, cvars, clients, per-level state)
//!   and the active [`Level`] (QC VM + collision world).
//! - Clients are in-memory slots ([`Client`]): the server appends the
//!   reliable byte stream and unreliable datagrams (svc_* in protocol
//!   15/666/999 encoding) to them, and accepts [`ClientMove`]s and string
//!   commands.

pub mod addons;
pub mod assists;
mod client;
mod commands;
pub mod delta;
mod ents;
pub mod fields;
mod host;
pub mod loc;
pub mod msg;
pub mod particles;
mod send;
mod save;
mod server;
mod vis;

pub use client::{Client, ClientMove, Signon};
pub use server::{Core, Level, LevelStats, ModelInfo, Server, Sv, EdictExt, Baseline};
