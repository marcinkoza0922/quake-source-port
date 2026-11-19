//! Drawing and path truncation helpers for the in-game directory explorer.

use std::path::Path;

use super::*;
use crate::draw::Canvas;

impl DirBrowser {
    pub(in crate::menu) fn draw(&self, c: &mut Canvas, drag: bool, pad_ui: bool, glyphs: &Glyphs) {
        c.text_center(320.0, 18.0, "Select User Data Directory", TXT * 1.15, GOLD_FG);

        // Current path box
        c.fill(LIST_X, PATH_Y, LIST_W, PATH_H, [0.0, 0.0, 0.0, 0.65]);
        c.frame(LIST_X, PATH_Y, LIST_W, PATH_H, BORDER);
        let cur_str = self.current.to_string_lossy();
        let cur_shown = fit_path_tail(c, &cur_str, LIST_W - 12.0, TXT);
        c.text_raw(LIST_X + 6.0, PATH_Y + 4.0, &cur_shown, TXT, GOLD_FG);

        // Live preview of `<selected>/quake-source-port`
        let target = crate::userdata::resolve_root(&self.current);
        let target_str = target.to_string_lossy();
        let prefix = qcore::tr!("Data folder: {}", "");
        let max_tail = (LIST_W - 8.0 - c.text_w_raw(&prefix, TXT)).max(80.0);
        let target_shown = fit_path_tail(c, &target_str, max_tail, TXT);
        let preview = qcore::tr!("Data folder: {}", target_shown);
        c.text_raw(LIST_X + 4.0, PREVIEW_Y, &preview, TXT, MUTED);

        // Directory list panel
        c.fill(LIST_X, LIST_Y, LIST_W, LIST_H, [0.0, 0.0, 0.0, 0.58]);
        c.frame(LIST_X, LIST_Y, LIST_W, LIST_H, [0.35, 0.27, 0.16, 0.9]);
        let top = LIST_Y + 4.0;
        for (slot, entry) in self.entries.iter().enumerate().skip(self.scroll).take(VIS_ROWS) {
            let y = top + (slot - self.scroll) as f32 * DIR_ROW_H;
            let sel = !self.focus_buttons && slot == self.cursor;
            if sel {
                c.fill(LIST_X + 2.0, y, LIST_W - 10.0, DIR_ROW_H, HILITE);
            }
            match entry {
                Entry::Parent => {
                    c.text_raw(LIST_X + 8.0, y + 3.0, "..", TXT, if sel { GOLD_FG } else { MUTED });
                    c.text(LIST_X + 28.0, y + 3.0, "Parent Folder", TXT, if sel { GOLD_FG } else { MUTED });
                }
                Entry::Dir { name, .. } => {
                    let shown = fit_tail_right(c, &format!("{name}/"), LIST_W - 24.0, TXT);
                    c.text_raw(LIST_X + 8.0, y + 3.0, &shown, TXT, if sel { GOLD_FG } else { FG });
                }
            }
        }
        if self.read_error {
            let y = top + (self.entries.len().min(VIS_ROWS) as f32) * DIR_ROW_H + 6.0;
            c.text_center(320.0, y.min(LIST_Y + LIST_H - 16.0), "Unable to read directory", TXT, MUTED);
        } else if self.entries.iter().all(|e| matches!(e, Entry::Parent)) {
            let y = top + (self.entries.len().min(VIS_ROWS) as f32) * DIR_ROW_H + 6.0;
            c.text_center(320.0, y.min(LIST_Y + LIST_H - 16.0), "No subfolders", TXT, MUTED);
        }

        if let Some((x, sb_top, h, ty, th)) = self.scrollbar() {
            c.fill(x, sb_top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, if drag { 1.0 } else { 0.9 }]);
        }

        if let Some(err) = &self.error {
            let shown = fit_tail_right(c, err, LIST_W, TXT);
            center_raw(c, 320.0, ERR_Y, &shown, TXT, [0.95, 0.45, 0.35, 1.0]);
        }

        // Bottom action buttons
        let labels = [qcore::t!("Select Folder"), qcore::t!("Default"), qcore::t!("Show Hidden"), qcore::t!("Cancel")];
        const BTN_KEYS: [Option<i32>; 4] = [Some(K_XBUTTON), None, Some(K_YBUTTON), Some(K_BBUTTON)];
        for (i, (&(bx, bw), &label)) in BTNS.iter().zip(&labels).enumerate() {
            let on = self.focus_buttons && self.btn_cursor == i;
            let bg = if on { HILITE } else if i == 0 { [0.22, 0.14, 0.06, 0.85] } else { [0.0, 0.0, 0.0, 0.7] };
            let fg = if on || i == 0 { GOLD_FG } else { FG };
            c.round_box(bx, BTN_Y, bw, BTN_H, 4.0, bg, Some(BORDER));
            let key = pad_ui.then_some(BTN_KEYS[i]).flatten();
            if i == 2 {
                let gh = crate::glyphs::H;
                let gw = key.map_or(0.0, |k| crate::glyphs::width(c, k, glyphs.family, gh) + 3.0);
                let tw = c.text_w(label, TXT);
                let mut sx = bx + ((bw - (gw + tw + 13.0)) / 2.0).max(3.0).floor();
                if let Some(k) = key {
                    glyphs.key_h(c, sx, BTN_Y + (BTN_H - gh) / 2.0, k, gh);
                    sx += gw;
                }
                c.round_box(sx, BTN_Y + 4.5, 9.0, 9.0, 1.5, [0.0, 0.0, 0.0, 0.5], Some(BORDER));
                if self.show_hidden {
                    c.round_box(sx + 2.0, BTN_Y + 6.5, 5.0, 5.0, 1.0, GOLD_FG, None);
                }
                c.text(sx + 13.0, crate::glyphs::center_text_y(BTN_Y, BTN_H, TXT), label, TXT, fg);
            } else {
                glyphs.button_label(c, [bx, BTN_Y, bw, BTN_H], key, label, TXT, fg);
            }
        }

        if let Some((chosen, cur)) = &self.confirm {
            self.draw_confirm(c, chosen, *cur, pad_ui, glyphs);
        }
    }

    fn draw_confirm(&self, c: &mut Canvas, chosen: &Path, cur: usize, pad_ui: bool, glyphs: &Glyphs) {
        let (x, y, w, h) = MODAL;
        c.fill(-c.ox, -c.oy, c.vw(), c.vh(), [0.0, 0.0, 0.0, 0.55]);
        c.round_box(x, y, w, h, 5.0, [0.06, 0.05, 0.04, 0.98], Some(BORDER));
        c.text_center(320.0, y + 14.0, "Copy existing user data?", TXT * 1.1, GOLD_FG);
        let body = qcore::lang::tr("Saves, configs, demos and downloaded map packs can be copied to the new folder.");
        let lines = qcore::lang::wrap(&body, w - 32.0, |t| c.text_w_raw(t, TXT));
        for (i, line) in lines.iter().take(3).enumerate() {
            center_raw(c, 320.0, y + 36.0 + i as f32 * 11.0, line, TXT, FG);
        }
        let dest = crate::userdata::resolve_root(chosen);
        let dest_shown = fit_path_tail(c, &dest.to_string_lossy(), w - 32.0, TXT);
        center_raw(c, 320.0, y + 76.0, &dest_shown, TXT, MUTED);

        let labels = [qcore::t!("Copy Data"), qcore::t!("Switch Only"), qcore::t!("Cancel")];
        const MODAL_KEYS: [Option<i32>; 3] = [None, None, Some(K_BBUTTON)];
        for (i, (&(bx, bw), &label)) in MODAL_BTNS.iter().zip(&labels).enumerate() {
            let on = i == cur;
            c.round_box(bx, MODAL_BTN_Y, bw, MODAL_BTN_H, 4.0, if on { HILITE } else { [0.0, 0.0, 0.0, 0.7] }, Some(BORDER));
            glyphs.button_label(c, [bx, MODAL_BTN_Y, bw, MODAL_BTN_H], pad_ui.then_some(MODAL_KEYS[i]).flatten(), label, TXT, if on { GOLD_FG } else { MUTED });
        }
    }
}

fn center_raw(c: &mut Canvas, cx: f32, y: f32, s: &str, size: f32, color: [f32; 4]) {
    let w = c.text_w_raw(s, size);
    c.text_raw((cx - w / 2.0).floor(), y, s, size, color);
}

/// Truncates a path from the left (`.../tail`) so it fits inside `max_w`.
pub(in crate::menu) fn fit_path_tail(c: &Canvas, s: &str, max_w: f32, size: f32) -> String {
    if c.text_w_raw(s, size) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut lo = 0;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let cand = format!("...{}", chars[mid..].iter().collect::<String>());
        if c.text_w_raw(&cand, size) <= max_w {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    format!("...{}", chars[lo.min(chars.len())..].iter().collect::<String>())
}

/// Truncates a string from the right (`head...`) so it fits inside `max_w`.
fn fit_tail_right(c: &Canvas, s: &str, max_w: f32, size: f32) -> String {
    if c.text_w_raw(s, size) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut lo = 0;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let cand = format!("{}...", chars[..mid].iter().collect::<String>());
        if c.text_w_raw(&cand, size) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    format!("{}...", chars[..lo].iter().collect::<String>())
}
