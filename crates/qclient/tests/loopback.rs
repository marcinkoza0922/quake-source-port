//! End-to-end: qserver byte streams parsed by qclient for protocols 15/666/999.

use std::path::{Path, PathBuf};

use qclient::{ClientEvent, ClientState, InputState, MoveSpeeds};
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    d.join("id1").is_dir().then_some(d)
}

fn feed(sv: &mut Server, cl: &mut ClientState, c: usize) {
    for rel in sv.take_reliable(c) {
        cl.parse_server_message(&rel).unwrap();
    }
    for d in sv.take_datagrams(c) {
        cl.parse_server_message(&d).unwrap();
    }
}

fn run(base: &Path, protocol: u32, pext2: u32) {
    let mut fs = Fs::new(base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.command(&format!("sv_protocol {protocol}"));
    sv.command(&format!("sv_protocol_pext2 {pext2}"));
    sv.spawn_server("start").unwrap();
    let c = sv.connect_local().unwrap();
    let mut cl = ClientState::new();
    cl.nolerp = true;
    feed(&mut sv, &mut cl, c);
    assert_eq!(cl.protocol, protocol);
    assert_eq!(cl.pext2, pext2);
    assert_eq!(cl.mapname, "start");
    assert!(cl.model_precache.len() > 10 && cl.sound_precache.len() > 10);
    let dt = 1.0 / 72.0;
    let mut max_particles = 0;
    for i in 0..200 {
        let inp = InputState { forward: 1.0, left: 0.3, attack: i > 100, ..Default::default() };
        let cmd = cl.base_move(&inp, &MoveSpeeds::default(), dt as f32);
        let _ = cl.build_move(&cmd);
        sv.set_move(c, ClientMove {
            angles: cmd.viewangles,
            forward: cmd.forwardmove,
            side: cmd.sidemove,
            up: cmd.upmove,
            buttons: cmd.buttons,
            impulse: cmd.impulse,
        });
        sv.frame(dt);
        assert!(sv.error().is_none(), "{:?}", sv.error());
        feed(&mut sv, &mut cl, c);
        cl.frame(dt);
        max_particles = max_particles.max(cl.particles.len());
    }
    assert!(max_particles > 0, "p{protocol}: no gunshot particles");
    let ev = cl.take_events();
    assert!(ev.contains(&ClientEvent::Signon(4)), "p{protocol}: signon {}", cl.signon);
    assert_eq!(cl.signon, 4);
    assert_eq!(cl.viewentity, 1);
    let active = cl.active_entities().count();
    assert!(active >= 2 && cl.num_entities() > 50 && cl.num_entities() < 2000, "p{protocol}: {active} active of {}", cl.num_entities());
    assert!(!cl.static_entities.is_empty());
    assert!(cl.stats[qnet::protocol::STAT_HEALTH] > 0);
    assert!(cl.lightstyles[0] == "m");

    let l = sv.level.as_ref().unwrap();
    let o = l.vm.field_ofs("origin").unwrap();
    let so = l.vm.ent_v(1, o);
    let co = cl.entities[1].origin;
    for k in 0..3 {
        assert!((so[k] - co[k]).abs() <= 0.125, "p{protocol}: server {so:?} client {co:?}");
    }
    let start = cl.entities[1].baseline.origin;
    assert!((co[0] - start[0]).abs() + (co[1] - start[1]).abs() > 50.0, "player did not move: {start:?} -> {co:?}");
}

#[test]
fn loopback_protocols() {
    let Some(base) = basedir() else {
        eprintln!("QUAKE_BASEDIR not found; skipping");
        return;
    };
    for p in [15, 666, 999] {
        // plain U_* updates, then FTE replacement deltas
        run(&base, p, 0);
        run(&base, p, qnet::delta::PEXT2_REPLACEMENTDELTAS);
    }
}

/// DP_ENT_TRAILEFFECTNUM: `.traileffectnum` / `.emiteffectnum` (fields of
/// Arcane Dimensions' progs; it sets them on gibs) reach the client through
/// replacement deltas, including later changes.
#[test]
fn ad_entity_effects() {
    let Some(base) = basedir().filter(|b| b.join("ad").is_dir()) else {
        eprintln!("ad not found; skipping");
        return;
    };
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    fs.add_game_dir("ad");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.spawn_server("start").unwrap();
    let c = sv.connect_local().unwrap();
    let mut cl = ClientState::new();
    feed(&mut sv, &mut cl, c);
    assert_eq!(cl.pext2, qnet::delta::PEXT2_REPLACEMENTDELTAS);
    let dt = 1.0 / 72.0;
    let run = |sv: &mut Server, cl: &mut ClientState, n: usize| {
        for _ in 0..n {
            sv.set_move(c, ClientMove::default());
            sv.frame(dt);
            assert!(sv.error().is_none(), "{:?}", sv.error());
            feed(sv, cl, c);
            cl.frame(dt);
        }
    };
    run(&mut sv, &mut cl, 72);
    // A visible non-player entity.
    let e = (2..cl.entities.len()).find(|&i| cl.entities[i].delta_live && cl.entities[i].model != 0).unwrap();
    let ofs = |sv: &Server, f: &str| sv.level.as_ref().unwrap().vm.field_ofs(f).unwrap();
    let (tr, em) = (ofs(&sv, "traileffectnum"), ofs(&sv, "emiteffectnum"));
    let set = |sv: &mut Server, t: f32, m: f32| {
        let vm = &mut sv.level.as_mut().unwrap().vm;
        vm.set_ent_f(e as u32, tr, t);
        vm.set_ent_f(e as u32, em, m);
    };
    set(&mut sv, 5.0, 9.0);
    run(&mut sv, &mut cl, 2);
    assert_eq!((cl.entities[e].traileffect, cl.entities[e].emiteffect), (5, 9));
    set(&mut sv, 6.0, 0.0);
    run(&mut sv, &mut cl, 2);
    assert_eq!((cl.entities[e].traileffect, cl.entities[e].emiteffect), (6, 0));
    set(&mut sv, 0.0, 0.0);
    run(&mut sv, &mut cl, 2);
    assert_eq!((cl.entities[e].traileffect, cl.entities[e].emiteffect), (0, 0));
}
