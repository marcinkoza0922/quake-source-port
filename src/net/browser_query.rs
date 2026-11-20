//! Non-blocking UDP master-server, LAN and direct server status query runner
//! and packet parsers for [`super::ServerEntry`].

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver};

use qnet::control::NET_PROTOCOL_VERSION;
use qnet::qw::{self as qw_proto, info_value};
use qnet::{Ccrep, Ccreq, Transport, UdpTransport};

use super::{SavedServers, ServerEntry, ServerMode, ServerProto, ServerTab};

const PROBES_PER_POLL: usize = 24;
const SEARCH_DURATION: f64 = 2.5;

/// Non-blocking UDP master-server, LAN and direct server status query runner.
pub struct ServerSearch {
    sock: UdpTransport,
    pub found: Vec<ServerEntry>,
    queue: VecDeque<(SocketAddr, ServerProto)>,
    sent_at: HashMap<SocketAddr, f64>,
    until: f64,
    broadcast_at: Option<f64>,
    resolving: Option<Receiver<InternetTargets>>,
    buf: Vec<u8>,
}

impl ServerSearch {
    /// LAN broadcasts and public master queries run on separate sockets.
    pub fn start(
        lan_port: u16,
        tab: ServerTab,
        nq_masters: &str,
        qw_masters: &str,
        saved: &SavedServers,
        now: f64,
    ) -> std::io::Result<Self> {
        let sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, 0).into())?;
        let mut s = Self {
            sock,
            found: Vec::new(),
            queue: VecDeque::new(),
            sent_at: HashMap::new(),
            until: now + 1.0,
            broadcast_at: None,
            resolving: None,
            buf: vec![0; 4096],
        };
        match tab {
            ServerTab::Lan => {
                s.sock.set_broadcast(true)?;
                s.send_lan_broadcast(lan_port);
                s.broadcast_at = Some(now);
            }
            ServerTab::Internet => {
                let (send, recv) = mpsc::channel();
                let (nq, qw, saved) = (nq_masters.to_string(), qw_masters.to_string(), saved.clone());
                std::thread::Builder::new().name("server-discovery-dns".into()).spawn(move || {
                    let _ = send.send(InternetTargets::resolve(&nq, &qw, &saved));
                })?;
                s.resolving = Some(recv);
            }
        }
        Ok(s)
    }

    fn send_lan_broadcast(&mut self, port: u16) {
        let nq_q = Ccreq::ServerInfo { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION }.encode();
        for a in [Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST] {
            let addr = SocketAddr::from((a, port));
            let _ = self.sock.send_to(&nq_q, addr);
            let qw_addr = SocketAddr::from((a, qw_proto::PORT_SERVER));
            let _ = self.sock.send_to(&qw_proto::oob(b"status 23\n"), qw_addr);
        }
    }

    fn poll_dns(&mut self, now: f64) {
        let Some(resolving) = &self.resolving else { return };
        let targets = match resolving.try_recv() {
            Ok(targets) => targets,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.resolving = None;
                return;
            }
        };
        self.resolving = None;
        self.until = now + SEARCH_DURATION;
        for addr in targets.nq_masters {
            for q in [
                &b"\xff\xff\xff\xffgetservers Quake 3 empty full\n"[..],
                &b"\xff\xff\xff\xffgetservers FTE-Quake 28 empty full\n"[..],
                &b"\xff\xff\xff\xffgetservers DarkPlaces-Quake 3 empty full\n"[..],
            ] {
                let _ = self.sock.send_to(q, addr);
            }
        }
        for addr in targets.qw_masters {
            let _ = self.sock.send_to(&[qw_proto::C2M_MASTER_REQUEST, b'\n', 0], addr);
        }
        for (addr, proto) in targets.saved {
            self.enqueue(addr, proto);
        }
    }

    fn enqueue(&mut self, addr: SocketAddr, proto: ServerProto) {
        if self.sent_at.contains_key(&addr) || self.queue.iter().any(|(a, _)| *a == addr) {
            return;
        }
        self.queue.push_back((addr, proto));
    }

    fn pump_queue(&mut self, now: f64) {
        let nq_q = Ccreq::ServerInfo { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION }.encode();
        for _ in 0..PROBES_PER_POLL {
            let Some((addr, proto)) = self.queue.pop_front() else { break };
            self.sent_at.insert(addr, now);
            match proto {
                ServerProto::QuakeWorld => {
                    let _ = self.sock.send_to(&qw_proto::oob(b"status 23\n"), addr);
                }
                ServerProto::FteDp => {
                    let _ = self.sock.send_to(b"\xff\xff\xff\xffgetinfo\n", addr);
                    let _ = self.sock.send_to(&nq_q, addr);
                }
                ServerProto::NetQuake => {
                    let _ = self.sock.send_to(&nq_q, addr);
                    let _ = self.sock.send_to(b"\xff\xff\xff\xffgetinfo\n", addr);
                }
            }
            if now + 1.0 > self.until {
                self.until = now + 1.0;
            }
        }
    }

    /// Reads any pending master or server replies and sends the next batch of probes.
    /// Returns `true` when at least one server entry was added or updated.
    pub fn poll(&mut self, now: f64) -> bool {
        self.poll_dns(now);
        self.pump_queue(now);
        let mut changed = false;
        while let Ok(Some((n, from))) = self.sock.recv_from(&mut self.buf) {
            let pkt = &self.buf[..n];
            let is_lan = self.broadcast_at.is_some();
            if !is_lan && let Some((addrs, proto)) = parse_master_reply(pkt) {
                for addr in addrs {
                    self.enqueue(addr, proto);
                }
                continue;
            }
            let Some(sent_time) = self.broadcast_at.or_else(|| self.sent_at.get(&from).copied()) else { continue };
            let ping_ms = Some(((now - sent_time).max(0.0) * 1000.0).round() as u32);
            if let Some(entry) = parse_server_reply(pkt, from, ping_ms, is_lan) {
                if let Some(existing) = self.found.iter_mut().find(|e| e.address == entry.address) {
                    // Both query formats can answer. A native NetQuake reply
                    // identifies the connection protocol more precisely.
                    if existing.protocol == ServerProto::NetQuake && entry.protocol == ServerProto::FteDp {
                        continue;
                    }
                    *existing = entry;
                } else {
                    self.found.push(entry);
                }
                changed = true;
            }
        }
        changed
    }

    pub fn done(&self, now: f64) -> bool {
        self.resolving.is_none() && self.queue.is_empty() && now >= self.until
    }
}

struct InternetTargets {
    nq_masters: Vec<SocketAddr>,
    qw_masters: Vec<SocketAddr>,
    saved: Vec<(SocketAddr, ServerProto)>,
}

impl InternetTargets {
    fn resolve(nq: &str, qw: &str, saved: &SavedServers) -> Self {
        let masters = |list: &str, port| list.split_whitespace().filter_map(|host| resolve_host_port(host, port)).collect();
        Self {
            nq_masters: masters(nq, 27950),
            qw_masters: masters(qw, qw_proto::PORT_MASTER),
            saved: saved.favorites.iter().chain(&saved.recent).filter_map(|entry| {
                let port = if entry.protocol == ServerProto::QuakeWorld { qw_proto::PORT_SERVER } else { crate::net::DEFAULT_PORT };
                resolve_host_port(entry.bare_address(), port).map(|addr| (addr, entry.protocol))
            }).collect(),
        }
    }
}

fn resolve_host_port(s: &str, default_port: u16) -> Option<SocketAddr> {
    let s = s.trim().strip_prefix("qw://").unwrap_or(s.trim()).trim_end_matches('/');
    if s.is_empty() {
        return None;
    }
    let with_port = if s.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        s.to_string()
    } else {
        format!("{s}:{default_port}")
    };
    let with_port = with_port.replace("localhost", "127.0.0.1");
    with_port.to_socket_addrs().ok()?.find(SocketAddr::is_ipv4)
}

/// Parses either a QuakeWorld master reply (`\xff\xff\xff\xffd\n`) or a
/// DarkPlaces / FTE master reply (`\xff\xff\xff\xffgetserversResponse`).
pub fn parse_master_reply(pkt: &[u8]) -> Option<(Vec<SocketAddr>, ServerProto)> {
    let body = pkt.strip_prefix(b"\xff\xff\xff\xff")?;
    if let Some(mut rest) = body.strip_prefix(b"d\n").or_else(|| body.strip_prefix(b"d")) {
        let mut out = Vec::new();
        while rest.len() >= 6 {
            let ip = Ipv4Addr::new(rest[0], rest[1], rest[2], rest[3]);
            let port = u16::from_be_bytes([rest[4], rest[5]]);
            rest = &rest[6..];
            if port == 0 || ip.is_unspecified() {
                continue;
            }
            out.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
        }
        return Some((out, ServerProto::QuakeWorld));
    }
    if let Some(mut rest) = body.strip_prefix(b"getserversResponse") {
        let mut out = Vec::new();
        while !rest.is_empty() {
            if rest.starts_with(b"\\EOT") || rest.starts_with(b"EOT") {
                break;
            }
            if rest[0] == b'\\' && rest.len() >= 7 {
                let ip = Ipv4Addr::new(rest[1], rest[2], rest[3], rest[4]);
                let port = u16::from_be_bytes([rest[5], rest[6]]);
                rest = &rest[7..];
                if port != 0 && !ip.is_unspecified() {
                    out.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
                }
            } else {
                rest = &rest[1..];
            }
        }
        return Some((out, ServerProto::FteDp));
    }
    None
}

/// Parses a server info/status response from NetQuake (`CCREP_SERVER_INFO`),
/// QuakeWorld (`\xff\xff\xff\xffn\...`), or FTE/DarkPlaces (`infoResponse` / `statusResponse`).
pub fn parse_server_reply(pkt: &[u8], from: SocketAddr, ping_ms: Option<u32>, lan: bool) -> Option<ServerEntry> {
    if crate::net::is_ctl(pkt)
        && let Some(Ccrep::ServerInfo { hostname, level, players, max_players, .. }) = Ccrep::decode(pkt)
    {
        return Some(ServerEntry {
            address: from.to_string(),
            hostname,
            map: level,
            players,
            max_players,
            ping_ms,
            gamedir: "id1".into(),
            mode: ServerMode::Deathmatch,
            protocol: ServerProto::NetQuake,
            favorite: false,
            recent: false,
            lan,
        });
    }

    let body = pkt.strip_prefix(b"\xff\xff\xff\xff")?;
    if let Some(rest) = body
        .strip_prefix(b"infoResponse\x0a")
        .or_else(|| body.strip_prefix(b"statusResponse\x0a"))
    {
        let text = String::from_utf8_lossy(rest);
        let info = text.lines().next().unwrap_or("");
        return parse_infostring_server(info, None, from, ping_ms, lan, ServerProto::FteDp);
    }
    if let Some(&b'n') = body.first() {
        let text = String::from_utf8_lossy(&body[1..]);
        let mut lines = text.lines();
        let info = lines.next().unwrap_or("");
        if !info.starts_with('\\') {
            return None;
        }
        let player_lines = lines.filter(|l| !l.trim().is_empty() && !l.starts_with('\0')).count() as u8;
        return parse_infostring_server(info, Some(player_lines), from, ping_ms, lan, ServerProto::QuakeWorld);
    }
    None
}

fn parse_infostring_server(
    info: &str,
    counted_players: Option<u8>,
    from: SocketAddr,
    ping_ms: Option<u32>,
    lan: bool,
    default_proto: ServerProto,
) -> Option<ServerEntry> {
    let get = |k: &str| info_value(info, k);
    let hostname = {
        let h = get("hostname");
        let h = if h.is_empty() { get("sv_hostname") } else { h };
        if h.is_empty() { from.to_string() } else { h.to_string() }
    };
    let map = {
        let m = get("map");
        if m.is_empty() { get("mapname").to_string() } else { m.to_string() }
    };
    let gamedir = {
        let g = get("*gamedir");
        let g = if g.is_empty() { get("gamedir") } else { g };
        let g = if g.is_empty() { get("modname") } else { g };
        if g.is_empty() {
            if default_proto == ServerProto::QuakeWorld { "qw".into() } else { "id1".into() }
        } else {
            g.to_string()
        }
    };
    let max_players = get("maxclients")
        .parse::<u8>()
        .or_else(|_| get("sv_maxclients").parse::<u8>())
        .unwrap_or(8);
    let players = get("clients").parse::<u8>().ok().or(counted_players).unwrap_or(0);
    let teamplay = get("teamplay").parse::<i32>().unwrap_or(0);
    let coop = get("coop").parse::<i32>().unwrap_or(0);
    let mode = if coop != 0 {
        ServerMode::Coop
    } else if teamplay != 0 {
        ServerMode::Teamplay
    } else {
        ServerMode::Deathmatch
    };
    let protocol = if get("protocol") == "28" || default_proto == ServerProto::QuakeWorld {
        ServerProto::QuakeWorld
    } else {
        default_proto
    };
    Some(ServerEntry {
        address: from.to_string(),
        hostname,
        map,
        players,
        max_players,
        ping_ms,
        gamedir,
        mode,
        protocol,
        favorite: false,
        recent: false,
        lan,
    })
}
