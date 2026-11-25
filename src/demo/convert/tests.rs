use super::*;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

#[test]
fn serverinfo_loses_the_fte_prefix() {
    let mut info = vec![SVC_SERVERINFO];
    info.extend(PROTOCOL_FTE_PEXT2.to_le_bytes());
    info.extend(8u32.to_le_bytes());
    info.extend(999u32.to_le_bytes());
    let mut ext = false;
    let out = strip_pext(&info, &mut ext);
    assert!(ext);
    assert_eq!(out, [&[SVC_SERVERINFO][..], &999u32.to_le_bytes()].concat());
    let mut ext = false;
    assert_eq!(strip_pext(&out, &mut ext), out);
    assert!(!ext);
}

/// e1m1 played by the local client (FTE deltas, as the host records),
/// written as a demo: walking and firing so entities move and appear.
fn record() -> Option<Vec<u8>> {
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
    sv.spawn_server("e1m1").ok()?;
    let slot = sv.connect_local()?;
    assert_ne!(sv.client_pext2(slot) & PEXT2_REPLACEMENTDELTAS, 0, "local client uses replacement deltas");
    let mut demo = b"-1\n".to_vec();
    let mut write = |data: &[u8]| {
        demo.extend((data.len() as i32).to_le_bytes());
        demo.extend([0u8; 12]);
        demo.extend(data);
    };
    for frame in 0..400 {
        let mv = ClientMove {
            angles: [0.0, (frame as f32 * 0.9) % 360.0, 0.0],
            forward: 200.0,
            buttons: u16::from(frame % 20 < 3),
            impulse: if frame == 5 { 9 } else { 0 },
            ..Default::default()
        };
        sv.set_move(slot, mv);
        sv.frame(1.0 / 72.0);
        for m in sv.take_reliable(slot).into_iter().chain(sv.take_datagrams(slot)) {
            write(&m);
        }
    }
    Some(demo)
}

/// Entities a client shows this frame: (number, origin, model, frame).
fn shown(cl: &ClientState, plain: bool) -> Vec<(usize, [f32; 3], usize, i32)> {
    cl.entities
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, e)| if plain { e.msgtime == cl.mtime[0] } else { e.delta_live })
        .map(|(n, e)| if plain { (n, e.msg_origins[0], e.model, e.frame) } else { (n, e.netstate.origin, e.netstate.modelindex as usize, e.netstate.frame as i32) })
        .collect()
}

/// Same entities, models and frames; origins within the plain protocol's
/// 0.1 unit threshold (smaller offsets from the baseline aren't sent).
fn same(a: &[(usize, [f32; 3], usize, i32)], b: &[(usize, [f32; 3], usize, i32)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| (x.0, x.2, x.3) == (y.0, y.2, y.3) && (0..3).all(|k| (x.1[k] - y.1[k]).abs() <= 0.15))
}

#[test]
fn converted_demo_plays_without_extensions_and_shows_the_same_entities() {
    let Some(demo) = record() else { return };
    let conv = to_plain(&demo).unwrap().expect("a delta demo needs converting");
    assert!(!conv.lost.any(), "id1 loses nothing: {:?}", conv.lost);
    assert_eq!(to_plain(&conv.data).unwrap().map(|c| c.data.len()), None, "converted demos are already plain");

    let (mut a, mut b) = (Demo::new(demo), Demo::new(conv.data));
    let (mut orig, mut plain) = (ClientState::new(), ClientState::new());
    let mut spans = Vec::new();
    let mut frames = 0;
    while let (Some(ma), Some(mb)) = (a.next_message(), b.next_message()) {
        assert!(mb.data.len() <= MAX_BLOCK);
        orig.parse_server_message(ma.data).unwrap();
        spans.clear();
        plain.parse_server_message_spans(mb.data, &mut spans).unwrap();
        for &(cmd, ..) in &spans {
            assert!(
                cmd & 0x80 != 0 || !matches!(cmd, 21 | 51 | 54 | 60..=62 | 66 | 78 | 79 | 86),
                "extension message {cmd} left in the plain demo"
            );
        }
        if spans.iter().any(|s| s.0 & 0x80 != 0) {
            frames += 1;
            let (p, o) = (shown(&plain, true), shown(&orig, false));
            assert!(same(&p, &o), "block {frames}:\n{p:?}\n{o:?}");
            assert_eq!(plain.mtime[0], orig.mtime[0]);
        }
        drain(&mut orig);
        drain(&mut plain);
    }
    assert_eq!(plain.pext2, 0);
    assert_eq!(orig.pext2 & PEXT2_REPLACEMENTDELTAS, PEXT2_REPLACEMENTDELTAS);
    assert!(frames > 300, "{frames} entity frames compared");
    assert!(a.next_message().is_none() && b.next_message().is_none(), "same number of blocks");
}

/// The recorded demo with a DP point-particle message spliced into the last
/// block before the disconnect-free end.
fn with_particles(demo: &[u8]) -> Vec<u8> {
    let mut d = Demo::new(demo.to_vec());
    let mut cl = ClientState::new();
    let mut blocks = Vec::new();
    while let Some(m) = d.next_message() {
        cl.parse_server_message(m.data).unwrap();
        drain(&mut cl);
        blocks.push(m.data.to_vec());
    }
    let mut p = SizeBuf::new(64);
    p.byte(SVCDP_POINTPARTICLES1 as i32);
    p.short(1);
    for v in [480.0, -352.0, 88.0] {
        p.coord(v, cl.protocolflags);
    }
    blocks.last_mut().unwrap().extend(p.data);
    let mut out = b"-1\n".to_vec();
    for b in blocks {
        out.extend((b.len() as i32).to_le_bytes());
        out.extend([0u8; 12]);
        out.extend(b);
    }
    out
}

#[test]
fn lossy_demos_keep_the_original_as_qsd() {
    let Some(demo) = record() else { return };
    let demo = with_particles(&demo);
    let conv = to_plain(&demo).unwrap().unwrap();
    assert_eq!(conv.lost, Lost { particle_msgs: 1, entity_effects: false });

    let dir = std::env::temp_dir().join(format!("qsp-demo-convert-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("lossy.dem");
    std::fs::write(&path, &demo).unwrap();
    let msg = convert_file(&path);
    assert!(msg.contains("lossy.qsd keeps them"), "{msg}");
    assert_eq!(std::fs::read(dir.join("lossy.qsd")).unwrap(), demo, "the original is kept untouched");
    assert_eq!(std::fs::read(&path).unwrap(), conv.data);
    assert!(!dir.join("lossy.dem.tmp").exists());
    // a lossless demo is converted in place, alone
    let (Some(plain), path2) = (record(), dir.join("clean.dem")) else { return };
    std::fs::write(&path2, &plain).unwrap();
    assert!(convert_file(&path2).contains("plays in other source ports too"));
    assert!(!dir.join("clean.qsd").exists());
    assert_eq!(convert_file(&path2), "", "already plain: nothing to do");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn entities_that_overflow_a_message_are_an_error() {
    let mut cl = ClientState::new();
    cl.protocol = 999;
    cl.entities.resize_with(12000, Default::default);
    for e in &mut cl.entities {
        e.delta_live = true;
        e.netstate.origin = [1.0, 2.0, 3.0];
        e.netstate.modelindex = 300;
    }
    let mut lost = Lost::default();
    assert!(entity_updates(&cl, &mut lost).is_err());
    cl.entities.truncate(100);
    assert!(entity_updates(&cl, &mut lost).is_ok_and(|b| b.len() > 100));
}
