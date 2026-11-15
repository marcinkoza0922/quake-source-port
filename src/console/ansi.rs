//! ANSI SGR escape sequence parser and styled scrollback cells.

use std::collections::VecDeque;

pub const DEFAULT_FG: [f32; 4] = [0.90, 0.91, 0.93, 1.0];
pub const BRONZE_FG: [f32; 4] = [0.92, 0.72, 0.36, 1.0];

const ANSI_16: [[f32; 3]; 16] = [
    [0.15, 0.16, 0.18], // 0 black
    [0.88, 0.32, 0.32], // 1 red
    [0.35, 0.80, 0.45], // 2 green
    [0.90, 0.76, 0.32], // 3 yellow
    [0.36, 0.62, 0.95], // 4 blue
    [0.78, 0.46, 0.90], // 5 magenta
    [0.34, 0.82, 0.86], // 6 cyan
    [0.85, 0.86, 0.88], // 7 white
    [0.45, 0.47, 0.52], // 8 bright black
    [0.98, 0.46, 0.46], // 9 bright red
    [0.50, 0.92, 0.58], // 10 bright green
    [0.98, 0.88, 0.46], // 11 bright yellow
    [0.54, 0.76, 0.99], // 12 bright blue
    [0.90, 0.60, 0.98], // 13 bright magenta
    [0.52, 0.94, 0.96], // 14 bright cyan
    [0.98, 0.99, 1.00], // 15 bright white
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellStyle {
    pub fg: [f32; 4],
    pub bg: Option<[f32; 4]>,
    pub bold: bool,
    pub dim: bool,
    pub inverse: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self { fg: DEFAULT_FG, bg: None, bold: false, dim: false, inverse: false }
    }
}

impl CellStyle {
    pub fn effective_fg(&self) -> [f32; 4] {
        let base = if self.inverse {
            self.bg.unwrap_or([0.08, 0.09, 0.11, 1.0])
        } else {
            self.fg
        };
        if self.dim {
            [base[0] * 0.65, base[1] * 0.65, base[2] * 0.65, base[3]]
        } else if self.bold {
            [(base[0] * 1.15).min(1.0), (base[1] * 1.15).min(1.0), (base[2] * 1.15).min(1.0), base[3]]
        } else {
            base
        }
    }

    pub fn effective_bg(&self) -> Option<[f32; 4]> {
        if self.inverse { Some(self.fg) } else { self.bg }
    }

    pub fn apply_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            *self = Self::default();
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => *self = Self::default(),
                1 => {
                    self.bold = true;
                    self.dim = false;
                }
                2 => {
                    self.dim = true;
                    self.bold = false;
                }
                7 => self.inverse = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                27 => self.inverse = false,
                30..=37 => self.fg = color_16((params[i] - 30) as usize + if self.bold { 8 } else { 0 }),
                39 => self.fg = DEFAULT_FG,
                40..=47 => self.bg = Some(color_16((params[i] - 40) as usize)),
                49 => self.bg = None,
                90..=97 => self.fg = color_16((params[i] - 90 + 8) as usize),
                100..=107 => self.bg = Some(color_16((params[i] - 100 + 8) as usize)),
                38 | 48 => {
                    let is_fg = params[i] == 38;
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        let c = color_256(params[i + 2] as u8);
                        if is_fg { self.fg = c; } else { self.bg = Some(c); }
                        i += 2;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        let c = [
                            params[i + 2].min(255) as f32 / 255.0,
                            params[i + 3].min(255) as f32 / 255.0,
                            params[i + 4].min(255) as f32 / 255.0,
                            1.0,
                        ];
                        if is_fg { self.fg = c; } else { self.bg = Some(c); }
                        i += 4;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

fn color_16(idx: usize) -> [f32; 4] {
    let [r, g, b] = ANSI_16[idx & 15];
    [r, g, b, 1.0]
}

fn color_256(n: u8) -> [f32; 4] {
    match n {
        0..=15 => color_16(n as usize),
        16..=231 => {
            let idx = n - 16;
            let (r, g, b) = (idx / 36, (idx / 6) % 6, idx % 6);
            let step = |v: u8| if v == 0 { 0.0 } else { (v as f32 * 40.0 + 55.0) / 255.0 };
            [step(r), step(g), step(b), 1.0]
        }
        232..=255 => {
            let v = ((n - 232) as f32 * 10.0 + 8.0) / 255.0;
            [v, v, v, 1.0]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyledCell {
    pub ch: char,
    pub style: CellStyle,
}

impl Default for StyledCell {
    fn default() -> Self {
        Self { ch: ' ', style: CellStyle::default() }
    }
}

/// Strip ANSI escape sequences from `text` so classic console / notify lines stay clean.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for ch in chars.by_ref() {
                        if ('@'..='~').contains(&ch) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '\x07' {
                            break;
                        }
                        if ch == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse a chunk of text (with optional Quake bronze `\x01`/`\x02` or ANSI SGR)
/// into wrapped [`StyledCell`] lines.
pub fn append_styled(
    text: &str,
    cur: &mut Vec<StyledCell>,
    lines: &mut VecDeque<Vec<StyledCell>>,
    style: &mut CellStyle,
    width: usize,
    max_lines: usize,
) {
    let width = width.max(10);
    let mut s = text;
    let mut line_bronze = false;
    if let Some(first) = s.as_bytes().first()
        && (*first == 1 || *first == 2)
    {
        line_bronze = true;
        style.fg = BRONZE_FG;
        s = &s[1..];
    }
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    let mut seq = String::new();
                    let mut final_ch = '\0';
                    for ch in chars.by_ref() {
                        if ('@'..='~').contains(&ch) {
                            final_ch = ch;
                            break;
                        }
                        seq.push(ch);
                    }
                    if final_ch == 'm' {
                        let params = parse_csi_params(&seq);
                        style.apply_sgr(&params);
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '\x07' {
                            break;
                        }
                        if ch == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }
        match c {
            '\n' => {
                lines.push_back(std::mem::take(cur));
                while lines.len() > max_lines {
                    lines.pop_front();
                }
                if line_bronze {
                    style.fg = DEFAULT_FG;
                    line_bronze = false;
                }
            }
            '\r' => cur.clear(),
            '\x08' => {
                cur.pop();
            }
            '\t' => {
                let spaces = 4 - (cur.len() % 4);
                for _ in 0..spaces {
                    cur.push(StyledCell { ch: ' ', style: *style });
                    if cur.len() >= width {
                        lines.push_back(std::mem::take(cur));
                        while lines.len() > max_lines {
                            lines.pop_front();
                        }
                    }
                }
            }
            _ => {
                let (ch, cell_style) = decode_quake_char(c, *style);
                if ch < ' ' {
                    continue;
                }
                cur.push(StyledCell { ch, style: cell_style });
                if cur.len() >= width {
                    lines.push_back(std::mem::take(cur));
                    while lines.len() > max_lines {
                        lines.pop_front();
                    }
                }
            }
        }
    }
    if line_bronze {
        style.fg = DEFAULT_FG;
    }
}

fn decode_quake_char(c: char, mut style: CellStyle) -> (char, CellStyle) {
    let u = c as u32;
    if (128..256).contains(&u) {
        let b = (u & 127) as u8;
        style.fg = BRONZE_FG;
        let mapped = match b {
            16 => '[',
            17 => ']',
            18..=27 => (b'0' + (b - 18)) as char,
            29..=31 => '-',
            b if (32..127).contains(&b) => b as char,
            _ => '·',
        };
        (mapped, style)
    } else if u < 32 {
        let mapped = match u as u8 {
            16 => '[',
            17 => ']',
            18..=27 => (b'0' + (u as u8 - 18)) as char,
            29..=31 => '-',
            _ => '\0',
        };
        (mapped, style)
    } else {
        (c, style)
    }
}

pub fn parse_csi_params(seq: &str) -> Vec<u16> {
    let s = seq.trim_start_matches(['?', '>', '!']);
    if s.is_empty() {
        return Vec::new();
    }
    s.split(';').map(|p| p.parse::<u16>().unwrap_or(0)).collect()
}
