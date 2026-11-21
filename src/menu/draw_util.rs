use super::*;

/// Quit prompt: box (x, y, w, h), button size, button row and left edges.
pub(super) const QUIT_BOX: (f32, f32, f32, f32) = (224.0, 144.0, 192.0, 64.0);
pub(super) const QUIT_BTN: (f32, f32) = (64.0, 14.0);
pub(super) const QUIT_BTN_Y: f32 = 178.0;
pub(super) const QUIT_BUTTONS: [f32; 2] = [248.0, 328.0];

pub(super) fn draw_inventory_icon(c: &mut Canvas, pic: &mut PicFn, x: f32, y: f32, max_w: f32, max_h: f32, icon: &crate::save_inventory::Icon) {
    if let Some((id, w, h)) = pic(&icon.name) {
        let frame = icon.frame;
        let width = if frame.is_some() { w / 16 } else { w };
        let scale = (max_w / width.max(1) as f32).min(max_h / h.max(1) as f32).min(1.0);
        let px = (c.ox + x) * c.scale;
        let py = (c.oy + y) * c.scale;
        let uv = frame.map_or([0.0,0.0,1.0,1.0], |n| [n as f32 / 16.0,0.0,(n+1) as f32 / 16.0,1.0]);
        c.push(qrender::Draw2D::Pic { x: px, y: py, w: width as f32 * scale * c.scale,
            h: h as f32 * scale * c.scale, pic: id, uv, color: [1.0; 4] });
    }
}

/// Main menu rows: the five of mainmenu.lmp plus Map Packs and Add-ons.
pub(super) const MAIN_ITEMS: usize = 7;

pub const BORDER: [f32; 4] = [0.55, 0.42, 0.25, 1.0];
pub const HILITE: [f32; 4] = [0.6, 0.38, 0.12, 0.45];

/// Modern menu text: em size and colours.
pub const TXT: f32 = 9.0;
pub const FG: [f32; 4] = [0.88, 0.86, 0.82, 1.0];
pub const GOLD_FG: [f32; 4] = [0.95, 0.70, 0.34, 1.0];
pub const MUTED: [f32; 4] = [0.62, 0.59, 0.54, 1.0];
/// Labels of rows that depend on a master toggle, and their tree guide.
pub const DEP_FG: [f32; 4] = [0.80, 0.70, 0.52, 1.0];
pub const DEP_GUIDE: [f32; 4] = [0.75, 0.55, 0.28, 0.75];

/// An options row label, left-aligned in the label column (`indent` for
/// dependent rows); long labels shrink to keep clear of the value column.
pub(super) fn row_label(c: &mut Canvas, indent: f32, y: f32, label: &str, color: [f32; 4]) {
    let x = LABEL_X + indent;
    let room = VALUE_X - 8.0 - x;
    let w = c.text_w(label, TXT);
    let size = if w > room { TXT * room / w } else { TXT };
    c.text(x, y + (TXT - size) / 2.0, label, size, color);
}

/// Arrow of a sub-page row, at the panel's right edge (clear of the scrollbar).
pub(super) const SUB_ARROW_X: f32 = PANEL_X + PANEL_W - 22.0;

/// Gold dot left of a row whose value differs from its default.
pub(super) fn changed_mark(c: &mut Canvas, y: f32, off: bool) {
    c.round_box(PANEL_X + 4.0, y + 2.5, 4.0, 4.0, 2.0, if off { MUTED } else { GOLD_FG }, None);
}

/// Width of an on/off switch.
pub(super) const SWITCH_W: f32 = 18.0;

/// An on/off switch: the knob slides right and the track fills when on.
pub(super) fn switch(c: &mut Canvas, x: f32, y: f32, on: bool, off: bool) {
    let (h, k) = (9.0, 6.0);
    let track = match (on, off) {
        (true, false) => [0.80, 0.55, 0.22, 1.0],
        (true, true) => [0.42, 0.36, 0.28, 1.0],
        _ => [0.0, 0.0, 0.0, 0.5],
    };
    c.round_box(x, y - 0.5, SWITCH_W, h, h / 2.0, track, Some(BORDER));
    let kx = if on { x + SWITCH_W - k - 1.5 } else { x + 1.5 };
    let knob = if off { MUTED } else if on { [0.98, 0.88, 0.66, 1.0] } else { [0.70, 0.66, 0.60, 1.0] };
    c.round_box(kx, y + 1.0, k, k, k / 2.0, knob, None);
}

/// Every other row of a group on options pages.
pub const ROW_STRIPE: [f32; 4] = [1.0, 0.95, 0.85, 0.05];
const HEADING_FG: [f32; 4] = [0.80, 0.62, 0.38, 1.0];

/// A group heading row: small capitals just left of the label column and a
/// rule across the rest of the panel.
pub(super) fn heading(c: &mut Canvas, y: f32, label: &str) {
    let size = TXT * 0.85;
    let text = qcore::lang::tr(label).to_uppercase();
    let x = LABEL_X - 6.0;
    let ty = y + (TXT - size) / 2.0 + 0.5;
    let w = c.text_raw(x, ty, &text, size, HEADING_FG);
    let rule = x + w + 6.0;
    let end = PANEL_X + PANEL_W - 12.0;
    if rule < end {
        c.fill(rule, (y + TXT / 2.0 + 0.5).floor(), end - rule, 1.0, [BORDER[0], BORDER[1], BORDER[2], 0.45]);
    }
}

/// Tree guide in front of a dependent row: a line down from the row above
/// (on through the row when the next one also depends) and a tick to the label.
pub(super) fn dep_guide(c: &mut Canvas, top: f32, h: f32, more: bool, color: [f32; 4]) {
    let x = LABEL_X + 2.0;
    let mid = (top + h / 2.0).floor();
    c.fill(x, top, 1.0, if more { h } else { mid - top + 1.0 }, color);
    c.fill(x, mid, DEP_INDENT - 5.0, 1.0, color);
}

/// Modern text at the conchars position (x, y); returns its width.
pub fn txt(c: &mut Canvas, x: f32, y: f32, s: &str) -> f32 {
    c.text(x, y, s, TXT, FG)
}
pub fn txt_gold(c: &mut Canvas, x: f32, y: f32, s: &str) -> f32 {
    c.text(x, y, s, TXT, GOLD_FG)
}

/// Brown/gold 8×8 text, translated ([`Canvas::ptext_gold`]); returns its width.
pub fn gold(c: &mut Canvas, x: f32, y: f32, s: &str) -> f32 {
    c.ptext_gold(x, y, s, crate::draw::WHITE)
}

/// Translucent box behind `n` option rows starting at `top`.
/// Options tooltip bar: left edge and width (fixed, wider than the panel).
pub(super) const TOOLTIP_BAR: (f32, f32) = (26.0, 588.0);
/// Options page scrollbar width.
pub(super) const SCROLL_W: f32 = 5.0;

/// The options panel; `solid` (a game running behind it) makes it more
/// opaque, so bright scenes don't wash out the text.
pub(super) fn panel(c: &mut Canvas, top: f32, n: usize, solid: bool) {
    panel_height(c, top, n as f32 * ROW_H, solid);
}

pub(super) fn panel_height(c: &mut Canvas, top: f32, height: f32, solid: bool) {
    let h = height + 8.0;
    c.fill(PANEL_X, top - 4.0, PANEL_W, h, [0.0, 0.0, 0.0, if solid { 0.82 } else { 0.55 }]);
    c.frame(PANEL_X, top - 4.0, PANEL_W, h, [0.35, 0.27, 0.16, 0.9]);
}

pub(super) fn highlight(c: &mut Canvas, y: f32) {
    c.fill(PANEL_X + 2.0, y, PANEL_W - 4.0, ROW_H, HILITE);
}

pub(super) fn clamp_scroll(d: &mut Drop) {
    if d.cursor < d.scroll {
        d.scroll = d.cursor;
    } else if d.cursor >= d.scroll + DROP_ROWS {
        d.scroll = d.cursor + 1 - DROP_ROWS;
    }
}

pub(super) fn set_num(cv: &mut Cvars, name: &str, v: f32) {
    cv.set(name, &qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
}

/// Index of the choice closest to `v`.
pub(super) fn nearest_choice(v: f32, choices: &[(&str, f32)]) -> usize {
    let mut best = 0;
    for (i, c) in choices.iter().enumerate() {
        if (c.1 - v).abs() < (choices[best].1 - v).abs() {
            best = i;
        }
    }
    best
}
