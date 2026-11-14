//! Local signon must deliver every baseline on large Arcane Dimensions maps.
use qcore::{cvar::Cvars, fs::Fs};
use qserver::{ClientMove, Server};
use std::path::{Path, PathBuf};

#[test]
fn sepulcher_spawn_can_walk() {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
        });
    let mut fs = Fs::new(base);
    fs.add_game_dir("id1");
    fs.add_game_dir("ad");
    if !fs.exists("maps/ad_sepulcher.bsp") || !fs.exists("progs.dat") {
        eprintln!("skip: AD game data absent");
        return;
    }
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.command("sv_autosave 0");
    sv.spawn_server("ad_sepulcher").unwrap();
    let c = sv.connect_local().unwrap();
    let mut cl = qclient::ClientState::new();
    cl.nolerp = true;
    for rel in sv.take_reliable(c) {
        cl.parse_server_message(&rel).unwrap();
    }
    let origin = sv.level.as_ref().unwrap().vm.field_ofs("origin").unwrap();
    let start = sv.level.as_ref().unwrap().vm.ent_v(1, origin);
    assert_eq!(
        cl.entities[1].baseline.modelindex,
        cl.model_precache
            .iter()
            .position(|m| m == "progs/player.mdl")
            .unwrap()
    );
    for i in 0..216 {
        sv.set_move(
            c,
            ClientMove {
                angles: [0.0, 270.0, 0.0],
                forward: if i >= 72 { 320.0 } else { 0.0 },
                ..Default::default()
            },
        );
        sv.frame(1.0 / 72.0);
        for rel in sv.take_reliable(c) {
            cl.parse_server_message(&rel).unwrap();
        }
        for d in sv.take_datagrams(c) {
            cl.parse_server_message(&d).unwrap();
        }
        cl.frame(1.0 / 72.0);
        for a in cl.ack_frames.drain(..) {
            sv.ack_frame(c, a);
        }
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
    let vm = &sv.level.as_ref().unwrap().vm;
    let end = vm.ent_v(1, origin);
    assert!(
        (cl.entities[1].origin[1] - end[1]).abs() < 1.0,
        "client {:?} server {end:?}",
        cl.entities[1].origin
    );
    assert!(start[1] - end[1] > 100.0, "{start:?} -> {end:?}");
}
