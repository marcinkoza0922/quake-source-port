//! Drop-down console (classic `console.c` + modern `con_modern` terminal with
//! fuzzy autocomplete, persistent history, ANSI styling, interactive `bash`
//! PTY and Quake-to-shell command piping), notify lines and centerprint.

pub mod ansi;
pub mod complete;
pub mod edit;
mod modern_draw;
pub mod shell;
// the terminal emulator only runs a shell on unix; the web build has none
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub mod vt;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use qinput::keys::*;

use crate::draw::Canvas;

pub(crate) const MAX_LINES: usize = 4096;
const NUM_NOTIFY: usize = 4;
/// Backdrop colour behind messages with a background.
pub const TEXT_BG: [f32; 4] = [0.0, 0.0, 0.0, 0.65];

#[derive(Default)]
pub struct Console {
    /// Completed lines (raw bytes; high bit = coloured text).
    pub lines: VecDeque<Vec<u8>>,
    /// Line being built by `print`.
    cur: Vec<u8>,
    /// Time each of the last lines was printed (notify area).
    notify: VecDeque<(f64, Vec<u8>)>,
    pub input: String,
    pub cursor: usize,
    pub(crate) history: Vec<String>,
    pub(crate) hist_pos: usize,
    pub backscroll: usize,
    /// Current drop-down fraction 0..1.
    pub frac: f32,
    pub open: bool,
    /// Forced full screen (no map running).
    pub forced: bool,
    pub centerprint: Vec<String>,
    pub center_time: f64,
    pub center_start: f64,
    pub realtime: f64,
    /// Width in characters used for wrapping.
    pub width: usize,
    /// con_notifytime: seconds notify lines stay up.
    pub notify_time: f64,
    /// Where the notify lines and the centerprint go (the HUD layout's
    /// message elements); an empty rectangle uses the classic spots.
    pub notify_place: crate::hud::MsgPlace,
    pub center_place: crate::hud::MsgPlace,
    /// `con_modern`: use modern terminal console (default true).
    pub modern: bool,
    /// scr_conalpha: classic console background opacity (forced-up
    /// consoles stay opaque).
    pub alpha: f32,
    /// Target screen height fraction in modern mode (0.30..0.92).
    pub height_frac: f32,
    /// Styled lines for the modern console.
    pub modern_lines: VecDeque<Vec<ansi::StyledCell>>,
    pub(crate) modern_cur: Vec<ansi::StyledCell>,
    pub(crate) style: ansi::CellStyle,
    pub catalog: complete::Catalog,
    pub completion: complete::CompletionState,
    pub(crate) tab_cycled: bool,
    pub search: edit::ReverseSearch,
    pub selection: edit::Selection,
    pub pty: Option<shell::PtySession>,
    pub jobs: Vec<shell::ShellJob>,
    pub shell_env: shell::ShellEnv,
    pub(crate) capture: Option<String>,
    pub(crate) grid_cols: usize,
    pub(crate) grid_rows: usize,
    pub(crate) last_body_top: f32,
    pub(crate) last_panel_h: f32,
    pub(crate) last_cell_w: f32,
    pub(crate) hover_cell: Option<(usize, usize)>,
    /// In-game chat prompt (`messagemode` / `messagemode2`, `Key_Message`).
    pub chat_active: bool,
    pub chat_team: bool,
    pub chat_buf: String,
}

impl Console {
    pub fn new() -> Self {
        Self {
            width: 78,
            notify_time: 3.0,
            modern: true,
            alpha: 1.0,
            height_frac: 0.58,
            grid_cols: 80,
            grid_rows: 24,
            ..Default::default()
        }
    }

    /// Con_Print: appends game text, wrapping at `width`.
    pub fn print(&mut self, text: &str) {
        self.print_text(text, false);
    }

    /// [`Self::print`] of the shell's output or the engine's own coloured
    /// text, whose escape sequences are followed.
    pub fn print_ansi(&mut self, text: &str) {
        self.print_text(text, true);
    }

    fn print_text(&mut self, text: &str, with_ansi: bool) {
        use std::io::Write;
        let plain = if with_ansi { ansi::strip_ansi(text).into() } else { std::borrow::Cow::Borrowed(text) };
        if let Some(cap) = &mut self.capture {
            cap.push_str(&plain);
            return;
        }
        let mut game_style = ansi::CellStyle::default();
        ansi::append_styled(
            text,
            &mut self.modern_cur,
            &mut self.modern_lines,
            if with_ansi { &mut self.style } else { &mut game_style },
            self.width,
            MAX_LINES,
            with_ansi,
        );
        // (the terminal may be gone; conchars' high codes aren't for it either)
        let _ = write!(std::io::stderr(), "{}", plain.replace(|c: char| c.is_control() && c != '\n', ""));
        let mut bytes: Vec<u8> = plain.chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }).collect();
        // \x01 / \x02: coloured line (Con_Print mask)
        let mut mask = 0u8;
        if let Some(&b) = bytes.first()
            && (b == 1 || b == 2)
        {
            mask = 128;
            bytes.remove(0);
        }
        for b in bytes {
            if b == b'\n' {
                self.newline();
            } else if b == b'\r' {
                self.cur.clear();
            } else {
                self.cur.push(b | mask);
                if self.cur.len() >= self.width.max(10) {
                    self.newline();
                }
            }
        }
    }

    fn newline(&mut self) {
        let l = std::mem::take(&mut self.cur);
        self.notify.push_back((self.realtime, l.clone()));
        while self.notify.len() > NUM_NOTIFY {
            self.notify.pop_front();
        }
        self.lines.push_back(l);
        if self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }

    pub fn clear_notify(&mut self) {
        self.notify.clear();
    }

    /// SCR_CenterPrint. `wrap_cols` > 0 word-wraps (scr_usekfont).
    pub fn center_print(&mut self, s: &str, centertime: f64, wrap_cols: usize) {
        let s = word_wrap(s, wrap_cols);
        self.centerprint = s.split('\n').map(str::to_string).collect();
        while self.centerprint.last().is_some_and(String::is_empty) {
            self.centerprint.pop();
        }
        self.center_start = self.realtime;
        self.center_time = self.realtime + centertime;
    }

    /// Classic key handler (`con_modern 0`). Returns a command line to execute.
    pub fn key(&mut self, key: i32, complete: &dyn Fn(&str) -> Vec<String>) -> Option<String> {
        match key {
            K_ENTER | K_KP_ENTER => {
                let line = std::mem::take(&mut self.input);
                self.cursor = 0;
                self.print(&format!("]{line}\n"));
                if !line.trim().is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                }
                self.hist_pos = self.history.len();
                self.backscroll = 0;
                return Some(line);
            }
            K_TAB => self.complete(complete),
            K_BACKSPACE => {
                if self.cursor > 0 {
                    self.cursor = self.prev_char();
                    self.input.remove(self.cursor);
                }
            }
            K_DEL => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            K_LEFTARROW => self.cursor = self.prev_char(),
            K_RIGHTARROW => self.cursor += self.input[self.cursor..].chars().next().map_or(0, char::len_utf8),
            K_HOME => self.cursor = 0,
            K_END => self.cursor = self.input.len(),
            K_UPARROW => {
                if self.hist_pos > 0 {
                    self.hist_pos -= 1;
                    self.input = self.history[self.hist_pos].clone();
                    self.cursor = self.input.len();
                }
            }
            K_DOWNARROW => {
                if self.hist_pos < self.history.len() {
                    self.hist_pos += 1;
                    self.input = self.history.get(self.hist_pos).cloned().unwrap_or_default();
                    self.cursor = self.input.len();
                }
            }
            K_PGUP | K_MWHEELUP => self.backscroll = (self.backscroll + 4).min(self.lines.len().saturating_sub(1)),
            K_PGDN | K_MWHEELDOWN => self.backscroll = self.backscroll.saturating_sub(4),
            _ => {}
        }
        None
    }

    /// Where the character before the cursor starts.
    fn prev_char(&self) -> usize {
        self.input[..self.cursor].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    pub fn text(&mut self, ch: char) {
        if self.modern {
            self.text_modern(ch);
            return;
        }
        if ch.is_ascii() && !ch.is_ascii_control() && ch != '`' && ch != '~' {
            self.input.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }

    /// Con_MessageMode_f / Con_MessageMode2_f.
    pub fn start_chat(&mut self, team: bool) {
        self.chat_active = true;
        self.chat_team = team;
        self.chat_buf.clear();
    }

    /// Key_EndChat.
    pub fn end_chat(&mut self) {
        self.chat_active = false;
        self.chat_buf.clear();
    }

    /// Key_Message (keys.c). Returns a `say`/`say_team` command on Enter.
    pub fn chat_key(&mut self, key: i32) -> Option<String> {
        match key {
            K_ENTER | K_KP_ENTER => {
                let msg = std::mem::take(&mut self.chat_buf).replace(['"', '\n', '\r'], "");
                let cmd = if self.chat_team { "say_team" } else { "say" };
                self.chat_active = false;
                (!msg.is_empty()).then(|| format!("{cmd} \"{msg}\"\n"))
            }
            K_ESCAPE => {
                self.end_chat();
                None
            }
            K_BACKSPACE => {
                self.chat_buf.pop();
                None
            }
            _ => None,
        }
    }

    /// Char_Message (keys.c).
    pub fn chat_text(&mut self, ch: char) {
        if !ch.is_control() && self.chat_buf.chars().count() < 255 {
            self.chat_buf.push(ch);
        }
    }

    /// Classic tab completion over the last word of the input line.
    fn complete(&mut self, complete: &dyn Fn(&str) -> Vec<String>) {
        let start = self.input[..self.cursor].rfind([' ', ';']).map_or(0, |i| i + 1);
        let partial = self.input[start..self.cursor].to_string();
        if partial.is_empty() {
            return;
        }
        let mut m = complete(&partial);
        m.retain(|s| complete::insertable(s));
        m.sort_unstable_by_key(|s| s.to_ascii_lowercase());
        m.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        if m.is_empty() {
            return;
        }
        let mut repl = if m.len() == 1 {
            format!("{} ", m[0])
        } else {
            self.print("\n");
            for s in m.iter().take(64) {
                self.print(&format!("  {s}\n"));
            }
            let mut p = m[0].clone();
            for s in &m[1..] {
                let n = p.bytes().zip(s.bytes()).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).count();
                p.truncate(p.floor_char_boundary(n));
            }
            p
        };
        if repl.len() < partial.len() {
            repl = partial;
        }
        self.input.replace_range(start..self.cursor, &repl);
        self.cursor = start + repl.len();
    }

    /// Slide animation (con_speed-like).
    pub fn update(&mut self, dt: f32) {
        let target = if self.forced || self.open { 1.0 } else { 0.0 };
        let step = dt * 4.0;
        if self.frac < target {
            self.frac = (self.frac + step).min(target);
        } else if self.frac > target {
            self.frac = (self.frac - step).max(target);
        }
    }

    /// Draws console, notify lines and centerprint.
    pub fn draw(&mut self, c: &mut Canvas, conback: Option<(qrender::PicId, u32, u32)>, show_notify: bool, centerprint: bool) {
        c.reset_origin();
        let (vw, vh) = (c.vw(), c.vh());
        let full = if self.forced { vh } else { (vh / 2.0).floor() };
        let lines_h = (full * self.frac).floor();
        if lines_h > 0.0 {
            if self.modern && c.ui.is_some() {
                self.draw_modern(c);
                return;
            }
            self.width = ((vw / 8.0) as usize).saturating_sub(2).max(10);
            let alpha = if self.forced { 1.0 } else { self.alpha.clamp(0.0, 1.0) };
            if let Some((pic, _, _)) = conback {
                let s = c.scale;
                if alpha > 0.0 {
                    c.pic_stretch_px_color([0.0, (lines_h - vh) * s, vw * s, vh * s], pic, [1.0, 1.0, 1.0, alpha]);
                }
            } else {
                c.fill(0.0, 0.0, vw, lines_h, [0.0, 0.0, 0.0, 0.8]);
            }
            // input line
            let y = lines_h - 16.0;
            let blink = (self.realtime * 4.0) as i64 & 1 == 0;
            let maxw = self.width;
            let shown_from = (self.cursor + 2).saturating_sub(maxw);
            c.ch(8.0, y, b']');
            for (i, b) in self.input.bytes().skip(shown_from).take(maxw).enumerate() {
                c.ch(16.0 + i as f32 * 8.0, y, b);
            }
            if blink {
                c.ch(16.0 + (self.cursor - shown_from) as f32 * 8.0, y, 11);
            }
            // text
            let mut row = y - 8.0;
            let mut n = self.lines.len().saturating_sub(self.backscroll);
            if self.backscroll > 0 {
                for i in 0..self.width / 4 {
                    c.ch(8.0 + i as f32 * 32.0, row, b'^');
                }
                row -= 8.0;
            }
            while row > -8.0 && n > 0 {
                n -= 1;
                c.qtext_grid(8.0, row, &self.lines[n], crate::draw::WHITE);
                row -= 8.0;
            }
        } else if show_notify {
            let base = c.scale;
            let p = self.notify_place;
            c.scale = (base * p.scale.clamp(1.0, 3.0)).round().max(1.0);
            let (rx, ry, rw, rh) = place_rect(&p, base / c.scale, (8.0, 0.0, c.vw() - 16.0, 32.0));
            let mut y = ry;
            if !p.hidden {
                let live: Vec<&Vec<u8>> =
                    self.notify.iter().filter(|(t, _)| self.realtime - t <= self.notify_time).map(|(_, l)| l).collect();
                y = if p.up { ry + rh - live.len() as f32 * 8.0 } else { ry };
                for l in live {
                    let w = c.qtext_w(l);
                    let x = align_x(p.align, rx, rw, w);
                    if p.bg && !l.is_empty() {
                        c.fill(x - 2.0, y - 1.0, w + 4.0, 10.0, TEXT_BG);
                    }
                    c.qtext(x, y, l, p.color);
                    y += 8.0;
                }
            }
            if self.chat_active {
                let prefix = if self.chat_team { "say_team: " } else { "say: " };
                let max_cols = ((rw / 8.0) as usize).saturating_sub(prefix.len() + 1).max(8);
                let tail: String = self.chat_buf.chars().rev().take(max_cols).collect::<String>().chars().rev().collect();
                let line = format!("{prefix}{tail}");
                let bytes = qcore::text::encode(&line);
                let w = c.qtext_w(&bytes) + 8.0;
                c.fill(rx - 2.0, y - 1.0, w + 4.0, 10.0, TEXT_BG);
                c.qtext(rx, y, &bytes, crate::draw::WHITE);
                if (self.realtime * 4.0) as i64 & 1 == 0 {
                    c.ch(rx + c.qtext_w(&bytes), y, 11);
                }
            }
            c.scale = base;
        }
        if centerprint && lines_h <= 0.0 && self.realtime < self.center_time && !self.center_place.hidden {
            self.draw_center(c, usize::MAX);
        }
    }

    /// SCR_DrawCenterString; `max_chars` limits revealed characters (finale).
    pub fn draw_center(&self, c: &mut Canvas, max_chars: usize) {
        let base = c.scale;
        c.scale = (base * self.center_place.scale.clamp(1.0, 3.0)).round().max(1.0);
        self.draw_center_scaled(c, max_chars, base / c.scale);
        c.scale = base;
    }

    /// `k` turns HUD units into the scaled text's units.
    fn draw_center_scaled(&self, c: &mut Canvas, max_chars: usize, k: f32) {
        let (vw, vh) = (c.vw(), c.vh());
        let p = self.center_place;
        let n = self.centerprint.len();
        let (rx, ry, rw, rh) = place_rect(&p, k, (0.0, (vh * 0.35).floor(), vw, 32.0));
        let align = if p.rect.w > 0.0 { p.align } else { 1 };
        let h = n as f32 * 8.0;
        // a long message (a level's story) still fits on screen
        let y0 = if p.up { ry + rh - h } else { ry };
        let mut y = y0.min(vh - h - 4.0).max(4.0).floor();
        let lines: Vec<Vec<u8>> = self.centerprint.iter().map(|l| qcore::text::encode(l)).collect();
        if p.bg && n > 0 {
            let w = lines.iter().map(|l| c.qtext_w(l)).fold(0.0, f32::max);
            c.fill(align_x(align, rx, rw, w) - 4.0, y - 3.0, w + 8.0, h + 6.0, TEXT_BG);
        }
        let mut left = max_chars;
        for l in &lines {
            let x = align_x(align, rx, rw, c.qtext_w(l));
            let shown = crate::draw::glyph_prefix(l, left);
            c.qtext(x, y, shown, p.color);
            left -= crate::draw::glyph_count(shown);
            if left == 0 {
                return;
            }
            y += 8.0;
        }
    }
}

/// The place's rectangle in text units (`k` per HUD unit), or `classic`
/// when the layout gave none.
fn place_rect(p: &crate::hud::MsgPlace, k: f32, classic: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let r = p.rect;
    if r.w <= 0.0 { classic } else { (r.x * k, r.y * k, r.w * k, r.h * k) }
}

/// Left edge of a line `w` wide in the span `x..x + room`.
fn align_x(align: usize, x: f32, room: f32, w: f32) -> f32 {
    match align {
        0 => x,
        1 => x + ((room - w) / 2.0).floor(),
        _ => x + room - w,
    }
    .floor()
}

/// SCR_GetCenterPrintWrapLimit: columns for a menu canvas `vw` units wide.
pub fn center_wrap_cols(vw: f32) -> usize {
    let w = 320.0 + (vw - 320.0) * 0.25;
    (w.clamp(320.0, 400.0) as usize) >> 3
}

/// Translated text with Chinese/Japanese/Korean characters or scripts with
/// combining marks (valid UTF-8 in the Quake text stream, drawn with the
/// TTF), which [`word_wrap`]'s byte columns would cut mid-character.
fn wide_text(b: &[u8]) -> Option<&str> {
    if !qcore::lang::unicode_text() || b.is_ascii() {
        return None;
    }
    std::str::from_utf8(b).ok().filter(|s| qcore::lang::wrap::needs_cluster_wrap(s))
}

/// [`word_wrap`] by grapheme clusters (a CJK glyph at `QTEXT_SIZE` is about
/// 9/8 of a column), breaking between CJK characters and at Thai/Khmer
/// zero width spaces.
fn wrap_wide(s: &str, maxcols: usize) -> String {
    let lines: Vec<String> = s.split('\n').flat_map(|l| qcore::lang::wrap(l, maxcols as f32, qcore::lang::wrap::cluster_cols)).collect();
    qcore::lang::quake_encode(&lines.join("\n")).into_owned()
}

/// COM_WordWrap: break `src` into lines of at most `maxcols` chars, moving
/// words that don't fit to the next line and splitting longer ones.
pub fn word_wrap(src: &str, maxcols: usize) -> String {
    if maxcols == 0 {
        return src.to_string();
    }
    let b = qcore::text::encode(src);
    if let Some(s) = wide_text(&b) {
        return wrap_wide(s, maxcols);
    }
    let sp = |c: u8| c.is_ascii_whitespace();
    let mut out = Vec::with_capacity(b.len() + 8);
    let mut p = 0;
    while p < b.len() {
        let s = &b[p..];
        let mut i = 0;
        let (len, adv) = 'line: {
            while i < maxcols && i < s.len() {
                if s[i] == b'\n' {
                    break 'line (i, i + 1);
                }
                if !sp(s[i]) && (i == 0 || sp(s[i - 1])) {
                    let wl = s[i..].iter().take_while(|&&c| !sp(c)).count();
                    if wl > maxcols {
                        break 'line (maxcols, maxcols);
                    }
                    if i + wl > maxcols {
                        break 'line (i, i);
                    }
                    i += wl;
                } else {
                    i += 1;
                }
            }
            (i, i + usize::from(s.get(i).is_some_and(|&c| sp(c))))
        };
        out.extend_from_slice(&s[..len]);
        p += adv.max(1).min(s.len());
        if p < b.len() {
            out.push(b'\n');
        }
    }
    qcore::text::decode(&out)
}
