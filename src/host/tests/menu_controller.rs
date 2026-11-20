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

#[test]
fn hud_editor_controller_events_and_button_glyphs() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") { return; }
    let Some(pal) = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()) else { return };
    let Ok(renderer) = Renderer::headless(1280, 720, &pal) else { return };
    let mut h = Host::new(fs, renderer, None, Vec::new());
    h.headless = true;
    h.exec_line("menu_hudedit");
    h.sync_key_dest();
    h.handle_event(HostEvent::Input(InputEvent::GamepadConnected(true)));
    assert_eq!(h.menu.hud_edit.sel, None);
    press(&mut h, GamepadButton::DpadDown);
    assert!(h.menu.pad_ui);
    assert_eq!(h.menu.hud_edit.sel, Some(0));
    press(&mut h, GamepadButton::DpadDown);
    assert_eq!(h.menu.hud_edit.sel, Some(1));
    capture(&mut h, "hudedit-pad-xbox");
    // PlayStation glyphs fit on the buttons too
    h.exec_line("joy_glyphs 2");
    h.sync_menu_ui();
    capture(&mut h, "hudedit-pad-ps");
    h.exec_line("joy_glyphs 1");
    h.sync_menu_ui();
    // A enters Move mode; capture; B cancels
    press(&mut h, GamepadButton::South);
    assert!(h.menu.hud_edit.moving());
    capture(&mut h, "hudedit-pad-moving");
    press(&mut h, GamepadButton::East);
    assert!(!h.menu.hud_edit.moving());
    // RB collapses; capture; RB expands
    press(&mut h, GamepadButton::RightShoulder);
    capture(&mut h, "hudedit-pad-collapsed");
    press(&mut h, GamepadButton::RightShoulder);
    // Start exits
    press(&mut h, GamepadButton::Start);
    assert_eq!(h.menu.screen, Screen::OptionsPage(crate::menu::Page::Hud));
}

#[test]
fn clickable_buttons_render_controller_glyphs_across_menus() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") { return; }
    let Some(pal) = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()) else { return };
    let Ok(renderer) = Renderer::headless(1280, 720, &pal) else { return };
    let mut h = Host::new(fs, renderer, None, Vec::new());
    h.headless = true;
    h.handle_event(HostEvent::Input(InputEvent::GamepadConnected(true)));

    // Options hub: [B] Back button + Y opens OSK with [Y] Shift, [X] Del, [B] Done
    h.exec_line("menu_options");
    h.sync_key_dest();
    press(&mut h, GamepadButton::DpadDown);
    assert!(h.menu.pad_ui);
    capture(&mut h, "btn-options-pad");
    press(&mut h, GamepadButton::North);
    capture(&mut h, "btn-osk-pad");
    press(&mut h, GamepadButton::East);

    // Quit prompt: Yes and [B] No
    h.menu.open(Screen::Quit);
    h.sync_key_dest();
    capture(&mut h, "btn-quit-pad");

    // Demos screen: populate Saved + Automatic demos so every button & tab is enabled
    let demo = |name: &str, auto: bool| crate::demo::library::DemoFile {
        path: std::path::PathBuf::from(format!("/tmp/{name}.dem")),
        name: name.into(),
        auto,
        bytes: 4096,
        modified: 1_700_000_000,
        map: Some("e1m1".into()),
        duration: Some(95.0),
    };
    h.menu.demos.set(vec![demo("speedrun", false)], vec![demo("20240101-120000_e1m1", true)], true);
    h.menu.open(Screen::Demos);
    h.sync_key_dest();
    // Switch to Automatic tab so Keep is enabled too
    press(&mut h, GamepadButton::RightShoulder);
    assert_eq!(h.menu.demos.tab, 1);
    capture(&mut h, "btn-demos-pad-xbox");
    h.exec_line("joy_glyphs 2");
    h.sync_menu_ui();
    capture(&mut h, "btn-demos-pad-ps");
    h.exec_line("joy_glyphs 1");
    h.sync_menu_ui();

    // Directory browser: [X] Select Folder, Default, [Y] Show Hidden, [B] Cancel
    h.menu.open_dir_browser();
    h.sync_key_dest();
    capture(&mut h, "btn-dirbrowser-pad");

    // Load screen with a synthetic own save: [X] Delete on card, [B] Back, and Confirm modal with [B] Cancel
    let sv = crate::menu::SaveSlot {
        name: "s0".into(),
        info: crate::saveinfo::SaveInfo {
            level: "The Slipgate Complex".into(),
            map: "e1m1".into(),
            skill: 1,
            time: 83.0,
            modified: None,
            health: 100,
            armor: 50,
            items: 0,
            kills: (12, 30),
            secrets: (2, 6),
            reason: None,
            addons: None,
            players: (1, 1),
            player: Default::default(),
        },
        thumb: None,
        source: crate::menu::SaveSource::Own,
    };
    h.menu.saves = vec![sv.clone()];
    h.menu.latest = Some(sv);
    h.menu.open(Screen::Load);
    h.sync_key_dest();
    capture(&mut h, "btn-load-pad");
    // X opens Delete confirmation modal with [B] Cancel
    press(&mut h, GamepadButton::West);
    capture(&mut h, "btn-confirm-pad");
    press(&mut h, GamepadButton::East);

    // Help screen: single controller prompt keymap (and none in keyboard mode)
    h.menu.open(Screen::Help);
    h.sync_key_dest();
    capture(&mut h, "btn-help-pad");

    // Map Packs & Level Select: [Y] filter box + OSK, [RT] sort button, [B] Back
    let m = |d: &str, t: &str| qui::mods::ModInfo { dir: d.into(), title: t.into(), base: qui::mods::Base::Id, blurb: vec![] };
    h.menu.set_mods(vec![m("id1", "Quake"), m("ad", "Arcane Dimensions"), m("mg1", "Dimension of the Machine")], "id1");
    h.menu.set_mod_maps(
        vec![
            qui::mods::MapInfo { name: "e1m1".into(), title: "The Slipgate Complex".into() },
            qui::mods::MapInfo { name: "e1m2".into(), title: "Castle of the Damned".into() },
        ],
        qui::stats::Stats::default(),
    );
    h.menu.open(Screen::Mods);
    h.sync_key_dest();
    capture(&mut h, "btn-mods-pad");
    // RT cycles sort, Y opens OSK for filtering
    h.handle_event(HostEvent::Pad(7, InputEvent::GamepadAxis { axis: GamepadAxis::RightTrigger, value: 1.0 }));
    h.handle_event(HostEvent::Pad(7, InputEvent::GamepadAxis { axis: GamepadAxis::RightTrigger, value: 0.0 }));
    h.sync_menu_ui();
    press(&mut h, GamepadButton::North);
    capture(&mut h, "btn-mods-osk-pad");
    press(&mut h, GamepadButton::East);
    // Level Select (ModMaps)
    h.menu.open(Screen::ModMaps);
    h.sync_key_dest();
    capture(&mut h, "btn-levels-pad");
}
