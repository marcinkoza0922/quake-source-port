//! Drawing the HUD editor: outlines around the elements, the dialog and
//! its popups.

use qcore::cvar::Cvars;

use super::editor::{ADD_W, AddRow, BUTTONS, CELL, Dims, Editor, LIST_ROWS, Popup, ROW, Row, TITLE_H, W, layout};
use super::editor_popup::add_rows;
use super::editor_rows::{self, Prop};
use super::kind::{CATEGORIES, IMMERSIVE_TIP, Kind, opt_tip};
use super::layout::Layout;
use crate::draw::Canvas;
use crate::menu::{BORDER, FG, GOLD_FG, HILITE, MUTED};

const SIZE: f32 = 8.0;
const BACK: [f32; 4] = [0.03, 0.035, 0.04, 0.94];
const TITLE_BG: [f32; 4] = [0.22, 0.15, 0.07, 0.95];
const BUTTON_BG: [f32; 4] = [0.12, 0.09, 0.05, 0.9];
const SELECTED: [f32; 4] = [1.0, 0.8, 0.3, 1.0];
const OUTLINE: [f32; 4] = [1.0, 1.0, 1.0, 0.35];
const GRID_CELL: [f32; 4] = [0.32, 0.25, 0.15, 0.95];

fn text_right(c: &mut Canvas, right: f32, y: f32, s: &str, color: [f32; 4]) {
    let w = c.text_w(s, SIZE);
    c.text(right - w, y, s, SIZE, color);
}

impl Editor {
    pub fn draw(&self, c: &mut Canvas, cv: &Cvars) {
        let l = layout(cv);
        let saved = c.scale;
        c.reset_origin();
        c.scale = self.geo.hud;
        for (i, r) in self.rects.iter().enumerate().take(l.elems.len()) {
            if i == self.sel {
                c.fill(r.x, r.y, r.w, r.h, [1.0, 0.8, 0.3, 0.12]);
            }
            c.frame(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, if i == self.sel { SELECTED } else { OUTLINE });
        }
        c.scale = self.geo.menu;
        let props = l.elems.get(self.sel).map(|e| editor_rows::props(e.kind)).unwrap_or_default();
        let d = self.dims(props.len());
        c.fill(d.x, d.y, W, d.h, BACK);
        c.fill(d.x, d.y, W, TITLE_H, TITLE_BG);
        c.frame(d.x, d.y, W, d.h, BORDER);
        c.text(d.x + 6.0, d.y + 3.0, "HUD Editor", SIZE, GOLD_FG);
        // grip: the title bar drags the dialog
        for k in 0..3 {
            c.fill(d.x + W - 14.0, d.y + 4.0 + k as f32 * 2.5, 8.0, 1.0, MUTED);
        }
        let rows = self.rows(&l);
        let cur = rows.get(self.cursor).copied();
        self.draw_list(c, &l, &d, cur);
        self.draw_buttons(c, &l, &d, cur);
        if let Some(e) = l.elems.get(self.sel) {
            c.fill(d.x + 4.0, d.hdr_y - 3.0, W - 8.0, 1.0, BORDER);
            c.text(d.x + 6.0, d.hdr_y, e.kind.label(), SIZE, GOLD_FG);
            for (j, &p) in props.iter().enumerate() {
                let y = d.props_y + j as f32 * ROW;
                if cur == Some(Row::Prop(p)) {
                    c.fill(d.x + 2.0, y, W - 4.0, ROW, HILITE);
                }
                c.text(d.x + 8.0, y + 1.5, editor_rows::label(p, e.kind), SIZE, FG);
                let value = match &self.typing {
                    Some((tp, s)) if *tp == p => format!("{s}_"),
                    _ => editor_rows::value_text(p, e, cv),
                };
                let (lo, hi) = (d.x + W - 92.0, d.x + W - 8.0);
                if p == Prop::Position {
                    grid_icon(c, lo, y + 2.0, e.anchor as usize);
                    text_right(c, hi, y + 1.5, &value, FG);
                } else {
                    c.text(lo, y + 1.5, "<", SIZE, MUTED);
                    text_right(c, hi, y + 1.5, ">", MUTED);
                    let w = c.text_w(&value, SIZE);
                    c.text(((lo + hi) / 2.0 - w / 2.0).floor(), y + 1.5, &value, SIZE, FG);
                }
            }
        }
        if self.popup.is_none() {
            let kind = l.elems.get(self.sel).map(|e| e.kind);
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
        self.draw_popup(c, &l, &d);
        c.scale = saved;
    }

    fn draw_list(&self, c: &mut Canvas, l: &Layout, d: &Dims, cur: Option<Row>) {
        for k in 0..LIST_ROWS {
            let i = self.scroll + k;
            let Some(e) = l.elems.get(i) else { break };
            let y = d.list_y + k as f32 * ROW;
            if i == self.sel {
                c.fill(d.x + 2.0, y, W - 4.0, ROW, HILITE);
            }
            if cur == Some(Row::Elem(i)) {
                c.frame(d.x + 2.0, y, W - 4.0, ROW, BORDER);
            }
            let color = if e.kind.permanent() { MUTED } else { FG };
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

    fn draw_buttons(&self, c: &mut Canvas, l: &Layout, d: &Dims, cur: Option<Row>) {
        for (b, label) in BUTTONS.iter().enumerate() {
            let r = Self::button_rect(d, b);
            let focus = cur == Some(Row::Buttons(b / 2)) && self.button == b % 2;
            c.fill(r.x, r.y, r.w, r.h, if focus { HILITE } else { BUTTON_BG });
            c.frame(r.x, r.y, r.w, r.h, if focus { SELECTED } else { BORDER });
            let disabled = b == 1 && l.elems.get(self.sel).is_none_or(|e| e.kind.permanent());
            let label = if b == 2 && self.confirm_reset { qcore::t!("Confirm") } else { label };
            let w = c.text_w(label, SIZE);
            c.text((r.x + (r.w - w) / 2.0).floor(), r.y + 3.0, label, SIZE, if disabled { MUTED } else { FG });
        }
    }

    fn draw_popup(&self, c: &mut Canvas, l: &Layout, d: &Dims) {
        let hover = self.popup_hit(l);
        match &self.popup {
            Some(Popup::Grid(cursor)) => {
                let (x, y) = self.popup_at(d, 0);
                let (w, h) = (3.0 * CELL.0 + 8.0, 3.0 * CELL.1 + 8.0);
                c.fill(x, y, w, h, BACK);
                c.frame(x, y, w, h, BORDER);
                let current = l.elems.get(self.sel).map_or(4, |e| e.anchor as usize);
                for cell in 0..9 {
                    let (cx, cy) = (x + 4.0 + (cell % 3) as f32 * CELL.0, y + 4.0 + (cell / 3) as f32 * CELL.1);
                    let fill = if cell == current { SELECTED } else if hover == Some(cell) { HILITE } else { GRID_CELL };
                    c.fill(cx + 1.0, cy + 1.0, CELL.0 - 2.0, CELL.1 - 2.0, fill);
                    if cell == *cursor {
                        c.frame(cx, cy, CELL.0, CELL.1, FG);
                    }
                }
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

/// The position button's icon: a 3×3 grid with the anchor lit.
fn grid_icon(c: &mut Canvas, x: f32, y: f32, lit: usize) {
    for cell in 0..9 {
        let color = if cell == lit { SELECTED } else { MUTED };
        c.fill(x + (cell % 3) as f32 * 3.5, y + (cell / 3) as f32 * 2.5, 2.5, 1.5, color);
    }
}
