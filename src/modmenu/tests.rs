use super::*;
use qui::mods::Base;

fn menu() -> ModMenu {
    let m = |d: &str| ModInfo { dir: d.into(), title: d.into(), base: Base::Id, blurb: vec![] };
    let mut mm = ModMenu::default();
    mm.set_mods(vec![m("id1"), m("ad"), m("mg1")], "ad");
    mm
}

fn cmds(out: &[Action]) -> Vec<String> {
    out.iter().filter_map(|a| if let Action::Command(c) = a { Some(c.clone()) } else { None }).collect()
}

#[test]
fn pick_mod_and_map() {
    let mut mm = menu();
    assert_eq!(mm.cursor, 3, "starts on the running mod, below the pinned row");
    let mut out = Vec::new();
    assert_eq!(mm.key(Page::List, K_UPARROW, &mut out), Some(Page::List));
    assert_eq!(mm.key(Page::List, K_RIGHTARROW, &mut out), Some(Page::Maps));
    assert_eq!(cmds(&out), ["_modmaps \"mg1\"\n"]);
    let maps = ["start", "mg1m1"].map(|n| MapInfo { name: n.into(), title: String::new() }).to_vec();
    mm.set_maps(maps, Stats::default());
    out.clear();
    mm.key(Page::Maps, K_DOWNARROW, &mut out);
    mm.key(Page::Maps, K_DOWNARROW, &mut out);
    out.clear();
    assert_eq!(mm.key(Page::Maps, K_ENTER, &mut out), None);
    assert_eq!(cmds(&out), ["game \"mg1\" \"mg1m1\"\n"]);
    // "Launch mod" on another mod boots it without a map
    mm.map_cursor = 0;
    out.clear();
    mm.key(Page::Maps, K_ENTER, &mut out);
    assert_eq!(cmds(&out), ["game \"mg1\"\n"]);
    assert_eq!(mm.key(Page::Maps, K_ESCAPE, &mut out), Some(Page::List));
    assert_eq!(mm.key(Page::List, K_ESCAPE, &mut out), None);
}

#[test]
fn enter_launches_the_mod() {
    let mut mm = menu();
    let mut out = Vec::new();
    mm.key(Page::List, K_UPARROW, &mut out);
    out.clear();
    assert_eq!(mm.key(Page::List, K_ENTER, &mut out), None);
    assert_eq!(cmds(&out), ["game \"mg1\"\n"]);
    // the pinned top row opens the Quaddicted browser
    mm.cursor = 0;
    out.clear();
    assert_eq!(mm.key(Page::List, K_ENTER, &mut out), Some(Page::Online));
    assert_eq!(cmds(&out), ["_quaddicted\n"]);
    assert_eq!(mm.key(Page::Online, K_ESCAPE, &mut out), Some(Page::List));
    // the running mod just closes the menu
    mm.cursor = 3;
    out.clear();
    mm.key(Page::List, K_ENTER, &mut out);
    assert!(cmds(&out).is_empty() && out.iter().any(|a| matches!(a, Action::Close)));
}

#[test]
fn long_lists_scroll_by_wheel_and_bar() {
    let m = |i: usize| ModInfo { dir: format!("m{i}"), title: String::new(), base: Base::Id, blurb: vec![] };
    let mut mm = ModMenu::default();
    mm.set_mods((0..100).map(m).collect(), "m0");
    assert!(mm.scrollbar(Page::List).is_some());
    mm.wheel(Page::List, false);
    assert_eq!((mm.scroll, mm.cursor), (3, 3));
    mm.drag_to(Page::List, 1000.0, 0.0);
    assert_eq!(mm.scroll, 101 - ROWS);
    mm.drag_to(Page::List, -1000.0, 0.0);
    assert_eq!((mm.scroll, mm.cursor), (0, ROWS - 1));
    mm.set_mods(vec![m(0)], "m0");
    assert!(mm.scrollbar(Page::List).is_none());
}

#[test]
fn text_helpers() {
    assert_eq!(clip("abcdef", 4), "abc~");
    assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
}

#[test]
fn fuzzy_filter_and_sort_mods_and_maps() {
    let mut mm = ModMenu::default();
    let m = |d: &str, t: &str| ModInfo { dir: d.into(), title: t.into(), base: Base::Id, blurb: vec![] };
    mm.set_mods(vec![m("z_mod", "Beta Pack"), m("a_mod", "Arcane Dimensions"), m("m_mod", "Alpha")], "z_mod");
    let mut out = Vec::new();
    // Tab cycles sort: Default -> Name -> Dir
    mm.key(Page::List, K_TAB, &mut out);
    assert_eq!(mm.mod_sort, ModSort::Name);
    assert_eq!(mm.mod_at_row(1).unwrap().1.title, "Alpha");
    mm.key(Page::List, K_TAB, &mut out);
    assert_eq!(mm.mod_sort, ModSort::Dir);
    assert_eq!(mm.mod_at_row(1).unwrap().1.dir, "a_mod");
    // Typing filters immediately without focusing search box
    mm.key(Page::List, b'a' as i32, &mut out);
    mm.key(Page::List, b'd' as i32, &mut out);
    assert_eq!(mm.mod_filter, "ad");
    assert_eq!(mm.mod_view.len(), 1);
    assert_eq!(mm.mod_at_row(0).unwrap().1.dir, "a_mod");
    // Escape clears filter first
    assert_eq!(mm.key(Page::List, K_ESCAPE, &mut out), Some(Page::List));
    assert!(mm.mod_filter.is_empty());
    assert_eq!(mm.mod_view.len(), 3);

    // Maps: filter hides "Launch mod" row and selects top match
    mm.set_maps(
        vec![
            MapInfo { name: "e1m1".into(), title: "Slipgate Complex".into() },
            MapInfo { name: "e1m2".into(), title: "Castle of the Damned".into() },
        ],
        Stats::default(),
    );
    mm.key(Page::Maps, b'c' as i32, &mut out);
    mm.key(Page::Maps, b'a' as i32, &mut out);
    mm.key(Page::Maps, b's' as i32, &mut out);
    assert_eq!(mm.rows(Page::Maps), 1);
    assert_eq!(mm.map_at_row(0).unwrap().name, "e1m2");
    mm.key(Page::Maps, K_BACKSPACE, &mut out);
    assert_eq!(mm.map_filter, "ca");
}

#[test]
fn default_sort_recent_then_official_then_alpha() {
    let mut mm = ModMenu::default();
    let m = |d: &str, t: &str| ModInfo { dir: d.into(), title: t.into(), base: Base::Id, blurb: vec![] };
    mm.set_mods(
        vec![
            m("z_pack", "Zeta"),
            m("mg1", "Dimension of the Machine"),
            m("rogue", "Dissolution of Eternity"),
            m("a_pack", "Alpha"),
            m("id1", "Quake"),
            m("hipnotic", "Scourge of Armagon"),
            m("dopa", "Dimension of the Past"),
        ],
        "id1",
    );
    // Mark z_pack as most recently played (200) and mg1 as played earlier (100)
    mm.set_last_played(vec![200, 100, 0, 0, 0, 0, 0]);
    let dirs: Vec<&str> = (1..8).map(|i| mm.mod_at_row(i).unwrap().1.dir.as_str()).collect();
    assert_eq!(dirs, ["z_pack", "mg1", "id1", "hipnotic", "rogue", "dopa", "a_pack"]);
}
