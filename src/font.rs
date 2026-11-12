//! Modern UI text and shapes, rasterised at the exact pixel size they are
//! drawn at (never scaled), packed into one CPU atlas that is uploaded to
//! the renderer when it changes.
//!
//! Submenus, save cards and button prompts use this; the top-level classic
//! screens and the console/HUD keep the 8×8 conchars. Fonts are bundled
//! (SIL OFL, from padwight's set, see `assets/fonts`), picked by
//! `ui_menufont`; `ui_fontfile` (game path or disk path) overrides them.

use std::collections::HashMap;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};

/// Bundled fonts: (menu name, bytes).
pub const FONTS: [(&str, &[u8]); 4] = [
    ("Inter", include_bytes!("../assets/fonts/Inter-Regular.ttf")),
    ("Rajdhani", include_bytes!("../assets/fonts/Rajdhani-Medium.ttf")),
    ("Titillium", include_bytes!("../assets/fonts/TitilliumWeb-Regular.ttf")),
    ("Kreon", include_bytes!("../assets/fonts/Kreon-Regular.ttf")),
];

const ATLAS: usize = 1024;

/// Vector shapes drawn as white coverage masks (tinted by the draw colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    /// Filled rounded rectangle with corner radius `r` px.
    Round { r: u16 },
    /// Outline of a rounded rectangle, `t` px thick.
    RoundEdge { r: u16, t: u16 },
    /// PlayStation symbols, stroked (`t` px).
    Cross { t: u16 },
    Ring { t: u16 },
    Square { t: u16 },
    Triangle { t: u16 },
    /// Filled arrowheads.
    ArrowDown,
    ArrowUp,
    ArrowRight,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph { ch: char, px: u16 },
    Shape { s: Shape, w: u16, h: u16 },
}

#[derive(Clone, Copy)]
pub struct Slot {
    /// Atlas texel rectangle.
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// Offset of the bitmap from the pen/baseline (glyphs) in px.
    pub ox: i16,
    pub oy: i16,
}

pub struct UiText {
    font: FontArc,
    /// (ui_menufont, ui_fontfile) the font was loaded for.
    pub source: (i32, String),
    pub pic: Option<qrender::PicId>,
    pub px: Vec<u8>,
    map: HashMap<Key, Option<Slot>>,
    shelf: (usize, usize, usize),
    pub dirty: bool,
    full: bool,
}

impl Default for UiText {
    fn default() -> Self {
        Self::new(FontArc::try_from_slice(FONTS[0].1).expect("bundled font"))
    }
}

impl UiText {
    pub fn new(font: FontArc) -> Self {
        Self {
            font,
            source: (0, String::new()),
            pic: None,
            px: vec![0; ATLAS * ATLAS * 4],
            map: HashMap::new(),
            shelf: (1, 1, 0),
            dirty: true,
            full: false,
        }
    }

    pub fn atlas_size() -> u32 {
        ATLAS as u32
    }

    /// Switches font; returns a warning when `file` could not be used.
    pub fn set_font(&mut self, fs: &qcore::fs::Fs, menufont: i32, file: &str) -> Option<String> {
        let mut warn = None;
        let custom = (!file.is_empty())
            .then(|| fs.open(file).or_else(|| std::fs::read(file).ok()))
            .flatten()
            .and_then(|d| FontArc::try_from_vec(d).ok());
        if !file.is_empty() && custom.is_none() {
            warn = Some(format!("ui_fontfile: can't load \"{file}\"\n"));
        }
        let idx = (menufont.max(0) as usize).min(FONTS.len() - 1);
        self.font = custom.unwrap_or_else(|| FontArc::try_from_slice(FONTS[idx].1).expect("bundled font"));
        self.source = (menufont, file.to_string());
        self.clear();
        warn
    }

    fn clear(&mut self) {
        self.px.fill(0);
        self.map.clear();
        self.shelf = (1, 1, 0);
        self.dirty = true;
        self.full = false;
    }

    /// Called after uploading: starts over when the atlas ran out of room
    /// (the next frame re-adds what it draws).
    pub fn after_upload(&mut self) {
        self.dirty = false;
        if self.full {
            self.clear();
        }
    }

    fn alloc(&mut self, w: usize, h: usize) -> Option<(usize, usize)> {
        let (mut x, mut y, mut sh) = self.shelf;
        if x + w + 1 > ATLAS {
            x = 1;
            y += sh + 1;
            sh = 0;
        }
        if y + h + 1 > ATLAS || w + 2 > ATLAS {
            self.full = true;
            return None;
        }
        self.shelf = (x + w + 1, y, sh.max(h));
        Some((x, y))
    }

    fn put(&mut self, w: usize, h: usize, cov: &[f32]) -> Option<(usize, usize)> {
        let (ax, ay) = self.alloc(w, h)?;
        for y in 0..h {
            for x in 0..w {
                let i = ((ay + y) * ATLAS + ax + x) * 4;
                self.px[i..i + 4].copy_from_slice(&[255, 255, 255, (cov[y * w + x].clamp(0.0, 1.0) * 255.0).round() as u8]);
            }
        }
        self.dirty = true;
        Some((ax, ay))
    }

    /// Pixel width of `s` at `px`, with kerning (pen positions rounded as
    /// [`Self::layout`] does).
    pub fn measure(&self, s: &str, px: f32) -> f32 {
        self.layout(s, px).last().map_or(0.0, |&(_, x, adv)| x + adv)
    }

    /// (char, pen x, advance) for each char, pen rounded to whole px.
    pub fn layout(&self, s: &str, px: f32) -> Vec<(char, f32, f32)> {
        let sf = self.font.as_scaled(PxScale::from(px));
        let mut pen = 0f32;
        let mut prev = None;
        let mut out = Vec::with_capacity(s.len());
        for ch in s.chars() {
            let id = self.font.glyph_id(ch);
            if let Some(p) = prev {
                pen += sf.kern(p, id);
            }
            let adv = sf.h_advance(id);
            out.push((ch, pen.round(), adv));
            pen += adv;
            prev = Some(id);
        }
        out
    }

    /// Atlas slot of `ch` at `px` (rasterised on first use).
    pub fn glyph(&mut self, ch: char, px: u16) -> Option<Slot> {
        let key = Key::Glyph { ch, px };
        if let Some(s) = self.map.get(&key) {
            return *s;
        }
        let g = self.font.glyph_id(ch).with_scale_and_position(PxScale::from(px as f32), ab_glyph::point(0.0, 0.0));
        let slot = match self.font.outline_glyph(g) {
            None => None,
            Some(o) => {
                let b = o.px_bounds();
                let (w, h) = (b.width() as usize, b.height() as usize);
                let mut cov = vec![0f32; w * h];
                o.draw(|x, y, v| {
                    if let Some(c) = cov.get_mut(y as usize * w + x as usize) {
                        *c = v;
                    }
                });
                // full: not cached, retried after the atlas resets
                let (x, y) = self.put(w, h, &cov)?;
                Some(Slot { x: x as u16, y: y as u16, w: w as u16, h: h as u16, ox: b.min.x as i16, oy: b.min.y as i16 })
            }
        };
        self.map.insert(key, slot);
        slot
    }

    /// Atlas slot of a `w`×`h` px shape.
    pub fn shape(&mut self, s: Shape, w: u16, h: u16) -> Option<Slot> {
        let key = Key::Shape { s, w, h };
        if let Some(sl) = self.map.get(&key) {
            return *sl;
        }
        // big shapes would crowd the atlas (use nine slices for boxes)
        if w == 0 || h == 0 || w > 256 || h > 256 {
            return None;
        }
        let (wu, hu) = (w as usize, h as usize);
        let mut cov = vec![0f32; wu * hu];
        const SS: usize = 4;
        for y in 0..hu {
            for x in 0..wu {
                let mut n = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let p = (x as f32 + (sx as f32 + 0.5) / SS as f32, y as f32 + (sy as f32 + 0.5) / SS as f32);
                        if inside(s, w as f32, h as f32, p) {
                            n += 1;
                        }
                    }
                }
                cov[y * wu + x] = n as f32 / (SS * SS) as f32;
            }
        }
        let at = self.put(wu, hu, &cov)?;
        let slot = Some(Slot { x: at.0 as u16, y: at.1 as u16, w, h, ox: 0, oy: 0 });
        self.map.insert(key, slot);
        slot
    }
}

fn seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (qx, qy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
    (qx * qx + qy * qy).sqrt()
}

/// Signed distance to a rounded box of half size (hw, hh), radius r, centred.
fn round_box(p: (f32, f32), hw: f32, hh: f32, r: f32) -> f32 {
    let r = r.min(hw).min(hh);
    let (qx, qy) = (p.0.abs() - hw + r, p.1.abs() - hh + r);
    let out = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    out + qx.max(qy).min(0.0) - r
}

fn inside(s: Shape, w: f32, h: f32, p: (f32, f32)) -> bool {
    let c = (p.0 - w / 2.0, p.1 - h / 2.0);
    let m = w.min(h);
    match s {
        Shape::Round { r } => round_box(c, w / 2.0, h / 2.0, r as f32) <= 0.0,
        Shape::RoundEdge { r, t } => {
            let d = round_box(c, w / 2.0, h / 2.0, r as f32);
            d <= 0.0 && d > -(t as f32)
        }
        Shape::Cross { t } => {
            let k = m * 0.32;
            seg(c, (-k, -k), (k, k)) <= t as f32 / 2.0 || seg(c, (-k, k), (k, -k)) <= t as f32 / 2.0
        }
        Shape::Ring { t } => ((c.0 * c.0 + c.1 * c.1).sqrt() - m * 0.34).abs() <= t as f32 / 2.0,
        Shape::Square { t } => (c.0.abs().max(c.1.abs()) - m * 0.3).abs() <= t as f32 / 2.0,
        Shape::Triangle { t } => {
            let k = m * 0.36;
            let (a, b, d) = ((0.0, -k), (-k, k * 0.75), (k, k * 0.75));
            let t = t as f32 / 2.0;
            seg(c, a, b) <= t || seg(c, b, d) <= t || seg(c, d, a) <= t
        }
        Shape::ArrowDown => p.1 >= h * 0.2 && p.1 <= h * 0.8 && c.0.abs() <= (h * 0.8 - p.1) / (h * 0.6) * w / 2.0,
        Shape::ArrowUp => p.1 >= h * 0.2 && p.1 <= h * 0.8 && c.0.abs() <= (p.1 - h * 0.2) / (h * 0.6) * w / 2.0,
        Shape::ArrowRight => p.0 >= w * 0.2 && p.0 <= w * 0.8 && c.1.abs() <= (w * 0.8 - p.0) / (w * 0.6) * h / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_parse_and_measure() {
        for (name, data) in FONTS {
            let f = FontArc::try_from_slice(data).unwrap_or_else(|_| panic!("{name}"));
            let t = UiText::new(f);
            let w = t.measure("Hello", 20.0);
            assert!(w > 20.0 && w < 100.0, "{name} {w}");
            assert!(t.measure("WW", 20.0) > t.measure("ii", 20.0));
        }
    }

    #[test]
    fn glyphs_and_shapes_are_cached_at_native_size() {
        let mut t = UiText::default();
        let a = t.glyph('A', 30).unwrap();
        assert!(a.h >= 18 && a.h <= 24, "{}", a.h);
        assert_eq!(t.glyph('A', 30).unwrap().x, a.x);
        assert!(t.glyph(' ', 30).is_none());
        let d = t.shape(Shape::Round { r: 15 }, 30, 30).unwrap();
        assert_eq!((d.w, d.h), (30, 30));
        let i = |x: usize, y: usize| t.px[((d.y as usize + y) * ATLAS + d.x as usize + x) * 4 + 3];
        assert_eq!(i(15, 15), 255);
        assert_eq!(i(0, 0), 0);
    }
}
