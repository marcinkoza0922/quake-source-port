//! Portal gun add-on against the real id1 e1m1 (skips without game data).

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
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

fn origin(sv: &Server) -> [f32; 3] {
    let l = sv.level.as_ref().unwrap();
    l.vm.ent_v(1, sv.core.sv.fields.origin)
}

fn gun(angles: [f32; 3], buttons: u8) -> ClientMove {
    ClientMove { angles, buttons: Toy::PortalGun.id() << TOY_SHIFT | buttons, ..Default::default() }
}

/// Fire down at the floor (blue) and along the start corridor (orange).
fn place_pair(sv: &mut Server, c: usize) {
    run(sv, c, 30, ClientMove::default());
    // the long flat floor of the start corridor
    sv.command("setpos 480 200 24");
    run(sv, c, 10, ClientMove::default());
    let (down, ahead) = ([89.0, 90.0, 0.0], [-8.0, 90.0, 0.0]);
    run(sv, c, 2, gun(ahead, 0));
    run(sv, c, 2, gun(ahead, BUTTON_TOY_ALT));
    run(sv, c, 2, gun(down, 0));
    run(sv, c, 1, gun(down, BUTTON_TOY_FIRE));
}

#[test]
fn floor_portal_drops_the_player_out_of_the_far_one() {
    let Some((mut sv, c)) = start(&["addons_enable 1"]) else { return };
    let before = origin(&sv);
    place_pair(&mut sv, c);
    let pair = sv.level.as_ref().unwrap().portals.pairs[0];
    let (Some(floor), Some(far)) = (pair[0], pair[1]) else { panic!("portals not placed: {pair:?}") };
    assert!(floor.normal.z > 0.99, "{floor:?}");
    run(&mut sv, c, 10, gun([0.0, 90.0, 0.0], 0));
    let after = origin(&sv);
    let d = |a: [f32; 3], b: qcore::math::Vec3| ((a[0] - b.x).powi(2) + (a[1] - b.y).powi(2)).sqrt();
    assert!(d(after, far.origin) < 120.0, "player at {after:?}, far portal {:?}, start {before:?}", far.origin);
}

#[test]
fn add_ons_off_ignores_the_gun() {
    let Some((mut sv, c)) = start(&[]) else { return };
    place_pair(&mut sv, c);
    assert!(!sv.level.as_ref().unwrap().portals.any_linked());
}

