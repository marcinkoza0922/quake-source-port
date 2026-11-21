use super::*;

#[test]
fn keyboard_has_no_footer_prompts_and_controller_does() {
    let mut menu = Menu::new();
    let cv = Cvars::new();
    let mut canvas = Canvas::new(640, 360);
    canvas.ui = Some(Default::default());
    for screen in [Screen::Main, Screen::SinglePlayer, Screen::Options, Screen::OptionsPage(Page::Controls), Screen::Quit, Screen::Help] {
        menu.open(screen);
        menu.pad_ui = false;
        canvas.list.clear();
        menu.draw_hints(&mut canvas, &cv);
        assert!(canvas.list.is_empty(), "{screen:?}");
        menu.pad_ui = true;
        menu.draw_hints(&mut canvas, &cv);
        assert!(!canvas.list.is_empty(), "{screen:?}");
    }
}

#[test]
fn help_draws_only_one_hint_box_in_controller_mode_and_none_on_keyboard() {
    let mut menu = Menu::new();
    let cv = Cvars::new();
    menu.open(Screen::Help);
    let mut no_pic = |_: &str| None;

    menu.pad_ui = false;
    let mut kb = Canvas::new(640, 360);
    kb.ui = Some(Default::default());
    menu.draw(&mut kb, &mut no_pic, &cv, 0.0);

    menu.pad_ui = true;
    let mut pad = Canvas::new(640, 360);
    pad.ui = Some(Default::default());
    menu.draw(&mut pad, &mut no_pic, &cv, 0.0);

    let mut hints_only = Canvas::new(640, 360);
    hints_only.ui = Some(Default::default());
    hints_only.menu_layout();
    menu.draw_hints(&mut hints_only, &cv);
    assert_eq!(pad.list.len() - kb.list.len(), hints_only.list.len(), "Help must draw its hint bar exactly once in pad mode");
}

#[test]
fn controller_sorts_and_filters_map_packs_and_level_select() {
    let mut menu = Menu::new();
    let mut cv = Cvars::new();
    let m = |d: &str, t: &str| qui::mods::ModInfo { dir: d.into(), title: t.into(), base: qui::mods::Base::Id, blurb: vec![] };
    menu.set_mods(vec![m("z_mod", "Beta Pack"), m("a_mod", "Arcane Dimensions")], "z_mod");
    menu.set_mod_maps(
        vec![
            qui::mods::MapInfo { name: "e1m1".into(), title: "Slipgate Complex".into() },
            qui::mods::MapInfo { name: "e1m2".into(), title: "Castle of the Damned".into() },
        ],
        qui::stats::Stats::default(),
    );

    // Map Packs: RT sorts, Y opens OSK to filter, B closes OSK then clears filter
    menu.open(Screen::Mods);
    menu.key(K_RTRIGGER, &mut cv, false);
    menu.key(K_YBUTTON, &mut cv, false);
    assert!(menu.osk_open());
    // row 1 col 0 is 'q'; move to 'a' (row 2 col 0) and type 'a'
    menu.key(K_DPAD_DOWN, &mut cv, false);
    menu.key(K_DPAD_DOWN, &mut cv, false);
    menu.key(K_ABUTTON, &mut cv, false);
    assert!(menu.mods.has_filter(modmenu::Page::List));
    menu.key(K_BBUTTON, &mut cv, false);
    assert!(!menu.osk_open());
    assert!(menu.mods.has_filter(modmenu::Page::List));
    menu.key(K_BBUTTON, &mut cv, false);
    assert!(!menu.mods.has_filter(modmenu::Page::List));
    assert_eq!(menu.screen, Screen::Mods);

    // Level Select opened from Single Player: RT sorts, Y filters, B clears filter before returning
    menu.open(Screen::SinglePlayer);
    menu.open(Screen::ModMaps);
    menu.key(K_RTRIGGER, &mut cv, false);
    menu.key(K_YBUTTON, &mut cv, false);
    assert!(menu.osk_open());
    menu.key(K_DPAD_DOWN, &mut cv, false);
    menu.key(K_DPAD_DOWN, &mut cv, false);
    menu.key(K_ABUTTON, &mut cv, false);
    assert!(menu.mods.has_filter(modmenu::Page::Maps));
    menu.key(K_BBUTTON, &mut cv, false);
    assert!(!menu.osk_open());
    menu.key(K_BBUTTON, &mut cv, false);
    assert!(!menu.mods.has_filter(modmenu::Page::Maps));
    assert_eq!(menu.screen, Screen::ModMaps, "first B clears the active filter instead of leaving Level Select");
    menu.key(K_BBUTTON, &mut cv, false);
    assert_eq!(menu.screen, Screen::SinglePlayer);
}

#[test]
fn back_button_follows_content_and_click_returns_to_parent() {
    let mut menu = Menu::new();
    let mut cv = Cvars::new();
    for (screen, parent) in [
        (Screen::SinglePlayer, Screen::Main),
        (Screen::Multiplayer, Screen::Main),
        (Screen::Options, Screen::Main),
        (Screen::OptionsPage(Page::Mouse), Screen::OptionsPage(Page::Input)),
        (Screen::OptionsPage(Page::Addons), Screen::OptionsPage(Page::Customize)),
        (Screen::OptionsPage(Page::NewGameAddons), Screen::OptionsPage(Page::NewGameCustomize)),
        (Screen::OptionsPage(Page::Customize), Screen::Main),
        (Screen::OptionsPage(Page::NewGameCustomize), Screen::SinglePlayer),
        (Screen::OptionsPage(Page::Modifiers), Screen::OptionsPage(Page::Customize)),
        (Screen::Load, Screen::SinglePlayer),
        (Screen::Save, Screen::SinglePlayer),
        (Screen::Mods, Screen::Main),
    ] {
        menu.open(screen);
        let (x, y, w, h) = menu.back_button().unwrap();
        assert!(y + h <= MENU_H, "{screen:?}");
        assert!(x > 12.0, "{screen:?}: should align with the menu");
        if let Screen::OptionsPage(page) = screen
            && !matches!(page, Page::Addons | Page::NewGameAddons)
        {
            assert!(y > PAGE_ROW0 + menu.page_vis(page) as f32 * ROW_H);
        }
        menu.mouse_move((x + w / 2.0, y + h / 2.0), &mut cv);
        menu.key(K_MOUSE1, &mut cv, false);
        assert_eq!(menu.screen, parent, "{screen:?}");
    }
    menu.open(Screen::OptionsPage(Page::Mouse));
    let short_y = menu.back_button().unwrap().1;
    menu.open(Screen::OptionsPage(Page::Graphics));
    assert!(menu.back_button().unwrap().1 > short_y);
}

#[test]
fn all_tooltips_fit_within_screen_width_and_portal_tooltips_stay_single_line() {
    let mut canvas = Canvas::new(1280, 720);
    canvas.ui = Some(Default::default());
    canvas.menu_layout();
    for item in [
        Item::Choice { label: "", cvar: "r_portal_quality", choices: &[] },
        Item::Toggle { label: "", cvar: "addon_portal_view" },
        Item::Choice { label: "", cvar: "addon_portal_max", choices: &[] },
        Item::Choice { label: "", cvar: "addon_portal_depth", choices: &[] },
        Item::Slider { label: "", cvar: "addon_portal_dist", min: 0.0, max: 1.0, step: 1.0, invert: false },
    ] {
        let text = tooltip(item).unwrap();
        let lines = wrap_tooltip_lines(&canvas, text, 588.0, TXT);
        assert_eq!(lines.len(), 1, "portal tooltip should fit on one line: {text}");
    }
    let mut menu = Menu::new();
    for &(_, page) in &PAGES {
        for sub in std::iter::once(page).chain(
            page_items(page).iter().filter_map(|i| if let Item::Sub { page: p, .. } = *i { Some(p) } else { None }),
        ) {
            menu.open(Screen::OptionsPage(sub));
            for row in 0..menu.page_len(sub) {
                menu.opt_cursor = row;
                if let Some(s) = tooltip(menu.page_item(sub, row)) {
                    for line in wrap_tooltip_lines(&canvas, s, 588.0, TXT) {
                        assert!(canvas.text_w(&line, TXT) <= 588.0, "tooltip line overflow on {sub:?}: {line}");
                    }
                }
            }
        }
    }
}

#[test]
fn text_input_only_on_text_rows() {
    let mut cv = Cvars::default();
    cv.register("_cl_name", "", 0);
    let mut m = Menu::new();
    m.open(Screen::Options);
    assert!(!m.wants_text(), "the hub search types from key presses");
    m.open(Screen::OptionsPage(Page::Setup));
    m.opt_cursor = 0;
    assert!(m.wants_text(), "Player Setup > Name");
    m.key(K_DOWNARROW, &mut cv, false);
    assert!(!m.wants_text(), "colour row");
    m.key(K_UPARROW, &mut cv, false);
    m.text('Q', &mut cv);
    m.text(':', &mut cv);
    assert_eq!(cv.by_name("_cl_name").map(|c| c.string.as_str()), Some("Q:"));
    // the on-screen keyboard types for itself
    m.key(K_ABUTTON, &mut cv, false);
    assert!(!m.wants_text());
}

#[test]
fn back_and_delete_buttons_draw_glyphs_in_controller_mode() {
    let mut menu = Menu::new();
    menu.open(Screen::Options);
    let mut kb = Canvas::new(640, 360);
    kb.ui = Some(Default::default());
    menu.pad_ui = false;
    menu.draw_back(&mut kb);
    let mut pad = Canvas::new(640, 360);
    pad.ui = Some(Default::default());
    menu.pad_ui = true;
    menu.draw_back(&mut pad);
    assert!(pad.list.len() > kb.list.len(), "Back button should draw controller glyph in pad mode");

    // Save/Load Delete button widens for the X glyph in controller mode.
    let sv = SaveSlot {
        name: "s0".into(),
        info: crate::saveinfo::SaveInfo {
            level: "e1m1".into(),
            map: "e1m1".into(),
            skill: 1,
            time: 10.0,
            modified: None,
            health: 100,
            armor: 0,
            items: 0,
            kills: (0, 0),
            secrets: (0, 0),
            reason: None,
            addons: None,
            players: (1, 1),
            player: Default::default(),
        },
        thumb: None,
        source: SaveSource::Own,
    };
    menu.saves = vec![sv.clone()];
    menu.latest = Some(sv);
    menu.open(Screen::Load);
    menu.pad_ui = false;
    let kb_rect = menu.delete_button().unwrap();
    menu.pad_ui = true;
    let pad_rect = menu.delete_button().unwrap();
    assert!(pad_rect.2 > kb_rect.2, "Delete button should widen for controller glyph");
    assert!((pad_rect.0 + pad_rect.2 - (kb_rect.0 + kb_rect.2)).abs() < 0.01, "right edge stays aligned");
}
