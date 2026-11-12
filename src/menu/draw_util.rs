use super::*;

/// Quit prompt: box (x, y, w, h), button size, button row and left edges.
pub(super) const QUIT_BOX: (f32, f32, f32, f32) = (224.0, 144.0, 192.0, 64.0);
pub(super) const QUIT_BTN: (f32, f32) = (64.0, 14.0);
pub(super) const QUIT_BTN_Y: f32 = 178.0;
pub(super) const QUIT_BUTTONS: [f32; 2] = [248.0, 328.0];

/// Main menu rows: the five of mainmenu.lmp plus Mods.
pub(super) fn draw_inventory_icon(c: &mut Canvas, pic: &mut PicFn, x: f32, y: f32, max_w: f32, max_h: f32, icon: &crate::save_inventory::Icon) {
    if let Some((id, w, h)) = pic(&icon.name) {
        let frame = icon.frame;
        let width = if frame.is_some() { w / 16 } else { w };
        let scale = (max_w / width.max(1) as f32).min(max_h / h.max(1) as f32).min(1.0);
        let px = (c.ox + x) * c.scale;
        let py = (c.oy + y) * c.scale;
        let uv = frame.map_or([0.0,0.0,1.0,1.0], |n| [n as f32 / 16.0,0.0,(n+1) as f32 / 16.0,1.0]);
        c.list.push(qrender::Draw2D::Pic { x: px, y: py, w: width as f32 * scale * c.scale,
            h: h as f32 * scale * c.scale, pic: id, uv, color: [1.0; 4] });
    }
}

pub(super) const MAIN_ITEMS: usize = 6;

pub const BORDER: [f32; 4] = [0.55, 0.42, 0.25, 1.0];
pub const HILITE: [f32; 4] = [0.6, 0.38, 0.12, 0.45];

/// Modern menu text: em size and colours.
pub const TXT: f32 = 9.0;
pub const FG: [f32; 4] = [0.88, 0.86, 0.82, 1.0];
pub const GOLD_FG: [f32; 4] = [0.95, 0.70, 0.34, 1.0];
pub const MUTED: [f32; 4] = [0.62, 0.59, 0.54, 1.0];

/// Modern text at the conchars position (x, y); returns its width.
pub fn txt(c: &mut Canvas, x: f32, y: f32, s: &str) -> f32 {
    c.text(x, y, s, TXT, FG)
}
pub fn txt_gold(c: &mut Canvas, x: f32, y: f32, s: &str) -> f32 {
    c.text(x, y, s, TXT, GOLD_FG)
}

/// Brown/gold conchars text.
pub fn gold(c: &mut Canvas, x: f32, y: f32, s: &str) {
    for (k, b) in s.bytes().enumerate() {
        c.ch(x + k as f32 * 8.0, y, b | 128);
    }
}

/// Translucent box behind `n` option rows starting at `top`.
pub(super) fn panel(c: &mut Canvas, top: f32, n: usize) {
    let h = n as f32 * ROW_H + 8.0;
    c.fill(PANEL_X, top - 4.0, PANEL_W, h, [0.0, 0.0, 0.0, 0.55]);
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
