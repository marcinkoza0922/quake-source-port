//! The user's own files survive: configs not yet read, games a bad save
//! would replace.

use super::super::*;

fn fresh() -> Option<Host> {
    let fs = qcore::fs::Fs::new(std::path::Path::new("."));
    let r = Renderer::headless(64, 64, &[[0u8; 3]; 256]).ok()?;
    Some(Host::new(fs, r, None, Vec::new()))
}

#[test]
fn quitting_during_startup_keeps_the_config() {
    let Some(mut h) = fresh() else { return };
    let p = h.user_dir().join("config.cfg");
    qcore::vfs::write(&p, b"bind x \"echo mine\"\n").unwrap();
    // the window is closed (or Ctrl+C pressed) while the engine loads
    h.frame(1.0 / 72.0, vec![HostEvent::Quit]);
    assert!(h.quit);
    h.write_config();
    assert_eq!(qcore::vfs::read(&p).unwrap(), b"bind x \"echo mine\"\n");

    // sent to the background before the first frame (Android)
    let mut h = fresh().unwrap();
    h.handle_event(HostEvent::WillEnterBackground);
    assert_eq!(qcore::vfs::read(&p).unwrap(), b"bind x \"echo mine\"\n");

    // once the configs ran, settings are saved
    let mut h = fresh().unwrap();
    h.frame(1.0 / 72.0, Vec::new());
    h.exec_line("bind y \"echo new\"");
    h.write_config();
    let s = String::from_utf8(qcore::vfs::read(&p).unwrap()).unwrap();
    assert!(s.contains("echo new"), "{s:.200}");
}

#[test]
fn a_config_that_never_stops_waiting_is_still_saved() {
    let Some(mut h) = fresh() else { return };
    h.exec_line("alias loop \"wait; loop\"");
    h.cbuf.add_text("loop\n");
    for _ in 0..5 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(!h.cbuf.is_empty() && !h.config_ready);
    h.exec_line("_config_done");
    assert!(h.config_ready);
}

#[test]
fn a_damaged_save_does_not_end_the_game() {
    let mut base = qcore::fs::Fs::default_basedir();
    if base.is_relative() && !base.exists() {
        base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(base);
    }
    if !base.join("id1").is_dir() {
        eprintln!("skip: no game data");
        return;
    }
    let mut fs = qcore::fs::Fs::new(&base);
    fs.add_game_dir("id1");
    let Ok(r) = Renderer::headless(64, 64, &[[0u8; 3]; 256]) else { return };
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    h.exec_line("sv_autosave 0");
    h.exec_line("cl_startdemos 0");
    h.exec_line("map e1m1");
    for _ in 0..60 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert_eq!(h.cl.signon, 4);
    h.exec_line("save good");
    let p = h.user_dir().join("good.sav");
    let text = qcore::vfs::read(&p).unwrap();
    qcore::vfs::write(&p.with_file_name("bad.sav"), &text[..text.len() / 2]).unwrap();
    let (local, spawns, origin) = (h.local, h.server.core.spawns, h.player_origin());
    h.exec_line("load bad");
    for _ in 0..30 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert_eq!((h.local, h.server.core.spawns, h.cl.signon), (local, spawns, 4));
    assert!((h.player_origin() - origin).length() < 1.0);
    // and a good one still loads
    h.exec_line("load good");
    for _ in 0..60 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(h.server.core.spawns > spawns && h.cl.signon == 4);
}
