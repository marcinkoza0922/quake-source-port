//! Fallback fonts for characters the menu font lacks: Inter (Latin,
//! Cyrillic, Greek, Vietnamese), Noto Sans Armenian and Georgian, the Noto
//! Sans fonts of scripts that need shaping (subsets, see
//! `tools/script_fonts.py`), and Noto Sans CJK per region (subset to the
//! text the port ships, see `tools/cjk_fonts.py`). Han characters look
//! different per region, so the CJK order follows the current language.

use ab_glyph::{Font, FontArc};
use qcore::lang::Lang;

/// Inter is [`super::FONTS`]`[0]`; these follow it in this order.
const SCRIPT_FONTS: [&[u8]; 2] = [
    include_bytes!("../../assets/fonts/NotoSansArmenian-Regular.ttf"),
    include_bytes!("../../assets/fonts/NotoSansGeorgian-Regular.ttf"),
];

/// Scripts drawn through the shaper (`super::shape`), after
/// [`SCRIPT_FONTS`]: Arabic (also Persian and Urdu), Hebrew, Devanagari
/// (Hindi, Marathi), Bengali, Gurmukhi (Punjabi), Gujarati, Tamil, Telugu,
/// Thai, Khmer; then Ethiopic (Amharic), which needs no shaping but is
/// scaled to the menu font's em like them.
const SHAPED_FONTS: [&[u8]; 11] = [
    include_bytes!("../../assets/fonts/NotoSansArabic-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansHebrew-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansDevanagari-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansBengali-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansGurmukhi-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansGujarati-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansTamil-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansTelugu-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansThai-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansKhmer-Subset.ttf"),
    include_bytes!("../../assets/fonts/NotoSansEthiopic-Subset.ttf"),
];

/// Index in [`Fallbacks::scripts`] of the first [`SHAPED_FONTS`] entry.
const FIRST_SHAPED: usize = 1 + SCRIPT_FONTS.len();

/// Japanese, Korean, Simplified and Traditional Chinese.
const CJK_FONTS: [&[u8]; 4] = [
    include_bytes!("../../assets/fonts/NotoSansJP-Subset.otf"),
    include_bytes!("../../assets/fonts/NotoSansKR-Subset.otf"),
    include_bytes!("../../assets/fonts/NotoSansSC-Subset.otf"),
    include_bytes!("../../assets/fonts/NotoSansTC-Subset.otf"),
];

/// Order of [`CJK_FONTS`] to try for language `l`.
pub fn cjk_order(l: Lang) -> [u8; 4] {
    match l {
        Lang::Japanese => [0, 3, 2, 1],
        Lang::Korean => [1, 3, 2, 0],
        Lang::TChinese | Lang::Cantonese => [3, 2, 0, 1],
        _ => [2, 3, 0, 1],
    }
}

/// Letters a menu font must all have to draw language `l` by itself; one
/// that has only some of them would mix fonts inside words. (A script it
/// lacks entirely, like Cyrillic, falls back whole words at a time.)
fn probe(l: Lang) -> &'static str {
    match l {
        Lang::Vietnamese => "ơưạảấầẩẫậắằẳẵặẹẻẽếềểễệỉịọỏốồổỗộớờởỡợụủứừửữựỷỹĩũ",
        Lang::Esperanto => "ĉĝĥĵŝŭ",
        Lang::Romanian => "ăâîșț",
        Lang::Greek => "αβγδεζηθικλμνξοπρστυφχψωάέήίόύώ",
        _ => "",
    }
}

/// Whether `font` has every [`probe`] letter of `l`.
pub fn covers(font: &FontArc, l: Lang) -> bool {
    probe(l).chars().all(|c| font.glyph_id(c).0 != 0)
}

/// Which font draws a character: the first font asked for (menu or
/// monospace), [`Fallbacks::scripts`]`[i]` or [`Fallbacks::cjk`]`[i]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Src {
    First,
    Script(u8),
    Cjk(u8),
}

pub struct Fallbacks {
    /// Inter, then [`SCRIPT_FONTS`], then [`SHAPED_FONTS`].
    pub scripts: Vec<FontArc>,
    /// Shaping faces, by [`Self::scripts`] index (None: drawn per character).
    faces: Vec<Option<rustybuzz::Face<'static>>>,
    pub cjk: Vec<FontArc>,
}

impl Default for Fallbacks {
    fn default() -> Self {
        let load = |b: &'static [u8]| FontArc::try_from_slice(b).expect("bundled font");
        let all = || std::iter::once(super::FONTS[0].1).chain(SCRIPT_FONTS).chain(SHAPED_FONTS);
        let ethiopic = FIRST_SHAPED + SHAPED_FONTS.len() - 1;
        Self {
            scripts: all().map(load).collect(),
            faces: all()
                .enumerate()
                .map(|(i, b)| (FIRST_SHAPED..ethiopic).contains(&i).then(|| rustybuzz::Face::from_slice(b, 0).expect("bundled font")))
                .collect(),
            cjk: CJK_FONTS.into_iter().map(load).collect(),
        }
    }
}

/// (ascent - descent) per em: ab_glyph scales fonts to that height.
fn height_per_em(f: &FontArc) -> f32 {
    f.height_unscaled() / f.units_per_em().unwrap_or(1000.0)
}

impl Fallbacks {
    /// The first fallback with a glyph for `ch`, CJK fonts in `order`.
    pub fn find(&self, ch: char, order: [u8; 4]) -> Option<Src> {
        if let Some(i) = self.scripts.iter().position(|f| f.glyph_id(ch).0 != 0) {
            return Some(Src::Script(i as u8));
        }
        order.iter().find(|&&i| self.cjk[i as usize].glyph_id(ch).0 != 0).map(|&i| Src::Cjk(i))
    }

    /// The font of `src` (`first` for [`Src::First`]).
    pub fn font<'a>(&'a self, first: &'a FontArc, src: Src) -> &'a FontArc {
        match src {
            Src::First => first,
            Src::Script(i) => &self.scripts[i as usize],
            Src::Cjk(i) => &self.cjk[i as usize],
        }
    }

    /// The shaping face of `src`, for scripts that need one.
    pub fn face(&self, src: Src) -> Option<&rustybuzz::Face<'static>> {
        match src {
            Src::Script(i) => self.faces.get(i as usize)?.as_ref(),
            _ => None,
        }
    }

    /// Pixel scale factor for `src` beside `first`: the [`SHAPED_FONTS`]
    /// have tall ascent/descent for stacked marks (Arabic: 2.1 em), so they
    /// are scaled for their em to match `first`'s. Others: 1.
    pub fn em_scale(&self, first: &FontArc, src: Src) -> f32 {
        match src {
            Src::Script(i) if i as usize >= FIRST_SHAPED => height_per_em(&self.scripts[i as usize]) / height_per_em(first),
            _ => 1.0,
        }
    }
}
