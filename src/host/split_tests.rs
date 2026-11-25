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

fn axis(id: u32, axis: GamepadAxis, value: f32) -> HostEvent {
    HostEvent::Pad(id, InputEvent::GamepadAxis { axis, value })
}

/// The shared menu: nothing a guest pressed there sticks, for either player.
#[test]
fn a_guest_in_the_menu_leaves_nothing_held() {
    let Some(mut h) = host() else { return };
    h.menu.open(Screen::None);
    h.sync_key_dest();
    run(&mut h, 1, || vec![button(7, GamepadButton::South, true), button(7, GamepadButton::South, false)]);
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, true), button(9, GamepadButton::Start, false)]);
    assert_eq!(h.guests.len(), 1);
    h.exec_line("bind RTRIGGER +attack");
    run(&mut h, 2, || vec![axis(9, GamepadAxis::RightTrigger, 1.0), axis(9, GamepadAxis::LeftY, -1.0)]);
    let attack = |h: &Host| h.guests[0].input.buttons[qinput::btn::ATTACK].is_down();
    assert!(attack(&h));
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, true)]);
    assert!(h.menu.active());
    // let go in the menu, and press a direction there
    run(&mut h, 1, || vec![
        button(9, GamepadButton::Start, false),
        axis(9, GamepadAxis::RightTrigger, 0.0),
        axis(9, GamepadAxis::LeftY, 0.0),
        button(9, GamepadButton::DpadDown, true),
    ]);
    assert!(!attack(&h));
    assert!(h.input.key_down(qinput::keys::K_DPAD_DOWN));
    run(&mut h, 1, || vec![button(9, GamepadButton::DpadDown, false), axis(9, GamepadAxis::LeftX, 1.0)]);
    assert!(!h.input.key_down(qinput::keys::K_DPAD_DOWN), "the menu would scroll on");
    // a trigger pulled in the menu doesn't fire
    run(&mut h, 1, || vec![axis(9, GamepadAxis::RightTrigger, 1.0)]);
    assert!(!attack(&h));
    h.menu.open(Screen::None);
    h.sync_key_dest();
    assert_eq!([h.input.axis(GamepadAxis::LeftX), h.input.axis(GamepadAxis::RightTrigger)], [0.0; 2], "player 1's pad is at rest");
    assert_eq!(h.guests[0].input.axis(GamepadAxis::LeftY), 0.0);
}

#[test]
fn another_pad_going_away_leaves_player_ones_sticks() {
    let Some(mut h) = host() else { return };
    h.menu.open(Screen::None);
    h.sync_key_dest();
    let add = |id| HostEvent::PadFamily(id, 0);
    run(&mut h, 1, || vec![add(7), add(9)]);
    run(&mut h, 1, || vec![HostEvent::Input(InputEvent::GamepadConnected(true)), axis(7, GamepadAxis::LeftY, -1.0)]);
    run(&mut h, 1, || vec![HostEvent::PadRemoved(9)]);
    assert!(h.input.gamepad_connected && h.input.axis(GamepadAxis::LeftY) == -1.0);
    run(&mut h, 1, || vec![HostEvent::PadRemoved(7)]);
    assert!(!h.input.gamepad_connected && h.input.axis(GamepadAxis::LeftY) == 0.0);
}

/// Keys let go over the fullscreen automap are let go.
#[test]
fn the_automap_passes_releases_on() {
    let Some(mut h) = host() else { return };
    h.menu.open(Screen::None);
    h.con.open = false;
    h.sync_key_dest();
    let key = |key: i32, down| HostEvent::Input(InputEvent::Key { key, down });
    // (deathmatch has no automap)
    for c in ["disconnect", "deathmatch 0", "coop 1", "map e1m1"] {
        h.exec_line(c);
    }
    run(&mut h, 60, Vec::new);
    h.menu.open(Screen::None);
    h.sync_key_dest();
    h.exec_line("bind w +forward");
    h.exec_line("bind m automap");
    run(&mut h, 1, || vec![key(b'w' as i32, true)]);
    let fwd = |h: &Host| h.input.buttons[qinput::btn::FORWARD].is_down();
    assert!(fwd(&h));
    run(&mut h, 1, || vec![key(b'm' as i32, true)]);
    run(&mut h, 1, || vec![key(b'm' as i32, false), key(b'w' as i32, false)]);
    assert!(h.automap.fullscreen);
    assert!(!fwd(&h));
    // closed with Escape, the next press of the map key counts
    run(&mut h, 1, || vec![key(qinput::keys::K_ESCAPE, true), key(qinput::keys::K_ESCAPE, false)]);
    assert!(!h.automap.fullscreen);
    run(&mut h, 1, || vec![key(b'm' as i32, true)]);
    run(&mut h, 1, || vec![key(b'm' as i32, false)]);
    assert!(h.automap.fullscreen);
}

#[test]
fn a_guests_binds_and_stufftext_are_the_guests() {
    let Some(mut h) = host() else { return };
    h.menu.open(Screen::None);
    h.sync_key_dest();
    // (a guest takes the binds when it joins)
    h.exec_line("bind RTHUMB centerview");
    h.exec_line("bind ABUTTON kill");
    h.exec_line("bind BBUTTON togglezoom");
    run(&mut h, 1, || vec![button(7, GamepadButton::West, true)]);
    run(&mut h, 1, || vec![button(7, GamepadButton::West, false)]);
    run(&mut h, 1, || vec![button(9, GamepadButton::Start, true)]);
    run(&mut h, 30, || vec![button(9, GamepadButton::Start, false)]);
    assert_eq!(h.guests.len(), 1);
    (h.cl.viewangles[0], h.guests[0].cl.viewangles[0]) = (30.0, 40.0);
    let press = |h: &mut Host, b| {
        h.handle_event(button(9, b, true));
        h.handle_event(button(9, b, false));
    };
    press(&mut h, GamepadButton::RightStick);
    assert_eq!((h.cl.viewangles[0], h.guests[0].cl.viewangles[0]), (30.0, 0.0));
    press(&mut h, GamepadButton::East);
    assert_eq!(h.zoom.1, 0.0, "player 1's zoom");
    press(&mut h, GamepadButton::South);
    run(&mut h, 5, Vec::new);
    // (id1's ClientKill takes two frags and respawns)
    let frags: Vec<i32> = [h.local.unwrap(), h.guests[0].slot].map(|s| h.cl.scores[s].frags).to_vec();
    assert_eq!(frags, [0, -2], "the guest killed itself");

    // QC stuffs the item flash to the player who took it
    let slot = h.guests[0].slot;
    h.server.core.clients[slot].message.byte(i32::from(qnet::protocol::SVC_STUFFTEXT));
    h.server.core.clients[slot].message.string(b"bf\n");
    h.guests[0].view_events.clear();
    h.server.frame(1.0 / 72.0);
    let mut g = h.guests.remove(0);
    g.feed(&mut h.server);
    assert!(g.view_events.iter().any(|e| matches!(e, ClientEvent::BonusFlash)));
}
