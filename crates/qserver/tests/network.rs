//! Text with Quake's own characters, and clients that don't acknowledge
//! (skip without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::Server;

fn start(slots: usize) -> Option<Server> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    if !d.join("id1").is_dir() {
        eprintln!("skip: no game data");
        return None;
    }
    let mut fs = Fs::new(&d);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), slots);
    sv.core.cvars.set("sv_protocol_pext2", "0");
    sv.spawn_server("e1m1").ok()?;
    Some(sv)
}

fn has(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn names_and_chat_keep_their_bytes() {
    let Some(mut sv) = start(2) else { return };
    let c = sv.connect_local_as("żółw", 0).unwrap();
    sv.frame(1.0 / 72.0);
    let first = sv.take_reliable(c).concat();
    assert!(has(&first, "żółw\0".as_bytes()), "a typed name goes out as UTF-8");
    let netname = |sv: &Server| {
        let vm = &sv.level.as_ref().unwrap().vm;
        vm.ent_str(1, sv.core.sv.fields.netname).unwrap().to_vec()
    };
    assert_eq!(netname(&sv), "żółw".as_bytes());
    // a stock client's gold name, as it comes off the network
    let gold: Vec<u8> = b"color".iter().map(|b| b | 128).collect();
    let cmd = qcore::text::decode(&[b"name \"", gold.as_slice(), b"\""].concat());
    sv.client_command(c, &cmd);
    sv.client_command(c, &qcore::text::decode(&[b"say hi ", gold.as_slice()].concat()));
    sv.frame(1.0 / 72.0);
    assert_eq!(netname(&sv), gold);
    let out = sv.take_reliable(c).concat();
    assert!(has(&out, &[&[13, 0], gold.as_slice(), &[0]].concat()), "svc_updatename");
    assert!(has(&out, &[&[1], gold.as_slice(), b": hi ", gold.as_slice(), b"\n\0"].concat()), "svc_print");
}

/// SV_SendClientMessages waits for NET_CanSendMessage; a client that never
/// acknowledges overflows its own buffer and is dropped.
#[test]
fn reliable_data_has_a_bound() {
    let Some(mut sv) = start(2) else { return };
    sv.connect_client(1);
    let mut frames = 0;
    while sv.core.clients[1].active && frames < 5000 {
        sv.client_command(1, "prespawn");
        sv.client_command(1, "status");
        sv.frame(1.0 / 72.0);
        frames += 1;
        let queued: usize = sv.core.clients[1].reliable_out.iter().map(Vec::len).sum();
        assert!(queued <= 3 * qserver::msg::MAX_MSGLEN, "{queued} bytes after {frames} frames");
    }
    assert!(!sv.core.clients[1].active, "still connected after {frames} frames");
}
