use super::*;

fn cvars() -> Cvars {
    let mut cv = Cvars::default();
    for &(n, d, f) in qserver::addons::CVARS.iter().chain(qserver::modifiers::CVARS) {
        cv.register(n, d, f);
    }
    cv
}

fn commands(acts: &[Action]) -> Vec<String> {
    acts.iter().filter_map(|a| match a { Action::Command(c) => Some(c.clone()), _ => None }).collect()
}

fn cursor(m: &Menu) -> Option<usize> {
    match m.prompt { Some(Prompt::Launch { cursor }) => Some(cursor), _ => None }
}

/// Single Player → New Game: the pop-up, nothing started yet.
fn popup(cv: &mut Cvars) -> Menu {
    let mut m = Menu::new();
    m.open(Screen::SinglePlayer);
    assert!(commands(&m.key(K_ENTER, cv, false)).is_empty());
    assert_eq!(cursor(&m), Some(0));
    assert_eq!(m.screen, Screen::SinglePlayer);
    m
}

fn button_center(i: usize) -> (f32, f32) {
    (BTN_X + BTN_W / 2.0, BTNS[i].0 + BTNS[i].1 / 2.0)
}

#[test]
fn single_player_has_no_customize_row() {
    let mut cv = cvars();
    let mut m = Menu::new();
    m.open(Screen::SinglePlayer);
    // New Game, Load, Save, Level Select, then back to New Game
    for _ in 0..4 {
        m.key(K_DOWNARROW, &mut cv, false);
    }
    assert_eq!(m.sp_cursor, 0);
}

#[test]
fn play_starts_a_vanilla_game_when_nothing_is_modified() {
    let mut cv = cvars();
    cv.set("custom_enable", "1");
    let mut m = popup(&mut cv);
    assert!(!customized(&cv));
    let acts = m.key(K_ENTER, &mut cv, false);
    assert_eq!(commands(&acts), [NEW_GAME_CMD]);
    assert_eq!(cv.value_of("custom_enable"), 0.0);
    assert_eq!(m.screen, Screen::None);
    assert!(m.prompt.is_none() && m.launch.is_none());
}

#[test]
fn customize_experience_returns_to_the_popup_and_play_is_custom() {
    let mut cv = cvars();
    let mut m = popup(&mut cv);
    m.key(K_DOWNARROW, &mut cv, false);
    assert!(commands(&m.key(K_ENTER, &mut cv, false)).is_empty());
    assert_eq!(m.screen, Screen::OptionsPage(Page::NewGameCustomize));
    assert!(m.prompt.is_none());
    // Modifiers: a harder rule
    m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Modifiers));
    m.key(K_RIGHTARROW, &mut cv, false);
    assert_eq!(cv.value_of("modifier_damagetaken"), 1.25);
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::NewGameCustomize));
    // Back from the hub: the pop-up again, on Customize Experience
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::SinglePlayer);
    assert_eq!(cursor(&m), Some(1));
    assert!(customized(&cv));
    m.key(K_UPARROW, &mut cv, false);
    let acts = m.key(K_ENTER, &mut cv, false);
    assert_eq!(commands(&acts), [NEW_GAME_CMD]);
    assert_eq!(cv.value_of("custom_enable"), 1.0);
}

#[test]
fn the_hub_and_addon_picker_play_the_pending_game() {
    let mut cv = cvars();
    let mut m = Menu::new();
    m.open(Screen::ModMaps);
    m.open_launch("game \"id1\" \"e1m3\"\n".into(), true, Some("The Necropolis".into()));
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ENTER, &mut cv, false);
    // Add-ons: turn the portal gun on, then Play from the picker (Y)
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::NewGameAddons));
    m.key(K_ENTER, &mut cv, false);
    assert_eq!(cv.value_of("addon_portalgun"), 1.0);
    m.key(K_YBUTTON, &mut cv, false);
    // a level asks for the skill over Level Select
    assert_eq!(m.screen, Screen::ModMaps);
    assert!(matches!(m.prompt, Some(Prompt::Skill { .. })));
    assert_eq!(cv.value_of("custom_enable"), 1.0);
    let acts = m.key(b'2' as i32, &mut cv, false);
    assert_eq!(commands(&acts), ["skill 1\ngame \"id1\" \"e1m3\"\n"]);

    // the hub's Play row does the same
    m.open(Screen::ModMaps);
    m.open_launch("game \"id1\" \"e1m3\"\n".into(), true, None);
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ENTER, &mut cv, false);
    m.opt_cursor = NEW_GAME_CUSTOMIZE.len() - 1;
    m.key(K_ENTER, &mut cv, false);
    assert!(matches!(m.prompt, Some(Prompt::Skill { .. })));
    assert_eq!(m.screen, Screen::ModMaps);
}

#[test]
fn escape_cancels_the_popup_without_starting() {
    let mut cv = cvars();
    for key in [K_ESCAPE, K_BBUTTON, K_MOUSE2] {
        let mut m = popup(&mut cv);
        assert!(commands(&m.key(key, &mut cv, false)).is_empty());
        assert!(m.prompt.is_none() && m.launch.is_none());
        assert_eq!(m.screen, Screen::SinglePlayer);
    }
}

#[test]
fn disable_all_works_from_every_input() {
    let modify = |cv: &mut Cvars| {
        cv.set("modifier_horde", "2");
        cv.set("modifier_infiniteammo", "1");
        cv.set("addon_portalgun", "1");
        cv.set("addon_hitstop", "1");
        cv.set("addon_portalgun_slot", "7");
        assert!(customized(cv));
    };
    let check = |cv: &Cvars, m: &Menu, how: &str| {
        assert!(!customized(cv), "{how}");
        assert_eq!(qserver::modifiers::changed(cv).count(), 0, "{how}");
        assert_eq!(cv.value_of("addon_portalgun"), 0.0, "{how}");
        // per-add-on settings survive
        assert_eq!(cv.value_of("addon_portalgun_slot"), 7.0, "{how}");
        // the pop-up stays, back on Play
        assert_eq!(cursor(m), Some(0), "{how}");
    };
    let mut cv = cvars();
    for key in [K_DEL, K_BACKSPACE, K_XBUTTON] {
        modify(&mut cv);
        let mut m = popup(&mut cv);
        assert!(commands(&m.key(key, &mut cv, false)).is_empty());
        check(&cv, &m, &format!("key {key}"));
    }
    // arrows + Enter (and the d-pad + A)
    for (down, select) in [(K_DOWNARROW, K_ENTER), (K_DPAD_DOWN, K_ABUTTON)] {
        modify(&mut cv);
        let mut m = popup(&mut cv);
        m.key(down, &mut cv, false);
        m.key(down, &mut cv, false);
        assert_eq!(cursor(&m), Some(2));
        m.key(select, &mut cv, false);
        check(&cv, &m, "navigate");
    }
    // mouse: hover highlights, click disables
    modify(&mut cv);
    let mut m = popup(&mut cv);
    assert!(m.mouse_move(button_center(2), &mut cv).is_some());
    assert_eq!(cursor(&m), Some(2));
    m.key(K_MOUSE1, &mut cv, false);
    check(&cv, &m, "mouse");
    // with nothing modified the button is gone: the shortcut does nothing
    assert_eq!(m.launch_at(button_center(2), &cv), None);
    m.key(K_DEL, &mut cv, false);
    assert_eq!(cursor(&m), Some(0));
}

#[test]
fn mouse_clicks_play_and_customize() {
    let mut cv = cvars();
    let mut m = popup(&mut cv);
    m.mouse_move(button_center(1), &mut cv);
    assert_eq!(cursor(&m), Some(1));
    m.key(K_MOUSE1, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::NewGameCustomize));
    m.key(K_ESCAPE, &mut cv, false);
    m.mouse_move(button_center(0), &mut cv);
    let acts = m.key(K_MOUSE1, &mut cv, false);
    assert_eq!(commands(&acts), [NEW_GAME_CMD]);
    // a click outside the buttons does nothing
    let mut m = popup(&mut cv);
    m.mouse_move((BOX.0 + 4.0, BOX.1 + 4.0), &mut cv);
    assert!(commands(&m.key(K_MOUSE1, &mut cv, false)).is_empty());
    assert_eq!(cursor(&m), Some(0));
}

#[test]
fn popup_draws_with_and_without_modifications() {
    let mut cv = cvars();
    let m = popup(&mut cv);
    let mut canvas = Canvas::new(640, 360);
    canvas.ui = Some(Default::default());
    m.draw_launch(&mut canvas, &cv, 0);
    let plain = canvas.list.len();
    cv.set("modifier_regen", "1");
    canvas.list.clear();
    m.draw_launch(&mut canvas, &cv, 2);
    // tag and Disable All button add draws
    assert!(canvas.list.len() > plain);
}

#[test]
fn map_of_reads_level_commands() {
    assert_eq!(map_of("game \"id1\" \"e1m3\"\n"), Some("e1m3"));
    assert_eq!(map_of("game \"mg1\"\n"), None);
}
