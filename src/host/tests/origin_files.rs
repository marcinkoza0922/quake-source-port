//! Commands that write, move or delete files, or choose programs, do so only
//! for the user; a server, demo, mod, config or bind (`Origin::Script`) is
//! refused or kept inside the user dir.

use std::path::{Path, PathBuf};

use super::super::*;

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("qsp-origin-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
    fn file(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, b"mine").unwrap();
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mine(p: &Path) -> bool {
    std::fs::read(p).is_ok_and(|d| d == b"mine")
}

fn host() -> Option<Host> {
    let fs = qcore::fs::Fs::new(Path::new("."));
    let r = Renderer::headless(64, 64, &[[0u8; 3]; 256]).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    Some(h)
}

fn run(h: &mut Host, origin: Origin, line: &str) {
    h.cbuf.add_text_from(&format!("{line}\n"), origin);
    for _ in 0..4 {
        h.frame(1.0 / 72.0, Vec::new());
    }
}

#[test]
fn screenshots_and_profiles_from_scripts_stay_in_the_user_dir() {
    let Some(mut h) = host() else { return };
    let d = Dir::new("shot");
    let (doc, json) = (d.file("thesis.pdf"), d.file("notes"));
    for line in [
        format!("screenshot {}", doc.display()),
        format!("screenshot \"{}/../thesis.pdf\"", d.0.join("x").display()),
        format!("delay 0 screenshot {}", doc.display()),
        format!("prof_start; prof_stop {}", json.display()),
        format!("prof_timedemo {}", json.display()),
    ] {
        run(&mut h, Origin::Script, &line);
        assert!(mine(&doc) && mine(&json), "{line}");
    }
    assert_eq!(h.cvs("prof_timedemo"), "");
    assert!(h.screenshot.is_none());
    // a plain name is a new PNG in the user dir, and never replaces one
    let user = h.user_dir();
    std::fs::create_dir_all(&user).unwrap();
    std::fs::write(user.join("shot.png"), b"mine").unwrap();
    run(&mut h, Origin::Script, "screenshot shot");
    assert!(mine(&user.join("shot.png")) && user.join("shot_2.png").is_file());
    run(&mut h, Origin::Script, "screenshot config.cfg");
    assert!(user.join("config.cfg.png").is_file() && !user.join("config.cfg").exists());
    // the user's own path is theirs to choose
    let own = d.0.join("own.png");
    run(&mut h, Origin::User, &format!("screenshot {}", own.display()));
    assert!(own.is_file());
    run(&mut h, Origin::User, &format!("prof_start; prof_stop {}", d.0.join("p.json").display()));
    assert!(d.0.join("p.json").is_file());
}

#[test]
fn demo_and_save_files_are_the_users() {
    let Some(mut h) = host() else { return };
    let user = h.user_dir();
    std::fs::create_dir_all(&user).unwrap();
    let d = Dir::new("files");
    let (key, dem) = (d.file("id_ed25519"), d.file("x.dem"));
    let own = user.join("own.dem");
    let save = user.join("s0.sav");
    for p in [&own, &save] {
        std::fs::write(p, b"mine").unwrap();
    }
    let attacks = [
        "_deletesave s0".to_string(),
        "_demo_delete own.dem".into(),
        "_demo_rename own.dem other".into(),
        "_demo_keep_file own.dem".into(),
        format!("_demo_rename {} k", key.display()),
        format!("_demo_keep_file {}", key.display()),
        format!("_demo_delete {}", dem.display()),
        // out of reach whoever asks
        format!("record {}", dem.display()),
        format!("save {}", d.0.join("s").display()),
        "save ../s".into(),
    ];
    for a in &attacks {
        run(&mut h, Origin::Script, a);
        assert!([&key, &dem, &own, &save].iter().all(|p| mine(p)), "{a}");
    }
    // the user, too, only names demos inside the user dir
    for a in &attacks[4..] {
        run(&mut h, Origin::User, a);
        assert!([&key, &dem, &own, &save].iter().all(|p| mine(p)), "{a}");
    }
    assert!(h.savegame_path("../s").is_none() && h.savegame_path("/s").is_none() && h.savegame_path("s0").is_some());
    assert!(h.find_save("../s0").is_none() && h.find_save("s0").is_some());
    // the menu's own buttons work
    h.with_origin(Origin::Menu, |h| {
        qcore::cmd::execute_words(h, &["_demo_rename", "own.dem", "a; quit \"b"]);
        qcore::cmd::execute_words(h, &["_demo_rename", "own.dem", "new name"]);
        qcore::cmd::execute_words(h, &["_deletesave", "s0"]);
    });
    assert!(user.join("new name.dem").is_file() && !own.exists() && !save.exists() && !h.quit);
}

#[test]
fn programs_and_folders_are_the_users() {
    let Some(mut h) = host() else { return };
    for (cvar, default) in [("capture_ffmpeg", "ffmpeg"), ("bugreport_dir", ""), ("_export_dest", ""), ("_export_name", "")] {
        let before = h.cvs(cvar).to_string();
        assert!(cvar != "capture_ffmpeg" || before == default);
        for line in ["{} evil", "set {} evil", "seta {} evil", "toggle {}", "inc {}", "cycle {} evil evil2", "delay 0 {} evil", "alias a \"{} evil\"; a"] {
            run(&mut h, Origin::Script, &line.replace("{}", cvar));
            assert_eq!(h.cvs(cvar), before, "{line} {cvar}");
        }
        // an alias the user runs is still somebody else's text
        run(&mut h, Origin::User, "a");
        assert_eq!(h.cvs(cvar), before);
        // QuakeC, even inside a command the user typed
        h.with_origin(Origin::User, |h| h.server.core.cvars.set_script(cvar, "evil"));
        assert_eq!(h.cvs(cvar), before);
        run(&mut h, Origin::User, &format!("{cvar} mine"));
        assert_eq!(h.cvs(cvar), "mine");
        assert!(h.server.core.cvars.archive_lines().iter().all(|l| !l.starts_with(cvar)), "a config must not carry {cvar}");
    }
}

#[test]
fn scripts_cannot_drive_the_menu() {
    let Some(mut h) = host() else { return };
    run(&mut h, Origin::Script, "menu_main");
    let before = h.menu.screen;
    run(&mut h, Origin::Script, "menu_keys ENTER");
    assert_eq!(h.menu.screen, before);
    run(&mut h, Origin::User, "menu_keys ENTER");
    assert_ne!(h.menu.screen, before);
}

#[test]
fn completed_text_stays_one_command() {
    let Some(mut h) = host() else { return };
    use crate::console::complete::{Catalog, compute};
    let cat = Catalog {
        cmds: vec!["map".into()],
        aliases: vec![("map|evil".into(), String::new()), ("!map".into(), String::new())],
        cvars: vec![("fov".into(), "90\n!evil".into(), "90".into())],
        maps: vec!["e1m1|evil".into(), "e1m1;evil".into(), "e1m1".into()],
        ..Default::default()
    };
    let texts = |line: &str| compute(line, line.len(), &cat).items.into_iter().map(|c| c.text).collect::<Vec<_>>();
    assert_eq!(texts("map"), ["map"]);
    assert_eq!(texts("fov "), ["90"]);
    assert_eq!(texts("map e1"), ["e1m1"]);
    // and the entered line is one line, whatever is in it
    h.cbuf.take_text();
    h.run_typed("echo a\n!evil");
    assert_eq!(h.cbuf.next_line_from(), Some(("echo a".into(), Origin::User)));
    assert!(h.cbuf.is_empty());
}
