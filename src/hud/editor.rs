//! The HUD editor (Options → HUD → Edit HUD): a dialog that can be moved
//! or collapsed, listing every element with its scale and offset, the
//! selected element's settings (shown once an element is actively selected),
//! and buttons to add (from a tree of kinds followed by a position grid),
//! delete, reset and finish. Elements can be dragged on screen with website-
//! style reflow displacement, snap guides, and undo/redo (`Ctrl+Z` or R3).

use std::path::PathBuf;

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::PreviewState;
use super::editor_rows::{self, Prop};
use super::editor_undo::Snapshot;
use super::kind::{CATEGORIES, Kind};
use super::layout::Layout;
use super::place::Rect;
use super::presets::{self, PRESET_NAME, SavedPreset};

pub const CVAR: &str = "hud_layout";
/// 0 modern, 1 classic, 2 custom (or saved preset named by `hud_preset_name`), 3 mod HUD.
pub const PRESET: &str = "hud_preset";

/// Dialog geometry, in menu units.
pub(super) const W: f32 = 240.0;
pub(super) const TITLE_H: f32 = 14.0;
pub(super) const ROW: f32 = 11.0;
pub(super) const LIST_ROWS: usize = 8;
pub(super) const BTN_H: f32 = 14.0;
/// Buttons across three rows: row 0 (2), row 1 (3), row 2 (2).
pub(super) const BUTTONS: [&str; 7] = ["Add", "Delete", "Presets", "Save", "Save As", "Reset to Defaults", "Done"];

pub(super) fn button_row_cols(row: usize) -> usize {
    if row == 1 { 3 } else { 2 }
}

pub(super) fn button_index(row: usize, col: usize) -> usize {
    match row {
        0 => col.min(1),
        1 => 2 + col.min(2),
        _ => 5 + col.min(1),
    }
}

pub(super) fn button_row_col(b: usize) -> (usize, usize) {
    match b {
        0 | 1 => (0, b),
        2..=4 => (1, b - 2),
        _ => (2, b.saturating_sub(5).min(1)),
    }
}
/// Add Element tree and position grid popups.
pub(super) const ADD_W: f32 = 150.0;
pub(super) const CELL: (f32, f32) = (18.0, 13.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Elem(usize),
    /// Button row 0 or 1.
    Buttons(usize),
    Prop(Prop),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AddRow {
    Cat(usize),
    Kind(Kind),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Popup {
    Add { open: Vec<bool>, cursor: usize },
    /// Picking the grid position for a newly added element, cursor 0..9.
    AddPlace { kind: Kind, cursor: usize },
    /// Anchor grid for the selected element, cursor 0..9.
    Grid(usize),
    /// Preset picker: 0 modern, 1 classic, 2.. user-saved presets.
    Presets { cursor: usize, confirm_delete: Option<usize> },
    /// Typing a name to create a new preset (confirms before overwriting an existing user preset).
    SaveAs { name: String, confirm_overwrite: bool },
}

#[derive(Clone, Debug)]
pub(super) enum Drag {
    Dialog((f32, f32)),
    /// Element, pointer offset from its top left (HUD units), press point (HUD units),
    /// whether the pointer has actually dragged past the threshold, original rect and pre-drag state.
    Elem { elem: usize, grab: (f32, f32), start: (f32, f32), moved: bool, origin: Rect, before: Snapshot },
    /// Controller moving an element: index, original `(x, y)` for cancel, original rect,
    /// floating-point screen position in HUD units, and pre-drag state.
    Pad { elem: usize, orig: (f32, f32), origin: Rect, want: Option<(f32, f32)>, before: Snapshot },
}

/// Screen sizes: HUD and menu pixels per unit, the menu's centred origin,
/// and the screen in menu and HUD units.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Geo {
    pub hud: f32,
    pub menu: f32,
    pub ox: f32,
    pub oy: f32,
    pub sw: f32,
    pub sh: f32,
    pub vw: f32,
    pub vh: f32,
}

/// What a key or click asks of the menu.
#[derive(Default)]
pub struct Out {
    pub sound: Option<&'static str>,
    pub exit: bool,
}

pub(super) fn sound(s: &'static str) -> Out {
    Out { sound: Some(s), exit: false }
}

#[derive(Default)]
pub struct Editor {
    /// Actively selected element (layout index); None until the user picks one.
    pub sel: Option<usize>,
    /// True when the last input came from a gamepad (keeps list/options navigation separated).
    pub pad_ui: bool,
    /// Modifier keys synced by the host before `key()`.
    pub ctrl: bool,
    pub shift: bool,
    /// Preview state shown while editing (Sample, Alert, Live).
    pub preview: PreviewState,
    pub(super) cursor: usize,
    /// Column of the focused button.
    pub(super) button: usize,
    pub(super) scroll: usize,
    pub(super) collapsed: bool,
    pub(super) pos: Option<(f32, f32)>,
    pub(super) drag: Option<Drag>,
    pub(super) popup: Option<Popup>,
    pub(super) typing: Option<(Prop, String)>,
    pub(super) confirm_reset: bool,
    /// Pointer in menu units from the screen's top left.
    pub(super) mouse: Option<(f32, f32)>,
    pub(super) rects: Vec<Rect>,
    pub(super) geo: Geo,
    pub(super) undo_stack: Vec<Snapshot>,
    pub(super) redo_stack: Vec<Snapshot>,
    /// Active snap guide coordinates (vertical x lines, horizontal y lines) in HUD units.
    pub(super) snap_guides: (Vec<f32>, Vec<f32>),
    /// User-saved HUD presets loaded from `<user data>/hud_presets/`.
    pub presets: Vec<SavedPreset>,
    /// Override directory for presets persistence (empty path disables disk writes in unit tests).
    pub(super) presets_path: Option<PathBuf>,
    /// Name of the user-saved preset currently being edited (None for built-in modern/classic or unsaved custom).
    pub editing_preset: Option<String>,
    /// True when the active mod provides a Simple CSQC HUD (`Mod HUD` preset available).
    pub has_mod_hud: bool,
}

pub(super) fn layout(cv: &Cvars) -> Layout {
    Layout::parse(cv.by_name(CVAR).map_or("", |c| c.string.as_str()))
}

/// Writes the edited layout; it is now an unsaved custom layout.
pub(super) fn save(cv: &mut Cvars, l: &Layout) {
    cv.set(CVAR, &l.to_text());
    cv.set(PRESET, "2");
    cv.set(PRESET_NAME, "");
}

impl Editor {
    /// Starts editing: no element selected yet, popups and history reset.
    /// Remembers which user-saved preset is active (built-ins are never overwritable).
    pub fn open(&mut self, cv: &Cvars) {
        let pos = self.pos;
        let pad_ui = self.pad_ui;
        let has_mod_hud = self.has_mod_hud;
        let presets_path = self.presets_path.clone();
        let presets = match &presets_path {
            Some(p) if p.as_os_str().is_empty() => std::mem::take(&mut self.presets),
            Some(p) => presets::load_from(p),
            None => presets::load(),
        };
        let editing_preset = if cv.value_of(PRESET) as i32 == 2 {
            let name = cv.by_name(PRESET_NAME).map_or("", |c| c.string.trim());
            presets::find(&presets, name).map(|p| p.name.clone())
        } else {
            None
        };
        *self = Editor { pos, pad_ui, geo: self.geo, presets, presets_path, editing_preset, has_mod_hud, ..Default::default() };
    }

    /// The frame's placement and screen, from the host.
    pub fn set_frame(&mut self, rects: &[Rect], hud_scale: f32, (w, h): (u32, u32)) {
        let mut c = crate::draw::Canvas::new(w, h);
        c.menu_layout();
        let (sw, sh) = (w as f32 / c.scale, h as f32 / c.scale);
        self.geo = Geo { hud: hud_scale, menu: c.scale, ox: c.ox, oy: c.oy, sw, sh, vw: w as f32 / hud_scale, vh: h as f32 / hud_scale };
        self.rects.clear();
        self.rects.extend_from_slice(rects);
        let (x, y) = self.pos.unwrap_or((sw - W - 8.0, 8.0));
        self.pos = Some((x.clamp(0.0, (sw - W).max(0.0)), y.clamp(0.0, (sh - 40.0).max(0.0))));
    }

    pub(super) fn rows(&self, l: &Layout) -> Vec<Row> {
        let mut out: Vec<Row> = (0..l.elems.len()).map(Row::Elem).collect();
        out.extend([Row::Buttons(0), Row::Buttons(1), Row::Buttons(2)]);
        if let Some(e) = self.sel.and_then(|i| l.elems.get(i)) {
            out.extend(editor_rows::props(e.kind).into_iter().map(Row::Prop));
        }
        out
    }

    pub(super) fn toggle_collapse(&mut self) -> Out {
        self.collapsed = !self.collapsed;
        if self.collapsed {
            self.popup = None;
            self.typing = None;
            self.confirm_reset = false;
        }
        sound("misc/menu2.wav")
    }

    pub(super) fn clamp(&mut self, l: &Layout) {
        if let Some(i) = self.sel {
            if l.elems.is_empty() {
                self.sel = None;
            } else {
                self.sel = Some(i.min(l.elems.len() - 1));
            }
        }
        let n = self.rows(l).len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
        self.scroll = self.scroll.min(l.elems.len().saturating_sub(LIST_ROWS));
    }

    pub(super) fn select(&mut self, i: usize, l: &Layout) {
        if l.elems.is_empty() {
            self.sel = None;
            return;
        }
        let i = i.min(l.elems.len() - 1);
        self.sel = Some(i);
        self.cursor = i;
        if i < self.scroll {
            self.scroll = i;
        } else if i >= self.scroll + LIST_ROWS {
            self.scroll = i + 1 - LIST_ROWS;
        }
        self.clamp(l);
    }

    pub(super) fn cursor_to(&mut self, row: Row, l: &Layout) {
        if let Some(i) = self.rows(l).iter().position(|&r| r == row) {
            self.cursor = i;
        }
    }

    /// Moves the cursor a row; landing on an element selects it.
    /// When no element is selected yet, the first arrow/D-pad/stick move selects one.
    /// With a gamepad, list and property navigation stay within their section.
    fn move_cursor(&mut self, dir: i32, l: &Layout) -> Out {
        if self.sel.is_none() && !l.elems.is_empty() {
            let i = if dir >= 0 { 0 } else { l.elems.len() - 1 };
            self.select(i, l);
            return sound("misc/menu1.wav");
        }
        let rows = self.rows(l);
        if self.pad_ui {
            match rows.get(self.cursor).copied() {
                Some(Row::Elem(i)) if !l.elems.is_empty() => {
                    let next = (i as i32 + dir).rem_euclid(l.elems.len() as i32) as usize;
                    self.select(next, l);
                    return sound("misc/menu1.wav");
                }
                Some(Row::Prop(_)) => {
                    let first = rows.iter().position(|r| matches!(r, Row::Prop(_))).unwrap_or(0);
                    let count = rows.len() - first;
                    if count > 0 {
                        let off = (self.cursor - first) as i32;
                        self.cursor = first + (off + dir).rem_euclid(count as i32) as usize;
                        return sound("misc/menu1.wav");
                    }
                }
                _ => {}
            }
        }
        let n = rows.len() as i32;
        self.cursor = (self.cursor as i32 + dir).rem_euclid(n) as usize;
        if let Row::Elem(i) = rows[self.cursor] {
            self.select(i, l);
        }
        sound("misc/menu1.wav")
    }

    /// Quick-nudge the selected element with Shift+Arrows (records undo).
    fn quick_nudge(&mut self, dx: f32, dy: f32, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(i) = self.sel else { return Out::default() };
        let Some(e) = l.elems.get_mut(i) else { return Out::default() };
        self.push_undo_now(cv);
        e.x = (e.x + dx).round();
        e.y = (e.y + dy).round();
        save(cv, l);
        sound("misc/menu1.wav")
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars) -> Out {
        let mut l = layout(cv);
        self.clamp(&l);
        if let Some(t) = self.typing.take() {
            return self.type_key(key, t, &mut l, cv);
        }
        if self.moving() {
            return self.pad_move_key(key, &mut l, cv);
        }
        if key != K_MOUSE1 && key != K_BACK && !matches!(self.rows(&l).get(self.cursor), Some(Row::Buttons(2))) {
            self.confirm_reset = false;
        }
        if self.popup.is_some() {
            return self.popup_key(key, &mut l, cv);
        }
        // Undo / Redo shortcuts (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y, and controller Right-Stick click)
        if key == K_RTHUMB {
            return self.undo(cv);
        }
        if self.ctrl && key == b'z' as i32 {
            return if self.shift { self.redo(cv) } else { self.undo(cv) };
        }
        if self.ctrl && key == b'y' as i32 {
            return self.redo(cv);
        }
        if key == K_LTHUMB {
            self.preview = self.preview.next();
            return sound("misc/menu1.wav");
        }
        let row = self.rows(&l).get(self.cursor).copied();
        // Shift+Arrow nudges the selected element cleanly from the list
        if self.shift && self.sel.is_some() && matches!(row, Some(Row::Elem(_))) {
            let step = if self.ctrl { 8.0 } else { 4.0 };
            match key {
                K_UPARROW | K_KP_UPARROW => return self.quick_nudge(0.0, -step, &mut l, cv),
                K_DOWNARROW | K_KP_DOWNARROW => return self.quick_nudge(0.0, step, &mut l, cv),
                K_LEFTARROW => return self.quick_nudge(-step, 0.0, &mut l, cv),
                K_RIGHTARROW => return self.quick_nudge(step, 0.0, &mut l, cv),
                _ => {}
            }
        }
        match key {
            K_ESCAPE => Out { sound: Some("misc/menu3.wav"), exit: true },
            K_BBUTTON => match row {
                Some(Row::Prop(_) | Row::Buttons(_)) => {
                    if let Some(i) = self.sel {
                        self.select(i, &l);
                    }
                    sound("misc/menu3.wav")
                }
                _ => Out { sound: Some("misc/menu3.wav"), exit: true },
            },
            K_MOUSE1 => self.click(&mut l, cv),
            K_MWHEELUP | K_MWHEELDOWN => {
                let up = key == K_MWHEELUP;
                let d = self.dims(0, self.sel.is_some());
                let over_list = !self.collapsed && self.mouse.is_some_and(|(_, my)| (d.list_y..d.list_y + LIST_ROWS as f32 * ROW).contains(&my));
                if over_list {
                    self.scroll = if up { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
                    self.clamp(&l);
                    Out::default()
                } else {
                    self.move_cursor(if up { -1 } else { 1 }, &l)
                }
            }
            K_UPARROW | K_DPAD_UP | K_KP_UPARROW => self.move_cursor(-1, &l),
            K_DOWNARROW | K_DPAD_DOWN | K_KP_DOWNARROW => self.move_cursor(1, &l),
            K_LEFTARROW | K_DPAD_LEFT | K_RIGHTARROW | K_DPAD_RIGHT => {
                let dir = if matches!(key, K_LEFTARROW | K_DPAD_LEFT) { -1 } else { 1 };
                match row {
                    Some(Row::Buttons(r)) => {
                        let cols = button_row_cols(r) as i32;
                        self.button = (self.button.min(cols as usize - 1) as i32 + dir).rem_euclid(cols) as usize;
                        sound("misc/menu1.wav")
                    }
                    Some(Row::Prop(Prop::Position)) => self.open_grid(&l),
                    Some(Row::Prop(p)) => self.step(p, dir, &mut l, cv),
                    _ => Out::default(),
                }
            }
            K_ABUTTON => match row {
                Some(Row::Elem(_)) => {
                    if self.sel.is_none() && !l.elems.is_empty() {
                        self.select(0, &l);
                        sound("misc/menu1.wav")
                    } else {
                        self.start_move(&l, cv)
                    }
                }
                Some(Row::Buttons(r)) => self.press(button_index(r, self.button), &mut l, cv),
                Some(Row::Prop(p @ (Prop::X | Prop::Y))) => self.step(p, 1, &mut l, cv),
                Some(Row::Prop(p)) => self.activate(p, 1, &mut l, cv),
                None => Out::default(),
            },
            K_ENTER | K_KP_ENTER => match row {
                Some(Row::Elem(_)) => self.focus_props(&l),
                Some(Row::Buttons(r)) => self.press(button_index(r, self.button), &mut l, cv),
                Some(Row::Prop(p)) => self.activate(p, 1, &mut l, cv),
                None => Out::default(),
            },
            K_XBUTTON => match row {
                Some(Row::Prop(_)) => {
                    if let Some(i) = self.sel {
                        self.select(i, &l);
                    }
                    sound("misc/menu1.wav")
                }
                _ => self.focus_props(&l),
            },
            K_DEL | K_LSHOULDER => self.press(1, &mut l, cv),
            K_YBUTTON | K_INS => {
                self.collapsed = false;
                self.press(0, &mut l, cv)
            }
            K_RSHOULDER => self.toggle_collapse(),
            K_BACK => {
                self.collapsed = false;
                self.press(5, &mut l, cv)
            }
            K_START => self.press(6, &mut l, cv),
            K_LTRIGGER | K_RTRIGGER => self.scale_selected(if key == K_LTRIGGER { -1 } else { 1 }, &mut l, cv),
            _ => Out::default(),
        }
    }

    /// Button `b`: 0 Add, 1 Delete, 2 Presets, 3 Save (current user preset), 4 Save As (new preset),
    /// 5 Reset to Defaults, 6 Done.
    pub(super) fn press(&mut self, b: usize, l: &mut Layout, cv: &mut Cvars) -> Out {
        match b {
            0 => {
                self.popup = Some(Popup::Add { open: vec![false; CATEGORIES.len()], cursor: 0 });
                sound("misc/menu2.wav")
            }
            1 => {
                let Some(sel) = self.sel else { return sound("misc/menu3.wav") };
                if l.elems.get(sel).is_none_or(|e| e.kind.permanent()) {
                    return sound("misc/menu3.wav");
                }
                self.push_undo_now(cv);
                l.elems.remove(sel);
                save(cv, l);
                let row = self.cursor;
                self.clamp(l);
                if row < l.elems.len()
                    && let Some(i) = self.sel
                {
                    self.select(i, l);
                }
                sound("misc/menu2.wav")
            }
            2 => {
                self.popup = Some(Popup::Presets { cursor: 0, confirm_delete: None });
                sound("misc/menu2.wav")
            }
            3 => self.save_current_preset(l, cv),
            4 => {
                let name = presets::next_variant_name(&self.presets, self.editing_preset.as_deref());
                self.popup = Some(Popup::SaveAs { name, confirm_overwrite: false });
                sound("misc/menu2.wav")
            }
            5 => {
                if self.confirm_reset {
                    self.push_undo_now(cv);
                    if self.has_mod_hud {
                        cv.set(CVAR, "mod");
                        cv.set(PRESET, "3");
                    } else {
                        cv.set(CVAR, "default");
                        cv.set(PRESET, "0");
                    }
                    cv.set(PRESET_NAME, "");
                    self.editing_preset = None;
                    self.confirm_reset = false;
                    self.sel = None;
                    self.cursor = 0;
                } else {
                    self.confirm_reset = true;
                }
                sound("misc/menu2.wav")
            }
            _ => Out { sound: Some("misc/menu2.wav"), exit: true },
        }
    }

    /// The element being actively dragged and its pre-drag rectangle (for reflow placement
    /// and hiding the dialog). A plain click without dragging does not trigger this.
    pub fn drag_info(&self) -> Option<(usize, Rect)> {
        match &self.drag {
            Some(Drag::Elem { elem, moved: true, origin, .. }) | Some(Drag::Pad { elem, origin, .. }) => Some((*elem, *origin)),
            _ => None,
        }
    }

    /// The element being dragged (for tests).
    #[cfg(test)]
    pub fn dragging(&self) -> Option<usize> {
        self.drag_info().map(|(i, _)| i)
    }
}

#[cfg(test)]
mod tests;
