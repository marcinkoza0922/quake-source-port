//! Exercise the menu through platform events, including stick-emulated arrows.
use super::super::*;
use qinput::{GamepadAxis, GamepadButton};

fn press(h: &mut Host, button: GamepadButton) {
    for down in [true, false] {
        h.handle_event(HostEvent::Pad(7, InputEvent::GamepadButton { button, down }));
    }
    h.sync_menu_ui();
}

fn capture(h: &mut Host, name: &str) {
    let Some(dir) = std::env::var_os("QUAKE_MENU_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    h.r.request_capture();
    h.render(None, None, 0.0);
    let (w, height) = h.r.size();
    let pixels = h.r.read_pixels().unwrap();
    std::fs::write(dir.join(format!("{name}.png")), crate::png::encode_rgba(w, height, &pixels)).unwrap();
}

#[test]
fn addon_controller_events_navigate_edit_and_switch_prompts() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") { return; }
    let Some(pal) = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()) else { return };
    let Ok(renderer) = Renderer::headless(1280, 720, &pal) else { return };
    let mut h = Host::new(fs, renderer, None, Vec::new());
    h.headless = true;
    h.exec_line("menu_options addons");
    h.sync_key_dest();
    h.handle_event(HostEvent::Input(InputEvent::GamepadConnected(true)));
    let old = h.cv("addon_portalgun");
    press(&mut h, GamepadButton::South);
    assert_ne!(h.cv("addon_portalgun"), old);
    assert!(h.menu.pad_ui);
    capture(&mut h, "addons-controller");
    press(&mut h, GamepadButton::RightShoulder);
    press(&mut h, GamepadButton::RightShoulder);
    let slot = h.cv("addon_portalgun_slot");
    for value in [1.0, 0.0] {
        h.handle_event(HostEvent::Pad(7, InputEvent::GamepadAxis { axis: GamepadAxis::LeftX, value }));
    }
    assert_ne!(h.cv("addon_portalgun_slot"), slot);
    for _ in 0..3 { press(&mut h, GamepadButton::DpadDown); }
    capture(&mut h, "addons-controller-settings");
    press(&mut h, GamepadButton::South);
    assert_eq!(h.menu.screen, Screen::OptionsPage(crate::menu::Page::PortalGun));
    press(&mut h, GamepadButton::East);
    assert_eq!(h.menu.screen, Screen::OptionsPage(crate::menu::Page::Addons));
    for _ in 0..2 { press(&mut h, GamepadButton::DpadUp); }
    h.input.set_binding(qinput::keys::K_BBUTTON, Some("+jump"));
    h.sync_menu_ui();
    press(&mut h, GamepadButton::South);
    assert!(h.menu.capturing());
    capture(&mut h, "addons-controller-binding");
    press(&mut h, GamepadButton::East);
    assert!(h.menu.capturing()); // Occupied binding awaits confirmation.
    capture(&mut h, "addons-controller-binding-conflict");
    press(&mut h, GamepadButton::East);
    assert!(!h.menu.capturing());
    h.handle_event(HostEvent::Input(InputEvent::Key { key: b'k' as i32, down: true }));
    h.sync_menu_ui();
    assert!(!h.menu.pad_ui);
    capture(&mut h, "addons-keyboard-after-controller");
    h.handle_event(HostEvent::Input(InputEvent::Key { key: qinput::keys::K_ENTER, down: true }));
    h.sync_menu_ui();
    assert!(h.menu.capturing());
    capture(&mut h, "addons-keyboard-binding");
    h.handle_event(HostEvent::Input(InputEvent::Key { key: qinput::keys::K_ESCAPE, down: true }));
    h.sync_menu_ui();
    h.exec_line("menu_options newgame");
    press(&mut h, GamepadButton::RightShoulder);
    capture(&mut h, "addons-newgame-controller");
    press(&mut h, GamepadButton::DpadUp);
    press(&mut h, GamepadButton::DpadDown);
    capture(&mut h, "addons-newgame-controller-start");
    press(&mut h, GamepadButton::South);
    assert_eq!(h.menu.screen, Screen::None);
}
