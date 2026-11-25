//! Male-to-male cable add-on against the real id1 e1m1 (skips without game
//! data): plug into an electronics panel, fire the bolt, get held back by
//! the tether, yank it out.

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qserver::addons::cable::{REACH, State};
use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, Toy};
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
    qserver::addons::enable_all(&mut sv.core.cvars);
    for c in cvars {
        sv.command(c);
    }
    sv.core.set_custom(true);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    Some((sv, c))
}

/// Runs frames; returns all stufftext.
fn run(sv: &mut Server, c: usize, frames: usize, mv: ClientMove) -> String {
    let mut text = String::new();
    for _ in 0..frames {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        text += &String::from_utf8_lossy(&sv.take_reliable(c).concat());
        for d in sv.take_datagrams(c) {
            text += &String::from_utf8_lossy(&d);
        }
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
    text
}

fn origin(sv: &Server) -> Vec3 {
    Vec3::from(sv.level.as_ref().unwrap().vm.ent_v(1, sv.core.sv.fields.origin))
}

fn eye(sv: &Server) -> Vec3 {
    let l = sv.level.as_ref().unwrap();
    origin(sv) + Vec3::from(l.vm.ent_v(1, sv.core.sv.fields.view_ofs))
}

fn plugged(sv: &Server) -> bool {
    matches!(sv.level.as_ref().unwrap().cables.state.first(), Some(State::Plugged(_)))
}

fn cable(angles: [f32; 3], buttons: u16) -> ClientMove {
    ClientMove { angles, buttons: Toy::Cable.bits() | buttons, ..Default::default() }
}

/// View angles (pitch down positive) looking from `from` at `to`.
fn aim(from: Vec3, to: Vec3) -> [f32; 3] {
    let d = to - from;
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = -d.z.atan2((d.x * d.x + d.y * d.y).sqrt()).to_degrees();
    [pitch, yaw, 0.0]
}

/// Stand in front of an electronics panel on a wall within reach; returns
/// its centre and the angles that look at it.
fn face_panel(sv: &mut Server, c: usize) -> Option<(Vec3, [f32; 3])> {
    run(sv, c, 20, ClientMove::default());
    let panels: Vec<(Vec3, Vec3)> = {
        let l = sv.level.as_ref().unwrap();
        l.hooks
            .surfaces
            .faces(0)
            .iter()
            .filter(|f| f.electronic && f.normal.z.abs() < 0.1 && f.verts.len() >= 3)
            .map(|f| (f.verts.iter().fold(Vec3::ZERO, |a, &v| a + v) * (1.0 / f.verts.len() as f32), f.normal))
            .collect()
    };
    for (p, n) in panels {
        let stand = p + n * 48.0;
        sv.command(&format!("setpos {} {} {}", stand.x, stand.y, stand.z));
        run(sv, c, 40, ClientMove::default());
        let e = eye(sv);
        let a = aim(e, p);
        let tr = sv.traceline(e, e + (p - e) * 1.05, true, 1)?;
        if (p - e).length() < REACH - 8.0 && (tr.endpos - p).length() < 2.0 {
            return Some((p, a));
        }
    }
    None
}

#[test]
fn plugs_into_electronics_fires_and_is_held_by_the_tether() {
    let Some((mut sv, c)) = start(&["addon_cable_length 160"]) else { return };
    let (socket, a) = face_panel(&mut sv, c).expect("an electronics panel within reach in e1m1");
    run(&mut sv, c, 2, cable(a, 0));
    let text = run(&mut sv, c, 1, cable(a, BUTTON_TOY_ALT));
    assert!(plugged(&sv), "did not plug in at {socket:?}");
    assert!(text.contains("//qsp_cable 0 1 0 "), "{text}");
    // the bolt: fire flag and the stock sounds' entity updates
    let text = run(&mut sv, c, 20, cable(a, BUTTON_TOY_FIRE));
    assert!(text.contains("//qsp_cable 0 1 1 "));
    // walk straight away for three seconds: the tether stops the player
    let away = [0.0, a[1] + 180.0, 0.0];
    run(&mut sv, c, 216, ClientMove { forward: 320.0, ..cable(away, 0) });
    assert!(plugged(&sv));
    let d = (origin(&sv) - socket).length();
    assert!(d < 160.0 + 24.0, "got {d} from the socket");
    assert!(d > 100.0, "barely moved: {d}");
    // alt-fire yanks it out
    run(&mut sv, c, 2, cable(away, 0));
    let text = run(&mut sv, c, 1, cable(away, BUTTON_TOY_ALT));
    assert!(!plugged(&sv));
    assert!(text.contains("//qsp_cable 0 0 "), "{text}");
}

#[test]
fn switching_weapons_unplugs_and_dead_surfaces_refuse() {
    let Some((mut sv, c)) = start(&[]) else { return };
    let (_, a) = face_panel(&mut sv, c).expect("panel");
    run(&mut sv, c, 2, cable(a, 0));
    run(&mut sv, c, 1, cable(a, BUTTON_TOY_ALT));
    assert!(plugged(&sv));
    run(&mut sv, c, 2, ClientMove { angles: a, ..Default::default() });
    assert!(!plugged(&sv));
    // the floor is not electronics
    let down = [80.0, a[1], 0.0];
    run(&mut sv, c, 2, cable(down, 0));
    let text = run(&mut sv, c, 1, cable(down, BUTTON_TOY_ALT));
    assert!(!plugged(&sv));
    assert!(text.contains("//qsp_cable 0 2 "), "{text}");
}

#[test]
fn the_plug_goes_when_add_ons_do() {
    let Some((mut sv, c)) = start(&[]) else { return };
    let (_, a) = face_panel(&mut sv, c).expect("an electronics panel within reach in e1m1");
    run(&mut sv, c, 2, cable(a, 0));
    run(&mut sv, c, 1, cable(a, BUTTON_TOY_ALT));
    assert!(plugged(&sv));
    sv.core.set_custom(false);
    let text = run(&mut sv, c, 3, cable(a, 0));
    assert!(!plugged(&sv));
    assert_eq!(text.matches("//qsp_cable 0 0 ").count(), 1, "told once: {text}");
}
