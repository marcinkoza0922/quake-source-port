//! Line layout: kerned per-character pen positions for simple text; for
//! text with Arabic, Hebrew, Indic, Thai or Khmer letters, the Unicode bidi
//! algorithm (`unicode-bidi`) orders it into runs, each run is split by the
//! font that draws it, and runs in a shaping font ([`Fallbacks::face`]) go
//! through `rustybuzz` (joining forms, conjuncts, reordered vowel signs,
//! stacked marks). Shaped results are cached per string.

use std::ops::Range;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use unicode_bidi::{BidiInfo, Level};

use super::UiText;
use super::fallback::{self, Src};

/// What to draw: a character (looked up in the font chain as before) or a
/// glyph id of a shaping font ([`Fallbacks`] script index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum G {
    Char(char),
    Shaped { font: u8, gid: u16 },
}

/// A glyph on the line: pen x and baseline offset (down) in px. `Char` pens
/// are whole px; shaped ones keep fractions (drawn at quarter-pixel steps,
/// so joined Arabic letters don't open hairline gaps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub g: G,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub glyphs: Vec<Placed>,
    /// Advance width in px.
    pub width: f32,
}

/// Format characters that draw nothing: soft hyphen, zero-width space and
/// joiners, bidi marks, embeddings and isolates, word joiner, BOM.
pub fn invisible(c: char) -> bool {
    matches!(c as u32, 0xAD | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x2069 | 0xFEFF)
}

/// Letters of right-to-left scripts (Hebrew, Arabic and their forms).
fn rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

/// Characters that need bidi or shaping: right-to-left letters, Indic
/// (Devanagari to Sinhala), Thai, Khmer, joiners and bidi controls.
pub fn complex(c: char) -> bool {
    rtl_letter(c)
        || matches!(c as u32, 0x0900..=0x0DFF | 0x0E00..=0x0E7F | 0x1780..=0x17FF | 0x19E0..=0x19FF | 0xA8E0..=0xA8FF
            | 0x200C..=0x200F | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Shaped lines kept before the cache starts over.
const CACHE_MAX: usize = 2048;

/// Shape cache key: text, em px, language (bidi base direction, fonts).
pub type CacheKey = (String, u16, u8);

impl UiText {
    /// The glyphs of `s` at em `px`, left to right as drawn.
    pub fn layout(&self, s: &str, px: f32) -> Line {
        let first = self.menu_font();
        let cjk = fallback::cjk_order(qcore::lang::current());
        if !s.chars().any(complex) {
            let mut line = Line::default();
            self.push_chars(&mut line, first, s, px, cjk, false);
            return line;
        }
        let key = (s.to_string(), px as u16, qcore::lang::current() as u8);
        if let Some(l) = self.shaped.borrow().get(&key) {
            return l.clone();
        }
        let line = self.layout_complex(first, s, px, cjk);
        let mut cache = self.shaped.borrow_mut();
        if cache.len() >= CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, line.clone());
        line
    }

    /// The font for `ch`: `first` if it has it, else the first fallback.
    pub(super) fn pick(&self, first: &FontArc, ch: char, cjk: [u8; 4]) -> Src {
        if first.glyph_id(ch).0 != 0 {
            return Src::First;
        }
        self.fallback.find(ch, cjk).unwrap_or(Src::First)
    }

    /// Per-character layout with kerning inside `first`; `rtl` lays the
    /// characters out reversed with mirrored brackets.
    fn push_chars(&self, line: &mut Line, first: &FontArc, s: &str, px: f32, cjk: [u8; 4], rtl: bool) {
        let mut pen = line.width;
        let mut prev = None;
        let chars: Box<dyn Iterator<Item = char>> = if rtl {
            Box::new(s.chars().rev().map(|c| unicode_bidi_mirroring::get_mirrored(c).unwrap_or(c)))
        } else {
            Box::new(s.chars())
        };
        for ch in chars {
            if invisible(ch) {
                continue;
            }
            let src = self.pick(first, ch, cjk);
            let font = self.fallback.font(first, src);
            let sf = font.as_scaled(PxScale::from(px * self.fallback.em_scale(first, src)));
            let id = font.glyph_id(ch);
            let own = src == Src::First;
            if let Some(p) = prev.filter(|_| own) {
                pen += sf.kern(p, id);
            }
            let adv = sf.h_advance(id);
            line.glyphs.push(Placed { g: G::Char(ch), x: pen.round(), y: 0.0 });
            line.width = pen.round() + adv;
            pen += adv;
            prev = own.then_some(id);
        }
    }

    fn layout_complex(&self, first: &FontArc, s: &str, px: f32, cjk: [u8; 4]) -> Line {
        // an RTL language reads mixed text right to left even when it
        // starts with a Latin word
        let base = (qcore::lang::rtl() && s.chars().any(rtl_letter)).then(Level::rtl);
        let bidi = BidiInfo::new(s, base);
        let mut line = Line::default();
        for para in &bidi.paragraphs {
            let (levels, runs) = bidi.visual_runs(para, para.range.clone());
            for run in runs {
                let rtl = levels[run.start].is_rtl();
                let mut segs = self.segments(first, &s[run.clone()], cjk);
                if rtl {
                    segs.reverse();
                }
                for (r, src) in segs {
                    let text = &s[run.start + r.start..run.start + r.end];
                    match self.fallback.face(src) {
                        Some(face) => self.push_shaped(&mut line, first, face, src, text, px, rtl),
                        None => self.push_chars(&mut line, first, text, px, cjk, rtl),
                    }
                }
            }
        }
        line
    }

    /// `s` (one bidi run) split into pieces drawn by one font each. Marks,
    /// joiners, spaces and punctuation stay with the shaping font before
    /// them when it has them, so clusters and words are shaped whole.
    fn segments(&self, first: &FontArc, s: &str, cjk: [u8; 4]) -> Vec<(Range<usize>, Src)> {
        let mut out: Vec<(Range<usize>, Src)> = Vec::new();
        for (i, ch) in s.char_indices() {
            let end = i + ch.len_utf8();
            if let Some((r, src)) = out.last_mut() {
                let shaping = self.fallback.face(*src).is_some();
                if invisible(ch) || (shaping && !ch.is_alphanumeric() && self.fallback.font(first, *src).glyph_id(ch).0 != 0) {
                    r.end = end;
                    continue;
                }
            }
            let src = self.pick(first, ch, cjk);
            match out.last_mut() {
                Some((r, s0)) if *s0 == src => r.end = end,
                _ => out.push((i..end, src)),
            }
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn push_shaped(&self, line: &mut Line, first: &FontArc, face: &rustybuzz::Face<'static>, src: Src, text: &str, px: f32, rtl: bool) {
        let Src::Script(fi) = src else { return };
        let font = self.fallback.font(first, src);
        // font units -> px, as ab_glyph scales (to ascent - descent)
        let k = px * self.fallback.em_scale(first, src) / font.height_unscaled();
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);
        buf.set_direction(if rtl { rustybuzz::Direction::RightToLeft } else { rustybuzz::Direction::LeftToRight });
        if let Ok(l) = qcore::lang::current().code().parse::<rustybuzz::Language>() {
            buf.set_language(l);
        }
        buf.guess_segment_properties();
        let out = rustybuzz::shape(face, &[], buf);
        let mut pen = line.width;
        for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
            let g = G::Shaped { font: fi, gid: info.glyph_id as u16 };
            line.glyphs.push(Placed { g, x: pen + pos.x_offset as f32 * k, y: -(pos.y_offset as f32 * k).round() });
            pen += pos.x_advance as f32 * k;
        }
        line.width = pen;
    }

    /// The glyph id and font of a placed glyph, with the em scale factor.
    #[cfg(test)]
    pub fn placed_font(&self, g: G) -> (FontArc, ab_glyph::GlyphId, f32) {
        let first = self.menu_font();
        match g {
            G::Char(ch) => {
                let src = self.pick(first, ch, fallback::cjk_order(qcore::lang::current()));
                let f = self.fallback.font(first, src);
                (f.clone(), f.glyph_id(ch), self.fallback.em_scale(first, src))
            }
            G::Shaped { font, gid } => {
                let src = Src::Script(font);
                (self.fallback.font(first, src).clone(), ab_glyph::GlyphId(gid), self.fallback.em_scale(first, src))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcore::lang::Lang;

    fn gids(l: &Line) -> Vec<u16> {
        l.glyphs.iter().map(|p| if let G::Shaped { gid, .. } = p.g { gid } else { 0 }).collect()
    }

    fn cmap(t: &UiText, c: char) -> u16 {
        t.placed_font(G::Char(c)).1.0
    }

    #[test]
    fn hebrew_is_laid_out_right_to_left() {
        let t = UiText::default();
        let l = t.layout("שלום", 20.0);
        assert_eq!(gids(&l), ["ם", "ו", "ל", "ש"].map(|c| cmap(&t, c.chars().next().unwrap())));
        assert!(l.glyphs.windows(2).all(|w| w[0].x < w[1].x));
        assert!((l.width - t.measure("שלום", 20.0)).abs() < 0.01);
    }

    #[test]
    fn arabic_letters_join() {
        qcore::lang::set(Lang::Arabic);
        let t = UiText::default();
        let word = t.layout("سلام", 20.0);
        assert_eq!(word.glyphs.len(), 4);
        // visual order: the first letter (seen) is rightmost, in its
        // initial form rather than the isolated one
        let iso: Vec<u16> = "سلام".chars().rev().map(|c| cmap(&t, c)).collect();
        assert_ne!(gids(&word), iso);
        assert_eq!(gids(&t.layout("س", 20.0)), [cmap(&t, 'س')]);
        // a Latin word in Arabic text keeps reading right to left
        let mixed = t.layout("FPS عرض", 20.0);
        let first_arabic = mixed.glyphs.iter().position(|p| matches!(p.g, G::Shaped { .. })).unwrap();
        assert_eq!(first_arabic, 0, "{mixed:?}");
        let latin: String = mixed.glyphs.iter().filter_map(|p| if let G::Char(c) = p.g { Some(c) } else { None }).collect();
        assert_eq!(latin.trim(), "FPS");
    }

    #[test]
    fn brackets_mirror_in_right_to_left_runs() {
        qcore::lang::set(Lang::Hebrew);
        let t = UiText::default();
        let l = t.layout("שמור (F5)", 20.0);
        let chars: String = l.glyphs.iter().filter_map(|p| if let G::Char(c) = p.g { Some(c) } else { None }).collect();
        assert_eq!(chars.trim(), "(F5)", "{l:?}");
    }

    #[test]
    fn devanagari_vowel_sign_i_goes_before_its_consonant() {
        let t = UiText::default();
        let l = t.layout("कि", 20.0);
        let ka = cmap(&t, 'क');
        let g = gids(&l);
        assert_eq!(g.len(), 2, "{l:?}");
        assert_eq!(g[1], ka, "the matra is drawn left of ka: {g:?}");
        // conjunct: ka + virama + ssa forms one glyph
        assert!(t.layout("क्ष", 20.0).glyphs.len() < 3);
    }

    #[test]
    fn thai_marks_stack_over_their_letter() {
        let t = UiText::default();
        let base = t.layout("ท", 20.0).width;
        let l = t.layout("ที่", 20.0);
        assert_eq!(l.glyphs.len(), 3);
        assert!((l.width - base).abs() < 0.5, "{} vs {base}", l.width);
    }

    #[test]
    fn invisible_characters_draw_nothing() {
        let t = UiText::default();
        assert_eq!(t.layout("a\u{200B}b", 20.0).glyphs.len(), 2);
        let th = t.layout("ไทย\u{200B}ไทย", 20.0);
        // the zero width space stays inside the Thai run, never a .notdef box
        assert!(gids(&th).iter().all(|&g| g != 0), "{th:?}");
    }

    #[test]
    fn script_fonts_match_the_menu_em_and_rasterise() {
        let mut t = UiText::default();
        // Arabic's ascent - descent is 2.1 em: scaled up to Inter's em
        let (_, _, k) = t.placed_font(t.layout("س", 20.0).glyphs[0].g);
        assert!(k > 1.5 && k < 2.0, "{k}");
        for s in ["سلام", "שלום", "हिन्दी", "मराठी", "বাংলা", "ਪੰਜਾਬੀ", "ગુજરાતી", "தமிழ்", "తెలుగు", "ไทย", "ខ្មែរ", "አማርኛ", "اردو", "فارسی"] {
            let l = t.layout(s, 20.0);
            assert!(l.width > 10.0 && l.width < 120.0, "{s}: {}", l.width);
            for p in &l.glyphs {
                let (f, id, _) = t.placed_font(p.g);
                assert_ne!(id.0, 0, "{s}: missing glyph in {:?}", f.units_per_em());
            }
            assert!(l.glyphs.iter().any(|p| t.placed(p, 20).is_some()), "{s}");
        }
    }
}
