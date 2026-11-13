//! Hookshot add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::hookshot::State;
use qserver::addons::{BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    d.join("id1").is_dir().then_some(d)
}

fn start(cvars: &[&str]) -> Option<(Server, usize)> {
    let base = basedir()?;
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    for c in cvars {
        sv.command(c);
    }
    sv.core.set_addons(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    Some((sv, c))
}

/// Runs frames; returns every hook state seen and all stufftext.
fn run(sv: &mut Server, c: usize, frames: usize, mv: ClientMove) -> (Vec<State>, String) {
    let mut states = Vec::new();
    let mut text = String::new();
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        text += &String::from_utf8_lossy(&sv.take_reliable(c).concat());
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
        states.push(sv.level.as_ref().unwrap().hooks.state.first().copied().unwrap_or_default());
    }
    (states, text)
}

fn origin(sv: &Server) -> [f32; 3] {
    sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin)
}

fn hook(angles: [f32; 3], buttons: u8) -> ClientMove {
    ClientMove { angles, buttons: Toy::Hookshot.id() << TOY_SHIFT | buttons, ..Default::default() }
}

/// In the start corridor, shoot the side wall.
fn shoot(sv: &mut Server, c: usize) -> (Vec<State>, String) {
    run(sv, c, 30, ClientMove::default());
    sv.command("setpos 480 200 24");
    run(sv, c, 10, ClientMove::default());
    let aim = [0.0, 0.0, 0.0];
    run(sv, c, 1, hook(aim, 0));
    run(sv, c, 200, hook(aim, BUTTON_TOY_FIRE))
}

#[test]
fn hook_latches_pulls_the_player_and_lets_go_safely() {
    let Some((mut sv, c)) = start(&[]) else { return };
    let before = origin(&sv);
    let (states, text) = shoot(&mut sv, c);
    assert!(states.iter().any(|s| matches!(s, State::Latched { .. })), "never latched: {:?}", &states[..20]);
    assert!(matches!(states.last(), Some(State::Idle)), "{:?}", states.last());
    assert!(text.contains("//qsp_hook 0 2 "));
    let after = origin(&sv);
    let moved = ((after[0] - before[0]).powi(2) + (after[1] - before[1]).powi(2)).sqrt();
    assert!(moved > 48.0, "pulled only {moved}: {before:?} -> {after:?}");
    // never left inside geometry
    let l = sv.level.as_ref().unwrap();
    assert_ne!(l.world.point_contents(after.into()), qworld::CONTENTS_SOLID);
}

#[test]
fn unhookable_materials_retract() {
    let Some((mut sv, c)) = start(&["addon_hook_stone 0", "addon_hook_metal 0", "addon_hook_wood 0"]) else { return };
    let before = origin(&sv);
    let (states, _) = shoot(&mut sv, c);
    assert!(!states.iter().any(|s| matches!(s, State::Latched { .. })));
    assert!(states.iter().any(|s| matches!(s, State::Retracting { .. })));
    let after = origin(&sv);
    assert!((after[0] - before[0]).abs() < 8.0);
}
