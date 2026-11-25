//! Rendering and mouse-selection hit-testing for the modern console.

use qrender::Draw2D;

use crate::draw::Canvas;

use super::Console;
use super::ansi::StyledCell;

/// Text selection in grid coordinates `(row_from_top, col)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (usize, usize),
    pub focus: (usize, usize),
    pub active: bool,
    pub dragging: bool,
}

impl Selection {
    pub fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.focus { (self.anchor, self.focus) } else { (self.focus, self.anchor) }
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        if !self.active || self.anchor == self.focus {
            return false;
        }
        let ((r0, c0), (r1, c1)) = self.ordered();
        if row < r0 || row > r1 {
            return false;
        }
        if r0 == r1 { col >= c0 && col <= c1 } else if row == r0 { col >= c0 } else if row == r1 { col <= c1 } else { true }
    }
}

pub const FONT_SZ: f32 = 6.8;
pub const ROW_H: f32 = 8.6;
pub const HEADER_H: f32 = 13.0;
pub const INPUT_H: f32 = 15.0;
pub const PAD_X: f32 = 8.0;

const BG_MAIN: [f32; 4] = [0.04, 0.05, 0.07, 0.93];
const BG_HEADER: [f32; 4] = [0.07, 0.08, 0.11, 0.97];
const BG_INPUT: [f32; 4] = [0.06, 0.07, 0.10, 0.97];
const BG_SELECT: [f32; 4] = [0.22, 0.42, 0.72, 0.55];
const ACCENT_QUAKE: [f32; 4] = [0.82, 0.62, 0.28, 0.92];
const ACCENT_BASH: [f32; 4] = [0.28, 0.78, 0.52, 0.92];
const TEXT_DIM: [f32; 4] = [0.52, 0.56, 0.64, 1.0];
const TEXT_GHOST: [f32; 4] = [0.44, 0.48, 0.56, 0.9];

impl Console {
    /// Draw the modern drop-down console (`con_modern 1`).
    pub(super) fn draw_modern(&mut self, c: &mut Canvas) {
        c.reset_origin();
        let (vw, vh) = (c.vw(), c.vh());
        let h_frac = self.height_frac.clamp(0.30, 0.92);
        let full = if self.forced { vh } else { (vh * h_frac).floor() };
        let panel_h = (full * self.frac).floor();
        if panel_h <= 2.0 {
            return;
        }

        let in_bash = self.pty.is_some();
        let accent = if in_bash { ACCENT_BASH } else { ACCENT_QUAKE };

        // Main backdrop + header bar + bottom border
        c.fill(0.0, 0.0, vw, panel_h, BG_MAIN);
        let header_h = HEADER_H.min(panel_h);
        c.fill(0.0, 0.0, vw, header_h, BG_HEADER);
        c.fill(0.0, header_h - 0.6, vw, 0.6, [1.0, 1.0, 1.0, 0.07]);
        c.fill(0.0, panel_h - 1.2, vw, 1.2, accent);

        // Compute grid dimensions from the bundled monospace font's exact pixel advance
        let cell_w = c.mono_cell_w(FONT_SZ);
        self.last_cell_w = cell_w;
        let cols = ((vw - PAD_X * 2.0 - 6.0) / cell_w).floor().max(20.0) as usize;
        self.width = cols;
        let bottom_reserve = if in_bash { 3.0 } else { INPUT_H };
        let body_top = HEADER_H + 2.0;
        let body_bot = (panel_h - bottom_reserve).max(body_top);
        let body_h = (body_bot - body_top).max(0.0);
        let rows = (body_h / ROW_H).floor().max(1.0) as usize;
        self.grid_cols = cols;
        self.grid_rows = rows;
        self.last_body_top = body_top;
        self.last_panel_h = panel_h;

        // sized for the open console: sliding it doesn't resize the terminal
        let open_rows = ((full - bottom_reserve - body_top).max(0.0) / ROW_H).floor().max(1.0) as usize;
        if let Some(pty) = &mut self.pty {
            pty.resize(cols, open_rows);
        }

        // Clip everything in the console panel
        c.clip_rows(Some((0.0, panel_h)));

        // Draw header bar
        self.draw_header(c, vw, in_bash, accent);

        // Draw body rows (either interactive PTY screen or Quake scrollback)
        c.clip_rows(Some((body_top, body_bot)));
        if in_bash {
            self.draw_pty_body(c, body_top, rows, cols, cell_w);
        } else {
            self.draw_scrollback_body(c, body_top, body_bot, rows, cols, cell_w);
        }
        c.clip_rows(Some((0.0, panel_h)));

        // Scrollbar on the right edge
        self.draw_scrollbar(c, vw, body_top, body_h, rows);

        // Input bar & completion popup (in Quake mode)
        if !in_bash && panel_h > HEADER_H + INPUT_H {
            let input_y = panel_h - INPUT_H;
            self.draw_input_bar(c, vw, input_y, cols, cell_w);
            if self.completion.visible && !self.completion.items.is_empty() {
                c.clip_rows(None);
                self.draw_completion_popup(c, vw, input_y, cell_w);
            }
        }
        c.clip_rows(None);
    }

    fn draw_header(&self, c: &mut Canvas, vw: f32, in_bash: bool, accent: [f32; 4]) {
        let badge = if in_bash { " BASH " } else { " QUAKE " };
        let bw = c.text_w(badge, 5.5) + 4.0;
        c.round_box(PAD_X, 2.2, bw, 8.6, 2.0, accent, None);
        c.text(PAD_X + 2.0, 3.0, badge, 5.5, [0.05, 0.06, 0.08, 1.0]);

        let status = if let Some(pty) = &self.pty {
            let dir = pty.cwd_display().display();
            if pty.vt.title.is_empty() {
                format!("{dir}  (type 'exit' or Ctrl+D to return to Quake)")
            } else {
                format!("{dir} — {}  ('exit' to return)", pty.vt.title)
            }
        } else if !self.jobs.is_empty() {
            format!("running {} background shell job(s)...", self.jobs.len())
        } else {
            let g = if self.shell_env.gamedir.is_empty() { "id1" } else { &self.shell_env.gamedir };
            if self.shell_env.mapname.is_empty() {
                g.to_string()
            } else {
                format!("{g} · {}", self.shell_env.mapname)
            }
        };
        c.text(PAD_X + bw + 6.0, 3.2, &status, 5.5, [0.78, 0.82, 0.88, 1.0]);

        let right_hint = if self.backscroll > 0 {
            qcore::tr!("↑ +{} lines (End to jump) ", self.backscroll)
        } else if in_bash {
            qcore::tr!("Shift+PgUp/Dn: height  ` : hide")
        } else {
            qcore::tr!("Tab: complete  Ctrl+R: search  bash | pipe")
        };
        c.text_right(vw - PAD_X, 3.2, &right_hint, 5.2, TEXT_DIM);
    }

    fn draw_pty_body(&self, c: &mut Canvas, body_top: f32, rows: usize, cols: usize, cell_w: f32) {
        let Some(pty) = &self.pty else { return };
        let vt = &pty.vt;
        let sb_len = vt.scrollback.len();
        let back = self.backscroll.min(sb_len);
        let total = sb_len + vt.rows;
        let end_idx = total.saturating_sub(back);
        let start_idx = end_idx.saturating_sub(rows);

        for (r, idx) in (start_idx..end_idx).enumerate() {
            let y = body_top + r as f32 * ROW_H;
            let row_slice = if idx < sb_len {
                vt.scrollback.get(idx).map(Vec::as_slice)
            } else {
                vt.cells.get(idx - sb_len).map(Vec::as_slice)
            };
            if let Some(cells) = row_slice {
                draw_cell_row(c, PAD_X, y, r, cells, cols, cell_w, &self.selection);
            }
        }

        // Draw PTY cursor when at the bottom of scrollback
        if back == 0 && vt.cursor_visible {
            let vis_rows = end_idx - start_idx;
            let cur_r = vt.cursor_row.min(vis_rows.saturating_sub(1));
            let cur_c = vt.cursor_col.min(cols.saturating_sub(1));
            let cx = PAD_X + cur_c as f32 * cell_w;
            let cy = body_top + cur_r as f32 * ROW_H;
            let blink = (self.realtime * 3.5) as i64 & 1 == 0;
            let alpha = if blink { 0.85 } else { 0.35 };
            c.fill(cx, cy + 0.5, cell_w, ROW_H - 1.0, [0.28, 0.85, 0.56, alpha]);
        }
    }

    fn draw_scrollback_body(&self, c: &mut Canvas, body_top: f32, body_bot: f32, rows: usize, cols: usize, cell_w: f32) {
        let total = self.modern_lines.len() + usize::from(!self.modern_cur.is_empty());
        let back = self.backscroll.min(total.saturating_sub(1));
        let end = total.saturating_sub(back);
        let start = end.saturating_sub(rows);
        let count = end - start;
        // Anchor lines to the bottom of the body area so new output sits right above the prompt.
        let first_y = (body_bot - count as f32 * ROW_H).max(body_top);

        for (r, idx) in (start..end).enumerate() {
            let y = first_y + r as f32 * ROW_H;
            let cells = if idx < self.modern_lines.len() {
                self.modern_lines.get(idx).map(Vec::as_slice)
            } else {
                Some(self.modern_cur.as_slice())
            };
            if let Some(cells) = cells {
                draw_cell_row(c, PAD_X, y, r, cells, cols, cell_w, &self.selection);
            }
        }
    }

    fn draw_scrollbar(&self, c: &mut Canvas, vw: f32, body_top: f32, body_h: f32, rows: usize) {
        let total = if let Some(pty) = &self.pty {
            pty.vt.scrollback.len() + pty.vt.rows
        } else {
            self.modern_lines.len()
        };
        if total <= rows || body_h <= 8.0 {
            return;
        }
        let track_x = vw - 4.0;
        c.fill(track_x, body_top, 2.0, body_h, [1.0, 1.0, 1.0, 0.06]);
        let ratio = (rows as f32 / total as f32).clamp(0.08, 1.0);
        let thumb_h = (body_h * ratio).max(6.0);
        let max_back = total.saturating_sub(rows).max(1) as f32;
        let pos = 1.0 - (self.backscroll as f32 / max_back).clamp(0.0, 1.0);
        let thumb_y = body_top + (body_h - thumb_h) * pos;
        c.fill(track_x, thumb_y, 2.0, thumb_h, [0.75, 0.78, 0.85, 0.45]);
    }

    fn draw_input_bar(&self, c: &mut Canvas, vw: f32, y: f32, cols: usize, cell_w: f32) {
        c.fill(0.0, y, vw, INPUT_H, BG_INPUT);
        c.fill(0.0, y, vw, 0.6, [1.0, 1.0, 1.0, 0.08]);

        let text_y = y + 3.2;
        if self.search.active {
            let badge = format!("(reverse-i-search)`{}': ", self.search.query);
            c.mono_str(PAD_X, text_y, &badge, FONT_SZ, cell_w, [0.95, 0.78, 0.35, 1.0]);
            let matched = self
                .search
                .match_idx
                .and_then(|i| self.history.get(i))
                .map_or("", String::as_str);
            let bx = PAD_X + badge.chars().count() as f32 * cell_w;
            c.mono_str(bx, text_y, matched, FONT_SZ, cell_w, [0.92, 0.94, 0.98, 1.0]);
            return;
        }

        // Prompt symbol `]`
        c.mono_str(PAD_X, text_y, "]", FONT_SZ, cell_w, ACCENT_QUAKE);

        let input_x = PAD_X + cell_w * 2.0;
        let max_chars = cols.saturating_sub(3).max(8);
        let cursor_chars = self.input[..self.cursor.min(self.input.len())].chars().count();
        let scroll_chars = (cursor_chars + 2).saturating_sub(max_chars);
        let visible_input: String = self.input.chars().skip(scroll_chars).take(max_chars).collect();

        c.mono_str(input_x, text_y, &visible_input, FONT_SZ, cell_w, [0.95, 0.96, 0.98, 1.0]);

        // Inline ghost completion preview when cursor is at the end of the line
        if self.cursor == self.input.len()
            && let Some(cand) = self.completion.current()
        {
            let typed = &self.input[self.completion.replace_start.min(self.input.len())..];
            if cand.text.len() > typed.len() && cand.text.to_ascii_lowercase().starts_with(&typed.to_ascii_lowercase()) {
                let ghost = &cand.text[typed.len()..];
                let gx = input_x + visible_input.chars().count() as f32 * cell_w;
                c.mono_str(gx, text_y, ghost, FONT_SZ, cell_w, TEXT_GHOST);
            }
        }

        // Cursor bar/block
        let blink = (self.realtime * 4.0) as i64 & 1 == 0;
        if blink {
            let cur_col = cursor_chars.saturating_sub(scroll_chars);
            let cx = input_x + cur_col as f32 * cell_w;
            c.fill(cx, text_y - 0.5, 1.2, ROW_H, ACCENT_QUAKE);
        }
    }

    fn draw_completion_popup(&self, c: &mut Canvas, vw: f32, input_y: f32, cell_w: f32) {
        let items = &self.completion.items;
        let max_show = items.len().min(8);
        if max_show == 0 {
            return;
        }
        let sel = self.completion.selected.min(items.len() - 1);
        let start = sel.saturating_sub(max_show / 2).min(items.len().saturating_sub(max_show));
        let shown = &items[start..start + max_show];

        let row_h = 10.0;
        let pop_h = max_show as f32 * row_h + 6.0;
        let pop_w = 240.0f32.min(vw - PAD_X * 2.0);
        let replace_chars = self.input[..self.completion.replace_start.min(self.input.len())].chars().count();
        let pop_x = (PAD_X + (replace_chars as f32 + 2.0) * cell_w).min(vw - pop_w - PAD_X).max(PAD_X);
        let pop_y = if input_y >= pop_h + HEADER_H + 4.0 {
            input_y - pop_h - 2.0
        } else {
            input_y + INPUT_H + 2.0
        };

        c.round_box(
            pop_x,
            pop_y,
            pop_w,
            pop_h,
            3.0,
            [0.08, 0.10, 0.14, 0.98],
            Some([0.32, 0.38, 0.48, 0.85]),
        );

        for (i, item) in shown.iter().enumerate() {
            let idx = start + i;
            let ry = pop_y + 3.0 + i as f32 * row_h;
            let is_sel = idx == sel;
            if is_sel {
                c.round_box(pop_x + 2.0, ry, pop_w - 4.0, row_h - 0.5, 2.0, [0.18, 0.25, 0.38, 0.95], None);
            }

            // Kind badge
            let k_label = item.kind.label();
            let k_col = item.kind.color();
            c.text(pop_x + 6.0, ry + 1.2, k_label, 5.0, k_col);

            // Candidate name
            let name_col = if is_sel { [1.0, 0.95, 0.78, 1.0] } else { [0.90, 0.92, 0.96, 1.0] };
            c.mono_str(pop_x + 28.0, ry + 1.0, &item.text, 5.8, cell_w, name_col);

            // Right-aligned detail (current/default value or command description)
            if !item.detail.is_empty() {
                let max_detail_w = pop_w - 115.0;
                if max_detail_w > 30.0 {
                    let det = clip_chars(&item.detail, |s| c.text_w(s, 5.0) <= max_detail_w);
                    c.text_right(pop_x + pop_w - 6.0, ry + 1.4, det, 5.0, TEXT_DIM);
                }
            }
        }
    }

    /// Visible row slice at row index `r` (0 = top visible row in body).
    pub(super) fn visible_row_cells(&self, r: usize) -> Option<&[StyledCell]> {
        let rows = self.grid_rows.max(1);
        if let Some(pty) = &self.pty {
            let vt = &pty.vt;
            let sb_len = vt.scrollback.len();
            let back = self.backscroll.min(sb_len);
            let total = sb_len + vt.rows;
            let end_idx = total.saturating_sub(back);
            let start_idx = end_idx.saturating_sub(rows);
            let idx = start_idx + r;
            if idx >= end_idx {
                return None;
            }
            if idx < sb_len {
                vt.scrollback.get(idx).map(Vec::as_slice)
            } else {
                vt.cells.get(idx - sb_len).map(Vec::as_slice)
            }
        } else {
            let total = self.modern_lines.len() + usize::from(!self.modern_cur.is_empty());
            let back = self.backscroll.min(total.saturating_sub(1));
            let end = total.saturating_sub(back);
            let start = end.saturating_sub(rows);
            let idx = start + r;
            if idx >= end {
                return None;
            }
            if idx < self.modern_lines.len() {
                self.modern_lines.get(idx).map(Vec::as_slice)
            } else {
                Some(self.modern_cur.as_slice())
            }
        }
    }

    /// Convert virtual canvas `(vx, vy)` into grid `(row, col)` for mouse selection.
    pub fn hit_cell(&self, vx: f32, vy: f32) -> Option<(usize, usize)> {
        if vy < self.last_body_top || vy > self.last_panel_h {
            return None;
        }
        let cw = self.last_cell_w.max(3.0);
        let r = ((vy - self.last_body_top) / ROW_H).floor().max(0.0) as usize;
        let c = ((vx - PAD_X) / cw).floor().max(0.0) as usize;
        Some((r.min(self.grid_rows.saturating_sub(1)), c.min(self.grid_cols.saturating_sub(1))))
    }

    /// Extract currently selected text from visible modern rows.
    pub fn selected_text(&self) -> Option<String> {
        if !self.selection.active || self.selection.anchor == self.selection.focus {
            return None;
        }
        let ((r0, c0), (r1, c1)) = self.selection.ordered();
        let mut out = String::new();
        for r in r0..=r1 {
            let Some(row) = self.visible_row_cells(r) else { continue };
            let start = if r == r0 { c0.min(row.len()) } else { 0 };
            let end = if r == r1 { (c1 + 1).min(row.len()) } else { row.len() };
            if start < end {
                let s: String = row[start..end].iter().map(|c| c.ch).collect();
                out.push_str(s.trim_end());
            }
            if r < r1 {
                out.push('\n');
            }
        }
        (!out.is_empty()).then_some(out)
    }
}

/// Fast single-borrow row renderer for a slice of [`StyledCell`]s.
#[allow(clippy::too_many_arguments)]
fn draw_cell_row(
    c: &mut Canvas,
    x: f32,
    y: f32,
    row_idx: usize,
    cells: &[StyledCell],
    max_cols: usize,
    cell_w: f32,
    sel: &super::edit::Selection,
) {
    let limit = cells.len().min(max_cols);
    // Background fills (cell backgrounds + selection highlight)
    for col in 0..max_cols {
        let selected = sel.contains(row_idx, col);
        let cell_bg = cells.get(col).and_then(|cell| cell.style.effective_bg());
        if selected {
            c.fill(x + col as f32 * cell_w, y, cell_w, ROW_H, BG_SELECT);
        } else if let Some(bg) = cell_bg {
            c.fill(x + col as f32 * cell_w, y, cell_w, ROW_H, bg);
        }
    }

    // letters that need shaping or bidi can't be drawn a cell at a time:
    // such a row is drawn as proportional text in its first colour
    if cells.iter().take(limit).any(|cell| crate::font::complex(cell.ch)) {
        let s: String = cells.iter().take(limit).map(|cell| cell.ch).collect();
        let fg = cells.iter().find(|cell| cell.ch > ' ').map_or([1.0; 4], |cell| cell.style.effective_fg());
        c.text_raw(x, y, s.trim_end(), FONT_SZ, fg);
        return;
    }
    let Some(ui) = c.ui.clone() else { return };
    let mut ui = ui.borrow_mut();
    let Some(pic) = ui.pic else { return };
    let px = (FONT_SZ * c.scale).round().max(4.0);
    let (x0, y0) = ((x + c.ox) * c.scale, (y + c.oy) * c.scale);
    let x0 = x0.round();
    let base = (y0 + px * 0.8).round();
    let cell_px = ui.mono_advance(px as u16);
    let a = crate::font::UiText::atlas_size() as f32;

    for (col, cell) in cells.iter().take(limit).enumerate() {
        if cell.ch <= ' ' {
            continue;
        }
        if let Some(g) = ui.mono_glyph(cell.ch, px as u16) {
            let cx = x0 + col as f32 * cell_px;
            let (gx, gy) = (cx + g.ox as f32, base + g.oy as f32);
            let uv = [g.x as f32 / a, g.y as f32 / a, (g.x + g.w) as f32 / a, (g.y + g.h) as f32 / a];
            c.push(Draw2D::Pic {
                x: gx,
                y: gy,
                w: g.w as f32,
                h: g.h as f32,
                pic,
                uv,
                color: cell.style.effective_fg(),
            });
        }
    }
}

/// The longest start of `s` (cut between characters) that fits, or a few bytes
/// when nothing does. Servers and mods write these texts.
fn clip_chars(s: &str, fits: impl Fn(&str) -> bool) -> &str {
    let mut s = s;
    while s.len() > 4 && !fits(s) {
        let mut c = s.chars();
        c.next_back();
        s = c.as_str();
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn clips_between_characters() {
        let clip = |s: &str, n: usize| super::clip_chars(s, |s| s.chars().count() <= n).to_string();
        assert_eq!(clip("abcdefgh", 6), "abcdef");
        assert_eq!(clip(&"Á".repeat(60), 10), "Á".repeat(10));
        assert_eq!(clip(&"Ż日".repeat(30), 0).chars().count(), 1);
        assert_eq!(clip("", 0), "");
    }
}
