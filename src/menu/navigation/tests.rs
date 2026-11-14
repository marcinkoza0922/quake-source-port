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
        (Screen::OptionsPage(Page::Addons), Screen::Main),
        (Screen::OptionsPage(Page::NewGameAddons), Screen::SinglePlayer),
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
