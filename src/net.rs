//! UDP network play (M8): the NetQuake datagram protocol between remote
//! clients and the in-process [`Server`], and a remote client connection.
//!
//! The server side ([`NetServer`]) listens on one UDP socket (default port
//! 26000). It answers CCREQ_CONNECT by reserving a server slot and replying
//! CCREP_ACCEPT with the *same* port, so all clients share the socket and
//! are told apart by address (NetQuake opened a socket per client; clients
//! simply send to whatever port the accept names, so both work). Each frame
//! [`NetServer::receive`] parses clc_* messages into the server slot and
//! [`NetServer::send`] ships the slot's reliable messages and newest
//! datagram. Protocol 15/666/999 are all served: the svc encoding is the
//! server's (`sv_protocol`), and clc_move angles follow it.
//!
//! The client side ([`NetClient`]) connects to a remote server and hands
//! complete messages to the host, which parses them with the same
//! `ClientState` used for the loopback game.

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};

use qnet::control::NET_PROTOCOL_VERSION;
use qnet::dgram::{NET_HEADERSIZE, NET_MAXMESSAGE, NETFLAG_CTL, NETFLAG_LENGTH_MASK};
use qnet::protocol::{CLC_DISCONNECT, CLC_MOVE, CLC_NOP, CLC_STRINGCMD, PROTOCOL_NETQUAKE};
use qnet::{Ccrep, Ccreq, Connection, MsgReader, Received, Transport, UdpTransport};
use qserver::{ClientMove, Server};
mod control;

pub const DEFAULT_PORT: u16 = 26000;
/// net_messagetimeout default.
pub const MESSAGE_TIMEOUT: f64 = 300.0;
/// Seconds between keepalive nops to a client that isn't receiving datagrams.
const NOP_INTERVAL: f64 = 5.0;

fn is_ctl(pkt: &[u8]) -> bool {
    pkt.len() >= 4 && u32::from_be_bytes(pkt[..4].try_into().unwrap()) & !NETFLAG_LENGTH_MASK == NETFLAG_CTL
}

struct Remote {
    addr: SocketAddr,
    slot: usize,
    conn: Connection,
    connect_time: f64,
    last_send: f64,
    /// The slot was dropped; close once the disconnect message is delivered.
    closing: Option<f64>,
}

/// A listening server socket and its remote clients.
pub struct NetServer {
    sock: UdpTransport,
    remotes: Vec<Remote>,
    buf: Vec<u8>,
}

impl NetServer {
    pub fn listen(port: u16) -> std::io::Result<Self> {
        let sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, port).into())?;
        Ok(Self { sock, remotes: Vec::new(), buf: vec![0; NET_MAXMESSAGE + NET_HEADERSIZE] })
    }

    pub fn port(&self) -> u16 {
        self.sock.local_addr().port()
    }

    /// Drop every remote client (server shutdown); sends svc_disconnect.
    pub fn shutdown(&mut self, sv: &mut Server, now: f64) {
        for i in 0..self.remotes.len() {
            let slot = self.remotes[i].slot;
            sv.drop_client(slot, false);
        }
        self.send(sv, now);
        self.remotes.clear();
    }

    /// Read every pending packet: connection requests, then clc_* messages
    /// into the server slots.
    pub fn receive(&mut self, sv: &mut Server, now: f64, hostname: &str) {
        while let Ok(Some((n, from))) = self.sock.recv_from(&mut self.buf) {
            let pkt = self.buf[..n].to_vec();
            if is_ctl(&pkt) {
                if let Some(q) = Ccreq::decode(&pkt) {
                    self.control(sv, from, q, now, hostname);
                }
                continue;
            }
            let Some(r) = self.remotes.iter_mut().find(|r| r.addr == from) else { continue };
            let slot = r.slot;
            let closing = r.closing.is_some();
            let got = r.conn.handle_packet(&pkt, now);
            if closing {
                continue;
            }
            if let Some(Received::Reliable(m) | Received::Unreliable(m)) = got
                && !read_client_message(sv, slot, &m)
            {
                sv.drop_client(slot, false);
            }
        }
        // Replacement deltas log each frame under the sequence its datagram
        // is sent with; `send` ships every datagram built until then.
        for r in &self.remotes {
            sv.set_datagram_sequence(r.slot, r.conn.unreliable_sequence_out());
        }
    }

    fn reply(&mut self, to: SocketAddr, p: Ccrep) {
        let _ = self.sock.send_to(&p.encode(), to);
    }

    /// Ship each remote slot's reliable output and newest datagram, resend
    /// lost fragments, and time out silent clients.
    pub fn send(&mut self, sv: &mut Server, now: f64) {
        let mut i = 0;
        while i < self.remotes.len() {
            let slot = self.remotes[i].slot;
            let active = sv.core.clients.get(slot).is_some_and(|c| c.active);
            let r = &mut self.remotes[i];
            if r.closing.is_none() && now - r.conn.last_receive_time.max(r.connect_time) > MESSAGE_TIMEOUT {
                sv.core.print(&format!("Client {} timed out\n", r.addr));
                sv.drop_client(slot, true);
                self.remotes.remove(i);
                continue;
            }
            if r.closing.is_none() {
                // drop_client leaves svc_disconnect in reliable_out.
                for m in sv.take_reliable(slot) {
                    let _ = r.conn.send_reliable(&m, now);
                }
                let dg = sv.take_datagrams(slot);
                // A delta snapshot may span several datagrams (all needed);
                // otherwise only the newest state matters.
                let all = sv.client_pext2(slot) & qnet::delta::PEXT2_REPLACEMENTDELTAS != 0;
                let start = if all { 0 } else { dg.len().saturating_sub(1) };
                if !dg.is_empty() {
                    for d in &dg[start..] {
                        r.conn.send_unreliable(d);
                    }
                    r.last_send = now;
                } else if now - r.last_send > NOP_INTERVAL {
                    r.conn.send_unreliable(&[qnet::protocol::SVC_NOP]);
                    r.last_send = now;
                }
                if !active {
                    r.closing = Some(now);
                }
            }
            r.conn.poll(now);
            for p in r.conn.take_outgoing() {
                let _ = self.sock.send_to(&p, r.addr);
            }
            if r.closing.is_some_and(|t| r.conn.idle() || now - t > 3.0) {
                self.remotes.remove(i);
                continue;
            }
            i += 1;
        }
    }
}

/// SV_ReadClientMessage for one message. Returns false on clc_disconnect
/// or a malformed message.
fn read_client_message(sv: &mut Server, slot: usize, m: &[u8]) -> bool {
    let mut r = MsgReader::new(m);
    let protocol = sv.core.sv.protocol;
    let flags = sv.core.sv.protocolflags;
    while !r.at_end() {
        let cmd = r.byte();
        if r.badread {
            return false;
        }
        match cmd as u8 {
            CLC_NOP => {}
            CLC_DISCONNECT => return false,
            CLC_STRINGCMD => {
                let s = r.string_lossy();
                sv.queue_client_command(slot, s.trim_end_matches('\n'));
            }
            qnet::delta::CLCDP_ACKFRAME => {
                let seq = r.long();
                sv.ack_frame(slot, seq);
            }
            CLC_MOVE => {
                let time = r.float();
                let mut angles = [0.0; 3];
                for a in &mut angles {
                    *a = if protocol == PROTOCOL_NETQUAKE { r.angle(flags) } else { r.angle16(flags) };
                }
                let forward = r.short() as f32;
                let side = r.short() as f32;
                let up = r.short() as f32;
                let buttons = r.byte() as u8;
                let impulse = r.byte() as u8;
                if r.badread {
                    return false;
                }
                sv.record_ping(slot, time);
                sv.set_move(slot, ClientMove { angles, forward, side, up, buttons, impulse });
            }
            _ => return false,
        }
    }
    !r.badread
}

/// Resolve `host[:port]`.
pub fn resolve(s: &str) -> Option<SocketAddr> {
    let s = s.trim();
    let with_port = if s.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        s.to_string()
    } else {
        format!("{s}:{DEFAULT_PORT}")
    };
    let with_port = with_port.replace("localhost", "127.0.0.1");
    with_port.to_socket_addrs().ok()?.find(SocketAddr::is_ipv4)
}

pub enum ClientNet {
    /// Waiting for CCREP_ACCEPT; (tries, time of last request).
    Connecting(u32, f64),
    Connected(Connection),
}

/// What [`NetClient::poll`] produced.
pub enum ClientPoll {
    /// Complete messages; datagrams carry their sequence number (acked by
    /// replacement deltas).
    Messages(Vec<(Vec<u8>, Option<u32>)>),
    Failed(String),
}

/// Connection to a remote server.
pub struct NetClient {
    sock: UdpTransport,
    pub server: SocketAddr,
    pub state: ClientNet,
    buf: Vec<u8>,
}

impl NetClient {
    pub fn connect(server: SocketAddr, now: f64) -> std::io::Result<Self> {
        let sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, 0).into())?;
        let mut c = Self { sock, server, state: ClientNet::Connecting(0, now - 10.0), buf: vec![0; NET_MAXMESSAGE + NET_HEADERSIZE] };
        c.poll(now);
        Ok(c)
    }

    pub fn connected(&self) -> bool {
        matches!(self.state, ClientNet::Connected(_))
    }

    /// Send pending packets, retry the connection request, read replies.
    pub fn poll(&mut self, now: f64) -> ClientPoll {
        let mut msgs = Vec::new();
        if let ClientNet::Connecting(tries, last) = &mut self.state
            && now - *last >= 2.5
        {
            if *tries >= 3 {
                return ClientPoll::Failed("No response\n".into());
            }
            *tries += 1;
            *last = now;
            let q = Ccreq::Connect { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION };
            let _ = self.sock.send_to(&q.encode(), self.server);
        }
        loop {
            let (n, from) = match self.sock.recv_from(&mut self.buf) {
                Ok(Some(x)) => x,
                Ok(None) => break,
                Err(e) => return ClientPoll::Failed(format!("{e}\n")),
            };
            if from.ip() != self.server.ip() {
                continue;
            }
            let pkt = &self.buf[..n];
            match &mut self.state {
                ClientNet::Connecting(..) => match Ccrep::decode(pkt) {
                    Some(Ccrep::Accept { port }) => {
                        self.server.set_port(port as u16);
                        let mut c = Connection::new();
                        c.last_receive_time = now;
                        self.state = ClientNet::Connected(c);
                    }
                    Some(Ccrep::Reject { reason }) => return ClientPoll::Failed(reason),
                    _ => {}
                },
                ClientNet::Connected(c) => {
                    if from != self.server {
                        continue;
                    }
                    match c.handle_packet(pkt, now) {
                        Some(Received::Reliable(m)) => msgs.push((m, None)),
                        Some(Received::Unreliable(m)) => msgs.push((m, Some(c.unreliable_sequence_in()))),
                        None => {}
                    }
                }
            }
        }
        if let ClientNet::Connected(c) = &mut self.state {
            if now - c.last_receive_time > MESSAGE_TIMEOUT {
                return ClientPoll::Failed("Server timed out\n".into());
            }
            self.flush(now);
        }
        ClientPoll::Messages(msgs)
    }

    fn flush(&mut self, now: f64) {
        if let ClientNet::Connected(c) = &mut self.state {
            c.poll(now);
            for p in c.take_outgoing() {
                let _ = self.sock.send_to(&p, self.server);
            }
        }
    }

    pub fn send_reliable(&mut self, data: &[u8], now: f64) {
        if let ClientNet::Connected(c) = &mut self.state {
            let _ = c.send_reliable(data, now);
        }
        self.flush(now);
    }

    pub fn send_unreliable(&mut self, data: &[u8], now: f64) {
        if let ClientNet::Connected(c) = &mut self.state {
            c.send_unreliable(data);
        }
        self.flush(now);
    }

    /// clc_disconnect, sent unreliably three times like CL_Disconnect.
    pub fn disconnect(&mut self, now: f64) {
        for _ in 0..3 {
            self.send_unreliable(&[CLC_DISCONNECT], now);
        }
    }
}

/// `slist`: broadcast CCREQ_SERVER_INFO on the LAN (and ask localhost),
/// collecting replies until the deadline.
pub struct LanSearch {
    sock: UdpTransport,
    pub deadline: f64,
    pub found: Vec<(SocketAddr, Ccrep)>,
    buf: Vec<u8>,
}

impl LanSearch {
    pub fn start(port: u16, now: f64) -> std::io::Result<Self> {
        let mut sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, 0).into())?;
        let _ = sock.set_broadcast(true);
        let q = Ccreq::ServerInfo { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION }.encode();
        let mut ports = vec![DEFAULT_PORT];
        if port != DEFAULT_PORT && port != 0 {
            ports.push(port);
        }
        for p in ports {
            let _ = sock.send_to(&q, (Ipv4Addr::BROADCAST, p).into());
            let _ = sock.send_to(&q, (Ipv4Addr::LOCALHOST, p).into());
        }
        Ok(Self { sock, deadline: now + 1.5, found: Vec::new(), buf: vec![0; 2048] })
    }

    /// Read replies; true when new servers arrived.
    pub fn poll(&mut self) -> bool {
        let mut new = false;
        while let Ok(Some((n, from))) = self.sock.recv_from(&mut self.buf) {
            if let Some(r @ Ccrep::ServerInfo { .. }) = Ccrep::decode(&self.buf[..n]) {
                // Broadcast and localhost may both reach a local server.
                let dup = self.found.iter().any(|(a, f)| {
                    *a == from || (a.port() == from.port() && (a.ip().is_loopback() || from.ip().is_loopback()) && *f == r)
                });
                if !dup {
                    self.found.push((from, r));
                    new = true;
                }
            }
        }
        new
    }

    pub fn done(&self, now: f64) -> bool {
        now >= self.deadline
    }
}

#[cfg(test)]
mod tests;
