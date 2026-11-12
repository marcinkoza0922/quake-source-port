//! NetQuake datagram protocol (net_dgrm.c): reliable messages split into
//! EOM-terminated fragments, each acked before the next is sent, resent
//! after 1 s; sequenced unreliable datagrams (stale ones dropped).
//!
//! [`Connection`] is transport-agnostic: feed received packets to
//! [`Connection::handle_packet`], call [`Connection::poll`] each frame and
//! send whatever [`Connection::take_outgoing`] returns. [`NetSocket`] wires
//! it to a [`Transport`].

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;

use crate::transport::Transport;

pub const NET_HEADERSIZE: usize = 8;
pub const NETFLAG_LENGTH_MASK: u32 = 0x0000_ffff;
pub const NETFLAG_DATA: u32 = 0x0001_0000;
pub const NETFLAG_ACK: u32 = 0x0002_0000;
pub const NETFLAG_NAK: u32 = 0x0004_0000;
pub const NETFLAG_EOM: u32 = 0x0008_0000;
pub const NETFLAG_UNRELIABLE: u32 = 0x0010_0000;
pub const NETFLAG_CTL: u32 = 0x8000_0000;
/// Largest reliable message (must fit NETFLAG_LENGTH_MASK).
pub const NET_MAXMESSAGE: usize = 64000;
/// Classic fragment size; every NetQuake peer accepts it.
pub const DEFAULT_FRAGMENT: usize = 1024;
/// Reliable resend timeout (seconds).
pub const RESEND_TIME: f64 = 1.0;

/// A complete message delivered by [`Connection::handle_packet`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Received {
    Reliable(Vec<u8>),
    Unreliable(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Connection {
    send_seq: u32,
    ack_seq: u32,
    unreliable_send_seq: u32,
    receive_seq: u32,
    unreliable_receive_seq: u32,
    /// Remaining bytes of the in-flight reliable message.
    send_message: Vec<u8>,
    can_send: bool,
    send_next: bool,
    last_send_time: f64,
    receive_message: Vec<u8>,
    pending: VecDeque<Vec<u8>>,
    out: Vec<Vec<u8>>,
    /// Fragment payload size.
    pub fragment: usize,
    pub dropped_datagrams: u32,
    pub resent: u32,
    /// Time a packet was last received (for timeouts).
    pub last_receive_time: f64,
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

fn header(len_flags: u32, seq: u32) -> [u8; 8] {
    let mut h = [0; 8];
    h[..4].copy_from_slice(&len_flags.to_be_bytes());
    h[4..].copy_from_slice(&seq.to_be_bytes());
    h
}

impl Connection {
    pub fn new() -> Self {
        Self {
            send_seq: 0,
            ack_seq: 0,
            unreliable_send_seq: 0,
            receive_seq: 0,
            unreliable_receive_seq: 0,
            send_message: Vec::new(),
            can_send: true,
            send_next: false,
            last_send_time: 0.0,
            receive_message: Vec::new(),
            pending: VecDeque::new(),
            out: Vec::new(),
            fragment: DEFAULT_FRAGMENT,
            dropped_datagrams: 0,
            resent: 0,
            last_receive_time: 0.0,
        }
    }

    /// Sequence number the next unreliable datagram is sent with
    /// (NET_QSocketGetSequenceOut).
    pub fn unreliable_sequence_out(&self) -> u32 {
        self.unreliable_send_seq
    }

    /// Sequence number of the last unreliable datagram received
    /// (NET_QSocketGetSequenceIn).
    pub fn unreliable_sequence_in(&self) -> u32 {
        self.unreliable_receive_seq.wrapping_sub(1)
    }

    /// NET_CanSendMessage: no reliable message in flight.
    pub fn can_send(&self) -> bool {
        self.can_send
    }
    /// Nothing in flight or queued.
    pub fn idle(&self) -> bool {
        self.can_send && self.pending.is_empty()
    }

    /// Queue a reliable message; consecutive queued messages are merged up
    /// to [`NET_MAXMESSAGE`]. Messages larger than that are rejected.
    pub fn send_reliable(&mut self, data: &[u8], now: f64) -> Result<(), &'static str> {
        if data.is_empty() {
            return Ok(());
        }
        if data.len() > NET_MAXMESSAGE {
            return Err("message too big");
        }
        match self.pending.back_mut() {
            Some(b) if b.len() + data.len() <= NET_MAXMESSAGE => b.extend_from_slice(data),
            _ => self.pending.push_back(data.to_vec()),
        }
        self.poll(now);
        Ok(())
    }

    /// Datagram_SendUnreliableMessage.
    pub fn send_unreliable(&mut self, data: &[u8]) {
        let len = (NET_HEADERSIZE + data.len()) as u32;
        let mut p = Vec::with_capacity(len as usize);
        p.extend_from_slice(&header(len | NETFLAG_UNRELIABLE, self.unreliable_send_seq));
        p.extend_from_slice(data);
        self.unreliable_send_seq = self.unreliable_send_seq.wrapping_add(1);
        self.out.push(p);
    }

    fn send_fragment(&mut self, seq: u32, now: f64) {
        let n = self.send_message.len().min(self.fragment);
        let eom = if n == self.send_message.len() { NETFLAG_EOM } else { 0 };
        let len = (NET_HEADERSIZE + n) as u32;
        let mut p = Vec::with_capacity(len as usize);
        p.extend_from_slice(&header(len | NETFLAG_DATA | eom, seq));
        p.extend_from_slice(&self.send_message[..n]);
        self.out.push(p);
        self.last_send_time = now;
    }

    /// Resend on timeout, send the next fragment, start the next queued message.
    pub fn poll(&mut self, now: f64) {
        if !self.can_send && !self.send_next && now - self.last_send_time > RESEND_TIME {
            self.send_fragment(self.send_seq.wrapping_sub(1), now);
            self.resent += 1;
        }
        if self.send_next {
            self.send_next = false;
            let s = self.send_seq;
            self.send_seq = s.wrapping_add(1);
            self.send_fragment(s, now);
        }
        if self.can_send
            && let Some(m) = self.pending.pop_front()
        {
            self.send_message = m;
            self.can_send = false;
            let s = self.send_seq;
            self.send_seq = s.wrapping_add(1);
            self.send_fragment(s, now);
        }
    }

    /// Packets ready for the transport.
    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.out)
    }

    /// Process one received packet (Datagram_GetMessage body). Control
    /// packets and garbage are ignored.
    pub fn handle_packet(&mut self, pkt: &[u8], now: f64) -> Option<Received> {
        if pkt.len() < NET_HEADERSIZE {
            return None;
        }
        let lf = u32::from_be_bytes(pkt[..4].try_into().unwrap());
        let flags = lf & !NETFLAG_LENGTH_MASK;
        let length = (lf & NETFLAG_LENGTH_MASK) as usize;
        if flags & NETFLAG_CTL != 0 || length != pkt.len() {
            return None;
        }
        let seq = u32::from_be_bytes(pkt[4..8].try_into().unwrap());
        let body = &pkt[NET_HEADERSIZE..];
        self.last_receive_time = now;
        if flags & NETFLAG_UNRELIABLE != 0 {
            if seq < self.unreliable_receive_seq {
                return None;
            }
            self.dropped_datagrams += seq - self.unreliable_receive_seq;
            self.unreliable_receive_seq = seq.wrapping_add(1);
            return Some(Received::Unreliable(body.to_vec()));
        }
        if flags & NETFLAG_ACK != 0 {
            if seq != self.send_seq.wrapping_sub(1) || seq != self.ack_seq || self.can_send {
                return None;
            }
            self.ack_seq = self.ack_seq.wrapping_add(1);
            let n = self.send_message.len().min(self.fragment);
            self.send_message.drain(..n);
            if self.send_message.is_empty() {
                self.can_send = true;
            } else {
                self.send_next = true;
            }
            self.poll(now);
            return None;
        }
        if flags & NETFLAG_DATA != 0 {
            self.out.push(header(NET_HEADERSIZE as u32 | NETFLAG_ACK, seq).to_vec());
            if seq != self.receive_seq {
                return None;
            }
            self.receive_seq = self.receive_seq.wrapping_add(1);
            if self.receive_message.len() + body.len() > NET_MAXMESSAGE {
                self.receive_message.clear();
                return None;
            }
            self.receive_message.extend_from_slice(body);
            if flags & NETFLAG_EOM != 0 {
                return Some(Received::Reliable(std::mem::take(&mut self.receive_message)));
            }
        }
        None
    }
}

/// A [`Connection`] bound to a transport and peer address.
pub struct NetSocket<T: Transport> {
    pub conn: Connection,
    pub transport: T,
    pub peer: SocketAddr,
}

impl<T: Transport> NetSocket<T> {
    pub fn new(transport: T, peer: SocketAddr) -> Self {
        Self { conn: Connection::new(), transport, peer }
    }
    pub fn send_reliable(&mut self, data: &[u8], now: f64) -> io::Result<()> {
        self.conn.send_reliable(data, now).map_err(io::Error::other)?;
        self.flush()
    }
    pub fn send_unreliable(&mut self, data: &[u8]) -> io::Result<()> {
        self.conn.send_unreliable(data);
        self.flush()
    }
    fn flush(&mut self) -> io::Result<()> {
        for p in self.conn.take_outgoing() {
            self.transport.send_to(&p, self.peer)?;
        }
        Ok(())
    }
    /// Drain the transport; returns complete messages in arrival order.
    /// Packets from other addresses are ignored.
    pub fn receive(&mut self, now: f64) -> io::Result<Vec<Received>> {
        let mut buf = vec![0u8; NET_MAXMESSAGE + NET_HEADERSIZE];
        let mut got = Vec::new();
        self.conn.poll(now);
        while let Some((n, from)) = self.transport.recv_from(&mut buf)? {
            if from != self.peer {
                continue;
            }
            if let Some(m) = self.conn.handle_packet(&buf[..n], now) {
                got.push(m);
            }
        }
        self.flush()?;
        Ok(got)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump(a: &mut Connection, b: &mut Connection, now: f64, drop_every: usize) -> Vec<Received> {
        let mut got = Vec::new();
        let mut n = 0;
        for _ in 0..1000 {
            let pa = a.take_outgoing();
            let pb = b.take_outgoing();
            if pa.is_empty() && pb.is_empty() {
                break;
            }
            for p in pa {
                n += 1;
                if drop_every != 0 && n % drop_every == 0 {
                    continue;
                }
                got.extend(b.handle_packet(&p, now));
            }
            for p in pb {
                a.handle_packet(&p, now);
            }
        }
        got
    }

    #[test]
    fn reliable_fragmented_and_unreliable() {
        let (mut a, mut b) = (Connection::new(), Connection::new());
        let big: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();
        a.send_reliable(&big, 0.0).unwrap();
        a.send_reliable(b"x", 0.0).unwrap();
        a.send_unreliable(b"u1");
        let got = pump(&mut a, &mut b, 0.0, 0);
        assert_eq!(got[0], Received::Unreliable(b"u1".to_vec()));
        assert!(got[1] == Received::Reliable(big.clone()) && got[2] == Received::Reliable(b"x".to_vec()));
        assert!(a.idle());

        // Loss: resend after timeout.
        a.send_reliable(&big, 1.0).unwrap();
        let mut got = pump(&mut a, &mut b, 1.0, 2);
        let mut t = 1.0;
        while !a.idle() && t < 30.0 {
            t += 1.1;
            a.poll(t);
            got.extend(pump(&mut a, &mut b, t, 3));
        }
        assert!(got == vec![Received::Reliable(big)], "{}", got.len());

        // Stale unreliable dropped.
        let mut c = Connection::new();
        c.send_unreliable(b"1");
        c.send_unreliable(b"2");
        let p = c.take_outgoing();
        assert!(b.handle_packet(&p[1], 0.0).is_some());
        assert!(b.handle_packet(&p[0], 0.0).is_none());
    }
}
