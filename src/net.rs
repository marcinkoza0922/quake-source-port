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
    }

    fn reply(&mut self, to: SocketAddr, p: Ccrep) {
        let _ = self.sock.send_to(&p.encode(), to);
    }

    fn control(&mut self, sv: &mut Server, from: SocketAddr, q: Ccreq, now: f64, hostname: &str) {
        let port = self.port() as i32;
        match q {
            Ccreq::Connect { game, version } => {
                if game != "QUAKE" {
                    return;
                }
                if version != NET_PROTOCOL_VERSION {
                    return self.reply(from, Ccrep::Reject { reason: "Incompatible version.\n".into() });
                }
                if !sv.active() {
                    return self.reply(from, Ccrep::Reject { reason: "Server is not running.\n".into() });
                }
                // A retransmitted request from a connected client: accept again
                // if nothing has been exchanged yet.
                if let Some(i) = self.remotes.iter().position(|r| r.addr == from) {
                    if now - self.remotes[i].connect_time < 2.0 && self.remotes[i].closing.is_none() {
                        return self.reply(from, Ccrep::Accept { port });
                    }
                    let slot = self.remotes[i].slot;
                    if self.remotes[i].closing.is_none() {
                        sv.drop_client(slot, true);
                    }
                    self.remotes.remove(i);
                }
                let Some(slot) = sv.core.clients.iter().position(|c| !c.active) else {
                    return self.reply(from, Ccrep::Reject { reason: "Server is full.\n".into() });
                };
                sv.connect_client(slot);
                if !sv.core.clients[slot].active {
                    return self.reply(from, Ccrep::Reject { reason: "Server is not running.\n".into() });
                }
                sv.core.clients[slot].max_datagram = qserver::msg::DATAGRAM_MTU;
                sv.core.print(&format!("Client {from} connected\n"));
                self.remotes.push(Remote { addr: from, slot, conn: Connection::new(), connect_time: now, last_send: now, closing: None });
                self.reply(from, Ccrep::Accept { port });
            }
            Ccreq::ServerInfo { game, version } => {
                if game != "QUAKE" || version != NET_PROTOCOL_VERSION {
                    return;
                }
                let c = &sv.core;
                self.reply(from, Ccrep::ServerInfo {
                    address: format!("{}", self.sock.local_addr()),
                    hostname: hostname.into(),
                    level: c.sv.name.clone(),
                    players: c.clients.iter().filter(|c| c.active).count() as u8,
                    max_players: c.maxclients() as u8,
                    version: NET_PROTOCOL_VERSION,
                });
            }
            Ccreq::PlayerInfo { player } => {
                let active: Vec<usize> = (0..sv.core.maxclients()).filter(|&i| sv.core.clients[i].active).collect();
                let Some(&slot) = active.get(player as usize) else { return };
                let cl = &sv.core.clients[slot];
                let frags = sv.level.as_ref().map_or(0, |l| l.vm.ent_f(slot as u32 + 1, sv.core.sv.fields.frags) as i32);
                let rem = self.remotes.iter().find(|r| r.slot == slot);
                let p = Ccrep::PlayerInfo {
                    player,
                    name: cl.name.clone(),
                    colors: cl.colors,
                    frags,
                    connect_time: rem.map_or(0, |r| (now - r.connect_time) as i32),
                    address: rem.map_or("local".into(), |r| r.addr.to_string()),
                };
                self.reply(from, p);
            }
            Ccreq::RuleInfo { prev } => {
                let names: Vec<String> = sv.core.cvars.iter().filter(|c| c.flags & qcore::cvar::CVAR_SERVERINFO != 0).map(|c| c.name.clone()).collect();
                let next = if prev.is_empty() {
                    names.first()
                } else {
                    names.iter().position(|n| n.eq_ignore_ascii_case(&prev)).and_then(|i| names.get(i + 1))
                };
                let p = match next {
                    Some(n) => Ccrep::RuleInfo { rule: n.clone(), value: sv.core.cvars.by_name(n).map_or(String::new(), |c| c.string.clone()) },
                    None => Ccrep::RuleInfo { rule: String::new(), value: String::new() },
                };
                self.reply(from, p);
            }
        }
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
                if let Some(d) = dg.last() {
                    r.conn.send_unreliable(d);
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
            CLC_MOVE => {
                let _time = r.float();
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
    Messages(Vec<Vec<u8>>),
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
                    if let Some(Received::Reliable(m) | Received::Unreliable(m)) = c.handle_packet(pkt, now) {
                        msgs.push(m);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_defaults_port() {
        assert_eq!(resolve("127.0.0.1"), Some("127.0.0.1:26000".parse().unwrap()));
        assert_eq!(resolve("localhost:27001"), Some("127.0.0.1:27001".parse().unwrap()));
    }

    use qclient::{ClientEvent, ClientState};
    use qcore::cvar::Cvars;
    use qcore::fs::Fs;

    fn server(maxclients: usize, protocol: &str) -> Option<Server> {
        let mut base = Fs::default_basedir();
        if base.is_relative() && !base.exists() {
            base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(base);
        }
        if !base.join("id1").is_dir() {
            eprintln!("skip: no game data");
            return None;
        }
        let mut fs = Fs::new(&base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.command("maxplayers 4");
        sv.command(&format!("sv_protocol {protocol}"));
        sv.command("map e1m1");
        assert!(sv.active() && sv.core.maxclients() == maxclients);
        Some(sv)
    }

    struct Peer {
        net: NetClient,
        cl: ClientState,
        prints: String,
    }

    impl Peer {
        fn new(name: &str, port: u16, now: f64) -> Self {
            let mut cl = ClientState::new();
            cl.name = name.into();
            let a = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
            Self { net: NetClient::connect(a, now).unwrap(), cl, prints: String::new() }
        }
        fn step(&mut self, now: f64) {
            match self.net.poll(now) {
                ClientPoll::Messages(m) => {
                    for m in m {
                        self.cl.parse_server_message(&m).unwrap();
                    }
                }
                ClientPoll::Failed(e) => panic!("connect failed: {e}"),
            }
            for e in self.cl.take_events() {
                if let ClientEvent::Print(s) = e {
                    self.prints.push_str(&s);
                }
            }
            let out = self.cl.take_reliable_out();
            if !out.is_empty() {
                self.net.send_reliable(&out, now);
            }
            let cmd = qclient::UserCmd { forwardmove: 200.0, ..Default::default() };
            if let Some(m) = self.cl.build_move(&cmd) {
                self.net.send_unreliable(&m, now);
            }
        }
    }

    /// Two clients over real UDP sockets: signon, movement, chat, disconnect.
    fn play(protocol: &str) {
        let Some(mut sv) = server(4, protocol) else { return };
        let mut ns = NetServer::listen(0).unwrap();
        let mut a = Peer::new("alice", ns.port(), 0.0);
        let mut b = Peer::new("bob", ns.port(), 0.0);
        let mut now = 0.0;
        let mut said = false;
        let mut start = None;
        while now < 20.0 {
            now += 1.0 / 72.0;
            a.step(now);
            b.step(now);
            ns.receive(&mut sv, now, "test");
            sv.frame(1.0 / 72.0);
            ns.send(&mut sv, now);
            std::thread::sleep(std::time::Duration::from_micros(200));
            if start.is_none() && sv.core.clients[0].spawned {
                start = Some(sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin));
            }
            if a.cl.signon == 4 && b.cl.signon == 4 && !said && now > 3.0 {
                a.cl.send_string_cmd("say hello bob");
                said = true;
            }
            if said && b.prints.contains("alice: hello bob") {
                break;
            }
        }
        assert_eq!((a.cl.signon, b.cl.signon), (4, 4), "signon");
        assert!(b.prints.contains("alice: hello bob"), "chat: {:?}", b.prints);
        let names: Vec<_> = sv.core.clients.iter().filter(|c| c.spawned).map(|c| c.name.clone()).collect();
        assert_eq!(names, ["alice", "bob"]);
        // forwardmove moved alice.
        let start = start.unwrap();
        let now_at = sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin);
        assert!((0..2).any(|k| (now_at[k] - start[k]).abs() > 8.0), "{start:?} -> {now_at:?}");
        // clc_disconnect frees the slot.
        a.net.disconnect(now);
        for _ in 0..20 {
            now += 1.0 / 72.0;
            ns.receive(&mut sv, now, "test");
            sv.frame(1.0 / 72.0);
            ns.send(&mut sv, now);
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        assert!(!sv.core.clients[0].active && sv.core.clients[1].active);
    }

    #[test]
    fn udp_play_protocol_15() {
        play("15");
    }
    #[test]
    fn udp_play_protocol_666() {
        play("666");
    }
    #[test]
    fn udp_play_protocol_999() {
        play("999");
    }
}
