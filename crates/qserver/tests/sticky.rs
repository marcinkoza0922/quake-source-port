//! Stickybomb launcher add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
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
    sv.command("addons_enable 1");
    sv.core.set_addons(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    Some((sv, c))
}

fn run(sv: &mut Server, c: usize, frames: usize, mv: ClientMove) {
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        let _ = sv.take_reliable(c);
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
}

fn toy(buttons: u8) -> ClientMove {
    ClientMove { angles: [30.0, 90.0, 0.0], buttons: Toy::Sticky.id() << TOY_SHIFT | buttons, ..Default::default() }
}

fn rockets(sv: &Server) -> f32 {
    sv.level.as_ref().unwrap().vm.ent_f(1, sv.core.sv.fields.ammo_rockets)
}

#[test]
fn bombs_stick_survive_a_save_and_alt_fire_detonates_them() {
    let Some((mut sv, c)) = start() else { return };
    run(&mut sv, c, 30, ClientMove::default());
    sv.command("setpos 480 200 24");
    sv.level.as_mut().unwrap().vm.set_ent_f(1, sv.core.sv.fields.ammo_rockets, 10.0);
    run(&mut sv, c, 2, toy(0));
    // two quick taps a refire period apart: two bombs
    for _ in 0..2 {
        run(&mut sv, c, 3, toy(BUTTON_TOY_FIRE));
        run(&mut sv, c, 50, toy(0));
    }
    run(&mut sv, c, 100, toy(0));
    let bombs = sv.level.as_ref().unwrap().stickies.bombs.clone();
    assert_eq!(bombs.len(), 2, "{bombs:?}");
    assert!(bombs.iter().all(|b| b.stuck.is_some()), "{bombs:?}");
    assert_eq!(rockets(&sv), 8.0);

    let text = sv.save_game();
    assert!(text.contains("\nsticky 0 "), "{}", &text[..200.min(text.len())]);
    let (mut other, oc) = start().unwrap();
    other.load_game(&text).unwrap();
    assert_eq!(other.level.as_ref().unwrap().stickies.bombs.len(), 2);

    let _ = oc;

    run(&mut sv, c, 2, toy(BUTTON_TOY_ALT));
    assert!(sv.level.as_ref().unwrap().stickies.bombs.is_empty());
}

#[test]
fn holding_fire_charges_a_faster_launch() {
    let Some((mut sv, c)) = start() else { return };
    run(&mut sv, c, 30, ClientMove::default());
    sv.level.as_mut().unwrap().vm.set_ent_f(1, sv.core.sv.fields.ammo_rockets, 10.0);
    run(&mut sv, c, 2, toy(0));
    let speed = |sv: &Server| sv.level.as_ref().unwrap().stickies.bombs.last().map(|b| b.vel.length());
    // nothing leaves while fire is held
    run(&mut sv, c, 36, toy(BUTTON_TOY_FIRE));
    assert!(sv.level.as_ref().unwrap().stickies.bombs.is_empty());
    run(&mut sv, c, 1, toy(0));
    let half = speed(&sv).expect("released shot");
    // held past the full charge it fires by itself at the top speed
    run(&mut sv, c, 60, toy(0));
    let n = sv.level.as_ref().unwrap().stickies.bombs.len();
    run(&mut sv, c, (qserver::addons::sticky::CHARGE * 72.0) as usize + 2, toy(BUTTON_TOY_FIRE));
    assert_eq!(sv.level.as_ref().unwrap().stickies.bombs.len(), n + 1);
    let full = speed(&sv).unwrap();
    assert!(half > 700.0 && full > half + 300.0, "{half} {full}");
}
