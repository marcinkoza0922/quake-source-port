//! Akimbo pistols add-on against the real id1 e1m1 (skips without game data).

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
    run(&mut sv, c, 30, Toy::Akimbo, 0);
    sv.command("notarget");
    sv.command("setpos 480 200 40");
    run(&mut sv, c, 5, Toy::Akimbo, 0);
    Some((sv, c))
}

/// Runs frames, returning the reliable text received.
fn run(sv: &mut Server, c: usize, frames: usize, toy: Toy, buttons: u16) -> String {
    let mv = ClientMove { angles: [0.0, 90.0, 0.0], buttons: toy.bits() | buttons, ..Default::default() };
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

fn akimbo(sv: &mut Server, c: usize, frames: usize, buttons: u16) -> String {
    run(sv, c, frames, Toy::Akimbo, buttons)
}

/// (left, right) events of `kind`.
fn count(text: &str, kind: u8) -> (usize, usize) {
    let n = |part: u8| text.matches(&format!("//qsp_gun 0 9 {part} {kind} ")).count();
    (n(0), n(1))
}

const DRY: u8 = 0;
const FIRE: u8 = 1;
const RELOAD: u8 = 2;
const CANCEL: u8 = 3;
const DONE: u8 = 4;

#[test]
fn each_trigger_shoots_its_own_pistol_and_holding_repeats_slowly() {
    let Some((mut sv, c)) = start() else { return };
    let t = akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE);
    assert_eq!(count(&t, FIRE), (1, 0));
    assert!(t.contains("//qsp_mag 0 9 12 11 0.00 12 0.00"), "{t}");
    let t = akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE | BUTTON_TOY_ALT);
    assert_eq!(count(&t, FIRE), (0, 1));
    // held for a second: about 1 / HOLD shots each
    let t = akimbo(&mut sv, c, 72, BUTTON_TOY_FIRE | BUTTON_TOY_ALT);
    let (l, r) = count(&t, FIRE);
    assert!((4..=5).contains(&l) && (4..=5).contains(&r), "{l} {r}");
    // tapping is faster
    let mut shots = 0;
    for k in 0..24 {
        let b = if k % 2 == 0 { BUTTON_TOY_FIRE } else { 0 };
        shots += count(&akimbo(&mut sv, c, 2, b), FIRE).0;
    }
    assert!(shots >= 5, "{shots}");
}

#[test]
fn swapped_hands() {
    let Some((mut sv, c)) = start() else { return };
    sv.core.cvars.set("addon_akimbo_swap", "1");
    assert_eq!(count(&akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE), FIRE), (0, 1));
    assert_eq!(count(&akimbo(&mut sv, c, 1, BUTTON_TOY_ALT), FIRE), (1, 0));
}

#[test]
fn reloads_are_staggered_and_the_other_pistol_still_shoots() {
    let Some((mut sv, c)) = start() else { return };
    akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE | BUTTON_TOY_ALT);
    akimbo(&mut sv, c, 20, 0);
    let t = akimbo(&mut sv, c, 1, BUTTON_TOY_RELOAD);
    assert_eq!(count(&t, RELOAD), (1, 1));
    // no shots meanwhile
    let t = akimbo(&mut sv, c, 60, BUTTON_TOY_FIRE | BUTTON_TOY_ALT);
    assert_eq!((count(&t, FIRE), count(&t, DONE)), ((0, 0), (0, 0)));
    let t = akimbo(&mut sv, c, 60, 0);
    assert_eq!(count(&t, DONE), (1, 0), "1.6 s: the left one first");
    let t = akimbo(&mut sv, c, 40, 0);
    assert_eq!(count(&t, DONE), (0, 1));
    assert!(t.contains("//qsp_mag 0 9 12 12 0.00 12 0.00"), "{t}");
    // full magazines don't reload
    assert_eq!(count(&akimbo(&mut sv, c, 1, BUTTON_TOY_RELOAD), RELOAD), (0, 0));
    // only the right one needs it: the left one shoots on
    akimbo(&mut sv, c, 2, BUTTON_TOY_ALT);
    let t = akimbo(&mut sv, c, 1, BUTTON_TOY_RELOAD);
    assert_eq!(count(&t, RELOAD), (0, 1));
    assert_eq!(count(&akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE), FIRE), (1, 0));
}

#[test]
fn an_empty_pistol_clicks_and_reloads_itself() {
    let Some((mut sv, c)) = start() else { return };
    sv.core.cvars.set("addon_akimbo_mag", "2");
    let t = akimbo(&mut sv, c, 72, BUTTON_TOY_FIRE);
    assert_eq!((count(&t, FIRE), count(&t, DRY), count(&t, RELOAD)), ((2, 0), (1, 0), (1, 0)));
    akimbo(&mut sv, c, 100, 0);
    sv.core.cvars.set("addon_akimbo_autoreload", "0");
    let t = akimbo(&mut sv, c, 144, BUTTON_TOY_FIRE);
    assert_eq!((count(&t, FIRE), count(&t, RELOAD)), ((2, 0), (0, 0)));
    assert!(count(&t, DRY).0 >= 2);
}

#[test]
fn putting_the_pistols_away_or_dying_interrupts_the_reload() {
    let Some((mut sv, c)) = start() else { return };
    akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE);
    akimbo(&mut sv, c, 5, 0);
    akimbo(&mut sv, c, 5, BUTTON_TOY_RELOAD);
    let t = run(&mut sv, c, 5, Toy::Railgun, 0);
    assert_eq!(count(&t, CANCEL), (1, 0));
    let t = akimbo(&mut sv, c, 150, 0);
    assert_eq!(count(&t, DONE), (0, 0), "the reload is lost");
    akimbo(&mut sv, c, 5, BUTTON_TOY_RELOAD);
    let health = sv.core.sv.fields.health;
    sv.level.as_mut().unwrap().vm.set_ent_f(1, health, 0.0);
    let t = akimbo(&mut sv, c, 5, BUTTON_TOY_FIRE);
    assert_eq!((count(&t, CANCEL), count(&t, FIRE)), ((1, 0), (0, 0)));
}

#[test]
fn shots_hurt_and_the_rounds_are_saved() {
    let Some((mut sv, c)) = start() else { return };
    let (e, org, hp) = {
        let vm = &sv.level.as_ref().unwrap().vm;
        let f = &sv.core.sv.fields;
        let e = (1..vm.num_edicts() as u32).find(|&e| vm.ent_str(e, f.classname).ok() == Some(b"monster_army")).unwrap();
        (e, vm.ent_v(e, f.origin), vm.ent_f(e, f.health))
    };
    // stand beside it, on whichever side is open, and shoot
    let mut hit = 0.0;
    for k in 0..8 {
        let yaw = k as f32 * 45.0;
        let (s, co) = yaw.to_radians().sin_cos();
        sv.command(&format!("setpos {} {} {}", org[0] - co * 60.0, org[1] - s * 60.0, org[2]));
        let mv = |b| ClientMove { angles: [0.0, yaw, 0.0], buttons: Toy::Akimbo.bits() | b, ..Default::default() };
        for b in [0, 0, BUTTON_TOY_FIRE] {
            sv.set_move(c, mv(b));
            sv.frame(1.0 / 72.0);
        }
        hit = hp - sv.level.as_ref().unwrap().vm.ent_f(e, sv.core.sv.fields.health);
        if hit > 0.0 {
            break;
        }
        akimbo(&mut sv, c, 10, 0);
    }
    assert_eq!(hit, 14.0);
    akimbo(&mut sv, c, 100, 0);
    akimbo(&mut sv, c, 1, BUTTON_TOY_RELOAD);
    akimbo(&mut sv, c, 150, 0);
    akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE);
    let text = sv.save_game();
    assert!(text.contains("\nakimbo 0 11 12\n"), "{}", &text[..400]);
    sv.load_game(&text).unwrap();
    let c = sv.connect_local().unwrap_or(c);
    akimbo(&mut sv, c, 30, 0);
    assert!(sv.save_game().contains("\nakimbo 0 11 12\n"));
}

#[test]
fn switched_off_it_does_nothing() {
    let Some((mut sv, c)) = start() else { return };
    akimbo(&mut sv, c, 1, BUTTON_TOY_FIRE);
    sv.core.cvars.set("addon_akimbo", "0");
    let t = akimbo(&mut sv, c, 30, BUTTON_TOY_FIRE);
    assert!(!t.contains("//qsp_gun") && t.contains("//qsp_mag 0 9\n"), "{t}");
}
