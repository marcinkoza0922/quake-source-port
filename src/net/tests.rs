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
    /// Extensions answered to `cmd pext`.
    pext2: u32,
}

impl Peer {
    fn new(name: &str, port: u16, now: f64, pext2: u32) -> Self {
        let mut cl = ClientState::new();
        cl.name = name.into();
        let a = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        Self { net: NetClient::connect(a, now).unwrap(), cl, prints: String::new(), pext2 }
    }
    fn step(&mut self, now: f64) {
        match self.net.poll(now) {
            ClientPoll::Messages(m) => {
                for (m, seq) in m {
                    self.cl.net_sequence = seq;
                    self.cl.parse_server_message(&m).unwrap();
                }
            }
            ClientPoll::Failed(e) => panic!("connect failed: {e}"),
        }
        for e in self.cl.take_events() {
            match e {
                ClientEvent::Print(s) => self.prints.push_str(&s),
                // A vanilla client forwards a bare `pext`.
                ClientEvent::StuffText(s) if s.trim() == "cmd pext" => self.cl.send_string_cmd(&match self.pext2 {
                    0 => "pext".to_string(),
                    p => format!("pext {:#x} {p:#x}", qnet::delta::PROTOCOL_FTE_PEXT2),
                }),
                _ => {}
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
    // alice negotiates replacement deltas, bob is vanilla.
    let mut a = Peer::new("alice", ns.port(), 0.0, qnet::delta::PEXT2_REPLACEMENTDELTAS);
    let mut b = Peer::new("bob", ns.port(), 0.0, 0);
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
    assert_eq!((a.cl.pext2, b.cl.pext2), (qnet::delta::PEXT2_REPLACEMENTDELTAS, 0));
    assert_eq!((sv.client_pext2(0), sv.client_pext2(1)), (qnet::delta::PEXT2_REPLACEMENTDELTAS, 0));
    // alice's entities arrive as deltas and track the server.
    assert!(a.cl.entities.iter().filter(|e| e.delta_live).count() > 3);
    let (so, co) = (sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin), a.cl.entities[1].netstate.origin);
    assert!((0..3).all(|k| (so[k] - co[k]).abs() < 16.0), "{so:?} vs {co:?}");
    // clc_move timestamps give round trips (sv.time - cl.mtime[0]).
    let b = &sv.core.clients[1];
    assert!(b.num_pings > 10 && (0.0..0.5).contains(&b.ping()), "{} {}", b.num_pings, b.ping());
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

#[test]
fn lan_search_finds_server() {
    let Some(mut sv) = server(4, "999") else { return };
    let mut ns = NetServer::listen(0).unwrap();
    let mut ls = LanSearch::start(ns.port(), 0.0).unwrap();
    for _ in 0..100 {
        ns.receive(&mut sv, 0.0, "lanhost");
        ls.poll();
        if !ls.found.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let Some((_, Ccrep::ServerInfo { hostname, level, max_players, .. })) = ls.found.first() else { panic!("no reply") };
    assert_eq!((hostname.as_str(), level.as_str(), *max_players), ("lanhost", "e1m1", 4));
}
