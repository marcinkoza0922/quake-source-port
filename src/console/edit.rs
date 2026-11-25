//! Modern console line editing, readline shortcuts, reverse history search
//! (`Ctrl+R`), persistent history, and text selection.

use std::path::Path;

use qinput::keys::*;

use super::Console;
use super::complete;

const MAX_HISTORY: usize = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

pub use super::modern_draw::Selection;

#[derive(Clone, Debug, Default)]
pub struct ReverseSearch {
    pub active: bool,
    pub query: String,
    pub match_idx: Option<usize>,
}

impl Console {
    /// Load persistent history from `<dir>/console_history.txt`.
    pub fn load_history(&mut self, dir: &Path) {
        let Ok(text) = qcore::vfs::read_to_string(&dir.join("console_history.txt")) else {
            return;
        };
        self.history.clear();
        for l in text.lines() {
            let trimmed = l.trim();
            if !trimmed.is_empty() && self.history.last().map(String::as_str) != Some(trimmed) {
                self.history.push(trimmed.to_string());
            }
        }
        if self.history.len() > MAX_HISTORY {
            let drop_n = self.history.len() - MAX_HISTORY;
            self.history.drain(..drop_n);
        }
        self.hist_pos = self.history.len();
    }

    /// Save persistent history to `<dir>/console_history.txt`.
    pub fn save_history(&self, dir: &Path) {
        if self.history.is_empty() {
            return;
        }
        let start = self.history.len().saturating_sub(MAX_HISTORY);
        let out: String = self.history[start..].iter().map(|l| format!("{l}\n")).collect();
        let _ = qcore::vfs::write_atomic(&dir.join("console_history.txt"), out.as_bytes());
    }

    /// Modern key handler with modifiers and optional clipboard callbacks.
    /// Returns a command line to execute when `Enter` is pressed.
    pub fn key_modern(
        &mut self,
        key: i32,
        mods: Modifiers,
        clipboard_get: &dyn Fn() -> Option<String>,
        clipboard_set: &dyn Fn(&str),
    ) -> Option<String> {
        // When an interactive PTY (`bash`) session is running, route keys to it.
        if self.pty.is_some() {
            if mods.ctrl && mods.shift && key == b'c' as i32 {
                if let Some(s) = self.selected_text() {
                    clipboard_set(&s);
                }
                return None;
            }
            if (mods.ctrl && mods.shift && key == b'v' as i32) || (mods.shift && key == K_INS) {
                if let Some(clip) = clipboard_get()
                    && let Some(pty) = &mut self.pty
                {
                    pty.send_paste(&clip);
                }
                return None;
            }
            if mods.shift && matches!(key, K_PGUP | K_KP_PGUP) {
                self.height_frac = (self.height_frac - 0.08).clamp(0.30, 0.92);
                return None;
            }
            if mods.shift && matches!(key, K_PGDN | K_KP_PGDN) {
                self.height_frac = (self.height_frac + 0.08).clamp(0.30, 0.92);
                return None;
            }
            if matches!(key, K_MWHEELUP) {
                self.backscroll = (self.backscroll + 3).min(self.max_backscroll());
                return None;
            }
            if matches!(key, K_MWHEELDOWN) {
                self.backscroll = self.backscroll.saturating_sub(3);
                return None;
            }
            if let Some(pty) = &mut self.pty {
                let _ = pty.send_key(key, mods.ctrl, mods.alt);
            }
            return None;
        }

        // Reverse search (`Ctrl+R`) mode.
        if self.search.active {
            return self.handle_search_key(key, mods);
        }

        // Height adjustment: Shift+PgUp / Shift+PgDn
        if mods.shift && matches!(key, K_PGUP | K_KP_PGUP) {
            self.height_frac = (self.height_frac - 0.08).clamp(0.30, 0.92);
            return None;
        }
        if mods.shift && matches!(key, K_PGDN | K_KP_PGDN) {
            self.height_frac = (self.height_frac + 0.08).clamp(0.30, 0.92);
            return None;
        }

        // Ctrl shortcuts
        if mods.ctrl {
            match key {
                k if k == b'r' as i32 => {
                    self.search = ReverseSearch {
                        active: true,
                        query: String::new(),
                        match_idx: None,
                    };
                    self.completion.clear();
                    return None;
                }
                k if k == b'a' as i32 => {
                    self.cursor = 0;
                    self.refresh_completion();
                    return None;
                }
                k if k == b'e' as i32 => {
                    self.cursor = self.input.len();
                    self.refresh_completion();
                    return None;
                }
                k if k == b'u' as i32 => {
                    self.input.drain(..self.cursor);
                    self.cursor = 0;
                    self.refresh_completion();
                    return None;
                }
                k if k == b'k' as i32 => {
                    self.input.truncate(self.cursor);
                    self.refresh_completion();
                    return None;
                }
                k if k == b'w' as i32 || k == K_BACKSPACE => {
                    self.delete_word_back();
                    self.refresh_completion();
                    return None;
                }
                K_DEL | K_KP_DEL => {
                    self.delete_word_forward();
                    self.refresh_completion();
                    return None;
                }
                k if k == b'l' as i32 => {
                    self.clear_scrollback();
                    return None;
                }
                k if k == b'c' as i32 => {
                    if let Some(s) = self.selected_text() {
                        clipboard_set(&s);
                        self.selection.active = false;
                    } else {
                        self.input.clear();
                        self.cursor = 0;
                        self.completion.clear();
                    }
                    return None;
                }
                k if k == b'v' as i32 => {
                    if let Some(clip) = clipboard_get() {
                        self.insert_str(&clip);
                    }
                    return None;
                }
                K_LEFTARROW | K_KP_LEFTARROW => {
                    self.cursor = prev_word_boundary(&self.input, self.cursor);
                    self.refresh_completion();
                    return None;
                }
                K_RIGHTARROW | K_KP_RIGHTARROW => {
                    self.cursor = next_word_boundary(&self.input, self.cursor);
                    self.refresh_completion();
                    return None;
                }
                K_HOME | K_KP_HOME => {
                    self.backscroll = self.max_backscroll();
                    return None;
                }
                K_END | K_KP_END => {
                    self.backscroll = 0;
                    return None;
                }
                _ => {}
            }
        }

        if mods.shift && key == K_INS {
            if let Some(clip) = clipboard_get() {
                self.insert_str(&clip);
            }
            return None;
        }

        match key {
            K_ENTER | K_KP_ENTER => {
                self.completion.clear();
                self.selection.active = false;
                let line = std::mem::take(&mut self.input);
                self.cursor = 0;
                self.print_ansi(&format!("\x1b[1;33m]\x1b[0m{line}\n"));
                if !line.trim().is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                }
                self.hist_pos = self.history.len();
                self.backscroll = 0;
                return Some(line);
            }
            K_TAB => {
                if !self.completion.visible {
                    self.refresh_completion();
                }
                if self.completion.visible {
                    if self.tab_cycled {
                        self.completion.step(if mods.shift { -1 } else { 1 });
                    }
                    self.apply_selected_completion();
                    self.tab_cycled = true;
                }
            }
            K_BACKSPACE => {
                if self.cursor > 0 {
                    let prev = self.input[..self.cursor].char_indices().next_back().map_or(0, |(i, _)| i);
                    self.input.drain(prev..self.cursor);
                    self.cursor = prev;
                    self.tab_cycled = false;
                    self.refresh_completion();
                }
            }
            K_DEL | K_KP_DEL => {
                if self.cursor < self.input.len() {
                    let next = self.input[self.cursor..].chars().next().map_or(0, char::len_utf8);
                    self.input.drain(self.cursor..self.cursor + next);
                    self.tab_cycled = false;
                    self.refresh_completion();
                }
            }
            K_LEFTARROW | K_KP_LEFTARROW => {
                if self.cursor > 0 {
                    self.cursor = self.input[..self.cursor].char_indices().next_back().map_or(0, |(i, _)| i);
                    self.tab_cycled = false;
                    self.refresh_completion();
                }
            }
            K_RIGHTARROW | K_KP_RIGHTARROW => {
                if self.cursor < self.input.len() {
                    let step = self.input[self.cursor..].chars().next().map_or(1, char::len_utf8);
                    self.cursor = (self.cursor + step).min(self.input.len());
                    self.tab_cycled = false;
                    self.refresh_completion();
                } else if self.completion.visible {
                    // Right arrow at end of line accepts the ghost suggestion!
                    self.apply_selected_completion();
                    self.completion.clear();
                }
            }
            K_HOME | K_KP_HOME => {
                self.cursor = 0;
                self.tab_cycled = false;
                self.refresh_completion();
            }
            K_END | K_KP_END => {
                self.cursor = self.input.len();
                self.tab_cycled = false;
                self.refresh_completion();
            }
            K_UPARROW | K_KP_UPARROW => {
                if self.completion.visible && self.completion.items.len() > 1 && self.tab_cycled {
                    self.completion.step(-1);
                    self.apply_selected_completion();
                } else if self.hist_pos > 0 {
                    self.hist_pos -= 1;
                    self.input = self.history[self.hist_pos].clone();
                    self.cursor = self.input.len();
                    self.completion.clear();
                }
            }
            K_DOWNARROW | K_KP_DOWNARROW => {
                if self.completion.visible && self.completion.items.len() > 1 && self.tab_cycled {
                    self.completion.step(1);
                    self.apply_selected_completion();
                } else if self.hist_pos < self.history.len() {
                    self.hist_pos += 1;
                    self.input = self.history.get(self.hist_pos).cloned().unwrap_or_default();
                    self.cursor = self.input.len();
                    self.completion.clear();
                }
            }
            K_PGUP | K_KP_PGUP | K_MWHEELUP => {
                let step = if key == K_MWHEELUP { 3 } else { 8 };
                self.backscroll = (self.backscroll + step).min(self.max_backscroll());
            }
            K_PGDN | K_KP_PGDN | K_MWHEELDOWN => {
                let step = if key == K_MWHEELDOWN { 3 } else { 8 };
                self.backscroll = self.backscroll.saturating_sub(step);
            }
            _ => {}
        }
        None
    }

    pub fn text_modern(&mut self, ch: char) {
        if ch.is_control() || ch == '`' || ch == '~' {
            return;
        }
        if let Some(pty) = &mut self.pty {
            let mut b = [0u8; 4];
            let s = ch.encode_utf8(&mut b);
            pty.write_bytes(s.as_bytes());
            self.backscroll = 0;
            return;
        }
        if self.search.active {
            self.search.query.push(ch);
            self.update_reverse_search(false);
            return;
        }
        self.input.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.tab_cycled = false;
        self.refresh_completion();
    }

    pub fn insert_str(&mut self, text: &str) {
        let clean: String = text.chars().filter(|&c| c >= ' ' && c != '`' && c != '~').collect();
        if clean.is_empty() {
            return;
        }
        self.input.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
        self.tab_cycled = false;
        self.refresh_completion();
    }

    pub fn refresh_completion(&mut self) {
        self.completion = complete::compute(&self.input, self.cursor, &self.catalog);
    }

    fn apply_selected_completion(&mut self) {
        let Some(cand) = self.completion.current().cloned() else { return };
        let start = self.completion.replace_start.min(self.input.len());
        let end = self.completion.replace_end.min(self.input.len()).max(start);
        let suffix = if self.completion.items.len() == 1 { " " } else { "" };
        let repl = format!("{}{suffix}", cand.text);
        self.input.replace_range(start..end, &repl);
        self.cursor = start + repl.len();
        self.completion.replace_end = self.cursor;
    }

    fn handle_search_key(&mut self, key: i32, mods: Modifiers) -> Option<String> {
        if mods.ctrl && key == b'r' as i32 {
            self.update_reverse_search(true);
            return None;
        }
        if (mods.ctrl && (key == b'g' as i32 || key == b'c' as i32)) || key == K_ESCAPE {
            self.search.active = false;
            return None;
        }
        match key {
            K_BACKSPACE => {
                self.search.query.pop();
                self.update_reverse_search(false);
            }
            K_ENTER | K_KP_ENTER => {
                if let Some(idx) = self.search.match_idx
                    && let Some(found) = self.history.get(idx).cloned()
                {
                    self.input = found;
                    self.cursor = self.input.len();
                }
                self.search.active = false;
                let line = std::mem::take(&mut self.input);
                self.cursor = 0;
                self.print_ansi(&format!("\x1b[1;33m]\x1b[0m{line}\n"));
                if !line.trim().is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                }
                self.hist_pos = self.history.len();
                return Some(line);
            }
            K_RIGHTARROW | K_LEFTARROW | K_HOME | K_END | K_TAB => {
                if let Some(idx) = self.search.match_idx
                    && let Some(found) = self.history.get(idx).cloned()
                {
                    self.input = found;
                    self.cursor = self.input.len();
                }
                self.search.active = false;
                self.refresh_completion();
            }
            _ => {}
        }
        None
    }

    fn update_reverse_search(&mut self, step_older: bool) {
        if self.search.query.is_empty() {
            self.search.match_idx = None;
            return;
        }
        let q = self.search.query.to_ascii_lowercase();
        let end = if step_older {
            self.search.match_idx.unwrap_or(self.history.len())
        } else {
            self.search.match_idx.map_or(self.history.len(), |i| (i + 1).min(self.history.len()))
        };
        self.search.match_idx = (0..end)
            .rev()
            .find(|&i| self.history[i].to_ascii_lowercase().contains(&q));
    }

    fn delete_word_back(&mut self) {
        let target = prev_word_boundary(&self.input, self.cursor);
        if target < self.cursor {
            self.input.drain(target..self.cursor);
            self.cursor = target;
            self.tab_cycled = false;
        }
    }

    fn delete_word_forward(&mut self) {
        let target = next_word_boundary(&self.input, self.cursor);
        if target > self.cursor {
            self.input.drain(self.cursor..target);
            self.tab_cycled = false;
        }
    }

    pub fn clear_scrollback(&mut self) {
        self.lines.clear();
        self.modern_lines.clear();
        self.cur.clear();
        self.modern_cur.clear();
        self.backscroll = 0;
    }

    pub fn max_backscroll(&self) -> usize {
        if let Some(pty) = &self.pty {
            pty.vt.scrollback.len()
        } else {
            self.modern_lines.len().saturating_sub(1)
        }
    }
}

pub fn prev_word_boundary(s: &str, cursor: usize) -> usize {
    let b = s.as_bytes();
    let mut i = cursor.min(b.len());
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    while i > 0 && !b[i - 1].is_ascii_whitespace() && b[i - 1] != b'/' && b[i - 1] != b'_' {
        i -= 1;
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn next_word_boundary(s: &str, cursor: usize) -> usize {
    let b = s.as_bytes();
    let mut i = cursor.min(b.len());
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'/' && b[i] != b'_' {
        i += 1;
    }
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    while i < b.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}
