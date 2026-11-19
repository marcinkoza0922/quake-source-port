use super::*;

fn file(name: &str, auto: bool) -> DemoFile {
    let dir = if auto { "/u/autodemos" } else { "/u" };
    DemoFile { path: PathBuf::from(format!("{dir}/{name}.dem")), name: name.into(), auto, bytes: 2048, modified: 0, map: None, duration: Some(61.0) }
}

fn names(b: &DemoBrowser) -> Vec<String> {
    (0..b.rows()).filter_map(|v| b.row(v)).map(|d| d.name.clone()).collect()
}

fn typed(b: &mut DemoBrowser, s: &str) {
    for ch in s.bytes() {
        b.key(ch as i32);
    }
}

#[test]
fn keys_and_buttons_pick_actions() {
    let mut b = DemoBrowser::default();
    b.set(vec![file("a", false), file("b", false)], vec![file("20240101-120000_e1m1", true)], true);
    assert_eq!(b.key(K_DOWNARROW), DemoAction::Moved);
    assert_eq!(b.key(K_ENTER), DemoAction::Play("b.dem".into()));
    // saved demos can't be kept; letters filter instead of acting
    assert!(!b.enabled(DemoBtn::Keep));
    assert_eq!(b.action(DemoBtn::Keep), DemoAction::None);
    assert_eq!(b.action(DemoBtn::Rename), DemoAction::Rename { file: "b.dem".into(), name: "b".into(), auto: false });
    assert_eq!(b.key(K_RIGHTARROW), DemoAction::Moved);
    assert_eq!(b.key(K_LTRIGGER), DemoAction::Keep("autodemos/20240101-120000_e1m1.dem".into()));
    assert_eq!(b.key(K_XBUTTON), DemoAction::Export("autodemos/20240101-120000_e1m1.dem".into()));
    // renaming an auto-demo suggests its kept name
    assert_eq!(b.key(K_YBUTTON), DemoAction::Rename { file: "autodemos/20240101-120000_e1m1.dem".into(), name: "e1m1_20240101-120000".into(), auto: true });
    assert_eq!(b.key(K_DEL), DemoAction::Delete("autodemos/20240101-120000_e1m1.dem".into(), "20240101-120000_e1m1".into()));
    assert_eq!(b.key(K_ESCAPE), DemoAction::Back);
    assert_eq!(auto_label(&b.lists[1][0]), "2024-01-01 12:00  e1m1");
}

#[test]
fn typing_filters_and_tab_sorts() {
    let mut b = DemoBrowser::default();
    let mut long = file("speedrun", false);
    long.duration = Some(300.0);
    long.map = Some("e1m3".into());
    let mut short = file("coop night", false);
    short.map = Some("e2m1".into());
    b.set(vec![file("ambush", false), long, short], vec![file("20240101-120000_e1m3", true)], true);
    // 'e' and 'k' used to be shortcuts: now they filter, across names and maps
    typed(&mut b, "e1m3");
    assert_eq!(names(&b), ["speedrun"]);
    assert_eq!(b.view[1].len(), 1, "the automatic list is filtered too");
    assert_eq!(b.key(K_BACKSPACE), DemoAction::Moved);
    assert_eq!(b.key(K_ESCAPE), DemoAction::Moved, "Esc clears the filter first");
    assert_eq!(names(&b), ["ambush", "speedrun", "coop night"]);
    b.key(K_DOWNARROW);
    assert_eq!(b.key(K_TAB), DemoAction::Moved);
    assert_eq!(b.sort, DemoSort::Name);
    assert_eq!(names(&b), ["ambush", "coop night", "speedrun"]);
    assert_eq!(b.selected().map(|d| d.name.as_str()), Some("speedrun"), "sorting keeps the selection");
    b.key(K_TAB);
    assert_eq!(names(&b), ["ambush", "speedrun", "coop night"], "by map, unknown first");
    b.key(K_TAB);
    assert_eq!(names(&b)[0], "speedrun", "longest first");
    typed(&mut b, "zzz");
    assert!(b.selected().is_none());
    assert!(!b.enabled(DemoBtn::Rename) && b.action(DemoBtn::Play) == DemoAction::None);
}

#[test]
fn clicks_select_then_play_and_wheel_scrolls() {
    let mut b = DemoBrowser::default();
    b.set((0..40).map(|i| file(&format!("d{i:02}"), false)).collect(), vec![], false);
    let row = |r: usize| (LIST.0 + 20.0, TOP + r as f32 * ROW_H + 2.0);
    assert_eq!(b.click(row(3)), DemoAction::Moved);
    assert_eq!(b.click(row(3)), DemoAction::Play("d03.dem".into()));
    let (bx, by, ..) = button_rect(2);
    assert_eq!(b.hit((bx + 4.0, by + 4.0)), Some(Hit::Button(DemoBtn::Rename)));
    assert!(matches!(b.click((bx + 4.0, by + 4.0)), DemoAction::Rename { .. }));
    // the wheel and the bar scroll the view, not the selection
    b.wheel(false);
    assert_eq!(b.scroll[0], 3);
    assert_eq!(b.selected().map(|d| d.name.as_str()), Some("d03"));
    let (_, top, h, _, th) = b.scrollbar().unwrap();
    b.drag_to(top + h, th / 2.0);
    assert_eq!(b.scroll[0], 40 - ROWS);
    // moving the selection brings it back into view
    b.key(K_DOWNARROW);
    assert_eq!(b.scroll[0], 4);
}

#[test]
fn selection_survives_rescan_and_opens_on_auto_when_empty() {
    let mut b = DemoBrowser::default();
    b.set(vec![file("a", false), file("b", false)], vec![], false);
    b.key(K_DOWNARROW);
    b.set(vec![file("new", false), file("a", false), file("b", false)], vec![], false);
    assert_eq!(b.selected().map(|d| d.name.as_str()), Some("b"));
    let mut e = DemoBrowser::default();
    e.set(vec![], vec![file("20240101-120000_e1m1", true)], true);
    e.open();
    assert_eq!(e.tab, 1);
    // a renamed auto-demo is selected in the saved list
    e.set(vec![file("mine", false)], vec![], true);
    e.focus(Path::new("/u/mine.dem"));
    assert_eq!((e.tab, e.selected().map(|d| d.name.as_str())), (0, Some("mine")));
}

#[test]
fn menu_routes_wheel_typing_and_rename_dialog() {
    use crate::menu::Menu;
    let mut cv = qcore::cvar::Cvars::default();
    cv.register("_demo_rename", "", 0);
    let mut m = Menu::new();
    m.demos.set((0..40).map(|i| file(&format!("d{i:02}"), false)).collect(), vec![file("20240101-120000_e1m1", true)], true);
    m.open(Screen::Demos);
    // the wheel scrolls the list instead of moving the selection
    m.key(K_MWHEELDOWN, &mut cv, false);
    assert_eq!((m.demos.scroll[0], m.demos.selected().map(|d| d.name.clone())), (3, Some("d00".into())));
    // letters filter rather than act
    for k in *b"d39" {
        assert!(m.key(k as i32, &mut cv, false).iter().all(|a| !matches!(a, Action::Command(_))));
    }
    assert_eq!(names(&m.demos), ["d39"]);
    m.key(K_ESCAPE, &mut cv, false);
    // Rename on an auto-demo: the dialog is filled in, typed text is checked,
    // and Enter in the field renames it
    m.key(K_RIGHTARROW, &mut cv, false);
    m.key(K_YBUTTON, &mut cv, false);
    assert_eq!(m.screen, Screen::OptionsPage(Page::RenameDemo));
    assert_eq!(cv.by_name("_demo_rename").unwrap().string, "e1m1_20240101-120000");
    for ch in "/x".chars() {
        m.text(ch, &mut cv);
    }
    assert_eq!(cv.by_name("_demo_rename").unwrap().string, "e1m1_20240101-120000x", "no path separators");
    let out = m.key(K_ENTER, &mut cv, false);
    assert!(out.iter().any(|a| matches!(a, Action::Command(c) if c == "_demo_rename \"autodemos/20240101-120000_e1m1.dem\" \"e1m1_20240101-120000x\"\n")));
    assert_eq!(m.screen, Screen::Demos);
}

#[test]
fn demo_buttons_emit_controller_glyphs_in_pad_mode() {
    let mut b = DemoBrowser::default();
    b.set(vec![file("a", false)], vec![file("20240101-120000_e1m1", true)], true);
    b.tab = 1;
    let glyphs = crate::glyphs::Glyphs::default();
    let mut kb = crate::draw::Canvas::new(640, 360);
    kb.ui = Some(Default::default());
    b.draw(&mut kb, None, false, &glyphs);
    let mut pad = crate::draw::Canvas::new(640, 360);
    pad.ui = Some(Default::default());
    b.draw(&mut pad, None, true, &glyphs);
    assert!(pad.list.len() > kb.list.len(), "Demos tabs, sort and action buttons should draw controller glyphs");
}
