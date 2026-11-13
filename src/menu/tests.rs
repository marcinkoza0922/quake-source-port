use super::*;

#[test]
fn long_page_scrolls_with_cursor() {
    let mut m = Menu::new();
    m.servers = (0..PAGE_ROWS).map(|i| ServerEntry {
        address: format!("127.0.0.1:{}", 26000+i), hostname: format!("Server {i}"),
        map: "start".into(), players: 1, max_players: 4,
    }).collect();
    m.open(Screen::OptionsPage(Page::Join));
    let n = m.page_len(Page::Join);
    assert!(n > PAGE_ROWS, "Server list should be long enough to scroll");
    assert_eq!(m.page_scroll(Page::Join), 0);
    m.opt_cursor = n - 1;
    assert_eq!(m.page_scroll(Page::Join), n - PAGE_ROWS);
    // the top visible row maps to the first scrolled-in item
    assert_eq!(m.item_at((320.0, PAGE_ROW0 + 1.0)), Some(n - PAGE_ROWS));
    // moving back up past the window scrolls up
    m.opt_cursor = 0;
    assert_eq!(m.page_scroll(Page::Join), 0);
}

#[test]
fn dropdown_picks_value() {
    let mut cv = Cvars::default();
    cv.register("r_anisotropy", "1", 0);
    let mut m = Menu::new();
    m.open(Screen::OptionsPage(Page::Graphics));
    let row = GRAPHICS.iter().position(|i| matches!(i, Item::Choice { cvar: "r_anisotropy", .. })).unwrap();
    m.opt_cursor = row;
    m.key(K_ENTER, &mut cv, false);
    assert!(m.drop.is_some());
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ENTER, &mut cv, false);
    assert!(m.drop.is_none());
    assert_eq!(cv.value_of("r_anisotropy"), 4.0);
    // escape closes without changing
    m.key(K_ENTER, &mut cv, false);
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ESCAPE, &mut cv, false);
    assert!(m.drop.is_none());
    assert_eq!(m.screen, Screen::OptionsPage(Page::Graphics));
    assert_eq!(cv.value_of("r_anisotropy"), 4.0);
    // two-value choices still cycle
    let filt = GRAPHICS.iter().position(|i| matches!(i, Item::Choice { cvar: "r_texfilter", .. })).unwrap();
    m.opt_cursor = filt;
    m.key(K_ENTER, &mut cv, false);
    assert!(m.drop.is_none());
}

#[cfg(test)]
mod quit_tests {
    use super::*;

    fn quits(acts: &[Action]) -> bool {
        acts.iter().any(|a| matches!(a, Action::Command(c) if c == "quit\n"))
    }

    #[test]
    fn quit_prompt_buttons_and_keys() {
        let mut cv = Cvars::default();
        let mut m = Menu::new();
        m.open(Screen::Main);
        m.open(Screen::Quit);
        assert!(quits(&m.key(b'y' as i32, &mut cv, false)));
        // click "No"
        m.mouse_move((QUIT_BUTTONS[1] + 4.0, QUIT_BTN_Y + 4.0), &mut Cvars::default());
        assert!(!quits(&m.key(K_MOUSE1, &mut cv, false)));
        assert_eq!(m.screen, Screen::Main);
        // click "Yes"
        m.open(Screen::Quit);
        m.mouse_move((QUIT_BUTTONS[0] + 4.0, QUIT_BTN_Y + 4.0), &mut Cvars::default());
        assert!(quits(&m.key(K_MOUSE1, &mut cv, false)));
        // keyboard: right then enter = No
        m.mouse = None;
        m.quit_cursor = 0;
        m.key(K_RIGHTARROW, &mut cv, false);
        assert!(!quits(&m.key(K_ENTER, &mut cv, false)));
        assert_eq!(m.screen, Screen::Main);
    }
}

#[cfg(test)]
mod multiplayer_tests {
    use super::*;

    pub(super) fn cvars() -> Cvars {
        let mut cv = Cvars::default();
        for (n, d) in [
            ("_cl_name", "player"),
            ("_cl_color", "0"),
            ("_mp_address", ""),
            ("_mp_maxplayers", "8"),
            ("_mp_coop", "0"),
            ("_mp_teamplay", "0"),
            ("_mp_skill", "1"),
            ("_mp_fraglimit", "0"),
            ("_mp_timelimit", "0"),
            ("_mp_map", ""),
        ] {
            cv.register(n, d, 0);
        }
        cv
    }

    pub(super) fn commands(acts: &[Action]) -> String {
        acts.iter().filter_map(|a| if let Action::Command(c) = a { Some(c.as_str()) } else { None }).collect()
    }

    #[test]
    fn join_by_typed_address() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open(Screen::Main);
        m.main_cursor = 1;
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::Multiplayer);
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::OptionsPage(Page::Join));
        for ch in "10.0.0.9:x".chars() {
            m.text(ch, &mut cv);
        }
        m.key(K_BACKSPACE, &mut cv, false);
        m.text('7', &mut cv);
        let acts = m.key(K_ENTER, &mut cv, false);
        assert_eq!(commands(&acts), "connect 10.0.0.9:7\n");
        assert_eq!(m.screen, Screen::None);
    }

    #[test]
    fn join_found_server_and_search() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open_multiplayer("join");
        m.opt_cursor = 2;
        assert_eq!(commands(&m.key(K_ENTER, &mut cv, false)), "slist\n");
        m.servers = vec![ServerEntry { address: "192.168.1.5:26000".into(), hostname: "h".into(), map: "dm3".into(), players: 1, max_players: 8 }];
        m.key(K_DOWNARROW, &mut cv, false);
        assert!(matches!(m.page_item(Page::Join, m.opt_cursor), Item::Server(0)));
        assert_eq!(commands(&m.key(K_ENTER, &mut cv, false)), "connect 192.168.1.5:26000\n");
    }

    #[test]
    fn host_game_start() {
        let mut cv = cvars();
        let mut m = Menu::new();
        assert!(m.open_multiplayer("host").is_some());
        m.net_maps = ["e1m1", "dm1", "dm2"].iter().map(|n| qui::mods::MapInfo { name: n.to_string(), title: String::new() }).collect();
        m.key(K_RIGHTARROW, &mut cv, false); // 8 -> 12 players
        m.opt_cursor = 4;
        m.key(K_RIGHTARROW, &mut cv, false); // frag limit 10
        m.opt_cursor = 6;
        m.key(K_RIGHTARROW, &mut cv, false); // dm1 (default) -> dm2
        m.opt_cursor = 7;
        let c = commands(&m.key(K_ENTER, &mut cv, false));
        assert_eq!(c, "disconnect\nmaxplayers 12\ncoop 0\ndeathmatch 1\nteamplay 0\nskill 1\nfraglimit 10\ntimelimit 0\nmap dm2\n");
        assert_eq!(cv.by_name("_mp_map").unwrap().string, "dm2");
    }

    #[test]
    fn setup_applies_name_and_colors_on_back() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open_multiplayer("setup");
        for _ in 0..6 {
            m.key(K_BACKSPACE, &mut cv, false);
        }
        for ch in "Ranger".chars() {
            m.text(ch, &mut cv);
        }
        m.opt_cursor = 1;
        m.key(K_LEFTARROW, &mut cv, false); // shirt 0 -> 13
        m.opt_cursor = 2;
        m.key(K_RIGHTARROW, &mut cv, false); // pants 0 -> 1
        let c = commands(&m.key(K_ESCAPE, &mut cv, false));
        assert_eq!(c, "name \"Ranger\"\ncolor 13 1\n");
        assert_eq!(m.screen, Screen::Multiplayer);
    }
}

#[cfg(test)]
mod save_menu_tests {
    use super::*;
    #[test]
    fn options_subpages_open_and_return_to_parent_row() {
        let mut m = Menu::new();
        let mut cv = Cvars::new();
        m.open(Screen::OptionsPage(Page::Input));
        m.key(K_DOWNARROW, &mut cv, false);
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::OptionsPage(Page::Controller));
        assert_eq!(m.opt_cursor, 0);
        m.key(K_ESCAPE, &mut cv, false);
        assert_eq!(m.screen, Screen::OptionsPage(Page::Input));
        assert_eq!(m.opt_cursor, 1);
        assert_eq!(parent_page(Page::Autosave).map(|p| p.0), Some(Page::Gameplay));
        assert!(option_cvars().any(|c| c == "sv_autosave"));
        assert!(option_cvars().any(|c| c == "joy_invert"));
    }

    #[test]
    fn autosave_submenu_navigation_and_slot_mapping() {
        let mut m = Menu::new();
        let mut cv = Cvars::new();
        m.open(Screen::Load);
        assert_eq!(m.save_slot_name(0).as_deref(), Some("autosave0"));
        assert_eq!(m.save_slot_name(2).as_deref(), Some("s0"));
        m.key(K_DOWNARROW, &mut cv, true);
        assert_eq!(m.load_cursor, 1);
        let actions = m.key(K_ENTER, &mut cv, true);
        assert!(m.autosaves);
        assert!(actions.iter().any(|a| matches!(a, Action::Command(s) if s == "_scansaves\n")));
        // An empty archive still handles every direction without underflow.
        for key in [K_UPARROW, K_DOWNARROW, K_LEFTARROW, K_RIGHTARROW] {
            m.key(key, &mut cv, true);
            assert_eq!(m.load_cursor, 0);
        }
        m.key(K_ESCAPE, &mut cv, true);
        assert!(!m.autosaves);
        assert_eq!(m.screen, Screen::Load);
        assert_eq!(m.load_cursor, 1);
        m.open(Screen::Save);
        assert_eq!(m.save_slot_name(0).as_deref(), Some("s0"));
        assert_eq!(m.save_rows(), MAX_SAVEGAMES);
    }
    #[test]
    fn card_mouse_coordinates_follow_scrolling() {
        let mut m = Menu::new();
        m.open(Screen::Load);
        let top = CARD_SCROLL_Y0;
        // the newest save is pinned above the scrolling list
        assert_eq!(m.item_at((100.0, CARD_Y0 + 4.0)), Some(0));
        assert_eq!(m.item_at((100.0, top + 4.0)), Some(1));
        assert_eq!(m.item_at((100.0, top + CARD_PITCH + 4.0)), Some(2));
        // the gap between cards and the area outside the column aren't cards
        assert_eq!(m.item_at((100.0, top + CARD_H + 1.0)), None);
        assert_eq!(m.item_at((20.0, top + 4.0)), None);
        // wheel scrolls the list in pixels; the pinned card stays
        let mut cv = Cvars::default();
        m.mouse_move((100.0, top + 4.0), &mut Cvars::default());
        m.key(K_MWHEELDOWN, &mut cv, false);
        assert_eq!(m.save_scroll, CARD_WHEEL);
        assert_eq!(m.item_at((100.0, CARD_Y0 + 4.0)), Some(0));
        assert_eq!(m.item_at((100.0, top + 4.0)), Some(1));
        assert_eq!(m.load_cursor, 1);
        m.key(K_MWHEELUP, &mut cv, false);
        m.key(K_MWHEELUP, &mut cv, false);
        assert_eq!(m.save_scroll, 0.0);
        // scrolling to the end stops at the last card
        for _ in 0..100 {
            m.key(K_MWHEELDOWN, &mut cv, false);
        }
        assert_eq!(m.save_scroll, m.card_max_scroll());
        let last = m.save_rows() - 1;
        assert!(m.card_y(last) + CARD_H <= CARD_BOTTOM + 0.01);
        // keyboard navigation brings the cursor card fully into view
        m.save_scroll = 0.0;
        m.load_cursor = 0;
        for _ in 0..6 {
            m.key(K_DOWNARROW, &mut cv, false);
        }
        let y = m.card_y(m.load_cursor);
        assert!(y >= top && y + CARD_H <= CARD_BOTTOM + 0.01);
        // dragging the scrollbar thumb reaches both ends
        let (x, t, h, _, th) = m.card_scrollbar().unwrap();
        m.mouse_move((x + 1.0, t + h - 1.0), &mut Cvars::default());
        m.key(K_MOUSE1, &mut cv, false);
        m.mouse_move((x, t + h + 50.0), &mut Cvars::default());
        assert_eq!(m.save_scroll, m.card_max_scroll());
        m.mouse_move((x, t - 50.0), &mut Cvars::default());
        assert_eq!(m.save_scroll, 0.0);
        m.mouse_up();
        let _ = th;
    }

    #[test]
    fn slider_follows_mouse_drag() {
        let mut m = Menu::new();
        let mut cv = Cvars::default();
        let Some(row) = page_items(Page::Mouse).iter().position(|i| matches!(i, Item::Slider { cvar: "sensitivity", .. })) else { return };
        m.open(Screen::OptionsPage(Page::Mouse));
        let y = PAGE_ROW0 + row as f32 * ROW_H + 4.0;
        cv.register("sensitivity", "3", 0);
        m.mouse_move((SLIDER_X.0 + 4.0, y), &mut cv);
        m.key(K_MOUSE1, &mut cv, false);
        assert_eq!(cv.value_of("sensitivity"), 1.0);
        m.mouse_move((SLIDER_X.1 + 40.0, y + 30.0), &mut cv);
        assert_eq!(cv.value_of("sensitivity"), 11.0);
        m.mouse_up();
        m.mouse_move((SLIDER_X.0, y), &mut cv);
        assert_eq!(cv.value_of("sensitivity"), 11.0);
    }

    #[test]
    fn controls_rebind_and_clear() {
        use super::multiplayer_tests::{commands, cvars};
        let mut cv = cvars();
        let mut m = Menu::new();
        m.bindings[K_MOUSE1 as usize] = Some("+attack".into());
        m.bindings[K_RTRIGGER as usize] = Some("+attack".into());
        m.open(Screen::OptionsPage(Page::Controls));
        m.opt_cursor = 0;
        m.key(K_ENTER, &mut cv, false);
        assert!(m.capturing());
        let c = commands(&m.key(b'f' as i32, &mut cv, false));
        assert_eq!(c, "bind \"f\" \"+attack\"\n");
        assert!(!m.capturing());
        // a third keyboard key replaces both keyboard keys, pad untouched
        m.key(K_ENTER, &mut cv, false);
        let c = commands(&m.key(b'g' as i32, &mut cv, false));
        assert!(c.contains("unbind \"MOUSE1\"") && c.contains("unbind \"f\"") && c.ends_with("bind \"g\" \"+attack\"\n"), "{c}");
        assert_eq!(keys_for(&m.bindings, "+attack"), vec![b'g' as i32, K_RTRIGGER]);
        // escape cancels capture
        m.key(K_ENTER, &mut cv, false);
        assert!(commands(&m.key(K_ESCAPE, &mut cv, false)).is_empty());
        assert_eq!(m.screen, Screen::OptionsPage(Page::Controls));
        // X clears the pad keys only
        let c = commands(&m.key(K_XBUTTON, &mut cv, false));
        assert_eq!(c, "unbind \"RTRIGGER\"\n");
        assert_eq!(keys_for(&m.bindings, "+attack"), vec![b'g' as i32]);
    }

    #[test]
    fn onscreen_keyboard_types_into_field() {
        use super::multiplayer_tests::cvars;
        let mut cv = cvars();
        cv.set("_cl_name", "");
        let mut m = Menu::new();
        m.open(Screen::OptionsPage(Page::Setup));
        m.opt_cursor = 0;
        m.key(K_ABUTTON, &mut cv, false);
        assert!(m.osk_open());
        m.key(K_DPAD_DOWN, &mut cv, false); // q row
        m.key(K_ABUTTON, &mut cv, false); // q
        m.key(K_YBUTTON, &mut cv, false); // shift
        m.key(K_DPAD_RIGHT, &mut cv, false);
        m.key(K_ABUTTON, &mut cv, false); // W
        m.key(K_DPAD_UP, &mut cv, false);
        m.key(K_DPAD_UP, &mut cv, false); // action row (wraps)
        m.key(K_DPAD_RIGHT, &mut cv, false); // shift -> space
        m.key(K_ABUTTON, &mut cv, false);
        assert_eq!(cv.by_name("_cl_name").unwrap().string, "qW ");
        m.key(K_XBUTTON, &mut cv, false);
        assert_eq!(cv.by_name("_cl_name").unwrap().string, "qW");
        m.key(K_BBUTTON, &mut cv, false);
        assert!(!m.osk_open());
        assert_eq!(m.screen, Screen::OptionsPage(Page::Setup));
    }
}

#[test]
fn death_shortcuts_and_controller_select_recovery() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    for key in b'1'..=b'4' {
        m.open(Screen::Death);
        let actions = m.key(key as i32, &mut cv, true);
        assert!(actions.iter().any(|a| matches!(a, Action::Command(c) if c == &format!("_death_choice {}\n", key - b'0'))));
    }
    m.open(Screen::Death);
    m.key(K_DPAD_DOWN, &mut cv, true);
    let actions = m.key(K_ABUTTON, &mut cv, true);
    assert!(actions.iter().any(|a| matches!(a, Action::Command(c) if c == "_death_choice 2\n")));
    m.key(K_ESCAPE, &mut cv, true);
    assert_eq!(m.screen, Screen::Death);
    assert_eq!(m.item_at((170.0, 190.0)), Some(2));
}

#[test]
fn enhancements_hub_and_subpages_navigate_back_to_their_parent() {
    let mut cv = Cvars::default();
    for name in ["r_enhancedwater", "r_blood", "r_debris"] {
        cv.register(name, "1", 0);
    }
    let mut menu = Menu::new();
    menu.open(Screen::Options);
    menu.page_cursor = PAGES.iter().position(|(_, page)| *page == Page::Enhancements).unwrap();
    menu.key(K_ENTER, &mut cv, false);
    assert_eq!(menu.screen, Screen::OptionsPage(Page::Enhancements));
    for page in [Page::Water, Page::Blood, Page::Debris, Page::Impacts] {
        let row = ENHANCEMENTS.iter().position(|item| matches!(item, Item::Sub {page:p,..} if *p==page)).unwrap();
        menu.opt_cursor = row;
        menu.key(K_ENTER, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(page));
        menu.key(K_ESCAPE, &mut cv, false);
        assert_eq!(menu.screen, Screen::OptionsPage(Page::Enhancements));
        assert_eq!(menu.opt_cursor, row);
    }
    menu.key(K_ESCAPE, &mut cv, false);
    assert_eq!(menu.screen, Screen::Options);
    for cvar in ["r_impact_nails", "r_blood_amount", "r_debris_gibs", "r_wateralpha"] {
        assert!(option_cvars().any(|name| name==cvar));
    }
}

mod shader_tests {
    use super::*;

    #[test]
    fn screen_shader_preset_and_params() {
        let mut cv = Cvars::default();
        for n in ["r_shader_preset", "r_shader_params"] {
            cv.register(n, "", 0);
        }
        cv.register("r_shader_scope", "0", 0);
        let mut m = Menu::new();
        m.shader_presets = vec!["crt/a".into(), "crt/b".into()];
        m.shader_params = vec![qrender::ShaderParam {
            id: "MASK".into(),
            description: "Mask".into(),
            initial: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.25,
        }];
        m.screen = Screen::OptionsPage(Page::ScreenShader);
        m.opt_cursor = 0;
        m.key(K_RIGHTARROW, &mut cv, false);
        assert_eq!(cv.by_name("r_shader_preset").unwrap().string, "crt/a");
        // the param row follows the static rows
        m.opt_cursor = 2;
        assert!(matches!(m.page_item(Page::ScreenShader, 2), Item::ShaderParam(0)));
        m.key(K_RIGHTARROW, &mut cv, false);
        assert_eq!(cv.by_name("r_shader_params").unwrap().string, "MASK=0.75");
        // back to the default drops the override
        m.key(K_LEFTARROW, &mut cv, false);
        assert_eq!(cv.by_name("r_shader_params").unwrap().string, "");
        // picking another preset clears overrides
        m.key(K_RIGHTARROW, &mut cv, false);
        m.opt_cursor = 0;
        m.key(K_RIGHTARROW, &mut cv, false);
        assert_eq!(cv.by_name("r_shader_preset").unwrap().string, "crt/b");
        assert_eq!(cv.by_name("r_shader_params").unwrap().string, "");
    }
}
