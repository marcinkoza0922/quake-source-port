//! Pausing in the console, and add-on choices that belong to a game's saves.

use super::super::*;

fn host() -> Option<Host> {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || fs.open("maps/e1m1.bsp").is_none() {
        eprintln!("skip: no game data");
        return None;
    }
    let r = Renderer::headless(64, 64, &[[0u8; 3]; 256]).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    run(&mut h, 4);
    for c in ["sv_autosave 0", "cl_startdemos 0"] {
        h.exec_line(c);
    }
    Some(h)
}

fn run(h: &mut Host, frames: usize) {
    for _ in 0..frames {
        h.frame(1.0 / 72.0, Vec::new());
    }
}

fn start(h: &mut Host) {
    h.exec_line("map e1m1");
    run(h, 60);
    assert_eq!(h.cl.signon, 4);
    assert_eq!(h.key_dest(), KeyDest::Game);
}

#[test]
fn the_console_and_chat_pause_single_player() {
    let Some(mut h) = host() else { return };
    start(&mut h);
    let passes = |h: &mut Host| {
        let t = h.server.time();
        run(h, 20);
        h.server.time() > t
    };
    assert!(passes(&mut h));
    for (open, close) in [("toggleconsole", "toggleconsole"), ("messagemode", "")] {
        h.exec_line(open);
        assert!(!passes(&mut h), "{open}");
        if close.is_empty() {
            h.con.end_chat();
        } else {
            h.exec_line(close);
        }
        assert!(passes(&mut h), "after {open}");
    }
    // commands typed there still run, and a level can start from it
    h.exec_line("toggleconsole");
    let spawns = h.server.core.spawns;
    h.cbuf.add_text_from("map e1m2\n", Origin::User);
    run(&mut h, 60);
    assert!(h.server.core.spawns > spawns);
    assert_eq!(h.cl.signon, 4, "signs on");
    // two slots: the world goes on
    h.con.open = false;
    h.exec_line("disconnect");
    h.exec_line("maxplayers 2");
    start(&mut h);
    h.exec_line("toggleconsole");
    assert!(passes(&mut h));
}

#[test]
fn choices_stay_with_the_game_and_its_saves() {
    let Some(mut h) = host() else { return };
    h.exec_line("custom_enable 1");
    h.exec_line("addon_portalgun 1");
    h.exec_line("addon_rail_damage 250");
    h.exec_line("modifier_horde 1");
    start(&mut h);
    assert!(h.server.core.custom);
    h.exec_line("save custom");
    // another game, with other choices
    for c in ["addon_portalgun 0", "addon_railgun 1", "addon_rail_damage 10", "modifier_horde 0"] {
        h.exec_line(c);
    }
    h.exec_line("load custom");
    run(&mut h, 60);
    assert_eq!(h.cl.signon, 4);
    let game = |h: &Host| ["addon_portalgun", "addon_railgun", "addon_rail_damage", "modifier_horde"].map(|c| h.cv(c));
    assert_eq!(game(&h), [1.0, 0.0, 250.0, 1.0]);
    // the New Game pop-up has its own, and the world waits meanwhile
    h.exec_line("menu_newgame");
    assert_eq!(game(&h), [0.0, 0.0, 100.0, 0.0]);
    h.server.core.cvars.set("addon_railgun", "1");
    run(&mut h, 10);
    assert_eq!(game(&h), [0.0, 1.0, 100.0, 0.0]);
    // saving now (the app is sent to the background) saves the game's own
    h.exec_line("save custom2");
    assert_eq!(game(&h), [1.0, 0.0, 250.0, 1.0]);
    h.exec_line("menu_newgame");
    assert_eq!(game(&h), [0.0, 1.0, 100.0, 0.0], "kept for this session");
    h.menu.open(Screen::Main);
    run(&mut h, 2);
    assert_eq!(game(&h), [1.0, 0.0, 250.0, 1.0]);
    h.exec_line("load custom2");
    run(&mut h, 60);
    assert_eq!(game(&h), [1.0, 0.0, 250.0, 1.0]);
    // Play starts the new game with the pop-up's choices
    h.menu.screen = Screen::None;
    h.exec_line("menu_newgame");
    h.exec_line_from("menu_keys ENTER", Origin::User);
    h.exec_line_from("menu_keys y", Origin::User);
    let spawns = h.server.core.spawns;
    run(&mut h, 90);
    assert!(h.server.core.spawns > spawns, "started");
    assert!(h.server.core.custom && !h.menu.setup_active());
    assert_eq!(game(&h), [0.0, 1.0, 100.0, 0.0]);
    // none of it is in the config
    let lines = h.server.core.cvars.archive_lines().join("\n");
    assert!(!lines.contains("addon_railgun ") && !lines.contains("modifier_") && !lines.contains("custom_enable"));
}
