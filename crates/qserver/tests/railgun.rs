//! Railgun add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::{BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
use qserver::{ClientMove, Server};

fn start() -> Option<(Server, usize)> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    if !d.join("id1").is_dir() {
        return None;
    }
    let mut fs = Fs::new(&d);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.core.set_addons(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    Some((sv, c))
}

/// Runs frames, returning the reliable text received.
fn run(sv: &mut Server, c: usize, frames: usize, buttons: u8) -> String {
    let mv = ClientMove { angles: [0.0, 90.0, 0.0], buttons: Toy::Railgun.id() << TOY_SHIFT | buttons, ..Default::default() };
    let mut out = Vec::new();
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        out.extend(sv.take_reliable(c).into_iter().flatten());
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn cells(sv: &Server) -> f32 {
    sv.level.as_ref().unwrap().vm.ent_f(1, sv.core.sv.fields.ammo_cells)
}

#[test]
fn fires_spends_cells_waits_for_refire_and_clicks_when_empty() {
    let Some((mut sv, c)) = start() else { return };
    run(&mut sv, c, 30, 0);
    sv.command("setpos 480 200 40");
    sv.command("give c 5");
    run(&mut sv, c, 5, 0);
    let text = run(&mut sv, c, 1, BUTTON_TOY_FIRE);
    assert!(text.contains("//qsp_rail 0 1 "), "{text}");
    assert_eq!(cells(&sv), 3.0);
    // held trigger: no second shot before 1.5 s, one right after
    let text = run(&mut sv, c, 100, BUTTON_TOY_FIRE);
    assert!(!text.contains("//qsp_rail"));
    let text = run(&mut sv, c, 10, BUTTON_TOY_FIRE);
    assert!(text.contains("//qsp_rail 0 1 "));
    assert_eq!(cells(&sv), 1.0);
    // one cell left: a dry click, nothing spent
    let text = run(&mut sv, c, 120, BUTTON_TOY_FIRE);
    assert!(text.contains("//qsp_rail 0 0 ") && !text.contains("//qsp_rail 0 1 "));
    assert_eq!(cells(&sv), 1.0);
    // toy switched off: nothing
    sv.core.cvars.set("addon_railgun", "0");
    sv.command("give c 50");
    assert!(!run(&mut sv, c, 120, BUTTON_TOY_FIRE).contains("//qsp_rail"));
}
