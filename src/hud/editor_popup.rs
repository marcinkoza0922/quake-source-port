//! The HUD editor's popups: the Add tree of element kinds by category,
//! the grid-position prompt when adding a new element, the anchor grid
//! behind the Position button, the Presets loader/deleter, and Save Preset.

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::editor::{ADD_W, AddRow, BTN_H, CELL, CVAR, Editor, Out, PRESET, Popup, ROW, W, save, sound};
use super::editor_draw::Dims;
use super::editor_rows;
use super::kind::{CATEGORIES, Kind};
use super::layout::{Anchor, Elem, Layout};
use super::place::{self, Rect};
use super::presets::{self, PRESET_NAME};

/// Dimensions of the grid, presets and save-preset popups (in menu units).
pub(super) const GRID_W: f32 = 86.0;
pub(super) const GRID_H: f32 = 3.0 * CELL.1 + ROW + 12.0;
pub(super) const ADD_PLACE_W: f32 = 116.0;
pub(super) const ADD_PLACE_H: f32 = ROW * 2.0 + 3.0 * CELL.1 + 14.0;
pub(super) const PRESETS_W: f32 = 196.0;
pub(super) const PRESETS_DEL_W: f32 = 46.0;
pub(super) const SAVE_W: f32 = 176.0;
pub(super) const SAVE_H: f32 = 56.0;

pub(super) fn add_rows(open: &[bool]) -> Vec<AddRow> {
    let mut out = Vec::new();
    for (i, (_, kinds)) in CATEGORIES.iter().enumerate() {
        out.push(AddRow::Cat(i));
        if open.get(i).copied().unwrap_or(false) {
            out.extend(kinds.iter().map(|&k| AddRow::Kind(k)));
        }
    }
    out
}

/// Top-left of the 3×3 cell grid inside a grid popup at `(px, py)` of width `pw`.
pub(super) fn grid_cells_origin(px: f32, py: f32, pw: f32, header_rows: usize) -> (f32, f32) {
    let gx = (px + (pw - 3.0 * CELL.0) / 2.0).floor();
    let gy = py + 4.0 + header_rows as f32 * ROW;
    (gx, gy)
}

/// Buttons inside the Save Preset popup at `(px, py)`: `(save_rect, cancel_rect)`.
pub(super) fn save_popup_buttons(px: f32, py: f32) -> (Rect, Rect) {
    let bw = (SAVE_W - 16.0) / 2.0;
    let by = py + SAVE_H - BTN_H - 5.0;
    (
        Rect { x: px + 6.0, y: by, w: bw, h: BTN_H },
        Rect { x: px + 10.0 + bw, y: by, w: bw, h: BTN_H },
    )
}

impl Editor {
    /// Reloads saved presets from disk.
    pub fn reload_presets(&mut self) {
        self.presets = match &self.presets_path {
            Some(p) if p.as_os_str().is_empty() => return,
            Some(p) => presets::load_from(p),
            None => presets::load(),
        };
    }

    pub(super) fn persist_saved_preset(&self, preset: &presets::SavedPreset) {
        match &self.presets_path {
            Some(p) if p.as_os_str().is_empty() => {}
            Some(p) => { let _ = presets::write_one_to(p, preset); }
            None => { let _ = presets::write_one(preset); }
        }
    }

    pub(super) fn persist_delete_preset(&self, name: &str) {
        match &self.presets_path {
            Some(p) if p.as_os_str().is_empty() => {}
            Some(p) => { let _ = presets::delete_one_in(p, name); }
            None => { let _ = presets::delete_one(name); }
        }
    }

    /// True when the editor is accepting text input (coordinate edit or Save Preset name).
    pub fn wants_text(&self) -> bool {
        self.typing.is_some() || self.saving_preset()
    }

    /// True while the Save As name popup is open.
    pub fn saving_preset(&self) -> bool {
        matches!(self.popup, Some(Popup::SaveAs { .. }))
    }

    /// Deletes the last character of the Save As name (used by the on-screen keyboard).
    pub fn preset_backspace(&mut self) {
        if let Some(Popup::SaveAs { name, confirm_overwrite }) = &mut self.popup {
            name.pop();
            *confirm_overwrite = false;
        }
    }

    /// True when editing an existing user-saved preset that has unsaved changes.
    /// Built-in presets (`modern`, `classic`) are never overwritable.
    pub(super) fn can_save_current(&self, cv: &Cvars) -> bool {
        let Some(name) = self.editing_preset.as_deref() else {
            return false;
        };
        let Some(saved) = presets::find(&self.presets, name) else {
            return false;
        };
        super::editor::layout(cv).to_text() != Layout::parse(&saved.layout).to_text()
    }

    /// Saves edits directly to the active user-saved preset (disabled for built-in presets).
    pub(super) fn save_current_preset(&mut self, l: &Layout, cv: &mut Cvars) -> Out {
        let Some(name) = self.editing_preset.clone() else {
            return sound("misc/menu3.wav");
        };
        if presets::find(&self.presets, &name).is_none() {
            return sound("misc/menu3.wav");
        }
        let text = l.to_text();
        let Some(idx) = presets::upsert(&mut self.presets, &name, &text) else {
            return sound("misc/menu3.wav");
        };
        let saved = self.presets[idx].clone();
        self.persist_saved_preset(&saved);
        cv.set(CVAR, &text);
        cv.set(PRESET, "2");
        cv.set(PRESET_NAME, &saved.name);
        sound("misc/menu2.wav")
    }

    /// Height of the Presets popup for the current saved list.
    pub(super) fn presets_popup_h(&self) -> f32 {
        let rows = 2 + self.presets.len();
        (rows + 1) as f32 * ROW + if self.presets.is_empty() { 8.0 } else { 11.0 }
    }

    /// Top left of the open popup.
    pub(super) fn popup_at(&self, d: &Dims, nrows: usize) -> (f32, f32) {
        let side_x = |pw: f32| {
            if d.x + W + 4.0 + pw <= self.geo.sw {
                d.x + W + 4.0
            } else {
                (d.x - pw - 4.0).max(0.0)
            }
        };
        match self.popup {
            Some(Popup::Grid(_)) => {
                let y = d.props_y + 6.0 * ROW;
                ((d.x + W - GRID_W - 6.0).max(0.0), y.min(self.geo.sh - GRID_H - 4.0).max(0.0))
            }
            Some(Popup::AddPlace { .. }) => (side_x(ADD_PLACE_W), d.btn_y.min(self.geo.sh - ADD_PLACE_H - 4.0).max(0.0)),
            Some(Popup::Presets { .. }) => {
                let h = self.presets_popup_h();
                (side_x(PRESETS_W), d.btn_y.min(self.geo.sh - h - 4.0).max(0.0))
            }
            Some(Popup::SaveAs { .. }) => (side_x(SAVE_W), d.btn_y.min(self.geo.sh - SAVE_H - 4.0).max(0.0)),
            _ => {
                let h = (nrows + 1) as f32 * ROW + 6.0;
                (side_x(ADD_W), d.btn_y.min(self.geo.sh - h).max(0.0))
            }
        }
    }

    pub(super) fn open_grid(&mut self, l: &Layout) -> Out {
        let a = self.sel.and_then(|i| l.elems.get(i)).map_or(Anchor::Center, |e| e.anchor);
        self.popup = Some(Popup::Grid(a as usize));
        sound("misc/menu2.wav")
    }

    /// Prompts the user to choose a grid position for `k`, pre-selecting its sane default.
    pub(super) fn prompt_add_place(&mut self, k: Kind) -> Out {
        self.popup = Some(Popup::AddPlace { kind: k, cursor: k.default_anchor() as usize });
        sound("misc/menu2.wav")
    }

    /// Adds `k` at `anchor`, stacking below (or above at bottom anchors) any existing elements there.
    pub(super) fn add_at(&mut self, k: Kind, anchor: Anchor, l: &mut Layout, cv: &mut Cvars) -> Out {
        self.push_undo_now(cv);
        let mut e = Elem::new(k, anchor);
        place::stack_new(&mut e, &l.elems, &self.rects, self.geo.vw, self.geo.vh);
        l.elems.push(e);
        save(cv, l);
        self.popup = None;
        self.select(l.elems.len() - 1, l);
        sound("misc/menu2.wav")
    }

    pub(super) fn set_anchor(&mut self, cell: usize, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(sel) = self.sel else {
            self.popup = None;
            return sound("misc/menu3.wav");
        };
        if let Some(e) = l.elems.get_mut(sel) {
            let fresh = Elem::new(e.kind, Anchor::ALL[cell.min(8)]);
            if (e.anchor, e.x, e.y) != (fresh.anchor, fresh.x, fresh.y) {
                self.push_undo_now(cv);
                (e.anchor, e.x, e.y) = (fresh.anchor, fresh.x, fresh.y);
                save(cv, l);
            }
        }
        self.popup = None;
        sound("misc/menu2.wav")
    }

    /// Creates a new preset under `raw_name` (confirms first if overwriting an existing user preset;
    /// built-in names are always rejected).
    pub(super) fn commit_save_as(&mut self, raw_name: &str, confirm_overwrite: bool, l: &Layout, cv: &mut Cvars) -> Out {
        let clean = presets::clean_name(raw_name);
        if clean.is_empty() || presets::is_reserved(&clean) {
            return sound("misc/menu3.wav");
        }
        if presets::find(&self.presets, &clean).is_some() && !confirm_overwrite {
            self.popup = Some(Popup::SaveAs { name: raw_name.to_string(), confirm_overwrite: true });
            return sound("misc/menu2.wav");
        }
        let text = l.to_text();
        let Some(idx) = presets::upsert(&mut self.presets, &clean, &text) else {
            return sound("misc/menu3.wav");
        };
        let saved = self.presets[idx].clone();
        self.persist_saved_preset(&saved);
        cv.set(CVAR, &text);
        cv.set(PRESET, "2");
        cv.set(PRESET_NAME, &saved.name);
        self.editing_preset = Some(saved.name);
        self.popup = None;
        sound("misc/menu2.wav")
    }

    /// Loads preset row `row` (0 modern, 1 classic, 2.. saved presets), pushing an undo step.
    pub(super) fn load_preset_row(&mut self, row: usize, cv: &mut Cvars) -> Out {
        self.push_undo_now(cv);
        match row {
            0 => {
                cv.set(CVAR, "default");
                cv.set(PRESET, "0");
                cv.set(PRESET_NAME, "");
                self.editing_preset = None;
            }
            1 => {
                cv.set(CVAR, "classic");
                cv.set(PRESET, "1");
                cv.set(PRESET_NAME, "");
                self.editing_preset = None;
            }
            r => {
                let Some(p) = self.presets.get(r - 2).cloned() else {
                    return sound("misc/menu3.wav");
                };
                cv.set(CVAR, &p.layout);
                cv.set(PRESET, "2");
                cv.set(PRESET_NAME, &p.name);
                self.editing_preset = Some(p.name);
            }
        }
        self.popup = None;
        self.sel = None;
        self.cursor = 0;
        sound("misc/menu2.wav")
    }

    fn delete_saved_preset(&mut self, idx: usize, cv: &mut Cvars) -> Out {
        if idx >= self.presets.len() {
            return sound("misc/menu3.wav");
        }
        let removed = self.presets.remove(idx);
        self.persist_delete_preset(&removed.name);
        if cv.by_name(PRESET_NAME).is_some_and(|c| c.string.eq_ignore_ascii_case(&removed.name)) {
            cv.set(PRESET_NAME, "");
        }
        if self.editing_preset.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&removed.name)) {
            self.editing_preset = None;
        }
        let total = 2 + self.presets.len();
        let next_cur = (idx + 2).min(total - 1);
        self.popup = Some(Popup::Presets { cursor: next_cur, confirm_delete: None });
        sound("misc/menu2.wav")
    }

    pub(super) fn popup_key(&mut self, key: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        if key == K_MOUSE1 {
            return self.popup_click(l, cv);
        }
        if let Some(Popup::SaveAs { mut name, confirm_overwrite }) = self.popup.clone() {
            match key {
                K_ESCAPE | K_BBUTTON => {
                    self.popup = None;
                    return sound("misc/menu3.wav");
                }
                K_BACKSPACE | K_XBUTTON => {
                    name.pop();
                    self.popup = Some(Popup::SaveAs { name, confirm_overwrite: false });
                    return Out::default();
                }
                K_ENTER | K_KP_ENTER | K_ABUTTON => return self.commit_save_as(&name, confirm_overwrite, l, cv),
                _ => return Out::default(),
            }
        }
        let back = matches!(key, K_ESCAPE | K_BBUTTON);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON);
        let (dx, dy) = match key {
            K_UPARROW | K_DPAD_UP | K_KP_UPARROW | K_MWHEELUP => (0, -1),
            K_DOWNARROW | K_DPAD_DOWN | K_KP_DOWNARROW | K_MWHEELDOWN => (0, 1),
            K_LEFTARROW | K_DPAD_LEFT => (-1, 0),
            K_RIGHTARROW | K_DPAD_RIGHT => (1, 0),
            _ => (0, 0),
        };
        if back {
            self.popup = None;
            return sound("misc/menu3.wav");
        }
        match self.popup.clone() {
            Some(Popup::Grid(c)) => {
                if enter {
                    return self.set_anchor(c, l, cv);
                }
                let (col, row) = ((c % 3) as i32 + dx, (c / 3) as i32 + dy);
                self.popup = Some(Popup::Grid((row.clamp(0, 2) * 3 + col.clamp(0, 2)) as usize));
                sound("misc/menu1.wav")
            }
            Some(Popup::AddPlace { kind, cursor: c }) => {
                if enter {
                    return self.add_at(kind, Anchor::ALL[c.min(8)], l, cv);
                }
                let (col, row) = ((c % 3) as i32 + dx, (c / 3) as i32 + dy);
                self.popup = Some(Popup::AddPlace { kind, cursor: (row.clamp(0, 2) * 3 + col.clamp(0, 2)) as usize });
                sound("misc/menu1.wav")
            }
            Some(Popup::Presets { mut cursor, mut confirm_delete }) => {
                let total = (2 + self.presets.len()) as i32;
                if enter {
                    return self.load_preset_row(cursor, cv);
                }
                if matches!(key, K_DEL | K_LSHOULDER) && cursor >= 2 {
                    let idx = cursor - 2;
                    if confirm_delete == Some(idx) {
                        return self.delete_saved_preset(idx, cv);
                    }
                    self.popup = Some(Popup::Presets { cursor, confirm_delete: Some(idx) });
                    return sound("misc/menu2.wav");
                }
                if dy != 0 {
                    cursor = (cursor as i32 + dy).rem_euclid(total.max(1)) as usize;
                    confirm_delete = None;
                    self.popup = Some(Popup::Presets { cursor, confirm_delete });
                    return sound("misc/menu1.wav");
                }
                Out::default()
            }
            Some(Popup::Add { mut open, mut cursor }) => {
                let rows = add_rows(&open);
                match (rows.get(cursor).copied(), enter, dx) {
                    (Some(AddRow::Kind(k)), true, _) | (Some(AddRow::Kind(k)), _, 1) => return self.prompt_add_place(k),
                    (Some(AddRow::Cat(i)), true, _) => open[i] = !open[i],
                    (Some(AddRow::Cat(i)), _, 1) => open[i] = true,
                    (Some(AddRow::Cat(i)), _, -1) => open[i] = false,
                    (Some(AddRow::Kind(k)), _, -1) => {
                        let ci = CATEGORIES.iter().position(|c| c.1.contains(&k)).unwrap_or(0);
                        open[ci] = false;
                        cursor = add_rows(&open).iter().position(|&r| r == AddRow::Cat(ci)).unwrap_or(0);
                    }
                    _ => {}
                }
                let n = add_rows(&open).len() as i32;
                cursor = (cursor as i32 + dy).rem_euclid(n.max(1)) as usize;
                self.popup = Some(Popup::Add { open, cursor });
                sound("misc/menu1.wav")
            }
            Some(Popup::SaveAs { .. }) | None => Out::default(),
        }
    }

    /// Y coordinate of row `i` inside the Presets popup at `py` (accounts for divider after classic).
    pub(super) fn preset_row_y(py: f32, i: usize) -> f32 {
        py + 3.0 + (i + 1) as f32 * ROW + if i >= 2 { 3.0 } else { 0.0 }
    }

    /// Framed delete button rect on saved preset row `i` (`i >= 2`) inside the Presets popup at `(px, py)`.
    pub(super) fn preset_del_rect(px: f32, py: f32, i: usize) -> Rect {
        Rect { x: px + PRESETS_W - PRESETS_DEL_W - 3.0, y: Self::preset_row_y(py, i) + 0.5, w: PRESETS_DEL_W, h: ROW - 1.0 }
    }

    /// The popup row or grid cell under the pointer.
    pub(super) fn popup_hit(&self, l: &Layout) -> Option<usize> {
        let (mx, my) = self.mouse?;
        let nprops = self.sel.and_then(|i| l.elems.get(i)).map_or(0, |e| editor_rows::props(e.kind).len());
        let d = self.dims(nprops, self.sel.is_some());
        match &self.popup {
            Some(Popup::Grid(_)) => {
                let (x, y) = self.popup_at(&d, 0);
                let (gx, gy) = grid_cells_origin(x, y, GRID_W, 0);
                let (cx, cy) = (((mx - gx) / CELL.0).floor(), ((my - gy) / CELL.1).floor());
                ((0.0..3.0).contains(&cx) && (0.0..3.0).contains(&cy)).then(|| cy as usize * 3 + cx as usize)
            }
            Some(Popup::AddPlace { .. }) => {
                let (x, y) = self.popup_at(&d, 0);
                let (gx, gy) = grid_cells_origin(x, y, ADD_PLACE_W, 1);
                let (cx, cy) = (((mx - gx) / CELL.0).floor(), ((my - gy) / CELL.1).floor());
                ((0.0..3.0).contains(&cx) && (0.0..3.0).contains(&cy)).then(|| cy as usize * 3 + cx as usize)
            }
            Some(Popup::Presets { .. }) => {
                let (x, y) = self.popup_at(&d, 0);
                if !(x..x + PRESETS_W).contains(&mx) {
                    return None;
                }
                let total = 2 + self.presets.len();
                (0..total).find(|&i| {
                    let ry = Self::preset_row_y(y, i);
                    (ry..ry + ROW).contains(&my)
                })
            }
            Some(Popup::Add { open, .. }) => {
                let n = add_rows(open).len();
                let (x, y) = self.popup_at(&d, n);
                let i = ((my - y - 3.0 - ROW) / ROW).floor();
                ((x..x + ADD_W).contains(&mx) && i >= 0.0 && (i as usize) < n).then_some(i as usize)
            }
            Some(Popup::SaveAs { .. }) | None => None,
        }
    }

    pub(super) fn popup_click(&mut self, l: &mut Layout, cv: &mut Cvars) -> Out {
        if let Some(Popup::SaveAs { name, confirm_overwrite }) = self.popup.clone() {
            let Some(m) = self.mouse else { return Out::default() };
            let nprops = self.sel.and_then(|i| l.elems.get(i)).map_or(0, |e| editor_rows::props(e.kind).len());
            let d = self.dims(nprops, self.sel.is_some());
            let (x, y) = self.popup_at(&d, 0);
            let (save_r, cancel_r) = save_popup_buttons(x, y);
            if save_r.contains(m) {
                return self.commit_save_as(&name, confirm_overwrite, l, cv);
            }
            if cancel_r.contains(m) || !(x..x + SAVE_W).contains(&m.0) || !(y..y + SAVE_H).contains(&m.1) {
                self.popup = None;
                return sound("misc/menu3.wav");
            }
            return Out::default();
        }
        let Some(i) = self.popup_hit(l) else {
            self.popup = None;
            return sound("misc/menu3.wav");
        };
        match self.popup.clone() {
            Some(Popup::Grid(_)) => self.set_anchor(i, l, cv),
            Some(Popup::AddPlace { kind, .. }) => self.add_at(kind, Anchor::ALL[i.min(8)], l, cv),
            Some(Popup::Presets { confirm_delete, .. }) => {
                if i >= 2
                    && let Some(m) = self.mouse
                {
                    let nprops = self.sel.and_then(|s| l.elems.get(s)).map_or(0, |e| editor_rows::props(e.kind).len());
                    let d = self.dims(nprops, self.sel.is_some());
                    let (x, y) = self.popup_at(&d, 0);
                    if Self::preset_del_rect(x, y, i).contains(m) {
                        let idx = i - 2;
                        if confirm_delete == Some(idx) {
                            return self.delete_saved_preset(idx, cv);
                        }
                        self.popup = Some(Popup::Presets { cursor: i, confirm_delete: Some(idx) });
                        return sound("misc/menu2.wav");
                    }
                }
                self.load_preset_row(i, cv)
            }
            Some(Popup::Add { mut open, .. }) => match add_rows(&open)[i] {
                AddRow::Kind(k) => self.prompt_add_place(k),
                AddRow::Cat(c) => {
                    open[c] = !open[c];
                    self.popup = Some(Popup::Add { open, cursor: i });
                    sound("misc/menu1.wav")
                }
            },
            Some(Popup::SaveAs { .. }) | None => Out::default(),
        }
    }
}
