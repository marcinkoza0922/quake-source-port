//! Demo seeking against straight playback, on a demo recorded here (needs
//! the game data; skipped without it).
use super::super::*;

fn host() -> Option<Host> {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || fs.open("maps/e1m1.bsp").is_none() {
        return None;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok())?;
    let r = Renderer::headless(640, 360, &pal).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    Some(h)
}

fn run(h: &mut Host, secs: f64) {
    for _ in 0..(secs * 72.0) as usize {
        h.frame(1.0 / 72.0, Vec::new());
    }
}

/// What a seek must reproduce exactly.
fn state(h: &Host) -> (u64, Vec<([u32; 3], usize, i32)>, Vec<i32>) {
    let ents = h.cl.entities.iter().map(|e| (e.msg_origins[0].map(f32::to_bits), e.model, e.frame)).collect();
    (h.cl.mtime[0].to_bits(), ents, h.cl.stats.to_vec())
}

#[test]
fn seeking_back_matches_straight_playback() {
    let Some(mut h) = host() else { return };
    // ragdolls on: seeking back must not trip over their entity numbers
    h.exec_line("r_debris 1");
    let dir = std::env::temp_dir().join(format!("qsp-demoseek-{}", std::process::id()));
    let path = dir.join("seek.dem");
    h.pending_record = Some((path.clone(), -1));
    h.exec_line("map e1m1");
    run(&mut h, 0.5);
    // walk around and shoot so entities change
    h.exec_line("+forward");
    run(&mut h, 2.0);
    h.exec_line("-forward; +right; +attack");
    run(&mut h, 2.0);
    h.exec_line("-right; -attack; +back");
    run(&mut h, 3.0);
    let mut w = h.recording.take().expect("recording");
    w.write(h.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).unwrap();
    w.finish().unwrap();
    h.disconnect_client();

    // reference: a fresh playback seeking forward from the start
    h.exec_line(&format!("playdemo {}", path.display()));
    run(&mut h, 0.5);
    let len = h.demo.as_ref().and_then(|d| d.play.duration()).expect("indexed");
    assert!(len > 6.0, "{len}");
    h.demo_seek(4.0);
    let want = state(&h);
    assert!((h.demo_clock().unwrap() - 4.0).abs() < 0.05);

    // play on past it, then come back through a snapshot
    run(&mut h, 2.5);
    assert!(h.demo_clock().unwrap() > 6.0);
    h.demo_seek(4.0);
    assert_eq!(state(&h), want);

    // holding rewind runs the clock backwards to the start and stops there
    let before = h.demo_clock().unwrap();
    h.demo.as_mut().unwrap().play.scrub = -1;
    run(&mut h, 0.2);
    let after = h.demo_clock().unwrap();
    assert!(after < before - 0.5, "{before} -> {after}");
    run(&mut h, 1.5);
    let start = h.demo_clock().unwrap();
    assert!(start < 0.1, "{start}");
    h.demo.as_mut().unwrap().play.scrub = 0;

    // the mouse: a drag across the timeline marks that segment, a click
    // seeks, the play/pause button toggles
    let size = (h.r.size().0 as f32, h.r.size().1 as f32);
    let l = super::super::demo_bar::layout(size.0, size.1, false);
    let ty = l.hit.1 + l.hit.3 / 2.0;
    let at = |f: f32| l.track.0 + l.track.2 * f;
    h.demo_pointer_move((at(0.25), ty), size);
    h.demo_pointer_button(1, true);
    h.demo_pointer_move((at(0.5), ty), size);
    h.demo_pointer_button(1, false);
    let [a, b] = h.demo.as_ref().unwrap().play.marks;
    assert!((a.unwrap() - len * 0.25).abs() < 0.1 && (b.unwrap() - len * 0.5).abs() < 0.1, "{a:?} {b:?}");
    h.demo_pointer_move((at(0.75), ty), size);
    h.demo_pointer_button(1, true);
    h.demo_pointer_button(1, false);
    assert!((h.demo_clock().unwrap() - len * 0.75).abs() < 0.1);
    let (_, r) = *l.buttons.iter().find(|(b, _)| *b == super::super::demo_bar::Btn::PlayPause).unwrap();
    let paused = h.demo.as_ref().unwrap().paused;
    h.demo_pointer_move((r.0 + 2.0, r.1 + 2.0), size);
    h.demo_pointer_button(1, true);
    h.demo_pointer_button(1, false);
    assert_ne!(h.demo.as_ref().unwrap().paused, paused);

    // Hide hides at once, mouse movement doesn't bring it back; the strip at
    // the bottom edge does when clicked
    let click = |h: &mut Host, p: (f32, f32)| {
        h.demo_pointer_move(p, size);
        h.demo_pointer_button(1, true);
        h.demo_pointer_button(1, false);
    };
    let (_, r) = *l.buttons.iter().find(|(b, _)| *b == super::super::demo_bar::Btn::Hide).unwrap();
    click(&mut h, (r.0 + 2.0, r.1 + 2.0));
    // a session choice: the Playback Bar setting is left alone
    assert_eq!(h.cv("cl_demobar"), 2.0);
    assert!(!h.demo_bar_visible());
    h.demo_pointer_move((size.0 / 2.0, size.1 / 2.0), size);
    assert!(!h.demo_bar_visible() && h.demo_peek().is_none());
    h.demo_pointer_move((size.0 / 2.0, size.1 - 4.0), size);
    assert!(h.demo_peek().is_some());
    // QSP_DEMO_SHOTS=<dir>: a picture of the strip
    if let Some(dir) = std::env::var_os("QSP_DEMO_SHOTS") {
        h.exec_line(&format!("screenshot {}", std::path::Path::new(&dir).join("peek").display()));
        run(&mut h, 0.1);
    }
    click(&mut h, (size.0 / 2.0, size.1 - 4.0));
    assert!(h.demo_bar_visible());

    // playing to the end pauses on the last frame (cl_demo_endpause)
    h.demo_seek(len - 0.2);
    run(&mut h, 1.0);
    assert!(h.demo.as_ref().is_some_and(|d| d.paused));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelled_export_leaves_no_files() {
    let Some(mut h) = host() else { return };
    let dir = std::env::temp_dir().join(format!("qsp-exportcancel-{}", std::process::id()));
    let path = dir.join("c.dem");
    h.pending_record = Some((path.clone(), -1));
    h.exec_line("map e1m1");
    h.exec_line("+forward");
    run(&mut h, 3.0);
    let mut w = h.recording.take().expect("recording");
    w.write(h.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).unwrap();
    w.finish().unwrap();
    h.disconnect_client();
    let out = dir.join("videos");
    h.exec_line(&format!("playdemo {}", path.display()));
    run(&mut h, 0.3);
    h.exec_line("menu_export");
    h.exec_line(&format!("_export_dest \"{}\"", out.display()));
    // the name field shows the demo's name; the extension isn't part of it
    assert_eq!(h.cvs("_export_name"), "c");
    h.exec_line("_export_name x");
    h.exec_line("_export_go whole");
    run(&mut h, 0.5);
    assert!(h.exporting());
    assert!(std::fs::read_dir(&out).is_ok_and(|d| d.count() > 0), "the export wrote nothing");
    h.cancel_export();
    assert!(!h.exporting());
    let left: Vec<_> = std::fs::read_dir(&out).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(left.is_empty(), "{left:?}");
    // back on the demo, paused
    assert!(h.demo.as_ref().is_some_and(|d| d.paused));
}

#[test]
fn hidden_bar_stays_hidden_until_shown() {
    let Some(mut h) = host() else { return };
    let dir = std::env::temp_dir().join(format!("qsp-demobar-{}", std::process::id()));
    let path = dir.join("bar.dem");
    h.pending_record = Some((path.clone(), -1));
    h.exec_line("map e1m1");
    run(&mut h, 2.0);
    let mut w = h.recording.take().expect("recording");
    w.write(h.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).unwrap();
    w.finish().unwrap();
    h.disconnect_client();
    h.exec_line("cl_demobar 2");
    h.exec_line(&format!("playdemo {}", path.display()));
    run(&mut h, 0.3);
    assert!(h.demo_bar_visible());
    // View hides; A (pause) used to bring it back for a few seconds, and the
    // next View then pinned it instead of hiding it
    let pad = |h: &mut Host, k| assert!(h.demo_key(k, true));
    pad(&mut h, qinput::keys::K_BACK);
    assert!(!h.demo_bar_visible());
    pad(&mut h, qinput::keys::K_ABUTTON);
    pad(&mut h, qinput::keys::K_RSHOULDER);
    run(&mut h, 0.1);
    assert!(!h.demo_bar_visible(), "control keys keep a hidden bar hidden");
    pad(&mut h, qinput::keys::K_BACK);
    assert!(h.demo_bar_visible());
    pad(&mut h, qinput::keys::K_BACK);
    assert!(!h.demo_bar_visible());
    // picking a Playback Bar setting takes over again
    h.exec_line("cl_demobar 1");
    pad(&mut h, qinput::keys::K_ABUTTON);
    assert!(h.demo_bar_visible(), "auto mode shows the bar after a control");
    pad(&mut h, qinput::keys::K_BACK);
    assert!(!h.demo_bar_visible(), "hiding the auto-shown bar hides it");
    let _ = std::fs::remove_dir_all(&dir);
}
