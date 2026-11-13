use super::*;

fn setup(page: Page) -> (Menu, Cvars) {
    let mut cv = Cvars::new();
    for &(name, default, flags) in qserver::addons::CVARS {
        cv.register(name, default, flags);
    }
    cv.register("addon_goggles", "1", 0);
    let mut menu = Menu::new();
    menu.open(Screen::OptionsPage(page));
    (menu, cv)
}

fn commands(actions: &[Action]) -> String {
    actions.iter().filter_map(|a| if let Action::Command(cmd) = a { Some(cmd.as_str()) } else { None }).collect()
}

#[test]
fn slot_assignment_wraps_without_rebinding_any_key() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.addon_ui.selected = 1;
    menu.addon_ui.focus = 2;
    menu.bindings[b'1' as usize] = Some("impulse 1".into());
    assert!(commands(&menu.key(K_LEFTARROW, &mut cv, false)).is_empty());
    assert_eq!(cv.value_of("addon_boomerang_slot"), 10.0);
    assert_eq!(menu.bindings[b'1' as usize].as_deref(), Some("impulse 1"));
    menu.key(K_RIGHTARROW, &mut cv, false);
    assert_eq!(cv.value_of("addon_boomerang_slot"), 1.0);
}

#[test]
fn occupied_keys_require_confirmation_for_shared_and_unique_actions() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.addon_ui.focus = 4; // shared alt fire
    menu.bindings[b'f' as usize] = Some("+jump".into());
    menu.key(K_ENTER, &mut cv, false);
    assert!(menu.capturing());
    assert!(commands(&menu.key(b'f' as i32, &mut cv, false)).is_empty());
    assert_eq!(menu.bindings[b'f' as usize].as_deref(), Some("+jump"));
    menu.key(K_ESCAPE, &mut cv, false);
    assert_eq!(menu.bindings[b'f' as usize].as_deref(), Some("+jump"));
    menu.key(K_ENTER, &mut cv, false);
    menu.key(b'f' as i32, &mut cv, false);
    assert!(commands(&menu.key(b'y' as i32, &mut cv, false)).contains("+attack2"));
    assert_eq!(menu.bindings[b'f' as usize].as_deref(), Some("+attack2"));
    // Selecting another add-on uses that same command and binding.
    menu.addon_ui.selected = 1;
    assert_eq!(menu.browser_command(), Some("+attack2"));
    assert_eq!(keys_for(&menu.bindings, menu.browser_command().unwrap()), vec![b'f' as i32]);
    // Goggles has one unique action and no weapon slot.
    menu.addon_ui.selected = 5;
    menu.addon_ui.focus = 2;
    assert_eq!(menu.selected_addon().slot, None);
    menu.key(K_ENTER, &mut cv, false);
    menu.key(b'f' as i32, &mut cv, false);
    assert_eq!(menu.bindings[b'f' as usize].as_deref(), Some("+attack2"));
    menu.key(K_ABUTTON, &mut cv, false);
    assert_eq!(menu.bindings[b'f' as usize].as_deref(), Some("goggles cycle"));
    assert!(!menu.capturing());
}

#[test]
fn bindings_clear_by_device_and_cancel_capture_without_leaving() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.addon_ui.focus = 3;
    for key in [K_MOUSE1, K_RTRIGGER] {
        menu.bindings[key as usize] = Some("+attack".into());
    }
    menu.key(K_XBUTTON, &mut cv, false);
    assert_eq!(menu.bindings[K_RTRIGGER as usize], None);
    assert_eq!(menu.bindings[K_MOUSE1 as usize].as_deref(), Some("+attack"));
    menu.key(K_DEL, &mut cv, false);
    assert_eq!(menu.bindings[K_MOUSE1 as usize], None);
    menu.key(K_ENTER, &mut cv, false);
    menu.key(K_ESCAPE, &mut cv, false);
    assert!(!menu.capturing());
    assert_eq!(menu.screen, Screen::OptionsPage(Page::Addons));
}

#[test]
fn every_parameter_link_returns_to_the_selected_addon() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    for index in 0..ADDON.len() {
        menu.addon_ui.selected = index;
        menu.addon_ui.focus = menu.browser_rows();
        let page = menu.selected_addon().page.unwrap();
        menu.key(K_ENTER, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(page));
        menu.key(K_ESCAPE, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(Page::NewGameAddons));
        assert_eq!(menu.addon_ui.selected, index);
    }
}

#[test]
fn controller_can_reach_start_and_description_scroll_is_bounded() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    menu.key(K_DPAD_RIGHT, &mut cv, false);
    for _ in 0..20 {
        menu.key(K_MWHEELDOWN, &mut cv, false);
    }
    assert_eq!(menu.addon_ui.scroll, menu.description_lines().len().saturating_sub(4));
    menu.key(K_DPAD_DOWN, &mut cv, false);
    assert_eq!(menu.addon_ui.focus, 2);
    for _ in 0..4 {
        menu.key(K_DPAD_DOWN, &mut cv, false);
    }
    let actions = menu.key(K_ABUTTON, &mut cv, false);
    assert!(commands(&actions).contains("map start"));
    assert_eq!(menu.screen, Screen::None);
}

#[test]
fn clicking_selects_and_toggles_but_locked_games_keep_remapping() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.mouse_move((70.0, 90.0), &mut cv);
    menu.key(K_MOUSE1, &mut cv, true);
    assert_eq!(menu.addon_ui.selected, 1);
    assert_eq!(cv.value_of("addon_boomerang"), 1.0);
    menu.mouse_move((450.0, 289.0), &mut cv); // Alt Fire
    menu.key(K_MOUSE1, &mut cv, true);
    menu.key(b'q' as i32, &mut cv, true);
    assert_eq!(menu.bindings[b'q' as usize].as_deref(), Some("+attack2"));
    cv.set("addons_active", "1");
    menu.mouse_move((70.0, 90.0), &mut cv);
    menu.key(K_MOUSE1, &mut cv, true);
    assert_eq!(cv.value_of("addon_boomerang"), 0.0);
}
