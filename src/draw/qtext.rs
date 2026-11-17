//! 8×8 Quake text (console, notify, centerprint, HUD labels).
//!
//! In English this is the classic conchars, one 8-unit cell per byte. When
//! the language isn't English ([`qcore::lang::unicode_text`]) text can carry
//! UTF-8 (translated re-release and engine text), which conchars can't draw,
//! so printable text switches to the TTF menu font: proportional for free
//! text ([`Canvas::qtext`]) and the monospace font on the 8-unit grid for
//! the classic console ([`Canvas::qtext_grid`]). Gold (+128) letters keep a
//! gold tint; symbols below 32 still come from conchars.

use qcore::text::{Glyph, glyphs_utf8};

use super::*;

/// Em size of TTF text standing in for an 8-unit conchars line.
pub const QTEXT_SIZE: f32 = 9.0;
/// Em size of the monospace font in an 8-unit console cell.
const GRID_SIZE: f32 = 10.0;
/// Tint for the gold half of conchars drawn with the TTF font.
const GOLD: [f32; 3] = [0.92, 0.7, 0.4];

enum Piece {
    /// Printable text and whether it is the gold set.
    Run(String, bool),
    /// A conchars-only symbol.
    Special(u8),
}

fn pieces(bytes: &[u8]) -> Vec<Piece> {
    let mut out = Vec::new();
    for g in glyphs_utf8(bytes, true) {
        let (c, gold) = match g {
            Glyph::Char(c) => (c, false),
            Glyph::Quake(b) if (32..127).contains(&(b & 127)) => ((b & 127) as char, b >= 128),
            Glyph::Quake(b) => {
                out.push(Piece::Special(b));
                continue;
            }
        };
        match out.last_mut() {
            Some(Piece::Run(s, g)) if *g == gold => s.push(c),
            _ => out.push(Piece::Run(c.to_string(), gold)),
        }
    }
    out
}

fn tint(c: [f32; 4], gold: bool) -> [f32; 4] {
    if gold { [c[0] * GOLD[0], c[1] * GOLD[1], c[2] * GOLD[2], c[3]] } else { c }
}

impl Canvas {
    /// Whether 8×8 text is drawn with the TTF font now.
    pub fn ttf_quake_text(&self) -> bool {
        qcore::lang::unicode_text() && self.ui.as_ref().is_some_and(|u| u.borrow().pic.is_some())
    }

    /// Width in virtual units of Quake text `bytes`.
    pub fn qtext_w(&self, bytes: &[u8]) -> f32 {
        if !self.ttf_quake_text() {
            return bytes.len() as f32 * 8.0;
        }
        pieces(bytes)
            .iter()
            .map(|p| match p {
                Piece::Run(s, _) => self.text_w_raw(s, QTEXT_SIZE),
                Piece::Special(_) => 8.0,
            })
            .sum()
    }

    /// Draws Quake text with its 8-unit line top at (x, y); returns the width.
    pub fn qtext(&mut self, x: f32, y: f32, bytes: &[u8], color: [f32; 4]) -> f32 {
        if !self.ttf_quake_text() {
            for (i, &b) in bytes.iter().enumerate() {
                self.ch_color(x + i as f32 * 8.0, y, b, color);
            }
            return bytes.len() as f32 * 8.0;
        }
        let mut cx = x;
        for p in pieces(bytes) {
            match p {
                Piece::Run(s, gold) => cx += self.text_raw(cx, y - 0.5, &s, QTEXT_SIZE, tint(color, gold)),
                Piece::Special(b) => {
                    self.ch_color(cx, y, b, color);
                    cx += 8.0;
                }
            }
        }
        cx - x
    }

    /// The port's own 8×8 text: translated ([`qcore::lang::tr`]), then
    /// drawn as Quake text (UTF-8 for non-English). Returns the width.
    pub fn ptext(&mut self, x: f32, y: f32, s: &str, color: [f32; 4]) -> f32 {
        let t = qcore::lang::tr(s);
        self.qtext(x, y, t.as_bytes(), color)
    }

    /// Width of [`Self::ptext`].
    pub fn ptext_w(&self, s: &str) -> f32 {
        self.qtext_w(qcore::lang::tr(s).as_bytes())
    }

    /// [`Self::ptext`] in the gold half of conchars (a gold tint in TTF).
    pub fn ptext_gold(&mut self, x: f32, y: f32, s: &str, color: [f32; 4]) -> f32 {
        let t = qcore::lang::tr(s);
        if self.ttf_quake_text() {
            return self.qtext(x, y, t.as_bytes(), tint(color, true));
        }
        let b: Vec<u8> = t.bytes().map(|b| if b == b' ' { b } else { b | 128 }).collect();
        self.qtext(x, y, &b, color)
    }

    /// Quake text on the 8-unit grid (classic console): conchars for each
    /// byte, decoded UTF-8 characters from the monospace font in the cell.
    /// A line with letters that need shaping or bidi (Arabic, Hebrew,
    /// Indic, Thai, Khmer) can't be drawn a cell at a time, so it is drawn
    /// as proportional [`Self::qtext`] instead.
    pub fn qtext_grid(&mut self, x: f32, y: f32, bytes: &[u8], color: [f32; 4]) {
        let ttf = self.ttf_quake_text();
        if ttf && glyphs_utf8(bytes, true).any(|g| matches!(g, Glyph::Char(c) if crate::font::complex(c))) {
            self.qtext(x, y, bytes, color);
            return;
        }
        for (i, g) in glyphs_utf8(bytes, ttf).enumerate() {
            let cx = x + i as f32 * 8.0;
            match g {
                Glyph::Quake(b) => self.ch_color(cx, y, b, color),
                Glyph::Char(c) => self.mono_char(cx, y, c, color),
            }
        }
    }

    /// One monospace TTF character centred in the 8×8 cell at (x, y).
    fn mono_char(&mut self, x: f32, y: f32, ch: char, color: [f32; 4]) {
        let Some(ui) = self.ui.clone() else { return };
        let mut ui = ui.borrow_mut();
        let Some(pic) = ui.pic else { return };
        let px = self.em_px(GRID_SIZE);
        let (x0, y0) = self.px(x, y);
        let adv = ui.mono_advance(px as u16);
        let left = (x0 + (8.0 * self.scale - adv) / 2.0).round();
        let base = (y0 + 7.0 * self.scale).round();
        let a = crate::font::UiText::atlas_size() as f32;
        if let Some(g) = ui.mono_glyph(ch, px as u16) {
            let uv = [g.x as f32 / a, g.y as f32 / a, (g.x + g.w) as f32 / a, (g.y + g.h) as f32 / a];
            let (gx, gy) = (left + g.ox as f32, base + g.oy as f32);
            self.push(Draw2D::Pic { x: gx, y: gy, w: g.w as f32, h: g.h as f32, pic, uv, color });
        }
    }
}

/// The first `n` glyphs of Quake text (a finale's typed-out reveal).
pub fn glyph_prefix(bytes: &[u8], n: usize) -> &[u8] {
    let utf8 = qcore::lang::unicode_text();
    let mut end = 0;
    for (i, g) in glyphs_utf8(bytes, utf8).enumerate() {
        if i == n {
            break;
        }
        end += match g {
            Glyph::Quake(_) => 1,
            Glyph::Char(c) => c.len_utf8(),
        };
    }
    &bytes[..end]
}

/// Number of glyphs in Quake text.
pub fn glyph_count(bytes: &[u8]) -> usize {
    glyphs_utf8(bytes, qcore::lang::unicode_text()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_split_gold_and_symbols() {
        let mut b = b"ab".to_vec();
        b.push(b'c' | 128);
        b.push(16);
        b.extend_from_slice("é".as_bytes());
        let p = pieces(&b);
        let desc: Vec<String> = p
            .iter()
            .map(|p| match p {
                Piece::Run(s, g) => format!("{s}{}", if *g { "*" } else { "" }),
                Piece::Special(b) => format!("#{b}"),
            })
            .collect();
        assert_eq!(desc, ["ab", "c*", "#16", "é"]);
    }
}
