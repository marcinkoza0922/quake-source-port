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
    assert!(menu.selected_addon().weapons.is_empty());
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
fn settings_down_stops_at_last_row_and_bumper_reaches_start() {
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
    assert_eq!(menu.addon_ui.focus, menu.browser_rows());
    let offset = menu.browser_offset();
    for key in [K_DPAD_DOWN, K_DOWNARROW] {
        menu.key(key, &mut cv, false);
        assert_eq!(menu.addon_ui.focus, menu.browser_rows());
        assert_eq!(menu.browser_offset(), offset);
    }
    menu.key(K_RSHOULDER, &mut cv, false);
    assert_eq!(menu.addon_ui.focus, menu.browser_last_focus());
    let actions = menu.key(K_ABUTTON, &mut cv, false);
    assert!(commands(&actions).contains("map start"));
    assert_eq!(menu.screen, Screen::None);
}

#[test]
fn controller_reaches_start_directly_below_the_checklist_and_returns() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    for _ in 0..ADDON.len() {
        menu.key(K_DPAD_DOWN, &mut cv, false);
    }
    assert_eq!(menu.addon_ui.focus, menu.browser_last_focus());
    menu.key(K_DPAD_UP, &mut cv, false);
    assert_eq!((menu.addon_ui.focus, menu.addon_ui.selected), (0, ADDON.len() - 1));
    menu.key(K_DPAD_DOWN, &mut cv, false);
    menu.key(K_DPAD_DOWN, &mut cv, false);
    assert_eq!((menu.addon_ui.focus, menu.addon_ui.selected), (0, 0));
    for _ in 0..ADDON.len() { menu.key(K_DPAD_DOWN, &mut cv, false); }
    let actions = menu.key(K_ABUTTON, &mut cv, false);
    assert!(commands(&actions).contains("map start"));
    assert_eq!(menu.screen, Screen::None);
}

#[test]
fn mouse_hover_and_click_share_the_start_buttons_focus() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    menu.mouse_move((100.0, 275.0), &mut cv);
    assert_eq!(menu.addon_ui.focus, menu.browser_last_focus());
    let actions = menu.key(K_MOUSE1, &mut cv, false);
    assert!(commands(&actions).contains("map start"));
}

#[test]
fn clicking_selects_and_toggles_but_locked_games_keep_remapping() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.mouse_move((70.0, 90.0), &mut cv);
    menu.key(K_MOUSE1, &mut cv, true);
    assert_eq!(menu.addon_ui.selected, 1);
    assert_eq!(cv.value_of("addon_boomerang"), 1.0);
    menu.mouse_move((450.0, menu.browser_row_y(2) + 1.0), &mut cv); // Alt Fire
    menu.key(K_MOUSE1, &mut cv, true);
    menu.key(b'q' as i32, &mut cv, true);
    assert_eq!(menu.bindings[b'q' as usize].as_deref(), Some("+attack2"));
    cv.set("addons_active", "1");
    menu.mouse_move((70.0, 90.0), &mut cv);
    menu.key(K_MOUSE1, &mut cv, true);
    assert_eq!(cv.value_of("addon_boomerang"), 0.0);
}

#[test]
fn multi_weapon_addon_lists_slots_before_shared_actions() {
    let addon = Addon {
        weapons: &[
            Weapon { name: "First", slot: "first_slot" },
            Weapon { name: "Second", slot: "second_slot" },
        ],
        ..ADDON[0]
    };
    assert_eq!(addon.row_count(), 5);
    assert_eq!(addon.command(0), None);
    assert_eq!(addon.command(1), None);
    assert_eq!(addon.command(2), Some("+attack"));
    assert_eq!(addon.command(3), Some("+attack2"));
    assert_eq!(addon.command(4), None);
    assert!(addon.row_y(1) + 16.0 <= addon.header_y());
    assert!(addon.row_y(2) >= addon.header_y() + 8.0);
}

#[test]
fn settings_scroll_is_independent_bounded_and_keeps_click_targets_stable() {
    let (mut menu, mut cv) = setup(Page::Addons);
    let description_scroll = menu.addon_ui.scroll;
    menu.mouse_move((600.0, 310.0), &mut cv);
    for _ in 0..10 {
        menu.key(K_MWHEELDOWN, &mut cv, true);
    }
    assert_eq!(menu.browser_offset(), menu.browser_max_offset());
    assert_eq!(menu.addon_ui.scroll, description_scroll);
    let row = menu.selected_addon().row_count() - 1;
    let y = menu.browser_row_y(row) + 4.0;
    let offset = menu.browser_offset();
    menu.mouse_move((300.0, y), &mut cv);
    assert_eq!(menu.addon_ui.focus, menu.browser_rows());
    assert_eq!(menu.browser_offset(), offset);
    menu.key(K_MOUSE1, &mut cv, true);
    assert_eq!(menu.screen, Screen::OptionsPage(Page::PortalGun));
}

#[test]
fn settings_keyboard_navigation_reveals_rows_and_page_keys_scroll() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.addon_ui.focus = 2;
    menu.key(K_PGDN, &mut cv, false);
    assert_eq!(menu.browser_offset(), menu.browser_max_offset());
    menu.key(K_PGUP, &mut cv, false);
    assert_eq!(menu.browser_offset(), 0.0);
    for _ in 0..3 {
        menu.key(K_DPAD_DOWN, &mut cv, false);
    }
    assert_eq!(menu.addon_ui.focus, menu.browser_rows());
    assert_eq!(menu.browser_offset(), menu.browser_max_offset());
    for _ in 0..3 {
        menu.key(K_DPAD_UP, &mut cv, false);
    }
    assert!(menu.browser_row_y(0) >= 236.0);
    menu.mouse_move((600.0, 280.0), &mut cv);
    for _ in 0..10 {
        menu.key(K_MWHEELUP, &mut cv, true);
    }
    assert_eq!(menu.browser_offset(), 0.0);
}

#[test]
fn controller_sections_reach_every_addons_action_without_a_keyboard() {
    let (mut menu, mut cv) = setup(Page::Addons);
    menu.pad_ui = true;
    for index in 0..ADDON.len() {
        assert_eq!(menu.addon_ui.selected, index);
        let a = menu.selected_addon();
        let enabled = cv.value_of(a.cvar);
        menu.key(K_ABUTTON, &mut cv, false);
        assert_ne!(cv.value_of(a.cvar), enabled);
        menu.key(K_RSHOULDER, &mut cv, false);
        assert_eq!(menu.addon_ui.focus, 1);
        for _ in 0..10 { menu.key(K_DPAD_DOWN, &mut cv, false); }
        assert!(menu.addon_ui.scroll <= menu.description_lines().len().saturating_sub(4));
        // Return to the description, then select its settings directly.
        menu.key(K_LSHOULDER, &mut cv, false);
        menu.key(K_ABUTTON, &mut cv, false);
        assert_eq!(menu.addon_ui.focus, 2);
        if let Some(slot) = menu.browser_slot() {
            let old = cv.value_of(slot);
            menu.key(K_DPAD_RIGHT, &mut cv, false);
            assert_ne!(cv.value_of(slot), old);
            menu.key(K_DPAD_DOWN, &mut cv, false);
        }
        if let Some(command) = menu.browser_command() {
            menu.key(K_ABUTTON, &mut cv, false);
            assert!(menu.capturing());
            menu.key(K_RTRIGGER, &mut cv, false);
            if menu.bind_pending.is_some() { menu.key(K_ABUTTON, &mut cv, false); }
            assert_eq!(menu.bindings[K_RTRIGGER as usize].as_deref(), Some(command));
            menu.key(K_XBUTTON, &mut cv, false);
            assert_eq!(menu.bindings[K_RTRIGGER as usize], None);
        }
        while menu.addon_ui.focus < menu.browser_rows() {
            menu.key(K_DPAD_DOWN, &mut cv, false);
        }
        assert!(menu.browser_row_y(a.row_count() - 1) + 16.0 <= 322.0);
        menu.key(K_ABUTTON, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(a.page.unwrap()));
        menu.key(K_BBUTTON, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(Page::Addons));
        menu.key(K_RSHOULDER, &mut cv, false);
        assert_eq!(menu.addon_ui.focus, 0);
        menu.key(K_DPAD_DOWN, &mut cv, false);
    }
    menu.key(K_BBUTTON, &mut cv, false);
    assert_eq!(menu.screen, Screen::Main);
}

#[test]
fn controller_bumpers_reach_start_and_shortcut_does_not_interrupt_binding() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    menu.key(K_LSHOULDER, &mut cv, false);
    assert!(menu.addon_ui.focus > menu.browser_rows());
    assert!(commands(&menu.key(K_ABUTTON, &mut cv, false)).contains("map start"));
    menu.open(Screen::OptionsPage(Page::NewGameAddons));
    menu.addon_ui.focus = 3;
    menu.key(K_ABUTTON, &mut cv, false);
    menu.key(K_YBUTTON, &mut cv, false);
    assert_eq!(menu.bindings[K_YBUTTON as usize].as_deref(), Some("+attack"));
    assert_eq!(menu.screen, Screen::OptionsPage(Page::NewGameAddons));
    assert!(commands(&menu.key(K_YBUTTON, &mut cv, false)).contains("map start"));
}

#[test]
fn reopening_regular_addons_cannot_activate_the_pickers_start_button() {
    let (mut menu, mut cv) = setup(Page::NewGameAddons);
    menu.key(K_LSHOULDER, &mut cv, false);
    menu.key(K_BBUTTON, &mut cv, false);
    menu.open(Screen::OptionsPage(Page::Addons));
    let actions = menu.key(K_ABUTTON, &mut cv, false);
    assert!(!commands(&actions).contains("map start"));
    assert_eq!(menu.screen, Screen::OptionsPage(Page::PortalGun));
}
