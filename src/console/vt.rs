//! Interactive VT100/ANSI screen state (`VtGrid`) for PTY (`bash`) sessions.

use std::collections::VecDeque;

use super::ansi::{CellStyle, StyledCell, parse_csi_params};

const SCROLLBACK: usize = 2048;
/// Longest escape sequence waited for.
const MAX_ESCAPE: usize = 4096;

pub struct VtGrid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Vec<StyledCell>>,
    alt_cells: Option<Vec<Vec<StyledCell>>>,
    alt_saved_cursor: (usize, usize),
    pub scrollback: VecDeque<Vec<StyledCell>>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    saved_cursor: (usize, usize),
    pub cursor_visible: bool,
    pub app_cursor_keys: bool,
    pub bracketed_paste: bool,
    pub style: CellStyle,
    scroll_top: usize,
    scroll_bot: usize,
    pub title: String,
    partial_esc: Vec<u8>,
}

impl VtGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(20);
        let rows = rows.max(5);
        Self {
            cols,
            rows,
            cells: vec![vec![StyledCell::default(); cols]; rows],
            alt_cells: None,
            alt_saved_cursor: (0, 0),
            scrollback: VecDeque::new(),
            cursor_row: 0,
            cursor_col: 0,
            saved_cursor: (0, 0),
            cursor_visible: true,
            app_cursor_keys: false,
            bracketed_paste: false,
            style: CellStyle::default(),
            scroll_top: 0,
            scroll_bot: rows - 1,
            title: String::new(),
            partial_esc: Vec::new(),
        }
    }

    pub fn in_alt_screen(&self) -> bool {
        self.alt_cells.is_some()
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(20);
        let rows = rows.max(5);
        if cols == self.cols && rows == self.rows {
            return;
        }
        // the rows that don't fit any more go off the top, so that the cursor's
        // row (the prompt) stays
        let gone = (self.cursor_row + 1).saturating_sub(rows);
        let off: Vec<_> = self.cells.drain(..gone).collect();
        self.cursor_row -= gone;
        self.saved_cursor.0 = self.saved_cursor.0.saturating_sub(gone);
        fit(&mut self.cells, cols, rows);
        match &mut self.alt_cells {
            None => self.push_scrollback(off),
            // the screen under a full-screen program
            Some(main) => {
                let gone = (self.alt_saved_cursor.0 + 1).saturating_sub(rows);
                let off: Vec<_> = main.drain(..gone).collect();
                self.alt_saved_cursor.0 -= gone;
                fit(main, cols, rows);
                self.push_scrollback(off);
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.scroll_top = 0;
        self.scroll_bot = rows - 1;
        for c in [&mut self.saved_cursor, &mut self.alt_saved_cursor] {
            *c = (c.0.min(rows - 1), c.1.min(cols - 1));
        }
        self.move_to((self.cursor_row, self.cursor_col));
    }

    fn push_scrollback(&mut self, rows: Vec<Vec<StyledCell>>) {
        self.scrollback.extend(rows);
        while self.scrollback.len() > SCROLLBACK {
            self.scrollback.pop_front();
        }
    }

    fn move_to(&mut self, (row, col): (usize, usize)) {
        self.cursor_row = row.min(self.rows - 1);
        self.cursor_col = col.min(self.cols - 1);
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut buf = std::mem::take(&mut self.partial_esc);
        buf.extend_from_slice(bytes);
        let tail = partial_char(&buf);
        let s = String::from_utf8_lossy(&buf[..buf.len() - tail]);
        self.partial_esc = buf[buf.len() - tail..].to_vec();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                if !is_complete_escape(chars.as_str()) {
                    // one that never ends isn't kept for ever
                    if chars.as_str().len() < MAX_ESCAPE {
                        let mut rest = format!("\x1b{}", chars.as_str()).into_bytes();
                        rest.append(&mut self.partial_esc);
                        self.partial_esc = rest;
                    }
                    return;
                }
                match chars.next() {
                    Some('[') => {
                        let mut seq = String::new();
                        let mut final_ch = '\0';
                        for ch in chars.by_ref() {
                            if ('@'..='~').contains(&ch) {
                                final_ch = ch;
                                break;
                            }
                            seq.push(ch);
                        }
                        self.handle_csi(&seq, final_ch);
                    }
                    Some(']') => {
                        let mut osc = String::new();
                        while let Some(ch) = chars.next() {
                            if ch == '\x07' {
                                break;
                            }
                            if ch == '\x1b' && chars.as_str().starts_with('\\') {
                                chars.next();
                                break;
                            }
                            osc.push(ch);
                        }
                        if let Some(t) = osc.strip_prefix("0;").or_else(|| osc.strip_prefix("2;")) {
                            self.title = t.to_string();
                        }
                    }
                    Some('7') => self.saved_cursor = (self.cursor_row, self.cursor_col),
                    Some('8') => self.move_to(self.saved_cursor),
                    Some('M') => self.reverse_index(),
                    Some('c') => *self = Self::new(self.cols, self.rows),
                    _ => {}
                }
                continue;
            }
            match c {
                '\r' => self.cursor_col = 0,
                '\n' | '\x0b' | '\x0c' => self.linefeed(),
                '\x08' => self.cursor_col = self.cursor_col.saturating_sub(1),
                '\x07' => {}
                '\t' => {
                    let next = ((self.cursor_col / 8) + 1) * 8;
                    self.cursor_col = next.min(self.cols - 1);
                }
                ch if ch >= ' ' => self.put_char(ch),
                _ => {}
            }
        }
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.linefeed();
        }
        self.cells[self.cursor_row][self.cursor_col] = StyledCell { ch, style: self.style };
        self.cursor_col += 1;
    }

    fn linefeed(&mut self) {
        if self.cursor_row >= self.scroll_bot {
            self.scroll_up(1);
        } else {
            self.cursor_row = (self.cursor_row + 1).min(self.rows - 1);
        }
    }

    fn reverse_index(&mut self) {
        if self.cursor_row <= self.scroll_top {
            self.scroll_down(1);
        } else {
            self.cursor_row = self.cursor_row.saturating_sub(1);
        }
    }

    fn scroll_up(&mut self, count: usize) {
        let top = self.scroll_top.min(self.rows - 1);
        let bot = self.scroll_bot.min(self.rows - 1).max(top);
        for _ in 0..count {
            let removed = self.cells.remove(top);
            if top == 0 && self.alt_cells.is_none() {
                self.push_scrollback(vec![removed]);
            }
            self.cells.insert(bot, vec![StyledCell::default(); self.cols]);
        }
    }

    fn scroll_down(&mut self, count: usize) {
        let top = self.scroll_top.min(self.rows - 1);
        let bot = self.scroll_bot.min(self.rows - 1).max(top);
        for _ in 0..count {
            self.cells.remove(bot);
            self.cells.insert(top, vec![StyledCell::default(); self.cols]);
        }
    }

    fn handle_csi(&mut self, seq: &str, final_ch: char) {
        let private = seq.starts_with('?');
        let params = parse_csi_params(seq);
        let p0 = params.first().copied().unwrap_or(0) as usize;
        let p1 = params.get(1).copied().unwrap_or(0) as usize;
        let n1 = p0.max(1);
        // lines inserted, deleted or scrolled: more than the screen has is all
        let lines = n1.min(self.rows);
        match final_ch {
            'm' => self.style.apply_sgr(&params),
            'A' => self.cursor_row = self.cursor_row.saturating_sub(n1),
            'B' => self.cursor_row = (self.cursor_row + n1).min(self.rows - 1),
            'C' => self.cursor_col = (self.cursor_col + n1).min(self.cols - 1),
            'D' => self.cursor_col = self.cursor_col.saturating_sub(n1),
            'E' => {
                self.cursor_row = (self.cursor_row + n1).min(self.rows - 1);
                self.cursor_col = 0;
            }
            'F' => {
                self.cursor_row = self.cursor_row.saturating_sub(n1);
                self.cursor_col = 0;
            }
            'G' | '`' => self.cursor_col = n1.saturating_sub(1).min(self.cols - 1),
            'd' => self.cursor_row = n1.saturating_sub(1).min(self.rows - 1),
            'H' | 'f' => {
                let r = p0.max(1).saturating_sub(1).min(self.rows - 1);
                let c = p1.max(1).saturating_sub(1).min(self.cols - 1);
                self.cursor_row = r;
                self.cursor_col = c;
            }
            'J' => match p0 {
                0 => {
                    for c in self.cursor_col.min(self.cols)..self.cols {
                        self.cells[self.cursor_row][c] = StyledCell::default();
                    }
                    for r in (self.cursor_row + 1)..self.rows {
                        self.cells[r].fill(StyledCell::default());
                    }
                }
                1 => {
                    for r in 0..self.cursor_row {
                        self.cells[r].fill(StyledCell::default());
                    }
                    for c in 0..=self.cursor_col.min(self.cols - 1) {
                        self.cells[self.cursor_row][c] = StyledCell::default();
                    }
                }
                2 | 3 => {
                    for r in &mut self.cells {
                        r.fill(StyledCell::default());
                    }
                    if p0 == 3 {
                        self.scrollback.clear();
                    }
                }
                _ => {}
            },
            'K' => {
                let row = &mut self.cells[self.cursor_row];
                match p0 {
                    0 => {
                        for cell in row.iter_mut().skip(self.cursor_col.min(self.cols)) {
                            *cell = StyledCell::default();
                        }
                    }
                    1 => {
                        for cell in row.iter_mut().take(self.cursor_col.min(self.cols - 1) + 1) {
                            *cell = StyledCell::default();
                        }
                    }
                    2 => row.fill(StyledCell::default()),
                    _ => {}
                }
            }
            'L' => {
                for _ in 0..lines {
                    if self.cursor_row <= self.scroll_bot {
                        self.cells.remove(self.scroll_bot);
                        self.cells.insert(self.cursor_row, vec![StyledCell::default(); self.cols]);
                    }
                }
            }
            'M' => {
                for _ in 0..lines {
                    if self.cursor_row <= self.scroll_bot {
                        self.cells.remove(self.cursor_row);
                        self.cells.insert(self.scroll_bot, vec![StyledCell::default(); self.cols]);
                    }
                }
            }
            'P' => {
                let row = &mut self.cells[self.cursor_row];
                let col = self.cursor_col.min(self.cols);
                let del = n1.min(self.cols - col);
                row.drain(col..col + del);
                row.resize(self.cols, StyledCell::default());
            }
            '@' => {
                let row = &mut self.cells[self.cursor_row];
                let col = self.cursor_col.min(self.cols);
                let ins = n1.min(self.cols - col);
                for _ in 0..ins {
                    row.insert(col, StyledCell::default());
                }
                row.truncate(self.cols);
            }
            'X' => {
                let row = &mut self.cells[self.cursor_row];
                let col = self.cursor_col.min(self.cols);
                for cell in row.iter_mut().skip(col).take(n1) {
                    *cell = StyledCell::default();
                }
            }
            'S' => self.scroll_up(lines),
            'T' => self.scroll_down(lines),
            'r' => {
                let top = p0.max(1).saturating_sub(1);
                let bot = if p1 == 0 { self.rows - 1 } else { p1.saturating_sub(1).min(self.rows - 1) };
                if top < bot {
                    self.scroll_top = top;
                    self.scroll_bot = bot;
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                }
            }
            's' => self.saved_cursor = (self.cursor_row, self.cursor_col),
            'u' => self.move_to(self.saved_cursor),
            'h' | 'l' if private => {
                let set = final_ch == 'h';
                for &p in &params {
                    match p {
                        1 => self.app_cursor_keys = set,
                        25 => self.cursor_visible = set,
                        47 | 1047 | 1049 => {
                            if set && self.alt_cells.is_none() {
                                self.alt_saved_cursor = (self.cursor_row, self.cursor_col);
                                let old = std::mem::replace(
                                    &mut self.cells,
                                    vec![vec![StyledCell::default(); self.cols]; self.rows],
                                );
                                self.alt_cells = Some(old);
                                self.cursor_row = 0;
                                self.cursor_col = 0;
                            } else if !set && let Some(old) = self.alt_cells.take() {
                                self.cells = old;
                                self.resize(self.cols, self.rows);
                                self.move_to(self.alt_saved_cursor);
                            }
                        }
                        2004 => self.bracketed_paste = set,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether the escape sequence whose ESC came before `s` ends in it.
fn is_complete_escape(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        None => false,
        Some('[') => chars.any(|c| ('@'..='~').contains(&c)),
        Some(']') => {
            let rest = chars.as_str();
            rest.contains('\x07') || rest.contains("\x1b\\")
        }
        Some(_) => true,
    }
}

/// Bytes of a character cut off at the end of `buf` (0 to 3).
pub(super) fn partial_char(buf: &[u8]) -> usize {
    let mut at = buf.len().saturating_sub(3);
    loop {
        match std::str::from_utf8(&buf[at..]) {
            Ok(_) => return 0,
            Err(e) if e.error_len().is_none() => return buf.len() - at - e.valid_up_to(),
            Err(e) => at += e.valid_up_to() + e.error_len().unwrap_or(1),
        }
    }
}

fn fit(cells: &mut Vec<Vec<StyledCell>>, cols: usize, rows: usize) {
    for row in cells.iter_mut() {
        row.resize(cols, StyledCell::default());
    }
    cells.resize_with(rows, || vec![StyledCell::default(); cols]);
}
