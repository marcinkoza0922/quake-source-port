//! Drop-down console (console.c / keys.c line editing), notify lines and
//! centerprint (screen.c).

use std::collections::VecDeque;

use qinput::keys::*;

use crate::draw::Canvas;

const MAX_LINES: usize = 4096;
const NUM_NOTIFY: usize = 4;
/// Backdrop colour behind messages when `text_bg` is on.
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
    history: Vec<String>,
    hist_pos: usize,
    pub backscroll: usize,
    /// Current drop-down fraction 0..1 of half screen.
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
    /// scr_textbg: dark backdrop behind notify and centerprint text.
    pub text_bg: bool,
    /// scr_textscale: size multiplier for notify and centerprint text.
    pub text_scale: f32,
    /// Center message position: 0 upper (classic), 1 middle, 2 lower.
    pub center_pos: u8,
    /// Notify line position: 0 top-left, 1 top-center, 2 bottom-left.
    pub notify_pos: u8,
}

impl Console {
    pub fn new() -> Self {
        Self { width: 78, notify_time: 3.0, text_scale: 1.0, ..Default::default() }
    }

    /// Con_Print: appends text, wrapping at `width`.
    pub fn print(&mut self, text: &str) {
        eprint!("{}", text.replace(|c: char| (c as u32) < 32 && c != '\n', ""));
        let mut bytes: Vec<u8> = text.chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }).collect();
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

    /// Key in console. Returns a command line to execute or completion request.
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
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            K_DEL => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            K_LEFTARROW => self.cursor = self.cursor.saturating_sub(1),
            K_RIGHTARROW => self.cursor = (self.cursor + 1).min(self.input.len()),
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

    pub fn text(&mut self, ch: char) {
        if ch.is_ascii() && !ch.is_ascii_control() && ch != '`' && ch != '~' {
            self.input.insert(self.cursor, ch);
            self.cursor += 1;
        }
    }

    /// Tab completion over the last word of the input line.
    fn complete(&mut self, complete: &dyn Fn(&str) -> Vec<String>) {
        let start = self.input[..self.cursor].rfind([' ', ';']).map_or(0, |i| i + 1);
        let partial = self.input[start..self.cursor].to_string();
        if partial.is_empty() {
            return;
        }
        let mut m = complete(&partial);
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
                p.truncate(n);
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
        let step = dt * 3.0;
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
        self.width = ((vw / 8.0) as usize).saturating_sub(2).max(10);
        let full = if self.forced { vh } else { (vh / 2.0).floor() };
        let lines_h = (full * self.frac).floor();
        if lines_h > 0.0 {
            if let Some((pic, _, _)) = conback {
                let s = c.scale;
                c.pic_stretch_px([0.0, (lines_h - vh) * s, vw * s, vh * s], pic);
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
                for (i, &b) in self.lines[n].iter().enumerate() {
                    c.ch(8.0 + i as f32 * 8.0, row, b);
                }
                row -= 8.0;
            }
        } else if show_notify {
            let base = c.scale;
            c.scale = (base * self.text_scale.clamp(1.0, 3.0)).round().max(1.0);
            let live: Vec<&Vec<u8>> =
                self.notify.iter().filter(|(t, _)| self.realtime - t <= self.notify_time).map(|(_, l)| l).collect();
            let (svw, svh) = (c.vw(), c.vh());
            // bottom-left sits above the modern HUD's health/armor stack
            let mut y = if self.notify_pos == 2 { (svh * 0.7).floor() - live.len() as f32 * 8.0 } else { 0.0 };
            for l in live {
                let x = if self.notify_pos == 1 { ((svw - l.len() as f32 * 8.0) / 2.0).floor() } else { 8.0 };
                if self.text_bg && !l.is_empty() {
                    c.fill(x - 2.0, y - 1.0, l.len() as f32 * 8.0 + 4.0, 10.0, TEXT_BG);
                }
                for (i, &b) in l.iter().enumerate() {
                    c.ch(x + i as f32 * 8.0, y, b);
                }
                y += 8.0;
            }
            c.scale = base;
        }
        if centerprint && lines_h <= 0.0 && self.realtime < self.center_time {
            self.draw_center(c, usize::MAX);
        }
    }

    /// SCR_DrawCenterString; `max_chars` limits revealed characters (finale).
    pub fn draw_center(&self, c: &mut Canvas, max_chars: usize) {
        let base = c.scale;
        c.scale = (base * self.text_scale.clamp(1.0, 3.0)).round().max(1.0);
        self.draw_center_scaled(c, max_chars);
        c.scale = base;
    }

    fn draw_center_scaled(&self, c: &mut Canvas, max_chars: usize) {
        let (vw, vh) = (c.vw(), c.vh());
        let n = self.centerprint.len();
        let mut y = match self.center_pos {
            _ if n > 4 => 48.0,
            1 => ((vh - n as f32 * 8.0) / 2.0).floor(),
            2 => (vh * 0.68).floor() - n as f32 * 8.0,
            _ => (vh * 0.35).floor(),
        };
        if self.text_bg && n > 0 {
            let w = self.centerprint.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * 8.0;
            c.fill(((vw - w) / 2.0).floor() - 4.0, y - 3.0, w + 8.0, n as f32 * 8.0 + 6.0, TEXT_BG);
        }
        let mut left = max_chars;
        for l in &self.centerprint {
            let x = ((vw - l.len() as f32 * 8.0) / 2.0).floor();
            for (i, b) in l.bytes().enumerate() {
                if left == 0 {
                    return;
                }
                left -= 1;
                c.ch(x + i as f32 * 8.0, y, b);
            }
            y += 8.0;
        }
    }
}

/// SCR_GetCenterPrintWrapLimit: columns for a menu canvas `vw` units wide.
pub fn center_wrap_cols(vw: f32) -> usize {
    let w = 320.0 + (vw - 320.0) * 0.25;
    (w.clamp(320.0, 400.0) as usize) >> 3
}

/// COM_WordWrap: break `src` into lines of at most `maxcols` chars, moving
/// words that don't fit to the next line and splitting longer ones.
pub fn word_wrap(src: &str, maxcols: usize) -> String {
    if maxcols == 0 {
        return src.to_string();
    }
    let b = src.as_bytes();
    let sp = |c: u8| c.is_ascii_whitespace();
    let mut out = Vec::with_capacity(b.len() + 8);
    let mut p = 0;
    while p < b.len() {
        let s = &b[p..];
        // COM_AdvanceLineWrapped
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
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod wrap_tests {
    use super::word_wrap;

    #[test]
    fn wraps_words() {
        // as COM_AdvanceLineWrapped, the space before a pushed word stays
        assert_eq!(word_wrap("the quick brown fox", 10), "the quick \nbrown fox");
        assert_eq!(word_wrap("abcdefghijkl xy", 5), "abcde\nfghij\nkl xy");
        assert_eq!(word_wrap("one\ntwo", 40), "one\ntwo");
        assert_eq!(word_wrap("no wrap", 0), "no wrap");
    }
}
