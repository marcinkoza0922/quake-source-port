//! Gameplay assists against the real id1 progs (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    d.join("id1").is_dir().then_some(d)
}

/// e1m1 with one local client; `cvars` are set before spawning.
fn start(cvars: &[&str]) -> Option<(Server, usize)> {
    let base = basedir()?;
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    for c in cvars {
        sv.command(c);
    }
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

fn field(sv: &Server, f: impl Fn(&qserver::fields::Fields) -> usize) -> f32 {
    let l = sv.level.as_ref().unwrap();
    l.vm.ent_f(1, f(&sv.core.sv.fields))
}

fn idle() -> ClientMove {
    ClientMove::default()
}

#[test]
fn infinite_ammo_keeps_shells() {
    let Some((mut sv, c)) = start(&["assist_infiniteammo 1"]) else { return };
    run(&mut sv, c, 30, idle());
    let before = field(&sv, |f| f.ammo_shells);
    run(&mut sv, c, 150, ClientMove { buttons: 1, ..Default::default() });
    assert_eq!(field(&sv, |f| f.ammo_shells), before);
}

/// Health lost from a rocket fired at the player's feet.
fn rocket_self_damage(setting: &str) -> Option<f32> {
    let (mut sv, c) = start(&[setting])?;
    run(&mut sv, c, 30, idle());
    run(&mut sv, c, 2, ClientMove { impulse: 9, ..Default::default() });
    run(&mut sv, c, 2, ClientMove { impulse: 7, ..Default::default() });
    run(&mut sv, c, 60, idle());
    let before = field(&sv, |f| f.health);
    let down = ClientMove { angles: [80.0, 0.0, 0.0], ..Default::default() };
    run(&mut sv, c, 1, ClientMove { buttons: 1, ..down });
    run(&mut sv, c, 60, down);
    Some(before - field(&sv, |f| f.health))
}

#[test]
fn damage_taken_scales_self_damage() {
    let Some(full) = rocket_self_damage("assist_damagetaken 1") else { return };
    let quarter = rocket_self_damage("assist_damagetaken 0.25").unwrap();
    assert!(full > 0.0, "rocket did no damage");
    assert!(quarter > 0.0 && quarter < full * 0.5, "full {full}, quarter {quarter}");
}

#[test]
fn triple_jump() {
    let Some((mut sv, c)) = start(&["assist_jumps 3", "sv_gravity 2400"]) else { return };
    run(&mut sv, c, 60, idle());
    let jump = ClientMove { buttons: 2, ..Default::default() };
    let mut air_jumps = 0;
    let mut vzs = Vec::new();
    // ground jump, then two presses in the air, then one too many; high
    // gravity keeps the hops under the low ceiling at the start
    for _ in 0..4 {
        run(&mut sv, c, 2, jump);
        vzs.push(field(&sv, |f| f.velocity + 2));
        if field(&sv, |f| f.velocity + 2) > 150.0 {
            air_jumps += 1;
        }
        run(&mut sv, c, 2, idle());
    }
    assert_eq!(air_jumps, 3, "ground jump + 2 extra; vz after each press {vzs:?}");
}

#[test]
fn no_self_damage() {
    let Some(lost) = rocket_self_damage("assist_noselfdamage 1") else { return };
    assert_eq!(lost, 0.0);
    // knockback still applies: a rocket at the feet launches the player
    let (mut sv, c) = start(&["assist_noselfdamage 1"]).unwrap();
    run(&mut sv, c, 30, idle());
    run(&mut sv, c, 2, ClientMove { impulse: 9, ..Default::default() });
    run(&mut sv, c, 2, ClientMove { impulse: 7, ..Default::default() });
    run(&mut sv, c, 60, idle());
    let down = ClientMove { angles: [80.0, 0.0, 0.0], ..Default::default() };
    run(&mut sv, c, 1, ClientMove { buttons: 1, ..down });
    let mut top = 0.0f32;
    for _ in 0..30 {
        run(&mut sv, c, 1, down);
        top = top.max(field(&sv, |f| f.velocity + 2));
    }
    assert!(top > 300.0, "no rocket jump, peak vz {top}");
}
