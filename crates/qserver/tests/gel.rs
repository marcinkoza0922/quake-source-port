//! Gel sprayer and powerwasher add-ons against the real id1 e1m1 (skip
//! without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qserver::addons::gel::{BLUE, ORANGE};
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

fn held(t: Toy, buttons: u8, pitch: f32) -> ClientMove {
    aimed(t, buttons, pitch, 90.0)
}

fn aimed(t: Toy, buttons: u8, pitch: f32, yaw: f32) -> ClientMove {
    ClientMove { angles: [pitch, yaw, 0.0], buttons: t.id() << TOY_SHIFT | buttons, ..Default::default() }
}

fn velocity(sv: &Server) -> Vec3 {
    Vec3::from(sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.velocity))
}

#[test]
fn spraying_coats_the_floor_saves_and_washes_off() {
    let Some((mut sv, c)) = start() else { return };
    run(&mut sv, c, 30, ClientMove::default());
    sv.command("setpos 480 200 24");
    run(&mut sv, c, 30, held(Toy::GelSprayer, BUTTON_TOY_FIRE, 60.0));
    run(&mut sv, c, 30, aimed(Toy::GelSprayer, BUTTON_TOY_ALT, 60.0, 270.0));
    run(&mut sv, c, 60, held(Toy::GelSprayer, 0, 0.0));
    let splats = sv.level.as_ref().unwrap().gel.splats.clone();
    assert!(splats.iter().any(|s| s.kind == BLUE) && splats.iter().any(|s| s.kind == ORANGE), "{splats:?}");
    assert!(splats.iter().all(|s| s.normal.z > 0.7), "floor only: {splats:?}");

    let text = sv.save_game();
    let (mut other, _) = start().unwrap();
    other.load_game(&text).unwrap();
    assert_eq!(other.level.as_ref().unwrap().gel.splats.len(), splats.len());

    // the powerwasher's jet cleans the floor it lands on
    run(&mut sv, c, 120, held(Toy::Washer, BUTTON_TOY_FIRE, 60.0));
    run(&mut sv, c, 120, aimed(Toy::Washer, BUTTON_TOY_FIRE, 60.0, 270.0));
    let left = sv.level.as_ref().unwrap().gel.splats.len();
    assert!(left < splats.len(), "{left} of {}", splats.len());
}

#[test]
fn blue_gel_bounces_a_fall_and_orange_gel_speeds_up_a_run() {
    let Some((mut sv, c)) = start() else { return };
    run(&mut sv, c, 30, ClientMove::default());
    sv.command("setpos 480 200 24");
    run(&mut sv, c, 30, ClientMove::default());
    let floor = Vec3::from(sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin)).z - 24.0;
    sv.level.as_mut().unwrap().gel.add(BLUE, Vec3::new(480.0, 200.0, floor), Vec3::new(0.0, 0.0, 1.0), 48.0);
    sv.command("setpos 480 200 120");
    let mut up = 0.0f32;
    for _ in 0..80 {
        run(&mut sv, c, 1, ClientMove::default());
        up = up.max(velocity(&sv).z);
    }
    assert!(up > 400.0, "bounce {up}");

    // plain run speed is capped near 320; on orange it climbs far past it
    let (mut sv, c) = start().unwrap();
    run(&mut sv, c, 30, ClientMove::default());
    sv.command("setpos 480 200 24");
    run(&mut sv, c, 30, ClientMove::default());
    for x in -2..=2 {
        for y in -2..=2 {
            let p = Vec3::new(480.0 + x as f32 * 50.0, 200.0 + y as f32 * 50.0, floor);
            sv.level.as_mut().unwrap().gel.add(ORANGE, p, Vec3::new(0.0, 0.0, 1.0), 64.0);
        }
    }
    let mut fast = 0.0f32;
    for _ in 0..20 {
        run(&mut sv, c, 1, ClientMove { angles: [0.0, 90.0, 0.0], forward: 400.0, ..Default::default() });
        let v = velocity(&sv);
        fast = fast.max((v.x * v.x + v.y * v.y).sqrt());
    }
    assert!(fast > 480.0, "orange speed {fast}");
}
