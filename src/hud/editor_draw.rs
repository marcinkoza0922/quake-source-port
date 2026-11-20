//! Drawing the HUD editor: outlines around the elements, snap guides,
//! the dialog (hidden while dragging an element) and its popups.

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::editor::{ADD_W, AddRow, BTN_H, BUTTONS, CELL, Editor, LIST_ROWS, PRESET, Popup, ROW, Row, TITLE_H, W, button_row_col, button_row_cols, layout};
use super::editor_popup::{ADD_PLACE_H, ADD_PLACE_W, GRID_H, GRID_W, PRESETS_W, SAVE_H, SAVE_W, add_rows, grid_cells_origin, save_popup_buttons};
use super::editor_rows::{self, Prop};
use super::kind::{CATEGORIES, IMMERSIVE_TIP, Kind, opt_tip};
use super::layout::{Anchor, Layout};
use super::place::Rect;
use super::presets::{self, PRESET_NAME};
use crate::draw::Canvas;
use crate::glyphs::{self, Glyphs, HINT_H};
use crate::menu::{BORDER, FG, GOLD_FG, HILITE, MUTED};

/// Where the dialog's parts are (menu units, screen origin).
pub(super) struct Dims {
    pub x: f32,
    pub y: f32,
    pub list_y: f32,
    pub btn_y: f32,
    pub hdr_y: f32,
    pub props_y: f32,
    pub h: f32,
}

impl Editor {
    pub(super) fn dims(&self, nprops: usize, has_sel: bool) -> Dims {
        let (x, y) = self.pos.unwrap_or((8.0, 8.0));
        let list_y = y + TITLE_H + 2.0;
        let btn_y = list_y + LIST_ROWS as f32 * ROW + 4.0;
        let btns_bottom = btn_y + 3.0 * BTN_H + 6.0;
        let hdr_y = btns_bottom + 6.0;
        let props_y = hdr_y + ROW + 3.0;
        let h = if self.collapsed {
            TITLE_H
        } else if !has_sel {
            btns_bottom + 4.0 - y
        } else {
            props_y + nprops as f32 * ROW + 5.0 - y
        };
        Dims { x, y, list_y, btn_y, hdr_y, props_y, h }
    }

    /// Click target for the hamburger collapse/expand button in the title bar.
    pub(super) fn toggle_rect(d: &Dims) -> Rect {
        Rect { x: d.x + W - 18.0, y: d.y, w: 18.0, h: TITLE_H }
    }

    /// Click target for the title-bar Undo button (keyboard/mouse UI).
    pub(super) fn undo_rect(d: &Dims) -> Rect {
        Rect { x: d.x + W - 52.0, y: d.y + 1.5, w: 32.0, h: TITLE_H - 3.0 }
    }

    /// Click target for the title-bar Preview state toggle (keyboard/mouse UI).
    pub(super) fn preview_rect(d: &Dims) -> Rect {
        Rect { x: d.x + W - 94.0, y: d.y + 1.5, w: 40.0, h: TITLE_H - 3.0 }
    }

    /// Click target for the Position grid button on property row `y`.
    pub(super) fn pos_button_rect(d: &Dims, row_y: f32) -> Rect {
        Rect { x: d.x + W - 94.0, y: row_y + 0.5, w: 88.0, h: ROW - 1.0 }
    }

    pub(super) fn button_rect(d: &Dims, b: usize) -> Rect {
        let (row, col) = button_row_col(b);
        let cols = button_row_cols(row) as f32;
        let bw = (W - 8.0 - 4.0 * (cols - 1.0)) / cols;
        Rect { x: d.x + 4.0 + col as f32 * (bw + 4.0), y: d.btn_y + row as f32 * (BTN_H + 3.0), w: bw, h: BTN_H }
    }
}

const SIZE: f32 = 8.0;
const BUTTON_KEYS: [Option<i32>; 7] = [Some(K_YBUTTON), Some(K_LSHOULDER), None, None, None, Some(K_BACK), Some(K_START)];
const BACK: [f32; 4] = [0.055, 0.055, 0.062, 0.95];
const LIST_BG: [f32; 4] = [0.024, 0.032, 0.048, 0.96];
const PROPS_BG: [f32; 4] = [0.145, 0.115, 0.085, 0.97];
const PROPS_HDR_BG: [f32; 4] = [0.22, 0.165, 0.105, 0.98];
const TITLE_BG: [f32; 4] = [0.22, 0.15, 0.07, 0.95];
const BUTTON_BG: [f32; 4] = [0.12, 0.09, 0.05, 0.92];
const SELECTED: [f32; 4] = [1.0, 0.8, 0.3, 1.0];
const OUTLINE: [f32; 4] = [1.0, 1.0, 1.0, 0.35];
const HIDDEN_OUTLINE: [f32; 4] = [1.0, 1.0, 1.0, 0.18];
const GHOST_FILL: [f32; 4] = [1.0, 0.8, 0.3, 0.07];
const GHOST_EDGE: [f32; 4] = [1.0, 0.8, 0.3, 0.45];
const GUIDE: [f32; 4] = [0.35, 0.85, 1.0, 0.55];
const GRID_CELL: [f32; 4] = [0.32, 0.25, 0.15, 0.95];

fn text_right(c: &mut Canvas, right: f32, y: f32, s: &str, color: [f32; 4]) {
    let w = c.text_w(s, SIZE);
    c.text(right - w, y, s, SIZE, color);
}

fn text_center(c: &mut Canvas, cx: f32, y: f32, s: &str, color: [f32; 4]) {
    let w = c.text_w(s, SIZE);
    c.text((cx - w / 2.0).floor(), y, s, SIZE, color);
}

fn draw_btn(c: &mut Canvas, r: Rect, label: &str, fg: [f32; 4], hover: bool, accent: bool) {
    c.fill(r.x, r.y, r.w, r.h, if hover { HILITE } else { BUTTON_BG });
    c.frame(r.x, r.y, r.w, r.h, if hover || accent { SELECTED } else { BORDER });
    text_center(c, r.x + r.w / 2.0, r.y + ((r.h - SIZE) / 2.0).floor(), label, fg);
}

impl Editor {
    pub fn draw(&self, c: &mut Canvas, cv: &Cvars, pad_ui: bool, glyphs: &Glyphs) {
        let l = layout(cv);
        let saved = c.scale;
        c.reset_origin();
        c.scale = self.geo.hud;
        let drag_info = self.drag_info();
        if let Some((_, origin)) = drag_info {
            // Alignment / snap guides across the screen
            for &gx in &self.snap_guides.0 {
                c.fill(gx, 0.0, 1.0, self.geo.vh, GUIDE);
            }
            for &gy in &self.snap_guides.1 {
                c.fill(0.0, gy, self.geo.vw, 1.0, GUIDE);
            }
            // Faint ghost outline at the dragged element's starting slot
            if origin.w > 0.0 && origin.h > 0.0 {
                c.fill(origin.x, origin.y, origin.w, origin.h, GHOST_FILL);
                c.frame(origin.x - 1.0, origin.y - 1.0, origin.w + 2.0, origin.h + 2.0, GHOST_EDGE);
            }
        }
        let dragging = drag_info.is_some();
        for (i, r) in self.rects.iter().enumerate().take(l.elems.len()) {
            let is_sel = self.sel == Some(i);
            if is_sel {
                c.fill(r.x, r.y, r.w, r.h, [1.0, 0.8, 0.3, if dragging { 0.24 } else { 0.12 }]);
                if dragging {
                    c.frame(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0, SELECTED);
                }
            }
            let edge = if is_sel {
                SELECTED
            } else if l.elems[i].hidden {
                HIDDEN_OUTLINE
            } else {
                OUTLINE
            };
            c.frame(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, edge);
        }
        c.scale = self.geo.menu;
        let rows = self.rows(&l);
        let cur = rows.get(self.cursor).copied();
        // Hide the entire element menu while actively dragging an element so the screen is unobstructed.
        if !dragging {
            self.draw_dialog(c, cv, &l, &rows, cur, pad_ui, glyphs);
        }
        if pad_ui {
            self.draw_pad_hints(c, glyphs, cur);
        }
        c.scale = saved;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_dialog(&self, c: &mut Canvas, cv: &Cvars, l: &Layout, _rows: &[Row], cur: Option<Row>, pad_ui: bool, glyphs: &Glyphs) {
        let sel_elem = self.sel.and_then(|i| l.elems.get(i));
        let props = sel_elem.map(|e| editor_rows::props(e.kind)).unwrap_or_default();
        let d = self.dims(props.len(), sel_elem.is_some());
        c.fill(d.x, d.y, W, d.h, BACK);
        c.fill(d.x, d.y, W, TITLE_H, TITLE_BG);
        c.text(d.x + 6.0, d.y + 3.0, "HUD Editor", SIZE, GOLD_FG);
        // Hamburger menu button: collapses/expands the dialog to a single bar
        let tr = Self::toggle_rect(&d);
        let hover_toggle = self.mouse.is_some_and(|m| tr.contains(m));
        if hover_toggle {
            c.fill(tr.x + 1.0, tr.y + 1.0, tr.w - 2.0, tr.h - 2.0, HILITE);
        }
        let bar_col = if hover_toggle || self.collapsed { GOLD_FG } else { MUTED };
        for k in 0..3 {
            c.fill(d.x + W - 14.0, d.y + 4.0 + k as f32 * 2.5, 8.0, 1.0, bar_col);
        }
        if pad_ui {
            let gw = glyphs::width(c, K_RSHOULDER, glyphs.family, glyphs::H);
            let gx = (tr.x - gw - 1.0).floor();
            glyphs.key(c, gx, d.y + (TITLE_H - glyphs::H) / 2.0, K_RSHOULDER);
        } else {
            // Compact Undo and Preview buttons in the title bar
            let ur = Self::undo_rect(&d);
            let hover_u = self.mouse.is_some_and(|m| ur.contains(m)) && self.can_undo();
            draw_btn(c, ur, "Undo", if self.can_undo() { FG } else { MUTED }, hover_u, false);

            let pr = Self::preview_rect(&d);
            let hover_p = self.mouse.is_some_and(|m| pr.contains(m));
            let active_p = self.preview != super::PreviewState::Sample;
            draw_btn(c, pr, self.preview.label(), if active_p { GOLD_FG } else { FG }, hover_p, active_p);
        }
        if !self.collapsed {
            // Distinct background for the element list section
            let list_h = (d.btn_y - 2.0) - (d.y + TITLE_H);
            c.fill(d.x + 1.0, d.y + TITLE_H, W - 2.0, list_h, LIST_BG);
            c.fill(d.x + 1.0, d.btn_y - 2.0, W - 2.0, 1.0, BORDER);
            self.draw_list(c, l, &d, cur);
            self.draw_buttons(c, cv, l, &d, cur, pad_ui, glyphs);
            if let Some(e) = sel_elem {
                self.draw_props(c, cv, e, &props, &d, cur, pad_ui, glyphs);
            }
            if self.popup.is_none() {
                let kind = sel_elem.map(|e| e.kind);
                let tip = match cur {
                    Some(Row::Prop(Prop::Show)) => Some(IMMERSIVE_TIP),
                    Some(Row::Prop(Prop::Opt(i))) => kind.and_then(|k| opt_tip(k.opts()[i].label)).or(kind.and_then(Kind::tip)),
                    Some(Row::Elem(_) | Row::Prop(_)) => kind.and_then(Kind::tip),
                    _ => None,
                };
                if let Some(tip) = tip {
                    draw_tip(c, &d, tip);
                }
            }
            self.draw_popup(c, cv, l, &d);
        }
        c.frame(d.x, d.y, W, d.h, BORDER);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_props(&self, c: &mut Canvas, cv: &Cvars, e: &super::layout::Elem, props: &[Prop], d: &Dims, cur: Option<Row>, pad_ui: bool, glyphs: &Glyphs) {
        let props_top = d.hdr_y - 2.0;
        let hdr_h = d.props_y - props_top;
        // Distinct warm background card for the selected element's config options
        c.fill(d.x + 1.0, props_top, W - 2.0, d.y + d.h - props_top - 1.0, PROPS_BG);
        c.fill(d.x + 1.0, props_top, W - 2.0, hdr_h, PROPS_HDR_BG);
        c.fill(d.x + 1.0, props_top, W - 2.0, 1.0, BORDER);
        c.fill(d.x + 1.0, d.props_y - 1.0, W - 2.0, 1.0, BORDER);
        let hdr_x = if pad_ui {
            let gh = 9.0;
            let gw = glyphs.key_h(c, d.x + 6.0, d.hdr_y + 0.5, K_XBUTTON, gh);
            d.x + 6.0 + gw + 3.0
        } else {
            d.x + 6.0
        };
        c.text(hdr_x, d.hdr_y + 1.0, e.kind.label(), SIZE, GOLD_FG);
        for (j, &p) in props.iter().enumerate() {
            let y = d.props_y + j as f32 * ROW;
            let focused = cur == Some(Row::Prop(p));
            if focused {
                c.fill(d.x + 2.0, y, W - 4.0, ROW, HILITE);
            }
            c.text(d.x + 8.0, y + 1.5, editor_rows::label(p, e.kind), SIZE, FG);
            let value = match &self.typing {
                Some((tp, s)) if *tp == p => format!("{s}_"),
                _ => editor_rows::value_text(p, e, cv),
            };
            let (lo, hi) = (d.x + W - 92.0, d.x + W - 8.0);
            if p == Prop::Position {
                let br = Self::pos_button_rect(d, y);
                let hover = self.mouse.is_some_and(|m| br.contains(m));
                c.fill(br.x, br.y, br.w, br.h, if hover || focused { HILITE } else { BUTTON_BG });
                c.frame(br.x, br.y, br.w, br.h, if hover || focused { SELECTED } else { BORDER });
                grid_icon(c, br.x + 3.5, y + 2.0, e.anchor as usize, editor_rows::is_at_anchor(e));
                text_right(c, br.right() - 3.0, y + 1.5, &value, FG);
            } else {
                c.text(lo, y + 1.5, "<", SIZE, MUTED);
                text_right(c, hi, y + 1.5, ">", MUTED);
                let w = c.text_w(&value, SIZE);
                c.text(((lo + hi) / 2.0 - w / 2.0).floor(), y + 1.5, &value, SIZE, FG);
            }
        }
    }

    fn draw_pad_hints(&self, c: &mut Canvas, glyphs: &Glyphs, cur: Option<Row>) {
        let mut row: Vec<(i32, &str)> = if self.moving() {
            vec![(K_LTHUMB, "Position"), (K_RTRIGGER, "Scale"), (K_ABUTTON, "Confirm"), (K_XBUTTON, "Options"), (K_BBUTTON, "Cancel")]
        } else if matches!(self.popup, Some(Popup::SaveAs { .. })) {
            vec![(K_ABUTTON, "Save"), (K_YBUTTON, "Name"), (K_XBUTTON, "Delete"), (K_BBUTTON, "Cancel")]
        } else if matches!(self.popup, Some(Popup::Presets { cursor, .. }) if cursor >= 2) {
            vec![(K_ABUTTON, "Load"), (K_LSHOULDER, "Delete"), (K_BBUTTON, "Back")]
        } else if self.popup.is_some() || (matches!(cur, Some(Row::Prop(_))) && !self.collapsed) {
            vec![(K_ABUTTON, "Select"), (K_BBUTTON, "Back")]
        } else if self.collapsed {
            vec![(K_ABUTTON, "Position"), (K_XBUTTON, "Options"), (K_YBUTTON, "Add"), (K_START, "Done"), (K_BBUTTON, "Back")]
        } else if self.sel.is_none() {
            vec![(K_ABUTTON, "Select"), (K_YBUTTON, "Add"), (K_LTHUMB, self.preview.label()), (K_BBUTTON, "Back")]
        } else {
            vec![(K_ABUTTON, "Position"), (K_XBUTTON, "Options"), (K_LTHUMB, self.preview.label()), (K_BBUTTON, "Back")]
        };
        if !self.moving() && self.popup.is_none() && self.can_undo() {
            let pos = row.len().saturating_sub(1);
            row.insert(pos, (K_RTHUMB, "Undo"));
        }
        let y = self.geo.sh - 6.0 - (HINT_H + 4.0);
        let cx = self.geo.sw / 2.0;
        let w = glyphs.hints(c, 0.0, y, &row, false);
        c.fill(cx - w / 2.0 - 8.0, y - 2.0, w + 16.0, HINT_H + 4.0, [0.0, 0.0, 0.0, 0.65]);
        glyphs.hints_centered(c, cx, y, &row);
    }

    fn draw_list(&self, c: &mut Canvas, l: &Layout, d: &Dims, cur: Option<Row>) {
        for k in 0..LIST_ROWS {
            let i = self.scroll + k;
            let Some(e) = l.elems.get(i) else { break };
            let y = d.list_y + k as f32 * ROW;
            if self.sel == Some(i) {
                c.fill(d.x + 2.0, y, W - 4.0, ROW, HILITE);
                if cur == Some(Row::Elem(i)) {
                    c.frame(d.x + 2.0, y, W - 4.0, ROW, BORDER);
                }
            }
            let color = if e.hidden || e.kind.permanent() { MUTED } else { FG };
            c.text(d.x + 6.0, y + 1.5, e.kind.label(), SIZE, color);
            let scale = if e.kind.is_message() { String::new() } else { format!("{}x  ", qcore::cvar::format_float(e.scale)) };
            let info = format!("{scale}{}, {}", e.x, e.y);
            text_right(c, d.x + W - 10.0, y + 1.5, &info, MUTED);
        }
        let n = l.elems.len();
        if n > LIST_ROWS {
            let h = LIST_ROWS as f32 * ROW;
            let th = (h * LIST_ROWS as f32 / n as f32).max(6.0);
            let ty = d.list_y + (h - th) * self.scroll as f32 / (n - LIST_ROWS) as f32;
            c.fill(d.x + W - 5.0, d.list_y, 2.0, h, [1.0, 1.0, 1.0, 0.1]);
            c.fill(d.x + W - 5.0, ty, 2.0, th, BORDER);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_buttons(&self, c: &mut Canvas, cv: &Cvars, l: &Layout, d: &Dims, cur: Option<Row>, pad_ui: bool, glyphs: &Glyphs) {
        let can_save = self.can_save_current(cv);
        for (b, label) in BUTTONS.iter().enumerate() {
            let r = Self::button_rect(d, b);
            let (row, col) = button_row_col(b);
            let disabled = (b == 1 && self.sel.and_then(|i| l.elems.get(i)).is_none_or(|e| e.kind.permanent()))
                || (b == 3 && !can_save);
            let hover = !disabled && self.mouse.is_some_and(|m| r.contains(m));
            let focus = cur == Some(Row::Buttons(row)) && self.button == col;
            let save_ready = b == 3 && can_save;
            c.fill(r.x, r.y, r.w, r.h, if focus || hover { HILITE } else { BUTTON_BG });
            c.frame(r.x, r.y, r.w, r.h, if focus || hover || save_ready { SELECTED } else { BORDER });
            let label = if b == 5 && self.confirm_reset { qcore::t!("Confirm") } else { label };
            let fg = if disabled { MUTED } else if save_ready { GOLD_FG } else { FG };
            glyphs.button_label(c, [r.x, r.y, r.w, r.h], pad_ui.then_some(BUTTON_KEYS[b]).flatten(), label, SIZE, fg);
        }
    }

    fn draw_popup(&self, c: &mut Canvas, cv: &Cvars, l: &Layout, d: &Dims) {
        let hover = self.popup_hit(l);
        match &self.popup {
            Some(Popup::Grid(cursor)) => {
                let (x, y) = self.popup_at(d, 0);
                let current = self.sel.and_then(|i| l.elems.get(i)).map_or(4, |e| e.anchor as usize);
                draw_grid_popup(c, x, y, GRID_W, GRID_H, None, current, *cursor, hover);
            }
            Some(Popup::AddPlace { kind, cursor }) => {
                let (x, y) = self.popup_at(d, 0);
                let default_cell = kind.default_anchor() as usize;
                draw_grid_popup(c, x, y, ADD_PLACE_W, ADD_PLACE_H, Some(kind.label()), default_cell, *cursor, hover);
            }
            Some(Popup::Presets { cursor, confirm_delete }) => {
                let (x, y) = self.popup_at(d, 0);
                let h = self.presets_popup_h();
                c.fill(x, y, PRESETS_W, h, BACK);
                c.frame(x, y, PRESETS_W, h, BORDER);
                c.text(x + 6.0, y + 3.0, "Presets", SIZE, GOLD_FG);
                if let Some(active) = &self.editing_preset {
                    let tag = if self.can_save_current(cv) { format!("{active}*") } else { active.clone() };
                    let w = c.text_w_raw(&tag, SIZE);
                    c.text_raw(x + PRESETS_W - w - 6.0, y + 3.0, &tag, SIZE, MUTED);
                }
                let preset_num = cv.value_of(PRESET) as i32;
                let preset_name = cv.by_name(PRESET_NAME).map_or("", |c| c.string.trim());
                for (i, label) in ["modern", "classic"].into_iter().enumerate() {
                    let ry = Self::preset_row_y(y, i);
                    if i == *cursor || hover == Some(i) {
                        c.fill(x + 2.0, ry, PRESETS_W - 4.0, ROW, HILITE);
                    }
                    let active = preset_num == i as i32 && self.editing_preset.is_none();
                    c.text(x + 8.0, ry + 1.5, label, SIZE, if active { GOLD_FG } else { FG });
                }
                if !self.presets.is_empty() {
                    let sep_y = Self::preset_row_y(y, 2) - 2.0;
                    c.fill(x + 4.0, sep_y, PRESETS_W - 8.0, 1.0, BORDER);
                }
                for (k, p) in self.presets.iter().enumerate() {
                    let i = k + 2;
                    let ry = Self::preset_row_y(y, i);
                    let dr = Self::preset_del_rect(x, y, i);
                    let del_hover = self.mouse.is_some_and(|m| dr.contains(m));
                    if i == *cursor || (hover == Some(i) && !del_hover) {
                        c.fill(x + 2.0, ry, PRESETS_W - 4.0, ROW, HILITE);
                    }
                    let active = self.editing_preset.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&p.name))
                        || (preset_num == 2 && preset_name.eq_ignore_ascii_case(&p.name));
                    c.text_raw(x + 8.0, ry + 1.5, &p.name, SIZE, if active { GOLD_FG } else { FG });
                    let confirming = *confirm_delete == Some(k);
                    let red = [1.0, 0.35, 0.3, 1.0];
                    c.fill(dr.x, dr.y, dr.w, dr.h, if confirming { [0.35, 0.08, 0.08, 0.95] } else if del_hover { HILITE } else { BUTTON_BG });
                    c.frame(dr.x, dr.y, dr.w, dr.h, if confirming { red } else if del_hover { SELECTED } else { BORDER });
                    let del_label = if confirming { "Confirm" } else { "Delete" };
                    let del_col = if confirming { red } else if del_hover { FG } else { MUTED };
                    text_center(c, dr.x + dr.w / 2.0, dr.y + 1.0, del_label, del_col);
                }
            }
            Some(Popup::SaveAs { name, confirm_overwrite }) => {
                let (x, y) = self.popup_at(d, 0);
                c.fill(x, y, SAVE_W, SAVE_H, BACK);
                c.frame(x, y, SAVE_W, SAVE_H, BORDER);
                c.text(x + 6.0, y + 3.0, "Save As", SIZE, GOLD_FG);
                let clean = presets::clean_name(name);
                let valid = !clean.is_empty() && !presets::is_reserved(&clean);
                let fx = x + 6.0;
                let fy = y + 16.0;
                let fw = SAVE_W - 12.0;
                let fh = 15.0;
                c.fill(fx, fy, fw, fh, LIST_BG);
                c.frame(fx, fy, fw, fh, if valid { SELECTED } else { [1.0, 0.35, 0.3, 1.0] });
                let shown = format!("{name}_");
                c.text_raw(fx + 4.0, fy + 3.5, &shown, SIZE, FG);
                let (sr, cr) = save_popup_buttons(x, y);
                let hover_s = valid && self.mouse.is_some_and(|m| sr.contains(m));
                let save_label = if *confirm_overwrite { "Confirm" } else { "Save" };
                let save_fg = if !valid { MUTED } else if *confirm_overwrite { GOLD_FG } else { FG };
                draw_btn(c, sr, save_label, save_fg, hover_s, *confirm_overwrite);
                draw_btn(c, cr, "Cancel", FG, self.mouse.is_some_and(|m| cr.contains(m)), false);
            }
            Some(Popup::Add { open, cursor }) => {
                let rows = add_rows(open);
                let (x, y) = self.popup_at(d, rows.len());
                let h = (rows.len() + 1) as f32 * ROW + 6.0;
                c.fill(x, y, ADD_W, h, BACK);
                c.frame(x, y, ADD_W, h, BORDER);
                c.text(x + 6.0, y + 3.0, "Add", SIZE, GOLD_FG);
                for (i, row) in rows.iter().enumerate() {
                    let ry = y + 3.0 + (i + 1) as f32 * ROW;
                    if i == *cursor || hover == Some(i) {
                        c.fill(x + 2.0, ry, ADD_W - 4.0, ROW, HILITE);
                    }
                    match *row {
                        AddRow::Cat(ci) => {
                            let mark = if open[ci] { "-" } else { "+" };
                            c.text(x + 6.0, ry + 1.5, mark, SIZE, MUTED);
                            c.text(x + 14.0, ry + 1.5, CATEGORIES[ci].0, SIZE, GOLD_FG);
                        }
                        AddRow::Kind(k) => {
                            c.text(x + 20.0, ry + 1.5, k.label(), SIZE, FG);
                        }
                    }
                }
            }
            None => {}
        }
    }
}

/// Draws a 3×3 anchor grid popup (used by both the Position button and Add-element placement).
#[allow(clippy::too_many_arguments)]
fn draw_grid_popup(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, title: Option<&str>, lit: usize, cursor: usize, hover: Option<usize>) {
    c.fill(x, y, w, h, BACK);
    c.frame(x, y, w, h, BORDER);
    let header_rows = usize::from(title.is_some());
    if let Some(title) = title {
        text_center(c, x + w / 2.0, y + 3.0, title, GOLD_FG);
    }
    let (gx, gy) = grid_cells_origin(x, y, w, header_rows);
    for cell in 0..9 {
        let (cx, cy) = (gx + (cell % 3) as f32 * CELL.0, gy + (cell / 3) as f32 * CELL.1);
        let fill = if cell == lit {
            SELECTED
        } else if hover == Some(cell) {
            HILITE
        } else {
            GRID_CELL
        };
        c.fill(cx + 1.0, cy + 1.0, CELL.0 - 2.0, CELL.1 - 2.0, fill);
        if cell == cursor || hover == Some(cell) {
            c.frame(cx, cy, CELL.0, CELL.1, FG);
        }
    }
    let active = hover.unwrap_or(cursor).min(8);
    text_center(c, x + w / 2.0, gy + 3.0 * CELL.1 + 3.0, Anchor::ALL[active].label(), FG);
}

/// Help for the focused row in a box under the dialog, word-wrapped.
fn draw_tip(c: &mut Canvas, d: &Dims, tip: &str) {
    const TIP: f32 = 7.0;
    let text = qcore::lang::tr(tip);
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(l) if c.text_w_raw(&format!("{l} {word}"), TIP) <= W - 12.0 => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    let h = lines.len() as f32 * (TIP + 2.0) + 6.0;
    let y = d.y + d.h + 2.0;
    c.fill(d.x, y, W, h, BACK);
    c.frame(d.x, y, W, h, BORDER);
    for (i, l) in lines.iter().enumerate() {
        c.text_raw(d.x + 6.0, y + 3.0 + i as f32 * (TIP + 2.0), l, TIP, MUTED);
    }
}

/// The position button's icon: a 3×3 grid with the anchor cell lit.
fn grid_icon(c: &mut Canvas, x: f32, y: f32, lit: usize, at_anchor: bool) {
    let accent = if at_anchor { SELECTED } else { [0.85, 0.65, 0.3, 0.75] };
    for cell in 0..9 {
        let color = if cell == lit { accent } else { MUTED };
        c.fill(x + (cell % 3) as f32 * 3.5, y + (cell / 3) as f32 * 2.5, 2.5, 1.5, color);
    }
}
