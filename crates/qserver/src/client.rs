//! In-memory client slots.

use std::collections::VecDeque;

use crate::msg::{MAX_MSGLEN, SizeBuf};

/// Client input for one frame (clc_move contents).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ClientMove {
    pub angles: [f32; 3],
    pub forward: f32,
    pub side: f32,
    pub up: f32,
    /// bit 0 attack (button0), bit 1 jump (button2).
    pub buttons: u8,
    pub impulse: u8,
}

/// Signon progress (PRESPAWN_* in sv_main.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Signon {
    #[default]
    Done,
    Flush,
    SignonBufs,
    SignonMsg,
}

/// Max unreliable datagrams kept for the consumer (oldest dropped).
pub const MAX_QUEUED_DATAGRAMS: usize = 64;

#[derive(Debug, Clone)]
pub struct Client {
    pub active: bool,
    pub spawned: bool,
    /// In-process client: the signon handshake runs automatically on level changes.
    pub local: bool,
    pub name: String,
    pub colors: i32,
    pub spawn_parms: [f32; 16],
    pub old_frags: i32,
    /// Pending reliable message (client->message).
    pub message: SizeBuf,
    /// Datagram size limit (64000 local, DATAGRAM_MTU otherwise).
    pub max_datagram: usize,
    pub sendsignon: Signon,
    pub signonidx: usize,
    /// Delivered reliable messages, one per flush, each at most MAX_MSGLEN
    /// (consumer drains with [`crate::Server::take_reliable`]).
    pub reliable_out: Vec<Vec<u8>>,
    /// Delivered unreliable datagrams.
    pub datagrams: VecDeque<Vec<u8>>,
    /// Last move received (applied every frame like `host_client->cmd`).
    pub cmd: ClientMove,
    pub(crate) new_move: bool,
    pub(crate) stringcmds: VecDeque<String>,
}

impl Default for Client {
    fn default() -> Self {
        Client {
            active: false,
            spawned: false,
            local: false,
            name: "unconnected".into(),
            colors: 0,
            spawn_parms: [0.0; 16],
            old_frags: 0,
            message: SizeBuf::new(MAX_MSGLEN),
            max_datagram: crate::msg::MAX_DATAGRAM,
            sendsignon: Signon::Done,
            signonidx: 0,
            reliable_out: Vec::new(),
            datagrams: VecDeque::new(),
            cmd: ClientMove::default(),
            new_move: false,
            stringcmds: VecDeque::new(),
        }
    }
}
