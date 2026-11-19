use super::*;
use crate::hud::layout::Anchor;

fn setup() -> (Editor, Cvars) {
    let mut cv = Cvars::new();
    for &(name, default) in &[(CVAR, ""), ("con_notifytime", "3"), ("scr_centertime", "2"), ("cl_captions", "0"), ("hud_pb_100", "0"), ("hud_pb_color", "2"), (PRESET, "0")] {
        cv.register(name, default, 0);
    }
    cv.set(CVAR, &Layout::default_layout().to_text());
    let mut ed = Editor::default();
    ed.open();
    ed.set_frame(&[], 3.0, (1920, 1080));
    (ed, cv)
}

fn kinds(cv: &Cvars) -> Vec<Kind> {
    layout(cv).elems.iter().map(|e| e.kind).collect()
}

#[test]
fn add_from_the_tree_and_delete() {
    let (mut ed, mut cv) = setup();
    let n = kinds(&cv).len();
    ed.key(K_INS, &mut cv);
    assert!(matches!(ed.popup, Some(Popup::Add { .. })));
    // open "Time & Stats" (5th category) and pick its first kind, the clock
    for _ in 0..4 {
        ed.key(K_DOWNARROW, &mut cv);
    }
    ed.key(K_RIGHTARROW, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());
    assert_eq!(kinds(&cv).len(), n + 1);
    assert_eq!(*kinds(&cv).last().unwrap(), Kind::Clock);
    assert_eq!(ed.sel, n);
    ed.key(K_DEL, &mut cv);
    assert_eq!(kinds(&cv).len(), n);
    // messages stay
    let center = kinds(&cv).iter().position(|&k| k == Kind::Center).unwrap();
    ed.select(center, &layout(&cv));
    ed.key(K_DEL, &mut cv);
    assert!(kinds(&cv).contains(&Kind::Center));
}

#[test]
fn properties_change_the_selected_element() {
    let (mut ed, mut cv) = setup();
    let l = layout(&cv);
    ed.select(0, &l);
    assert_eq!(l.elems[0].kind, Kind::Health);
    // Enter jumps to the first property (font), right steps it
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.rows(&l)[ed.cursor], Row::Prop(Prop::Font));
    assert_eq!(cv.value_of(PRESET), 0.0);
    ed.key(K_RIGHTARROW, &mut cv);
    assert_eq!(layout(&cv).elems[0].font, crate::hud::kind::Font::Quake);
    // any change makes the HUD custom
    assert_eq!(cv.value_of(PRESET), 2.0);
    // the position grid snaps to an anchor with no offset
    ed.cursor_to(Row::Prop(Prop::Position), &l);
    ed.key(K_ENTER, &mut cv);
    assert!(matches!(ed.popup, Some(Popup::Grid(6))));
    ed.key(K_UPARROW, &mut cv);
    ed.key(K_UPARROW, &mut cv);
    ed.key(K_RIGHTARROW, &mut cv);
    ed.key(K_ENTER, &mut cv);
    let e = &layout(&cv).elems[0];
    assert_eq!((e.anchor, e.x, e.y), (Anchor::TopCenter, 0.0, 0.0));
    // typed coordinates
    ed.cursor_to(Row::Prop(Prop::X), &l);
    ed.key(K_ENTER, &mut cv);
    for ch in "-12".chars() {
        ed.text(ch);
    }
    ed.key(K_ENTER, &mut cv);
    assert_eq!(layout(&cv).elems[0].x, -12.0);
    // Escape leaves the editor
    assert!(ed.key(K_ESCAPE, &mut cv).exit);
}

#[test]
fn dragging_moves_elements_and_the_dialog() {
    let (mut ed, mut cv) = setup();
    let l = layout(&cv);
    // HUD 640×360 units at 3×, menu 640×360 at 3×: the units match
    let rects: Vec<Rect> = (0..l.elems.len()).map(|i| Rect { x: 20.0 + i as f32 * 30.0, y: 300.0, w: 20.0, h: 20.0 }).collect();
    ed.set_frame(&rects, 3.0, (1920, 1080));
    ed.mouse_move((25.0, 305.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    assert_eq!((ed.sel, ed.dragging()), (0, Some(0)));
    ed.mouse_move((35.0, 205.0), &mut cv);
    let e = &layout(&cv).elems[0];
    let r = place::wanted(e, (20.0, 20.0), 640.0, 360.0);
    assert_eq!((r.x, r.y), (30.0, 200.0));
    ed.mouse_up();
    assert_eq!(ed.dragging(), None);
    // the title bar moves the dialog
    let (x, y) = ed.pos.unwrap();
    ed.mouse_move((x + 20.0, y + 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    ed.mouse_move((x - 80.0, y + 45.0), &mut cv);
    ed.mouse_up();
    assert_eq!(ed.pos, Some((x - 100.0, y + 40.0)));
}

#[test]
fn reset_asks_first() {
    let (mut ed, mut cv) = setup();
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 0;
    ed.key(K_ENTER, &mut cv);
    assert!(ed.confirm_reset);
    assert_ne!(cv.by_name(CVAR).unwrap().string, "default");
    cv.set(PRESET, "2");
    ed.key(K_ENTER, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, "default");
    assert_eq!(cv.value_of(PRESET), 0.0);
}
