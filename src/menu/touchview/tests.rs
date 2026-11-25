use super::home::Tile;
use super::page::SHEET_ROW;
use super::rows::Kind;
use super::*;

/// A landscape phone: 2400x1080 pixels at 2.625 pixels per unit.
fn phone(m: &mut Menu, screen: Screen) {
    m.open(screen);
    m.set_touch_ui(true);
    m.display_scale = 2.625;
    layout(m, 2400, 1080);
}

fn layout(m: &mut Menu, w: u32, h: u32) {
    let mut c = Canvas::new(w, h);
    m.canvas_layout(&mut c);
}

fn mid([x, y, w, h]: Rect) -> (f32, f32) {
    (x + w / 2.0, y + h / 2.0)
}

fn tap(m: &mut Menu, cv: &mut Cvars, pos: (f32, f32)) -> Vec<Action> {
    m.mouse_move(pos, cv);
    let running = m.game_running;
    let out = m.key(K_MOUSE1, cv, running);
    m.mouse_up();
    out
}

/// Scrolls row `i` of page `p` to the top of the list.
fn reveal(m: &mut Menu, p: Page, i: usize) {
    let (top, _) = m.tv_row_span(p, i);
    m.scroll_touch(top);
}

fn row_of(p: Page, f: impl Fn(&Item) -> bool) -> usize {
    page_items(p).iter().position(f).unwrap()
}

fn audio_cvars() -> Cvars {
    let mut cv = Cvars::default();
    for (n, d) in [("volume", "0.7"), ("bgmvolume", "1"), ("s_footsteps", "1"), ("s_reverb", "1"), ("s_reverb_amount", "1")] {
        cv.register(n, d, 0);
    }
    cv
}

#[test]
fn units_follow_the_display_scale_with_a_minimum_size() {
    let mut m = Menu::new();
    phone(&mut m, Screen::Options);
    let (w, h) = m.tv_size();
    assert!((w - 2400.0 / 2.625).abs() < 0.5 && (h - 1080.0 / 2.625).abs() < 0.5, "{w}x{h}");
    // a huge scale still leaves 320 units on the short side
    m.display_scale = 6.0;
    layout(&mut m, 2400, 1080);
    assert!((m.tv_size().1 - 320.0).abs() < 0.01);
    // the layout starts at the safe area
    m.display_scale = 1.0;
    m.touch_insets = [0.1, 0.0, 0.0, 0.0];
    let mut c = Canvas::new(2400, 1080);
    m.canvas_layout(&mut c);
    assert!((c.ox - 240.0).abs() < 0.01 && (m.tv_size().0 - 2160.0).abs() < 0.01);
}

#[test]
fn touch_layout_only_while_touch_is_active_and_no_dialog_is_open() {
    let mut m = Menu::new();
    m.open(Screen::Options);
    assert!(!m.tv_screen());
    m.set_touch_ui(true);
    assert!(m.tv_input());
    // dialogs keep the classic layout's input
    m.osk = Some(Osk::default());
    assert!(m.tv_screen() && !m.tv_input());
    m.osk = None;
    // screens without a touch layout yet
    m.open(Screen::DirBrowser);
    assert!(!m.tv_screen());
    m.open(Screen::Demos);
    assert!(m.tv_screen());
}

#[test]
fn rows_are_finger_sized() {
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::Audio));
    for i in 0..m.page_len(Page::Audio) {
        let [_, _, _, h] = m.tv_row_rect(Page::Audio, i);
        // 48 units is the platforms' minimum touch target
        assert!(h >= 48.0 || matches!(m.page_item(Page::Audio, i), Item::Heading(_)), "row {i}: {h}");
    }
    // a portrait phone puts controls under their labels, but not switches
    layout(&mut m, 1080, 2400);
    let g = m.tv_geom(Page::Audio);
    assert!(g.stacked);
    let foot = row_of(Page::Audio, |i| matches!(i, Item::Toggle { cvar: "s_footsteps", .. }));
    let vol = row_of(Page::Audio, |i| matches!(i, Item::Slider { cvar: "volume", .. }));
    assert_eq!(m.tv_row_rect(Page::Audio, foot)[3], ROW);
    assert_eq!(m.tv_row_rect(Page::Audio, vol)[3], STACK_ROW);
}

#[test]
fn tapping_a_row_toggles_it_and_reset_in_the_footer_restores_it() {
    let mut cv = audio_cvars();
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::Audio));
    let foot = row_of(Page::Audio, |i| matches!(i, Item::Toggle { cvar: "s_footsteps", .. }));
    reveal(&mut m, Page::Audio, foot);
    // anywhere on the row, the label included
    let [x, y, _, h] = m.tv_row_rect(Page::Audio, foot);
    tap(&mut m, &mut cv, (x + 30.0, y + h / 2.0));
    assert_eq!(cv.value_of("s_footsteps"), 0.0);
    assert_eq!((m.opt_cursor, m.tv.footer), (foot, Some(foot)));
    // changed from its default: the footer offers Reset
    let g = m.tv_geom(Page::Audio);
    let (_, reset) = m.tv_footer_rects(&cv, Page::Audio, &g).unwrap();
    tap(&mut m, &mut cv, mid(reset.unwrap()));
    assert_eq!(cv.value_of("s_footsteps"), 1.0);
    assert!(m.tv_footer_rects(&cv, Page::Audio, &g).is_none_or(|(_, r)| r.is_none()));
}

#[test]
fn slider_steps_drags_and_reports_its_track() {
    let mut cv = audio_cvars();
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::Audio));
    let p = Page::Audio;
    let row = row_of(p, |i| matches!(i, Item::Slider { cvar: "volume", .. }));
    let g = m.tv_geom(p);
    let ctrl = m.tv_ctrl_rect(&g, m.tv_row_rect(p, row), Kind::Slider);
    let (minus, plus, track) = Menu::tv_steps(ctrl, Kind::Slider);
    tap(&mut m, &mut cv, mid(plus));
    assert!((cv.value_of("volume") - 0.8).abs() < 1e-4);
    tap(&mut m, &mut cv, mid(minus));
    tap(&mut m, &mut cv, mid(minus));
    assert!((cv.value_of("volume") - 0.6).abs() < 1e-4);
    // the host drags only from the track, not the buttons
    assert_eq!(m.slider_at(mid(track), &cv), Some(row));
    assert_eq!(m.slider_at(mid(plus), &cv), None);
    // press on the track, then drag to its ends
    m.mouse_move(mid(track), &mut cv);
    m.key(K_MOUSE1, &mut cv, false);
    m.mouse_move((track[0] + track[2], track[1]), &mut cv);
    assert_eq!(cv.value_of("volume"), 1.0);
    m.mouse_move((track[0] - 50.0, track[1]), &mut cv);
    assert_eq!(cv.value_of("volume"), 0.0);
    m.mouse_up();
    m.mouse_move((track[0] + track[2], track[1]), &mut cv);
    assert_eq!(cv.value_of("volume"), 0.0);
}

#[test]
fn long_lists_open_a_sheet_and_short_ones_show_every_value() {
    let mut cv = Cvars::default();
    let p = Page::Crosshair;
    let (row, cvar, choices) = page_items(p)
        .iter()
        .enumerate()
        .find_map(|(i, it)| match *it {
            Item::Choice { cvar, choices, .. } if choices.len() > 3 => Some((i, cvar, choices)),
            _ => None,
        })
        .unwrap();
    cv.register(cvar, &choices[0].1.to_string(), 0);
    cv.register("crosshair", "1", 0);
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(p));
    let g = m.tv_geom(p);
    assert_eq!(m.tv_kind(page_items(p)[row], &g), Kind::List);
    let pos = mid(m.tv_row_rect(p, row));
    tap(&mut m, &mut cv, pos);
    let s = m.tv.sheet.expect("sheet open");
    let (panel, top, _) = m.tv_sheet_geom(s);
    tap(&mut m, &mut cv, (panel[0] + 80.0, top + 2.5 * SHEET_ROW - s.scroll));
    assert!(m.tv.sheet.is_none());
    assert_eq!(cv.value_of(cvar), choices[2].1);
    // a tap outside, or Back, closes it unchanged
    let pos = mid(m.tv_row_rect(p, row));
    tap(&mut m, &mut cv, pos);
    tap(&mut m, &mut cv, (2.0, 2.0));
    assert!(m.tv.sheet.is_none());
    let pos = mid(m.tv_row_rect(p, row));
    tap(&mut m, &mut cv, pos);
    m.key(K_ESCAPE, &mut cv, false);
    assert!(m.tv.sheet.is_none());
    assert_eq!(m.screen, Screen::OptionsPage(p));
    assert_eq!(cv.value_of(cvar), choices[2].1);
    // two or three short values are buttons
    assert!(matches!(m.tv_kind(Item::Choice { label: "x", cvar: "y", choices: &[("off", 0.0), ("on", 1.0)] }, &g), Kind::Segments(2)));
}

#[test]
fn home_tiles_lead_to_the_hub_and_pages_and_back() {
    let mut cv = audio_cvars();
    let mut m = Menu::new();
    m.game_running = true;
    phone(&mut m, Screen::Main);
    let (primary, _) = m.tv_home_tiles();
    assert_eq!(primary, [Tile::Resume, Tile::Save, Tile::Load, Tile::Options]);
    let rect = |m: &Menu, t: Tile| m.tv_home_rects().into_iter().find(|r| r.0 == t).unwrap().1;
    for (_, r, _) in m.tv_home_rects() {
        assert!(r[3] >= 48.0);
    }
    let pos = mid(rect(&m, Tile::Options));
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::Options);
    let (p, _, card) = m.tv_hub_cards()[6];
    assert_eq!(p, Page::Audio);
    tap(&mut m, &mut cv, mid(card));
    assert_eq!(m.screen, Screen::OptionsPage(Page::Audio));
    let pos = mid(m.tv_back_rect());
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::Options);
    let pos = mid(m.tv_back_rect());
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::Main);
    let pos = mid(rect(&m, Tile::Resume));
    let out = tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::None);
    assert!(out.iter().any(|a| matches!(a, Action::Close)));
    // without a game: no Resume or Save
    m.game_running = false;
    phone(&mut m, Screen::Main);
    let (primary, rest) = m.tv_home_tiles();
    assert!(!primary.contains(&Tile::Resume) && !primary.contains(&Tile::Save) && rest.contains(&Tile::Quit));
}

#[test]
fn load_from_home_returns_home() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    phone(&mut m, Screen::Main);
    let r = m.tv_home_rects().into_iter().find(|r| r.0 == Tile::Load).unwrap().1;
    tap(&mut m, &mut cv, mid(r));
    assert_eq!(m.screen, Screen::Load);
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Main);
}

#[test]
fn search_field_takes_text_and_results_open_their_row() {
    let mut cv = audio_cvars();
    let mut m = Menu::new();
    phone(&mut m, Screen::Options);
    assert!(!m.wants_text());
    let pos = mid(m.tv_search_rect());
    tap(&mut m, &mut cv, pos);
    assert!(m.wants_text(), "the field asks for the OS keyboard");
    for ch in "footsteps".chars() {
        m.text(ch, &mut cv);
    }
    assert_eq!(m.search.query, "footsteps");
    let k = m.search_results().iter().position(|h| h.label == "Footsteps").unwrap();
    let [x, y, w, _] = m.tv_search_rect();
    tap(&mut m, &mut cv, (x + w / 2.0, y + 48.0 + 12.0 + (k as f32 + 0.5) * 60.0));
    assert_eq!(m.screen, Screen::OptionsPage(Page::Audio));
    assert!(matches!(m.page_item(Page::Audio, m.opt_cursor), Item::Toggle { cvar: "s_footsteps", .. }));
    assert!(!m.wants_text());
}

#[test]
fn drags_scroll_long_pages_within_their_content() {
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::Gameplay));
    assert_eq!(m.scroll_touch(-50.0), Some(false));
    assert_eq!(m.scroll_touch(120.0), Some(true));
    assert_eq!(m.tv_scroll(), 120.0);
    while m.scroll_touch(500.0) == Some(true) {}
    let last = m.page_len(Page::Gameplay) - 1;
    let [_, y, _, h] = m.tv_row_rect(Page::Gameplay, last);
    // the last row ends above the footer
    assert!(y + h <= m.tv_size().1 - FOOT_H, "{} > {}", y + h, m.tv_size().1 - FOOT_H);
    // a new screen starts at the top
    m.open(Screen::OptionsPage(Page::Audio));
    m.tv_sync();
    assert_eq!(m.tv_scroll(), 0.0);
}

/// Button `k` of the open touch dialog.
fn dialog_button(m: &Menu, cv: &Cvars, k: usize) -> (f32, f32) {
    let ask = m.tv_ask().expect("a dialog is open");
    let (_, rects, _) = m.tv_dialog_geom(&m.tv_dialog(ask, cv));
    mid(rects[k])
}

#[test]
fn quit_dialog_buttons() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    phone(&mut m, Screen::Main);
    m.open(Screen::Quit);
    assert!(m.tv_dialog_open());
    let pos = dialog_button(&m, &cv, 1);
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::Main);
    m.open(Screen::Quit);
    let pos = dialog_button(&m, &cv, 0);
    let out = tap(&mut m, &mut cv, pos);
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c == "quit\n")));
}

#[test]
fn dialogs_take_taps_over_classic_screens() {
    let mut cv = Cvars::default();
    cv.register("skill", "1", 0);
    let mut m = Menu::new();
    // the skill picker over Map Packs (no touch layout of its own here)
    phone(&mut m, Screen::Mods);
    m.ask_skill("map e1m1\n".into(), &cv);
    assert!(m.tv_input() && m.tv_dialog_open());
    let mut c = Canvas::new(2400, 1080);
    m.canvas_layout(&mut c);
    assert_eq!(c.scale, 2.625, "taps are in touch units while the dialog is up");
    let pos = dialog_button(&m, &cv, 3);
    let out = tap(&mut m, &mut cv, pos);
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c == "skill 3\nmap e1m1\n")));
    // a tap outside a confirmation cancels it
    m.open(Screen::Save);
    m.confirm(vec!["Overwrite?".into()], qcore::t!("Overwrite"), "save s0\n".to_string(), true);
    let out = tap(&mut m, &mut cv, (1.0, 1.0));
    assert!(m.prompt.is_none() && !out.iter().any(|a| matches!(a, Action::Command(_))));
    // the New Game pop-up: Cancel is its last button
    phone(&mut m, Screen::Main);
    m.open_launch("map start\n".into(), false, None, &mut cv);
    let ask = m.tv_ask().unwrap();
    let n = m.tv_dialog(ask, &cv).buttons.len();
    let pos = dialog_button(&m, &cv, n - 1);
    tap(&mut m, &mut cv, pos);
    assert!(m.prompt.is_none());
    assert_eq!(m.screen, Screen::Main);
}

#[test]
fn single_player_and_multiplayer_are_tiles() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    phone(&mut m, Screen::Multiplayer);
    assert!(m.tv_screen());
    let r = m.tv_home_rects().into_iter().find(|r| r.0 == Tile::Setup).unwrap().1;
    tap(&mut m, &mut cv, mid(r));
    assert_eq!(m.screen, Screen::OptionsPage(Page::Setup));
    assert!(m.tv_screen(), "Player Setup has a touch layout");
    phone(&mut m, Screen::SinglePlayer);
    let (big, _) = m.tv_home_tiles();
    assert_eq!(big, [Tile::NewGame, Tile::Load]);
    let r = m.tv_home_rects().into_iter().find(|r| r.0 == Tile::Load).unwrap().1;
    tap(&mut m, &mut cv, mid(r));
    assert_eq!(m.screen, Screen::Load);
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::SinglePlayer);
}

fn mod_info(dir: &str, title: &str) -> qui::mods::ModInfo {
    qui::mods::ModInfo { dir: dir.into(), title: title.into(), base: qui::mods::Base::Id, blurb: vec!["A test pack.".into()] }
}

#[test]
fn list_screens_select_then_act_and_filter_from_the_keyboard() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    m.set_mods(vec![mod_info("id1", "Quake"), mod_info("ad", "Arcane Dimensions")], "id1");
    phone(&mut m, Screen::Mods);
    let v = m.tv_list().expect("Map Packs is a touch list");
    let g = m.tv_list_geom(&v);
    let ad = v.rows.iter().position(|r| r.title == "Arcane Dimensions").unwrap();
    let row = |m: &Menu, i: usize| {
        let v = m.tv_list().unwrap();
        let g = m.tv_list_geom(&v);
        [g.list[0], g.list[1] + 4.0 + i as f32 * 60.0, g.list[2], 60.0]
    };
    // a tap selects; the info and buttons follow the selection
    let pos = mid(row(&m, ad));
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.mods.tv_cursor(modmenu::Page::List), ad);
    assert_eq!(m.tv_list().unwrap().info[0].0, "Arcane Dimensions");
    // Level Select (its second button) opens the pack's maps
    let pos = mid(m.tv_list_geom(&m.tv_list().unwrap()).actions[1]);
    let out = tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::ModMaps);
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c.contains("_modmaps \"ad\""))));
    // back, then filter with the OS keyboard
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Mods);
    let pos = mid(g.field.unwrap());
    tap(&mut m, &mut cv, pos);
    assert!(m.wants_text());
    for ch in "arc".chars() {
        m.text(ch, &mut cv);
    }
    // the same letters as keys are ignored (they arrive as text too)
    m.key(b'x' as i32, &mut cv, false);
    let v = m.tv_list().unwrap();
    assert_eq!(v.filter.as_deref(), Some("arc"));
    assert_eq!(v.rows.len(), 1);
    // filtering selects the first match; a tap on the selected row runs
    // its first button (Launch mod)
    assert_eq!(m.mods.tv_cursor(modmenu::Page::List), 0);
    let pos = mid(row(&m, 0));
    let out = tap(&mut m, &mut cv, pos);
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c == "game \"ad\"\n")));
}

#[test]
fn help_opens_a_section_and_back_returns_to_the_list() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    phone(&mut m, Screen::Help);
    let v = m.tv_list().unwrap();
    assert!(v.opens && v.rows.len() > 3);
    let g = m.tv_list_geom(&v);
    tap(&mut m, &mut cv, (g.list[0] + 40.0, g.list[1] + 4.0 + 2.5 * 60.0));
    assert_eq!(m.help.section, 2);
    assert!(m.tv.help_reading && m.tv_list().is_none());
    let pos = mid(m.tv_back_rect());
    tap(&mut m, &mut cv, pos);
    assert_eq!(m.screen, Screen::Help);
    assert!(!m.tv.help_reading);
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Main);
}

#[test]
fn weapon_priority_moves_with_buttons() {
    let mut cv = Cvars::default();
    cv.register("cl_weaponpriority", qserver::weaponswitch::DEFAULT_PRIORITY, 0);
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::WeaponPriority));
    let before = qserver::weaponswitch::read_priority(&cv);
    let (_, up, _) = m.tv_wp_rects(1);
    tap(&mut m, &mut cv, mid(up));
    let after = qserver::weaponswitch::read_priority(&cv);
    assert_eq!((after[0], after[1]), (before[1], before[0]));
    let pos = mid(m.tv_wp_reset());
    tap(&mut m, &mut cv, pos);
    assert_eq!(qserver::weaponswitch::read_priority(&cv), before);
}

#[test]
fn text_rows_take_the_keyboard_only_once_tapped() {
    let mut cv = multiplayer_cvars();
    let mut m = Menu::new();
    phone(&mut m, Screen::OptionsPage(Page::Setup));
    assert!(matches!(m.page_item(Page::Setup, m.opt_cursor), Item::Text { .. }));
    assert!(!m.wants_text(), "opening the page doesn't raise the keyboard");
    let pos = mid(m.tv_row_rect(Page::Setup, 0));
    tap(&mut m, &mut cv, pos);
    assert!(m.wants_text());
    m.text('!', &mut cv);
    assert_eq!(cv.by_name("_cl_name").unwrap().string, "player!");
}

fn multiplayer_cvars() -> Cvars {
    let mut cv = Cvars::default();
    cv.register("_cl_name", "player", 0);
    cv.register("_cl_color", "0", 0);
    cv
}
