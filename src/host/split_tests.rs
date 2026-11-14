//! Split-screen through a real headless host: pads join, drive their own
//! player, and Start opens the shared menu.

use super::*;
use qinput::{GamepadAxis, GamepadButton};

fn host() -> Option<Host> {
    let mut base = qcore::fs::Fs::default_basedir();
    if base.is_relative() && !base.exists() {
        base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(base);
    }
    let mut fs = qcore::fs::Fs::new(&base);
    if !fs.add_game_dir("id1") {
        eprintln!("skip: no game data");
        return None;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok())?;
    let Ok(r) = Renderer::headless(320, 240, &pal) else {
        eprintln!("skip: no Vulkan device");
        return None;
    };
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    for c in ["sv_autosave 0", "maxplayers 4", "deathmatch 1", "map dm3"] {
        h.exec_line(c);
    }
    run(&mut h, 30, Vec::new);
    Some(h)
}

fn run(h: &mut Host, frames: usize, ev: impl Fn() -> Vec<HostEvent>) {
    for _ in 0..frames {
        h.frame(1.0 / 72.0, ev());
    }
}

fn button(id: u32, button: GamepadButton, down: bool) -> HostEvent {
    HostEvent::Pad(id, InputEvent::GamepadButton { button, down })
}

fn origin(cl: &ClientState) -> [f32; 3] {
    cl.entities[cl.viewentity].origin
}

#[test]
fn pads_join_and_play_their_own_player() {
    let Some(mut h) = host() else { return };
    assert_eq!(h.cl.signon, 4);
    h.exec_line("r_teleportwindows 0");
    assert!(h.cull_fte_for_view());
    h.exec_line("r_teleportwindows 1");
    assert!(!h.cull_fte_for_view());
    h.exec_line("r_teleportwindows 0");
    h.menu.open(Screen::None);
    h.sync_key_dest();
    h.exec_line("bind ABUTTON +jump");

    // The first pad used is player 1's; it doesn't join.
    run(&mut h, 1, || vec![button(7, GamepadButton::South, true)]);
    run(&mut h, 1, || vec![button(7, GamepadButton::South, false)]);
    assert_eq!(h.p1_pad, Some(7));
    assert!(h.guests.is_empty());

    // Sticks of a free pad do nothing; Start joins.
    run(&mut h, 1, || vec![HostEvent::Pad(9, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: -1.0 })]);
    assert!(h.guests.is_empty());
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, true)]);
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, false)]);
    assert_eq!(h.guests.len(), 1);
    assert!(!h.cull_fte_for_view(), "guests share particle geometry");
    assert_eq!(h.guests[0].pad, Some(9));
    assert!(!h.menu.active(), "joining doesn't open the menu");
    run(&mut h, 30, Vec::new);
    assert_eq!(h.guests[0].cl.signon, 4);

    // The guest's stick moves the guest, not player 1.
    let (p0, g0) = (origin(&h.cl), origin(&h.guests[0].cl));
    run(&mut h, 60, || vec![HostEvent::Pad(9, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: -1.0 })]);
    let (p1, g1) = (origin(&h.cl), origin(&h.guests[0].cl));
    let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    assert!(d(g0, g1) > 32.0, "guest moved {g0:?} -> {g1:?}");
    assert!(d(p0, p1) < 1.0, "player 1 stayed {p0:?} -> {p1:?}");

    // Binds act on the pressing player's buttons only.
    run(&mut h, 1, || vec![button(9, GamepadButton::South, true)]);
    assert!(h.guests[0].input.buttons[qinput::btn::JUMP].is_down());
    assert!(!h.input.buttons[qinput::btn::JUMP].is_down());
    run(&mut h, 1, || vec![button(9, GamepadButton::South, false)]);
    assert!(!h.guests[0].input.buttons[qinput::btn::JUMP].is_down());

    // Start on a guest pad opens the shared menu.
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, true)]);
    assert!(h.menu.active());

    // A removed pad leaves its player padless; a new pad takes it over.
    h.handle_event(HostEvent::PadRemoved(9));
    assert_eq!(h.guests[0].pad, None);
    h.menu.open(Screen::None);
    h.sync_key_dest();
    run(&mut h, 1, || vec![button(11, GamepadButton::Start, true)]);
    assert_eq!(h.guests.len(), 1);
    assert_eq!(h.guests[0].pad, Some(11));
}
