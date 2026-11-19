use super::*;

fn labels(q: &str) -> Vec<&'static str> {
    matches(q).iter().map(|h| h.label).collect()
}

fn type_on_hub(m: &mut Menu, cv: &mut Cvars, s: &str) {
    // key presses, as the host sends them (no text events in the menu)
    for ch in s.chars() {
        m.key(ch as i32, cv, false);
    }
}

fn row_of(p: Page, label: &str) -> usize {
    page_items(p).iter().position(|&i| item_label(i) == Some(label)).unwrap()
}

#[test]
fn names_match_before_search_words() {
    let l = labels("light");
    assert!(l.contains(&"Bloom"), "bloom is found through its search words: {l:?}");
    let first = l.iter().position(|&x| x == "Dynamic Lights").unwrap();
    assert!(first < l.iter().position(|&x| x == "Bloom").unwrap(), "{l:?}");
    // alternate terms
    assert_eq!(labels("vertical sync").first(), Some(&"VSync"));
    assert!(labels("subtitles").contains(&"Sound Captions"));
    assert!(labels("colour").contains(&"Normal Color"));
    assert!(labels("gamepad").contains(&"Controller"));
}

#[test]
fn several_words_and_pages() {
    // page names narrow a word that many pages share
    let l = matches("bloom quality");
    assert_eq!(l[0].label, "Bloom Quality");
    assert!(matches("hit sound volume").iter().any(|h| h.label == "Volume" && h.page() == Page::HitSound));
    // a loose letter match still finds abbreviations
    assert!(labels("mblur").contains(&"Motion Blur"));
    assert!(matches("zzqx").is_empty());
}

#[test]
fn top_level_pages_are_listed() {
    let h = matches("audio");
    assert!(h[0].item.is_none() && h[0].label == "Audio");
    // every row is indexed once
    assert_eq!(index().iter().filter(|h| h.label == "Edit HUD").count(), 1);
    // settings that moved into the HUD editor lead to it
    for q in ["fps", "clock", "captions"] {
        assert!(labels(q).contains(&"Edit HUD"), "{q}: {:?}", labels(q));
    }
    assert!(labels("classic").contains(&"HUD Preset"));
}

#[test]
fn picking_a_result_opens_its_row() {
    let mut cv = Cvars::default();
    cv.register("r_bloom", "1", 0);
    let mut m = Menu::new();
    m.open(Screen::Options);
    type_on_hub(&mut m, &mut cv, "bloom intensity");
    assert!(m.searching());
    let out = m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Bloom));
    assert_eq!(m.opt_cursor, row_of(Page::Bloom, "Bloom Intensity"));
    assert!(out.iter().any(|a| matches!(a, Action::Sound(_))));
    assert_eq!(m.crumbs(Page::Bloom), ["Options", "Enhancements", "Bloom Settings"]);
    // Back follows the breadcrumbs, then the hub still shows the results
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Enhancements));
    assert_eq!(m.opt_cursor, row_of(Page::Enhancements, "Bloom Settings"));
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Options);
    assert!(m.searching());
    // Escape clears the search before leaving the hub
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Options);
    assert!(!m.searching());
    m.key(K_ESCAPE, &mut cv, false);
    assert_eq!(m.screen, Screen::Main);
}

#[test]
fn switched_off_page_selects_its_row_on_the_parent() {
    let mut cv = Cvars::default();
    cv.register("r_bloom", "0", 0);
    let mut m = Menu::new();
    m.open(Screen::Options);
    type_on_hub(&mut m, &mut cv, "bloom intensity");
    m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Enhancements));
    assert_eq!(m.opt_cursor, row_of(Page::Enhancements, "Bloom Settings"));
}

#[test]
fn graphics_rows_refresh_display_modes() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    m.open(Screen::Options);
    type_on_hub(&mut m, &mut cv, "vsync");
    let out = m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::Graphics));
    assert_eq!(m.opt_cursor, row_of(Page::Graphics, "VSync"));
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c == "_vid_menu\n")));
    assert_eq!(m.crumbs(Page::Graphics), ["Options", "Graphics"]);
}

#[test]
fn controller_types_with_the_on_screen_keyboard() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    m.open(Screen::Options);
    m.key(K_YBUTTON, &mut cv, false);
    assert!(m.osk_open());
    // "f" is on the third row; Done leaves the results in place
    for k in [K_DOWNARROW, K_DOWNARROW, K_RIGHTARROW, K_RIGHTARROW, K_RIGHTARROW, K_ABUTTON] {
        m.key(k, &mut cv, false);
    }
    assert_eq!(m.search.query, "f");
    m.key(K_XBUTTON, &mut cv, false);
    assert_eq!(m.search.query, "");
    m.key(K_ABUTTON, &mut cv, false);
    m.key(K_BBUTTON, &mut cv, false);
    assert!(!m.osk_open() && m.searching());
    // results are picked with the pad too
    m.key(K_DOWNARROW, &mut cv, false);
    assert_eq!(m.search.cursor, 1);
}

#[test]
fn hub_mouse_rows_follow_the_results() {
    let mut cv = Cvars::default();
    let mut m = Menu::new();
    m.open(Screen::Options);
    type_on_hub(&mut m, &mut cv, "sensitivity");
    let n = m.search_hits();
    assert!(n > 2);
    assert_eq!(m.item_at((320.0, ROW0 + ROW_H + 1.0)), Some(1));
    assert_eq!(m.item_at((320.0, ROW0 + n.min(SEARCH_ROWS) as f32 * ROW_H + 1.0)), None);
    // a fresh visit to the hub starts with an empty search
    m.open(Screen::Main);
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_DOWNARROW, &mut cv, false);
    m.key(K_ENTER, &mut cv, false);
    assert_eq!(m.screen, Screen::Options);
    assert!(!m.searching());
}

#[test]
fn net_and_sub_page_breadcrumbs() {
    let mut m = Menu::new();
    assert_eq!(m.crumbs(Page::Host), ["Multiplayer", "Host Game"]);
    assert_eq!(m.crumbs(Page::Gyro), ["Options", "Input", "Controller", "Gyro & Flick Stick"]);
    // a page with two parents shows the one it was opened from
    m.sub_return = Some((Page::NewGameAddons, Page::PortalGun, 0));
    let c = m.crumbs(Page::PortalGun);
    assert_eq!(c[c.len() - 2..], [page_name(Page::NewGameAddons), page_name(Page::PortalGun)]);
    m.sub_return = None;
    let c = m.crumbs(Page::PortalGun);
    assert_eq!(c[c.len() - 2..], [page_name(Page::Addons), page_name(Page::PortalGun)]);
}
