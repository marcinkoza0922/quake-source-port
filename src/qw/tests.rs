//! The client against the scripted QW server (`tools/qwfake`) over real
//! UDP on e1m1: handshake, lists, a download, prespawn with the map
//! checksum, spawn, then game frames with server-side movement.

use super::*;
use qclient::{ClientEvent, ClientState};
use qnet::protocol as nq;

fn base_fs() -> Option<Fs> {
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
    Some(fs)
}

const DL_NAME: &str = "progs/qsp_qwtest.mdl";

#[test]
fn connects_downloads_and_plays() {
    let Some(mut fs) = base_fs() else { return };
    let map = fs.open("maps/e1m1.bsp").unwrap();
    let sock = UdpTransport::bind((Ipv4Addr::LOCALHOST, 0).into()).unwrap();
    let mut sv = qwfake::FakeServer::new(sock, "maps/e1m1.bsp", &map).unwrap();
    sv.download = Some((DL_NAME.into(), b"IDPO\x06\0\0\0fake".to_vec()));
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, sv.port()));
    let tmp = std::env::temp_dir().join(format!("qsp-qwtest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut qc = QwClient::connect(addr, "\\name\\tester\\topcolor\\3\\bottomcolor\\5".into(), 0.0).unwrap();
    qc.download_root = Some(tmp.clone());
    let mut cl = ClientState::new();
    let mut info = String::new();
    let mut prints = String::new();
    let mut now = 0.0;
    let mut active_at = None;
    let mut said = false;
    while now < 15.0 {
        now += 1.0 / 72.0;
        let msgs = match qc.poll(&mut fs, now) {
            QwPoll::Messages(m, p) => {
                prints.extend(p);
                m
            }
            QwPoll::Failed(e) => panic!("failed: {e}"),
        };
        for m in msgs {
            cl.parse_server_message(&m).unwrap();
        }
        for e in cl.take_events() {
            if let ClientEvent::Print(s) = e {
                prints.push_str(&s);
            }
        }
        let fwd = if cl.signon == 4 { 400 } else { 0 };
        let yaw = sv.spawn_angle;
        qc.send_cmd(UserCmd { forwardmove: fwd, angles: [0.0, yaw, 0.0], ..Default::default() }, now);
        sv.frame(&mut info);
        std::thread::sleep(std::time::Duration::from_micros(300));
        if cl.signon == 4 && active_at.is_none() {
            active_at = Some(now);
        }
        if active_at.is_some_and(|t| now - t > 0.3) && !said {
            qc.stringcmd("say hello");
            said = true;
        }
        if active_at.is_some_and(|t| now - t > 1.0) {
            break;
        }
    }
    let downloaded = std::fs::read(tmp.join("qw").join(DL_NAME));
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(cl.signon, 4, "log {:?} prints {prints}", sv.log);
    let cmds: Vec<&str> = sv.log.iter().map(|s| s.split_whitespace().next().unwrap()).filter(|c| *c != "setinfo").collect();
    assert_eq!(&cmds[..8], ["new", "soundlist", "modellist", "download", "nextdl", "nextdl", "prespawn", "spawn"], "{:?}", sv.log);
    assert!(!sv.log.iter().any(|l| l.starts_with("BAD")), "{:?}", sv.log);
    assert_eq!(downloaded.unwrap(), b"IDPO\x06\0\0\0fake");
    assert!(prints.contains(&format!("Downloaded {DL_NAME}")), "{prints}");
    assert!(prints.contains("welcome to qwfake") && prints.contains("player: hello"), "{prints}");
    assert_eq!(info, "\\name\\tester\\topcolor\\3\\bottomcolor\\5");
    assert_eq!(sv.bad_checksums, 0);
    assert!(sv.moves > 30 && sv.last_cmd.forwardmove == 400);
    assert_eq!(cl.mapname, "e1m1");
    assert_eq!((cl.scores[0].name.as_str(), cl.scores[1].name.as_str()), ("tester", "bot"));
    assert_eq!(cl.cdtrack, 6);
    assert_eq!(cl.stats[nq::STAT_HEALTH], 100);
    assert_eq!(cl.stats[nq::STAT_ARMOR], 50);
    cl.frame(1.0);
    assert_eq!(cl.entities[40].model, 4);
    assert_eq!(cl.entities[2].model, 2, "bot uses player.mdl");
    // The server walked us forward with our own commands.
    let s = sv.spawn;
    let p = sv.player.origin.to_array();
    let moved = ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2)).sqrt();
    assert!(moved > 50.0, "{s:?} -> {p:?}");
    // Prediction replays the unacknowledged commands on top of the last
    // server state, landing at or ahead of it along the run.
    qc.predict(now + 0.05, false);
    assert!(qc.world.is_some() && qc.qw.predicted);
    let sim = qc.qw.simorg;
    let ahead = ((sim[0] - s[0]).powi(2) + (sim[1] - s[1]).powi(2)).sqrt();
    assert!(ahead >= moved - 0.5 && ahead < moved + 40.0, "server {p:?} predicted {sim:?}");
    assert!((sim[2] - p[2]).abs() < 1.0);
    qc.disconnect();
}
