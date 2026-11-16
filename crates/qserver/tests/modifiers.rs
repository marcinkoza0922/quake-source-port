//! Gameplay modifiers against the real id1 progs (skips without game data).

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

/// e1m1 with one local client in a customized game; `cvars` are set
/// before spawning.
fn start(cvars: &[&str]) -> Option<(Server, usize)> {
    start_on("e1m1", true, cvars)
}

fn start_on(map: &str, custom: bool, cvars: &[&str]) -> Option<(Server, usize)> {
    let base = basedir()?;
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    for c in cvars {
        sv.command(c);
    }
    sv.core.set_custom(custom);
    sv.spawn_server(map).ok()?;
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
    let Some((mut sv, c)) = start(&["modifier_infiniteammo 1"]) else { return };
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
    let Some(full) = rocket_self_damage("modifier_damagetaken 1") else { return };
    let quarter = rocket_self_damage("modifier_damagetaken 0.25").unwrap();
    assert!(full > 0.0, "rocket did no damage");
    assert!(quarter > 0.0 && quarter < full * 0.5, "full {full}, quarter {quarter}");
}

#[test]
fn triple_jump() {
    let Some((mut sv, c)) = start(&["modifier_jumps 3", "sv_gravity 2400"]) else { return };
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
    let Some(lost) = rocket_self_damage("modifier_noselfdamage 1") else { return };
    assert_eq!(lost, 0.0);
    // knockback still applies: a rocket at the feet launches the player
    let (mut sv, c) = start(&["modifier_noselfdamage 1"]).unwrap();
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

#[test]
fn vanilla_game_ignores_modifiers() {
    let Some((mut sv, c)) = start_on("e1m1", false, &["modifier_infiniteammo 1"]) else { return };
    run(&mut sv, c, 30, idle());
    let before = field(&sv, |f| f.ammo_shells);
    run(&mut sv, c, 150, ClientMove { buttons: 1, ..Default::default() });
    assert!(field(&sv, |f| f.ammo_shells) < before, "a vanilla game must not apply modifiers");
}

fn monsters(sv: &Server) -> usize {
    let l = sv.level.as_ref().unwrap();
    let flags = sv.core.sv.fields.flags;
    (1..l.vm.num_edicts() as u32).filter(|&e| !l.vm.is_free(e) && (l.vm.ent_f(e, flags) as i32) & 32 != 0).count()
}

#[test]
fn extra_monsters_multiply_in_custom_games_only() {
    let Some((sv, _)) = start_on("e1m1", false, &["modifier_horde 1"]) else { return };
    let plain = monsters(&sv);
    let (sv, _) = start_on("e1m1", true, &["modifier_horde 1"]).unwrap();
    assert!(plain > 0);
    assert_eq!(monsters(&sv), plain * 2);
}

#[test]
fn pistol_start_resets_items_on_changelevel() {
    for (pistol, keeps) in [("0", true), ("1", false)] {
        let Some((mut sv, c)) = start(&[&format!("modifier_pistolstart {pistol}")]) else { return };
        run(&mut sv, c, 30, idle());
        run(&mut sv, c, 2, ClientMove { impulse: 9, ..Default::default() });
        run(&mut sv, c, 2, idle());
        sv.command("changelevel e1m2");
        // id1 parm6 carries the rockets into the next level
        let rockets = sv.core.clients[c].spawn_parms[5];
        assert_eq!(rockets > 0.0, keeps, "pistol start {pistol}: rockets {rockets}");
    }
}

#[test]
fn health_pickups_scale() {
    // simulate a 15 hp pickup by raising health between frames
    let Some((mut sv, c)) = start(&["modifier_health 2"]) else { return };
    run(&mut sv, c, 30, idle());
    let l = sv.level.as_mut().unwrap();
    let hf = sv.core.sv.fields.health;
    l.vm.set_ent_f(1, hf, 10.0);
    run(&mut sv, c, 2, idle());
    // +15 is doubled to +30
    let l = sv.level.as_mut().unwrap();
    l.vm.set_ent_f(1, hf, 25.0);
    run(&mut sv, c, 2, idle());
    assert_eq!(field(&sv, |f| f.health), 40.0);
}

#[test]
fn modifiers_are_saved_with_a_customized_game_only() {
    let Some((mut sv, c)) = start(&["modifier_damagetaken 2", "modifier_pistolstart 1"]) else { return };
    run(&mut sv, c, 10, idle());
    let text = sv.save_game();
    assert!(text.starts_with("QSPADDONS "), "customized saves are .qsa");
    assert!(text.contains("\nmodifier modifier_damagetaken 2\n"), "{}", &text[..300]);
    // the user changes the rules elsewhere; loading restores this game's
    sv.command("modifier_damagetaken 0.5");
    sv.command("modifier_pistolstart 0");
    sv.load_game(&text).unwrap();
    assert!(sv.custom());
    assert_eq!(sv.core.cvars.value_of("modifier_damagetaken"), 2.0);
    assert_eq!(sv.core.cvars.value_of("modifier_pistolstart"), 1.0);

    // a vanilla game writes a plain v5 save, whatever the modifier cvars say
    let (mut sv, c) = start_on("e1m1", false, &["modifier_damagetaken 2"]).unwrap();
    run(&mut sv, c, 10, idle());
    let text = sv.save_game();
    assert!(text.starts_with("5\n"));
    assert!(!text.contains("modifier"));
}
