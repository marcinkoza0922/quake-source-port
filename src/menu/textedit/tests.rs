use super::*;

/// Player Setup with the cursor on Name = `name`.
fn setup(name: &str) -> (Menu, Cvars) {
    let mut cv = Cvars::default();
    cv.register("_cl_name", name, 0);
    cv.register("_mp_address", "", 0);
    let mut m = Menu::new();
    m.open(Screen::OptionsPage(Page::Setup));
    m.opt_cursor = 0;
    (m, cv)
}

fn name(cv: &Cvars) -> &str {
    &cv.by_name("_cl_name").unwrap().string
}

fn draw_row(m: &Menu, cv: &Cvars, p: Page, row: usize) -> Canvas {
    let mut c = Canvas::new(640, 360);
    m.draw_text_row(&mut c, cv, p, row, PAGE_ROW0 + 2.0, true);
    c
}

#[test]
fn caret_moves_and_selection_is_replaced() {
    let (mut m, mut cv) = setup("ranger");
    m.key(K_HOME, &mut cv, false);
    m.text('X', &mut cv);
    assert_eq!(name(&cv), "Xranger");
    m.key(K_RIGHTARROW, &mut cv, false);
    m.shift = true;
    m.key(K_RIGHTARROW, &mut cv, false);
    m.key(K_RIGHTARROW, &mut cv, false);
    assert_eq!(m.text_selection(&cv).as_deref(), Some("an"));
    m.shift = false;
    m.text('!', &mut cv);
    assert_eq!(name(&cv), "Xr!ger");
    m.key(K_BACKSPACE, &mut cv, false);
    m.key(K_DEL, &mut cv, false);
    assert_eq!(name(&cv), "Xrer");
    m.key(K_END, &mut cv, false);
    m.key(K_BACKSPACE, &mut cv, false);
    assert_eq!(name(&cv), "Xre");
    // Left/Right edit the text; Up/Down still leave the row
    m.key(K_DOWNARROW, &mut cv, false);
    assert_eq!(m.opt_cursor, 1);
}

#[test]
fn select_all_cut_and_paste_keep_the_rules() {
    let (mut m, mut cv) = setup("ranger");
    m.text_select_all(&mut cv);
    assert_eq!(m.text_selection(&cv).as_deref(), Some("ranger"));
    m.text_insert("", &mut cv);
    assert_eq!(name(&cv), "");
    // quotes and semicolons are dropped, the rest cut at the row's 15
    m.text_insert("a\"b;c and a very long name", &mut cv);
    assert_eq!(name(&cv), "abc and a very ");
    assert_eq!(m.text_selection(&cv), None);
}

#[test]
fn click_places_the_caret_and_drag_selects_without_connecting() {
    let mut cv = Cvars::default();
    cv.register("_mp_address", "192.168.1.5:26000", 0);
    let mut m = Menu::new();
    m.open(Screen::OptionsPage(Page::Join));
    m.opt_cursor = 0;
    draw_row(&m, &cv, Page::Join, 0);
    let x_of = |m: &Menu, k: usize| m.text_layouts.borrow()[0].xs[k];
    let (x3, x7) = (x_of(&m, 3), x_of(&m, 7));
    let y = PAGE_ROW0 + 4.0;
    m.mouse_move((x3, y), &mut cv);
    let out = m.key(K_MOUSE1, &mut cv, false);
    assert!(out.iter().all(|a| !matches!(a, Action::Command(_))), "a click must not connect");
    m.mouse_move((x7 + 1.0, y), &mut cv);
    m.mouse_up();
    assert_eq!(m.text_selection(&cv).as_deref(), Some(".168"));
    // the label side selects the row without moving the caret
    m.mouse_move((LABEL_X + 10.0, y), &mut cv);
    m.key(K_MOUSE1, &mut cv, false);
    assert_eq!(m.text_selection(&cv).as_deref(), Some(".168"));
    // Enter still connects
    let out = m.key(K_ENTER, &mut cv, false);
    assert!(out.iter().any(|a| matches!(a, Action::Command(_))));
}

#[test]
fn long_values_scroll_to_the_caret() {
    let mut cv = Cvars::default();
    let path = format!("/home/player/{}/videos", "deep/".repeat(30));
    cv.register("capture_ffmpeg", &path, 0);
    let mut m = Menu::new();
    m.open(Screen::OptionsPage(Page::Video));
    m.opt_cursor = super::super::demos::VIDEO.iter().position(|i| matches!(i, Item::Text { .. })).unwrap();
    let row = m.opt_cursor;
    draw_row(&m, &cv, Page::Video, row);
    let end_first = m.text_layouts.borrow()[0].first;
    assert!(end_first > 0, "the end is shown, the start cut");
    m.key(K_HOME, &mut cv, false);
    draw_row(&m, &cv, Page::Video, row);
    assert_eq!(m.text_layouts.borrow()[0].first, 0);
}
