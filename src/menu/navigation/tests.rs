use super::*;

#[test]
fn keyboard_has_no_footer_prompts_and_controller_does() {
    let mut menu = Menu::new();
    let mut canvas = Canvas::new(640, 360);
    canvas.ui = Some(Default::default());
    for screen in [Screen::Main, Screen::SinglePlayer, Screen::Options, Screen::OptionsPage(Page::Controls), Screen::Quit] {
        menu.open(screen);
        menu.pad_ui = false;
        canvas.list.clear();
        menu.draw_hints(&mut canvas);
        assert!(canvas.list.is_empty(), "{screen:?}");
        menu.pad_ui = true;
        menu.draw_hints(&mut canvas);
        assert!(!canvas.list.is_empty(), "{screen:?}");
    }
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
