//! Fallback fonts for characters the menu font lacks: Inter (Latin,
//! Cyrillic, Greek, Vietnamese), Noto Sans Armenian and Georgian, and Noto
//! Sans CJK per region (subset to the text the port ships, see
//! `tools/cjk_fonts.py`). Han characters look different per region, so the
//! CJK order follows the current language.

use ab_glyph::{Font, FontArc};
use qcore::lang::Lang;

/// Inter is [`super::FONTS`]`[0]`; these follow it in this order.
const SCRIPT_FONTS: [&[u8]; 2] = [
    include_bytes!("../../assets/fonts/NotoSansArmenian-Regular.ttf"),
    include_bytes!("../../assets/fonts/NotoSansGeorgian-Regular.ttf"),
];

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
        Lang::TChinese => [3, 2, 0, 1],
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

pub struct Fallbacks {
    /// Inter, then [`SCRIPT_FONTS`].
    pub scripts: Vec<FontArc>,
    pub cjk: Vec<FontArc>,
}

impl Default for Fallbacks {
    fn default() -> Self {
        let load = |b: &'static [u8]| FontArc::try_from_slice(b).expect("bundled font");
        Self {
            scripts: std::iter::once(super::FONTS[0].1).chain(SCRIPT_FONTS).map(load).collect(),
            cjk: CJK_FONTS.into_iter().map(load).collect(),
        }
    }
}

impl Fallbacks {
    /// The first fallback with a glyph for `ch`, CJK fonts in `order`.
    pub fn find(&self, ch: char, order: [u8; 4]) -> Option<&FontArc> {
        self.scripts
            .iter()
            .chain(order.iter().map(|&i| &self.cjk[i as usize]))
            .find(|f| f.glyph_id(ch).0 != 0)
    }
}
