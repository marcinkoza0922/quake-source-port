//! Non-blocking UDP master-server, LAN and direct server status query runner
//! and packet parsers for [`super::ServerEntry`].

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};

use qnet::control::NET_PROTOCOL_VERSION;
use qnet::qw::{self as qw_proto, info_value};
use qnet::{Ccrep, Ccreq, Transport, UdpTransport};

use super::{SavedServers, ServerEntry, ServerMode, ServerProto};

const PROBES_PER_POLL: usize = 24;
const SEARCH_DURATION: f64 = 2.5;

/// Non-blocking UDP master-server, LAN and direct server status query runner.
pub struct ServerSearch {
    sock: UdpTransport,
    pub found: Vec<ServerEntry>,
    queue: VecDeque<(SocketAddr, ServerProto, bool)>,
    sent_at: HashMap<SocketAddr, (f64, bool)>,
    until: f64,
    buf: Vec<u8>,
}

impl ServerSearch {
    /// Starts a LAN + Favorites/Recent search (`internet == false`) or a full
    /// Master Server + LAN + Favorites/Recent search (`internet == true`).
    pub fn start(
        lan_port: u16,
        internet: bool,
        nq_masters: &str,
        qw_masters: &str,
        saved: &SavedServers,
        now: f64,
    ) -> std::io::Result<Self> {
        let sock = UdpTransport::bind((Ipv4Addr::UNSPECIFIED, 0).into())?;
        let _ = sock.set_broadcast(true);
        let mut s = Self {
            sock,
            found: Vec::new(),
            queue: VecDeque::new(),
            sent_at: HashMap::new(),
            until: now + if internet { SEARCH_DURATION } else { 1.0 },
            buf: vec![0; 4096],
        };
        s.send_lan_broadcast(lan_port, now);
        for sv in saved.favorites.iter().chain(saved.recent.iter()) {
            let default_port = if sv.protocol == ServerProto::QuakeWorld {
                qw_proto::PORT_SERVER
            } else {
                crate::net::DEFAULT_PORT
            };
            if let Some(addr) = resolve_host_port(sv.bare_address(), default_port) {
                s.enqueue(addr, sv.protocol, false);
            }
        }
        if internet {
            s.query_masters(nq_masters, qw_masters);
        }
        s.pump_queue(now);
        Ok(s)
    }

    fn send_lan_broadcast(&mut self, port: u16, now: f64) {
        let nq_q = Ccreq::ServerInfo { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION }.encode();
        for a in [Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST] {
            let addr = SocketAddr::from((a, port));
            self.sent_at.insert(addr, (now, true));
            let _ = self.sock.send_to(&nq_q, addr);
            let qw_addr = SocketAddr::from((a, qw_proto::PORT_SERVER));
            self.sent_at.insert(qw_addr, (now, true));
            let _ = self.sock.send_to(&qw_proto::oob(b"status 23\n"), qw_addr);
        }
    }

    fn query_masters(&mut self, nq_masters: &str, qw_masters: &str) {
        for token in nq_masters.split_whitespace() {
            if let Some(addr) = resolve_host_port(token, 27950) {
                for q in [
                    &b"\xff\xff\xff\xffgetservers Quake 3 empty full\n"[..],
                    &b"\xff\xff\xff\xffgetservers FTE-Quake 28 empty full\n"[..],
                    &b"\xff\xff\xff\xffgetservers DarkPlaces-Quake 3 empty full\n"[..],
                ] {
                    let _ = self.sock.send_to(q, addr);
                }
            }
        }
        for token in qw_masters.split_whitespace() {
            if let Some(addr) = resolve_host_port(token, qw_proto::PORT_MASTER) {
                let _ = self.sock.send_to(&[qw_proto::C2M_MASTER_REQUEST, b'\n', 0], addr);
            }
        }
    }

    pub fn enqueue(&mut self, addr: SocketAddr, proto: ServerProto, lan: bool) {
        if self.sent_at.contains_key(&addr) || self.queue.iter().any(|(a, _, _)| *a == addr) {
            return;
        }
        self.queue.push_back((addr, proto, lan));
    }

    fn pump_queue(&mut self, now: f64) {
        let nq_q = Ccreq::ServerInfo { game: "QUAKE".into(), version: NET_PROTOCOL_VERSION }.encode();
        for _ in 0..PROBES_PER_POLL {
            let Some((addr, proto, lan)) = self.queue.pop_front() else { break };
            self.sent_at.insert(addr, (now, lan));
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
        self.pump_queue(now);
        let mut changed = false;
        while let Ok(Some((n, from))) = self.sock.recv_from(&mut self.buf) {
            let pkt = self.buf[..n].to_vec();
            let (sent_time, is_lan) = self.sent_at.get(&from).copied().unwrap_or((now, from.ip().is_loopback()));
            let ping_ms = Some(((now - sent_time).max(0.0) * 1000.0).round() as u32);

            if let Some((addrs, proto)) = parse_master_reply(&pkt) {
                for a in addrs {
                    self.enqueue(a, proto, false);
                }
                self.pump_queue(now);
                continue;
            }

            if let Some(mut entry) = parse_server_reply(&pkt, from, ping_ms, is_lan) {
                if let Some(existing) = self.found.iter_mut().find(|e| e.address == entry.address) {
                    entry.favorite = existing.favorite;
                    entry.recent = existing.recent;
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
        self.queue.is_empty() && now >= self.until
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
