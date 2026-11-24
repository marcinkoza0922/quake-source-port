//! Assault rifle add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, BUTTON_TOY_RELOAD, Toy};
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
    qserver::addons::enable_all(&mut sv.core.cvars);
    sv.core.set_custom(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    let mut t = (sv, c);
    run(&mut t.0, c, 30, 0);
    t.0.command("notarget");
    t.0.command("god");
    Some(t)
}

/// Runs frames with the rifle held, returning the reliable text received.
fn run(sv: &mut Server, c: usize, frames: usize, buttons: u16) -> String {
    step(sv, c, frames, Toy::Rifle.bits() | buttons, 1.0 / 72.0, 0.0)
}

fn step(sv: &mut Server, c: usize, frames: usize, buttons: u16, dt: f64, forward: f32) -> String {
    let mv = ClientMove { angles: [0.0, 90.0, 0.0], buttons, forward, ..Default::default() };
    let mut out = Vec::new();
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(dt);
        out.extend(sv.take_reliable(c).into_iter().flatten());
        let _ = sv.take_datagrams(c);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn field(sv: &Server, f: usize) -> f32 {
    sv.level.as_ref().unwrap().vm.ent_f(1, f)
}
fn nails(sv: &Server) -> f32 {
    field(sv, sv.core.sv.fields.ammo_nails)
}
/// Events of `kind` (toy 10, part 0).
fn count(text: &str, kind: u8) -> usize {
    text.matches(&format!("//qsp_gun 0 10 0 {kind} ")).count()
}

#[test]
fn fires_full_auto_and_reloads_from_nails() {
    let Some((mut sv, c)) = start() else { return };
    sv.command("give n 40");
    let text = run(&mut sv, c, 72, BUTTON_TOY_FIRE);
    assert!((12..=14).contains(&count(&text, 1)), "{} shots a second", count(&text, 1));
    assert_eq!(nails(&sv), 40.0, "the magazine, not the reserve, is spent");
    // a reload takes its time and only what was missing
    run(&mut sv, c, 5, 0);
    let shots = count(&text, 1) as f32;
    let text = run(&mut sv, c, 5, BUTTON_TOY_RELOAD);
    assert!(count(&text, 2) == 1 && text.contains("//qsp_mag 0 10 30 "), "{text}");
    assert_eq!(count(&run(&mut sv, c, 100, BUTTON_TOY_FIRE), 1), 0, "no shots while reloading");
    let text = run(&mut sv, c, 72, 0);
    assert_eq!(count(&text, 4), 1);
    assert_eq!(nails(&sv), 40.0 - shots);
    assert!(text.contains("//qsp_mag 0 10 30 30 0.00"), "{text}");
    // full: nothing to reload
    assert_eq!(count(&run(&mut sv, c, 3, BUTTON_TOY_RELOAD), 2), 0);
    // emptied: reloads by itself what the reserve holds, then clicks dry
    let text = run(&mut sv, c, 72 * 3, BUTTON_TOY_FIRE);
    assert_eq!((count(&text, 1), count(&text, 2)), (30, 1));
    let left = nails(&sv);
    let text = run(&mut sv, c, 72 * 3, 0);
    assert_eq!((count(&text, 4), nails(&sv)), (1, (left - 30.0).max(0.0)));
    run(&mut sv, c, 72 * 4, BUTTON_TOY_FIRE);
    assert_eq!(nails(&sv), 0.0);
    run(&mut sv, c, 2, 0);
    let text = run(&mut sv, c, 72, BUTTON_TOY_FIRE);
    assert_eq!((count(&text, 0), count(&text, 1), count(&text, 2)), (1, 0, 0), "{text}");
}

#[test]
fn putting_it_away_cancels_a_reload_and_keeps_the_nails() {
    let Some((mut sv, c)) = start() else { return };
    sv.command("give n 100");
    run(&mut sv, c, 36, BUTTON_TOY_FIRE);
    run(&mut sv, c, 3, BUTTON_TOY_RELOAD);
    let text = step(&mut sv, c, 3, 0, 1.0 / 72.0, 0.0);
    assert_eq!(count(&text, 3), 1);
    step(&mut sv, c, 300, 0, 1.0 / 72.0, 0.0);
    assert_eq!(nails(&sv), 100.0);
    // the rounds left are saved and come back
    run(&mut sv, c, 2, 0);
    let saved = sv.save_game();
    let rounds: u32 = saved.lines().find_map(|l| l.strip_prefix("rifle 0 ")).expect("rifle line").parse().unwrap();
    assert!((20..30).contains(&rounds), "{rounds}");
    // toy off: state dropped, the clients told
    sv.core.cvars.set("addon_rifle", "0");
    assert!(run(&mut sv, c, 2, BUTTON_TOY_FIRE).contains("//qsp_mag 0 10\n"));
}

#[test]
fn the_rate_holds_at_low_frame_rates_and_aiming_slows_the_walk() {
    let Some((mut sv, c)) = start() else { return };
    sv.core.cvars.set("addon_rifle_mag", "100");
    run(&mut sv, c, 3, 0);
    sv.command("give n 200");
    run(&mut sv, c, 3, BUTTON_TOY_RELOAD);
    run(&mut sv, c, 200, 0);
    let text = step(&mut sv, c, 60, Toy::Rifle.bits() | BUTTON_TOY_FIRE, 0.05, 0.0);
    assert!((37..=39).contains(&count(&text, 1)), "{} shots in 3 s at 20 fps", count(&text, 1));
    let speed = |sv: &Server| {
        let v = sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.velocity);
        (v[0] * v[0] + v[1] * v[1]).sqrt()
    };
    // e1m1's start corridor runs along +y
    sv.command("setpos 480 -300 40 0 90 0");
    step(&mut sv, c, 20, Toy::Rifle.bits(), 1.0 / 72.0, 0.0);
    step(&mut sv, c, 25, Toy::Rifle.bits(), 1.0 / 72.0, 400.0);
    let fast = speed(&sv);
    sv.command("setpos 480 -300 40 0 90 0");
    step(&mut sv, c, 20, Toy::Rifle.bits() | BUTTON_TOY_ALT, 1.0 / 72.0, 0.0);
    step(&mut sv, c, 25, Toy::Rifle.bits() | BUTTON_TOY_ALT, 1.0 / 72.0, 400.0);
    let slow = speed(&sv);
    assert!(slow > 60.0 && slow < fast * 0.75, "{slow} against {fast}");
}
