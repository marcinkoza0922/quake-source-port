use super::*;
use crate::hud::layout::Anchor;
use crate::hud::place;

fn setup() -> (Editor, Cvars) {
    let mut cv = Cvars::new();
    for &(name, default) in &[
        (CVAR, ""),
        ("con_notifytime", "3"),
        ("scr_centertime", "2"),
        ("cl_captions", "0"),
        ("hud_pb_100", "0"),
        ("hud_pb_color", "2"),
        (PRESET, "0"),
        (PRESET_NAME, ""),
    ] {
        cv.register(name, default, 0);
    }
    cv.set(CVAR, &Layout::default_layout().to_text());
    let mut ed = Editor { presets_path: Some(PathBuf::new()), ..Default::default() };
    ed.open(&cv);
    ed.set_frame(&[], 3.0, (1920, 1080));
    (ed, cv)
}

fn kinds(cv: &Cvars) -> Vec<Kind> {
    layout(cv).elems.iter().map(|e| e.kind).collect()
}

#[test]
fn config_is_hidden_until_an_element_is_actively_selected() {
    let (mut ed, mut cv) = setup();
    let l = layout(&cv);
    // Freshly opened: no element selected, no property rows in the dialog
    assert_eq!(ed.sel, None);
    assert!(!ed.rows(&l).iter().any(|r| matches!(r, Row::Prop(_))));
    let compact_h = ed.dims(0, false).h;
    let expanded_h = ed.dims(5, true).h;
    assert!(compact_h < expanded_h);

    // D-pad / arrow input activates selection on the first element
    ed.key(K_DPAD_DOWN, &mut cv);
    assert_eq!(ed.sel, Some(0));
    assert!(ed.rows(&l).iter().any(|r| matches!(r, Row::Prop(_))));

    // Clicking empty space outside dialog & elements deselects
    ed.mouse_move((5.0, 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    assert_eq!(ed.sel, None);
}

#[test]
fn add_prompts_grid_with_sane_defaults_stacks_and_deletes() {
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
    // Immediately prompted for grid position, pre-highlighted at Clock's sane default (TopRight)
    assert_eq!(ed.popup, Some(Popup::AddPlace { kind: Kind::Clock, cursor: Anchor::TopRight as usize }));
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());
    assert_eq!(kinds(&cv).len(), n + 1);
    let added = layout(&cv).elems.last().unwrap().clone();
    assert_eq!((added.kind, added.anchor), (Kind::Clock, Anchor::TopRight));
    assert_eq!(ed.sel, Some(n));

    // Adding FPS at TopRight stacks below the existing Clock at TopRight
    ed.key(K_INS, &mut cv);
    for _ in 0..5 {
        ed.key(K_DOWNARROW, &mut cv);
    }
    ed.key(K_RIGHTARROW, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.popup, Some(Popup::AddPlace { kind: Kind::Fps, cursor: Anchor::TopRight as usize }));
    ed.key(K_ENTER, &mut cv);
    let l = layout(&cv);
    let fps = l.elems.last().unwrap();
    assert_eq!((fps.kind, fps.anchor), (Kind::Fps, Anchor::TopRight));
    assert!(fps.y > added.y, "second TopRight element should stack below first: {fps:?} vs {added:?}");

    // Delete removes FPS, then Clock
    ed.key(K_DEL, &mut cv);
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
fn dragging_moves_elements_reflows_siblings_and_moves_dialog() {
    let (mut ed, mut cv) = setup();
    let l = layout(&cv);
    // HUD 640×360 units at 3×, menu 640×360 at 3×: the units match
    let rects: Vec<Rect> = (0..l.elems.len()).map(|i| Rect { x: 20.0 + i as f32 * 30.0, y: 300.0, w: 20.0, h: 20.0 }).collect();
    ed.set_frame(&rects, 3.0, (1920, 1080));
    ed.mouse_move((25.0, 305.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    // Simply clicking an element selects it without hiding the dialog / starting a drag yet
    assert_eq!((ed.sel, ed.dragging()), (Some(0), None));
    ed.mouse_move((35.0, 205.0), &mut cv);
    assert_eq!(ed.dragging(), Some(0));
    let e = &layout(&cv).elems[0];
    let r = place::wanted(e, (20.0, 20.0), 640.0, 360.0);
    assert_eq!((r.x, r.y), (30.0, 200.0));
    ed.mouse_up(&mut cv);
    assert_eq!(ed.dragging(), None);
    assert!(ed.can_undo());

    // the title bar moves the dialog
    let (x, y) = ed.pos.unwrap();
    ed.mouse_move((x + 20.0, y + 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    ed.mouse_move((x - 80.0, y + 45.0), &mut cv);
    ed.mouse_up(&mut cv);
    assert_eq!(ed.pos, Some((x - 100.0, y + 40.0)));
}

#[test]
fn undo_and_redo_work_with_keyboard_and_right_stick_click() {
    let (mut ed, mut cv) = setup();
    let orig_text = cv.by_name(CVAR).unwrap().string.clone();
    let l = layout(&cv);
    ed.select(0, &l);
    // Step font
    ed.key(K_ENTER, &mut cv);
    ed.key(K_RIGHTARROW, &mut cv);
    let changed_text = cv.by_name(CVAR).unwrap().string.clone();
    assert_ne!(changed_text, orig_text);
    assert!(ed.can_undo());

    // Controller right-stick click (K_RTHUMB) undoes the change
    ed.key(K_RTHUMB, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, orig_text);
    assert!(!ed.can_undo());

    // Ctrl+Y redoes it
    ed.ctrl = true;
    ed.key(b'y' as i32, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, changed_text);

    // Ctrl+Z undoes it again
    ed.key(b'z' as i32, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, orig_text);

    // Ctrl+Shift+Z redoes it again
    ed.shift = true;
    ed.key(b'z' as i32, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, changed_text);
}

#[test]
fn reset_asks_first_and_is_undoable() {
    let (mut ed, mut cv) = setup();
    let before = cv.by_name(CVAR).unwrap().string.clone();
    let l = layout(&cv);
    ed.select(0, &l);
    ed.cursor_to(Row::Buttons(2), &l);
    ed.button = 0;
    ed.key(K_ENTER, &mut cv);
    assert!(ed.confirm_reset);
    assert_ne!(cv.by_name(CVAR).unwrap().string, "default");
    cv.set(PRESET, "2");
    ed.key(K_ENTER, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, "default");
    assert_eq!(cv.value_of(PRESET), 0.0);
    // R-stick click undoes the accidental reset
    ed.key(K_RTHUMB, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, before);
}

#[test]
fn save_as_save_edits_protect_builtins_and_delete_presets() {
    let (mut ed, mut cv) = setup();
    // Starting from built-in modern: cannot overwrite built-in via Save
    assert_eq!(ed.editing_preset, None);
    assert!(!ed.can_save_current(&cv));

    // Customize the layout slightly (make Health appear-on-change)
    let l = layout(&cv);
    ed.select(0, &l);
    ed.cursor_to(Row::Prop(Prop::Show), &l);
    ed.key(K_RIGHTARROW, &mut cv);
    let custom_layout = cv.by_name(CVAR).unwrap().string.clone();
    assert!(custom_layout.contains("health@bl,i"));
    // Still based on a built-in foundation: Save remains disabled
    assert!(!ed.can_save_current(&cv));

    // Open Save As (Row::Buttons(1), col 2 -> button 4)
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 2;
    ed.key(K_ENTER, &mut cv);
    assert!(ed.saving_preset());
    assert!(ed.wants_text());
    // Built-in name "modern" is rejected
    for _ in 0..12 {
        ed.key(K_BACKSPACE, &mut cv);
    }
    for ch in "modern".chars() {
        ed.text(ch);
    }
    ed.key(K_ENTER, &mut cv);
    assert!(ed.saving_preset(), "built-in name must be rejected");
    assert!(ed.presets.is_empty());

    // Type "Minimal" and create new preset
    for _ in 0..12 {
        ed.key(K_BACKSPACE, &mut cv);
    }
    for ch in "Minimal".chars() {
        ed.text(ch);
    }
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());
    assert_eq!(ed.presets.len(), 1);
    assert_eq!(ed.presets[0].name, "Minimal");
    assert_eq!(ed.presets[0].layout, custom_layout);
    assert_eq!(ed.editing_preset.as_deref(), Some("Minimal"));
    assert_eq!(cv.by_name(PRESET_NAME).unwrap().string, "Minimal");
    // Unmodified Minimal: nothing dirty to save yet
    assert!(!ed.can_save_current(&cv));

    // Edit Armor while on Minimal -> now Save IS enabled for saving edits to Minimal!
    let l = layout(&cv);
    ed.select(1, &l);
    ed.cursor_to(Row::Prop(Prop::Show), &l);
    ed.key(K_RIGHTARROW, &mut cv);
    let edited_minimal = cv.by_name(CVAR).unwrap().string.clone();
    assert_ne!(edited_minimal, custom_layout);
    assert!(ed.can_save_current(&cv));
    // Press Save (Row::Buttons(1), col 1 -> button 3) to update Minimal in-place
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 1;
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.presets.len(), 1);
    assert_eq!(ed.presets[0].layout, edited_minimal);
    assert!(!ed.can_save_current(&cv));

    // Save As while editing Minimal suggests a fresh variant name ("Minimal 2") to create a new preset
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 2;
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.popup, Some(Popup::SaveAs { name: "Minimal 2".into(), confirm_overwrite: false }));
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.presets.len(), 2);
    assert_eq!(ed.editing_preset.as_deref(), Some("Minimal 2"));

    // Save As with an existing user preset's name requires overwrite confirmation
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 2;
    ed.key(K_ENTER, &mut cv);
    for _ in 0..12 {
        ed.key(K_BACKSPACE, &mut cv);
    }
    for ch in "Minimal".chars() {
        ed.text(ch);
    }
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.popup, Some(Popup::SaveAs { name: "Minimal".into(), confirm_overwrite: true }));
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());

    // Open Presets (Row::Buttons(1), col 0 -> button 2): built-ins cannot be deleted, user presets can
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 0;
    ed.key(K_ENTER, &mut cv);
    assert!(matches!(ed.popup, Some(Popup::Presets { cursor: 0, .. })));
    // Del on built-in modern (0) and classic (1) does nothing
    ed.key(K_DEL, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_DEL, &mut cv);
    assert_eq!(ed.presets.len(), 2);
    // Load classic (row 1): clears editing_preset so classic can be used as foundation, never overwritten
    ed.key(K_ENTER, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, "classic");
    assert_eq!(ed.editing_preset, None);
    assert!(!ed.can_save_current(&cv));

    // Delete "Minimal" (row 2) via two-step confirmation
    let l = layout(&cv);
    ed.cursor_to(Row::Buttons(1), &l);
    ed.button = 0;
    ed.key(K_ENTER, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_DEL, &mut cv);
    assert_eq!(ed.presets.len(), 2);
    ed.key(K_DEL, &mut cv);
    assert_eq!(ed.presets.len(), 1);
    assert_eq!(ed.presets[0].name, "Minimal 2");
}

#[test]
fn hamburger_collapses_and_expands_the_dialog() {
    let (mut ed, mut cv) = setup();
    let (x, y) = ed.pos.unwrap();
    let full_h = ed.dims(5, true).h;
    assert!(full_h > TITLE_H);
    // click the hamburger button in the top-right corner
    ed.mouse_move((x + W - 10.0, y + 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    assert!(ed.collapsed);
    assert_eq!(ed.dims(5, true).h, TITLE_H);
    // title bar still drags while collapsed
    ed.mouse_move((x + 20.0, y + 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    ed.mouse_move((x - 10.0, y + 25.0), &mut cv);
    ed.mouse_up(&mut cv);
    assert_eq!(ed.pos, Some((x - 30.0, y + 20.0)));
    // clicking the hamburger button again expands it
    let (x2, y2) = ed.pos.unwrap();
    ed.mouse_move((x2 + W - 10.0, y2 + 5.0), &mut cv);
    ed.key(K_MOUSE1, &mut cv);
    assert!(!ed.collapsed);
    assert_eq!(ed.dims(5, true).h, full_h);
}

#[test]
fn controller_navigates_moves_and_configures_elements() {
    let (mut ed, mut cv) = setup();
    ed.pad_ui = true;
    let l = layout(&cv);
    let rects: Vec<Rect> = l.elems.iter().map(|e| place::wanted(e, (20.0, 20.0), 640.0, 360.0)).collect();
    ed.set_frame(&rects, 3.0, (1920, 1080));

    // Starts unselected; first D-pad press selects an element
    assert_eq!(ed.sel, None);
    let n = l.elems.len();
    ed.key(K_DPAD_UP, &mut cv);
    assert_eq!(ed.sel, Some(n - 1));
    ed.key(K_DPAD_DOWN, &mut cv);
    assert_eq!(ed.sel, Some(0));

    // A starts moving the selected element; left stick & D-pad move it
    let orig = (layout(&cv).elems[0].x, layout(&cv).elems[0].y);
    ed.key(K_ABUTTON, &mut cv);
    assert!(ed.moving());
    assert_eq!(ed.dragging(), Some(0));
    ed.pad_stick([1.0, 0.0], [0.0, 0.0], 0.1, &mut cv);
    let after_stick = (layout(&cv).elems[0].x, layout(&cv).elems[0].y);
    assert!(after_stick.0 > orig.0);
    ed.key(K_DPAD_DOWN, &mut cv);
    let after_nudge = (layout(&cv).elems[0].x, layout(&cv).elems[0].y);
    assert_eq!(after_nudge.1, after_stick.1 + 1.0);
    // B cancels and restores original coordinates
    ed.key(K_BBUTTON, &mut cv);
    assert!(!ed.moving());
    assert_eq!((layout(&cv).elems[0].x, layout(&cv).elems[0].y), orig);

    // A again, nudge, A confirms
    ed.key(K_ABUTTON, &mut cv);
    ed.key(K_DPAD_RIGHT, &mut cv);
    ed.key(K_ABUTTON, &mut cv);
    assert!(!ed.moving());
    assert_eq!(layout(&cv).elems[0].x, orig.0 + 1.0);

    // R-stick click undoes the confirmed move
    ed.key(K_RTHUMB, &mut cv);
    assert_eq!((layout(&cv).elems[0].x, layout(&cv).elems[0].y), orig);

    // L-stick click cycles preview state
    assert_eq!(ed.preview, crate::hud::PreviewState::Sample);
    ed.key(K_LTHUMB, &mut cv);
    assert_eq!(ed.preview, crate::hud::PreviewState::Alert);
    ed.key(K_LTHUMB, &mut cv);
    assert_eq!(ed.preview, crate::hud::PreviewState::Live);

    // X jumps to config options; D-pad adjusts; B returns to element list
    ed.key(K_XBUTTON, &mut cv);
    assert!(matches!(ed.rows(&layout(&cv))[ed.cursor], Row::Prop(_)));
    ed.key(K_DPAD_RIGHT, &mut cv);
    assert_eq!(layout(&cv).elems[0].font, crate::hud::kind::Font::Quake);
    assert!(!ed.key(K_BBUTTON, &mut cv).exit);
    assert_eq!(ed.rows(&layout(&cv))[ed.cursor], Row::Elem(0));

    // Right stick moves the HUD editor panel
    let p0 = ed.pos.unwrap();
    ed.pad_stick([0.0, 0.0], [-1.0, 1.0], 0.1, &mut cv);
    let p1 = ed.pos.unwrap();
    assert!(p1.0 < p0.0 && p1.1 > p0.1);

    // RB collapses/expands; LT/RT scale; LB deletes; Back resets (with confirm); Start exits
    ed.key(K_RSHOULDER, &mut cv);
    assert!(ed.collapsed);
    ed.key(K_RSHOULDER, &mut cv);
    assert!(!ed.collapsed);

    let s0 = layout(&cv).elems[0].scale;
    ed.key(K_RTRIGGER, &mut cv);
    assert!(layout(&cv).elems[0].scale > s0);
    ed.key(K_LTRIGGER, &mut cv);
    assert_eq!(layout(&cv).elems[0].scale, s0);

    let count_before = layout(&cv).elems.len();
    ed.key(K_LSHOULDER, &mut cv);
    assert_eq!(layout(&cv).elems.len(), count_before - 1);

    ed.key(K_BACK, &mut cv);
    assert!(ed.confirm_reset);
    ed.key(K_BACK, &mut cv);
    assert_eq!(cv.by_name(CVAR).unwrap().string, "default");

    assert!(ed.key(K_START, &mut cv).exit);
}

#[test]
fn add_automap_element_from_tree_and_edit_options() {
    let (mut ed, mut cv) = setup();
    for &(name, default) in &[
        ("cl_automap_view", "0"),
        ("cl_automap_rotate", "1"),
        ("cl_automap_reveal", "0"),
        ("cl_automap_zoom", "1"),
        ("cl_automap_markers", "1"),
    ] {
        cv.register(name, default, 0);
    }
    let n = kinds(&cv).len();
    ed.key(K_INS, &mut cv);
    // Navigate to the "Automap" category (7th category, index 6).
    for _ in 0..6 {
        ed.key(K_DOWNARROW, &mut cv);
    }
    ed.key(K_RIGHTARROW, &mut cv);
    ed.key(K_DOWNARROW, &mut cv);
    ed.key(K_ENTER, &mut cv);
    // Confirm default grid placement (TopRight)
    assert_eq!(ed.popup, Some(Popup::AddPlace { kind: Kind::Automap, cursor: Anchor::TopRight as usize }));
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());
    let l = layout(&cv);
    assert_eq!(l.elems.len(), n + 1);
    let am = l.elems.last().unwrap();
    assert_eq!(am.kind, Kind::Automap);
    assert_eq!(am.anchor, Anchor::TopRight);
    assert!(am.bg);

    // Background toggles cleanly and View Mode updates cl_automap_view.
    ed.cursor_to(Row::Prop(Prop::Background), &l);
    ed.key(K_RIGHTARROW, &mut cv);
    assert!(!layout(&cv).elems.last().unwrap().bg);
    ed.cursor_to(Row::Prop(Prop::Opt(0)), &l);
    ed.key(K_RIGHTARROW, &mut cv);
    assert_eq!(cv.value_of("cl_automap_view"), 1.0);
}

#[test]
fn add_speedometer_element_from_time_and_stats() {
    let (mut ed, mut cv) = setup();
    let n = kinds(&cv).len();
    ed.key(K_INS, &mut cv);
    // Open "Time & Stats" (5th category, index 4) and step to Speedometer (6th item).
    for _ in 0..4 {
        ed.key(K_DOWNARROW, &mut cv);
    }
    ed.key(K_RIGHTARROW, &mut cv);
    for _ in 0..6 {
        ed.key(K_DOWNARROW, &mut cv);
    }
    ed.key(K_ENTER, &mut cv);
    assert_eq!(ed.popup, Some(Popup::AddPlace { kind: Kind::Speed, cursor: Anchor::TopRight as usize }));
    ed.key(K_ENTER, &mut cv);
    assert!(ed.popup.is_none());
    let l = layout(&cv);
    assert_eq!(l.elems.len(), n + 1);
    let spd = l.elems.last().unwrap();
    assert_eq!((spd.kind, spd.anchor, spd.a), (Kind::Speed, Anchor::TopRight, 1));
    assert!(super::editor_rows::props(Kind::Speed).contains(&Prop::LabelColor));
    assert!(super::editor_rows::props(Kind::Speed).contains(&Prop::TextColor));
    // Toggle Show Label off (a: 1 -> 0) and verify hud_layout round-trips it.
    ed.cursor_to(Row::Prop(Prop::Opt(0)), &l);
    ed.key(K_RIGHTARROW, &mut cv);
    assert_eq!(layout(&cv).elems.last().unwrap().a, 0);
}
