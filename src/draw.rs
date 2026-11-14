//! 2D drawing helpers over `qrender::Draw2D` (draw.c equivalents) and the
//! picture cache (gfx.wad lumps and gfx/*.lmp files).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::font::{Shape, UiText};

use qcore::fs::Fs;
use qformats::Wad;
use qformats::image::{Pic, parse_pic};
use qrender::{Draw2D, PicId, Renderer};
pub(crate) mod addon_video;

/// Loaded pictures by name ("sbar", "gfx/qplaque.lmp", ...).
#[derive(Default)]
pub struct Pics {
    wad: Option<Wad>,
    cache: HashMap<String, Option<(PicId, u32, u32)>>,
}

impl Pics {
    pub fn new(fs: &Fs, r: &mut Renderer) -> Self {
        let wad = fs.open("gfx.wad").and_then(|d| Wad::parse(&d).ok());
        if let Some(c) = wad.as_ref().and_then(|w| w.get("conchars"))
            && c.data.len() >= 128 * 128
        {
            let _ = r.set_conchars(&c.data[..128 * 128]);
        }
        Self { wad, cache: HashMap::new() }
    }

    /// Picture by wad lump name, or by path when it contains '/'.
    pub fn get(&mut self, fs: &Fs, r: &mut Renderer, name: &str) -> Option<(PicId, u32, u32)> {
        if let Some(p) = self.cache.get(name) {
            return *p;
        }
        if name.ends_with(".tga") {
            let v = fs.open(name).and_then(|d| qformats::image::parse_tga(&d).ok())
                .or_else(|| addon_video::frame(name))
                .filter(|p| p.width <= 1024 && p.height <= 1024)
                .and_then(|p| r.register_pic_rgba_linear(p.width, p.height, &p.pixels)
                    .ok().map(|id| (id, p.width, p.height)));
            self.cache.insert(name.to_string(), v);
            return v;
        }
        // "path#top,bottom": the picture with player colours (menuplyr.lmp).
        if let Some((base, cols)) = name.split_once('#') {
            let (t, b) = cols.split_once(',').unwrap_or((cols, cols));
            let (t, b) = (t.parse().unwrap_or(0), b.parse().unwrap_or(0));
            let pic = fs.open(base).and_then(|d| parse_pic(&d).ok()).map(|mut p| {
                translate_player(&mut p.pixels, t, b);
                p
            });
            let v = pic.and_then(|p| r.register_pic(&p).ok().map(|id| (id, p.width, p.height)));
            self.cache.insert(name.to_string(), v);
            return v;
        }
        let pic: Option<Pic> = if name.contains('/') {
            fs.open(name).and_then(|d| parse_pic(&d).ok())
        } else {
            self.wad.as_ref().and_then(|w| w.get(name)).and_then(|l| parse_pic(&l.data).ok())
        };
        let v = pic.and_then(|p| r.register_pic(&p).ok().map(|id| (id, p.width, p.height)));
        self.cache.insert(name.to_string(), v);
        v
    }
}

/// Player colour rows 0..=13 (a mid entry of each palette row).
pub const PLAYER_COLORS: [[u8; 3]; 14] = [
    [123, 123, 123], [83, 59, 27], [79, 79, 115], [55, 55, 7], [71, 0, 0], [95, 71, 7], [143, 67, 51],
    [127, 83, 63], [95, 63, 75], [107, 59, 75], [123, 99, 83], [55, 75, 63], [139, 115, 7], [47, 47, 143],
];

/// M_BuildTranslationTable: palette rows 1 (shirt) and 6 (pants) take the
/// player's colour rows; rows 8 and up run backwards.
pub fn translate_player(pixels: &mut [u8], top: u8, bottom: u8) {
    let row = |c: u8, j: u8| if c < 8 { c * 16 + j } else { c * 16 + 15 - j };
    let (top, bottom) = (top.min(13), bottom.min(13));
    for p in pixels {
        match *p {
            16..=31 => *p = row(top, *p - 16),
            96..=111 => *p = row(bottom, *p - 96),
            _ => {}
        }
    }
}

/// RGBA of player colour row `c` (scoreboard boxes, setup swatches).
pub fn player_color(c: i32) -> [f32; 4] {
    let [r, g, b] = PLAYER_COLORS[c.clamp(0, 13) as usize].map(|x| x as f32 / 255.0);
    [r, g, b, 1.0]
}

#[cfg(test)]
mod translate_tests {
    #[test]
    fn rows_map_like_the_original() {
        let mut px = [16u8, 31, 96, 111, 5, 255];
        super::translate_player(&mut px, 2, 9);
        assert_eq!(px, [32, 47, 159, 144, 5, 255]);
        let mut px = [16u8, 96];
        super::translate_player(&mut px, 1, 6);
        assert_eq!(px, [16, 96]);
    }
}

/// A 2D draw list in virtual units: `scale` pixels per unit, offset `ox,oy`.
pub struct Canvas {
    pub list: Vec<Draw2D>,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
    /// Modern text/shape atlas ([`crate::font`]); without it text is skipped.
    pub ui: Option<Rc<RefCell<UiText>>>,
    /// Vertical clip in pixels (top, bottom) for everything pushed.
    pub clip: Option<(f32, f32)>,
}

pub const WHITE: [f32; 4] = [1.0; 4];

/// Move draws by (`dx`, `dy`) pixels (a split-screen cell's HUD into place).
pub fn translate(list: &mut [Draw2D], dx: f32, dy: f32) {
    for d in list {
        let (Draw2D::Pic { x, y, .. } | Draw2D::Char { x, y, .. } | Draw2D::Fill { x, y, .. } | Draw2D::Tile { x, y, .. }) = d;
        *x += dx;
        *y += dy;
    }
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        let scale = (h as f32 / 240.0).floor().clamp(1.0, 8.0);
        Self { list: Vec::new(), w: w as f32, h: h as f32, scale, ox: 0.0, oy: 0.0, ui: None, clip: None }
    }
    /// Virtual width/height at the current scale.
    pub fn vw(&self) -> f32 {
        self.w / self.scale
    }
    pub fn vh(&self) -> f32 {
        self.h / self.scale
    }
    /// Origin for a centred 320×200 layout (menus, intermission).
    pub fn center_320x200(&mut self) {
        self.ox = ((self.vw() - 320.0) / 2.0).floor();
        self.oy = ((self.vh() - 200.0) / 2.0).floor();
    }
    /// Switch to the 16:9 menu layout: a centred 640×360 area at the largest
    /// integer scale that fits (3x at 1080p). Callers restore `scale`.
    pub fn menu_layout(&mut self) {
        self.scale = (self.h / 360.0).min(self.w / 640.0).floor().clamp(1.0, 8.0);
        self.ox = ((self.vw() - 640.0) / 2.0).floor();
        self.oy = ((self.vh() - 360.0) / 2.0).floor();
    }
    /// Outline of a rectangle, one virtual pixel thick.
    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.fill(x, y, w, 1.0, color);
        self.fill(x, y + h - 1.0, w, 1.0, color);
        self.fill(x, y + 1.0, 1.0, h - 2.0, color);
        self.fill(x + w - 1.0, y + 1.0, 1.0, h - 2.0, color);
    }
    pub fn reset_origin(&mut self) {
        self.ox = 0.0;
        self.oy = 0.0;
    }
    fn px(&self, x: f32, y: f32) -> (f32, f32) {
        ((x + self.ox) * self.scale, (y + self.oy) * self.scale)
    }
    pub fn pic(&mut self, x: f32, y: f32, p: Option<(PicId, u32, u32)>) {
        self.pic_color(x, y, p, WHITE);
    }
    pub fn pic_color(&mut self, x: f32, y: f32, p: Option<(PicId, u32, u32)>, color: [f32; 4]) {
        let Some((pic, w, h)) = p else { return };
        let (px, py) = self.px(x, y);
        self.push(Draw2D::Pic {
            x: px,
            y: py,
            w: w as f32 * self.scale,
            h: h as f32 * self.scale,
            pic,
            uv: [0.0, 0.0, 1.0, 1.0],
            color,
        });
    }
    /// Picture stretched over a pixel rectangle (ignores scale/origin).
    pub fn pic_stretch_px(&mut self, rect: [f32; 4], pic: PicId) {
        let [x, y, w, h] = rect;
        self.list.push(Draw2D::Pic { x, y, w, h, pic, uv: [0.0, 0.0, 1.0, 1.0], color: WHITE });
    }
    pub fn ch(&mut self, x: f32, y: f32, c: u8) {
        self.ch_color(x, y, c, WHITE);
    }
    pub fn ch_color(&mut self, x: f32, y: f32, c: u8, color: [f32; 4]) {
        if c == b' ' {
            return;
        }
        let (px, py) = self.px(x, y);
        self.push(Draw2D::Char { x: px, y: py, size: 8.0 * self.scale, ch: c, color });
    }
    /// Plain string.
    pub fn string(&mut self, x: f32, y: f32, s: &str) {
        for (i, b) in s.bytes().enumerate() {
            self.ch(x + i as f32 * 8.0, y, b);
        }
    }
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let (px, py) = self.px(x, y);
        self.push(Draw2D::Fill { x: px, y: py, w: w * self.scale, h: h * self.scale, color });
    }

    /// Adds a draw, cut to [`Self::clip`].
    pub fn push(&mut self, d: Draw2D) {
        let Some((top, bot)) = self.clip else {
            self.list.push(d);
            return;
        };
        match d {
            Draw2D::Fill { x, y, w, h, color } => {
                let (y0, y1) = (y.max(top), (y + h).min(bot));
                if y1 > y0 {
                    self.list.push(Draw2D::Fill { x, y: y0, w, h: y1 - y0, color });
                }
            }
            Draw2D::Pic { x, y, w, h, pic, uv, color } => {
                let (y0, y1) = (y.max(top), (y + h).min(bot));
                if y1 > y0 && h > 0.0 {
                    let dv = uv[3] - uv[1];
                    let v0 = uv[1] + (y0 - y) / h * dv;
                    let v1 = uv[1] + (y1 - y) / h * dv;
                    self.list.push(Draw2D::Pic { x, y: y0, w, h: y1 - y0, pic, uv: [uv[0], v0, uv[2], v1], color });
                }
            }
            Draw2D::Char { y, size, .. } => {
                if y >= top && y + size <= bot {
                    self.list.push(d);
                }
            }
            other => self.list.push(other),
        }
    }

    /// Sets the clip to virtual rows `y0..y1` (None clears).
    pub fn clip_rows(&mut self, rows: Option<(f32, f32)>) {
        self.clip = rows.map(|(a, b)| ((a + self.oy) * self.scale, (b + self.oy) * self.scale));
    }

    fn em_px(&self, size: f32) -> f32 {
        (size * self.scale).round().max(4.0)
    }

    /// Width in virtual units of modern text `s` at em size `size`.
    pub fn text_w(&self, s: &str, size: f32) -> f32 {
        self.ui.as_ref().map_or(s.len() as f32 * size * 0.6, |u| u.borrow().measure(s, self.em_px(size)) / self.scale)
    }

    /// Modern text with its em box top at (x, y), drawn at native pixel
    /// size. Returns the width in virtual units.
    pub fn text(&mut self, x: f32, y: f32, s: &str, size: f32, color: [f32; 4]) -> f32 {
        let Some(ui) = self.ui.clone() else { return 0.0 };
        let mut ui = ui.borrow_mut();
        let px = self.em_px(size);
        let Some(pic) = ui.pic else { return 0.0 };
        let (x0, y0) = self.px(x, y);
        let (x0, base) = (x0.round(), (y0 + px * 0.8).round());
        let a = crate::font::UiText::atlas_size() as f32;
        let lay = ui.layout(s, px);
        let width = lay.last().map_or(0.0, |&(_, x, adv)| x + adv);
        for (ch, pen, _) in lay {
            if ch == ' ' {
                continue;
            }
            if let Some(g) = ui.glyph(ch, px as u16) {
                let (gx, gy) = (x0 + pen + g.ox as f32, base + g.oy as f32);
                let uv = [g.x as f32 / a, g.y as f32 / a, (g.x + g.w) as f32 / a, (g.y + g.h) as f32 / a];
                self.push(Draw2D::Pic { x: gx, y: gy, w: g.w as f32, h: g.h as f32, pic, uv, color });
            }
        }
        width / self.scale
    }

    /// Text right-aligned to `x`.
    pub fn text_right(&mut self, x: f32, y: f32, s: &str, size: f32, color: [f32; 4]) {
        let w = self.text_w(s, size);
        self.text(x - w, y, s, size, color);
    }

    /// Text centred on `cx`.
    pub fn text_center(&mut self, cx: f32, y: f32, s: &str, size: f32, color: [f32; 4]) {
        let w = self.text_w(s, size);
        self.text(cx - w / 2.0, y, s, size, color);
    }

    /// A vector shape filling the virtual rectangle, rasterised at its pixel
    /// size. `r`/`t` in [`Shape`] are given in pixels by the caller; use
    /// [`Self::upx`] to convert.
    pub fn shape(&mut self, x: f32, y: f32, w: f32, h: f32, s: Shape, color: [f32; 4]) {
        let Some(ui) = self.ui.clone() else {
            self.fill(x, y, w, h, color);
            return;
        };
        let mut ui = ui.borrow_mut();
        let Some(pic) = ui.pic else { return };
        let (px, py) = self.px(x, y);
        let (px, py) = (px.round(), py.round());
        let (pw, ph) = ((w * self.scale).round().max(1.0), (h * self.scale).round().max(1.0));
        if let Some(g) = ui.shape(s, pw as u16, ph as u16) {
            let a = crate::font::UiText::atlas_size() as f32;
            let uv = [g.x as f32 / a, g.y as f32 / a, (g.x + g.w) as f32 / a, (g.y + g.h) as f32 / a];
            self.push(Draw2D::Pic { x: px, y: py, w: pw, h: ph, pic, uv, color });
        }
    }

    /// Virtual units to whole pixels (at least 1), for shape radii/strokes.
    pub fn upx(&self, v: f32) -> u16 {
        (v * self.scale).round().max(1.0) as u16
    }

    /// Rounded box: fill plus an optional 1 px edge. Drawn as nine slices
    /// (corners cut from a small 2r×2r shape, the rest as flat fills), so
    /// big panels don't need big atlas entries.
    #[allow(clippy::too_many_arguments)]
    pub fn round_box(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: [f32; 4], edge: Option<[f32; 4]>) {
        let (px, py) = self.px(x, y);
        let (x0, y0) = (px.round(), py.round());
        let (x1, y1) = ((px + w * self.scale).round(), (py + h * self.scale).round());
        let (pw, ph) = (x1 - x0, y1 - y0);
        if pw < 1.0 || ph < 1.0 {
            return;
        }
        let r = (r * self.scale).round().min((pw / 2.0).floor()).min((ph / 2.0).floor()).max(0.0);
        let t = 1.0;
        self.slices(x0, y0, pw, ph, r, Shape::Round { r: r as u16 }, fill, None);
        if let Some(e) = edge {
            self.slices(x0, y0, pw, ph, r, Shape::RoundEdge { r: r as u16, t: 1 }, e, Some(t));
        }
    }

    /// Rounded outline with a stroke width in virtual units.
    pub fn round_frame(&mut self, rect: [f32; 4], r: f32, thickness: f32, color: [f32; 4]) {
        let [x, y, w, h] = rect;
        let (px, py) = self.px(x, y);
        let (w, h) = ((px + w * self.scale).round() - px.round(),
            (py + h * self.scale).round() - py.round());
        let t = self.upx(thickness).min((w.min(h) / 2.0).max(0.0) as u16);
        if t == 0 { return; }
        let r = self.upx(r).max(t).min((w.min(h) / 2.0) as u16);
        self.slices(px.round(), py.round(), w, h, r as f32,
            Shape::RoundEdge { r, t }, color, Some(t as f32));
    }

    /// Nine-slice draw in pixels. `edge` = Some(thickness): only the border
    /// strips are filled between the corners.
    #[allow(clippy::too_many_arguments)]
    fn slices(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, s: Shape, color: [f32; 4], edge: Option<f32>) {
        let fill_px = |c: &mut Self, fx: f32, fy: f32, fw: f32, fh: f32| {
            if fw > 0.0 && fh > 0.0 {
                c.push(Draw2D::Fill { x: fx, y: fy, w: fw, h: fh, color });
            }
        };
        if r >= 1.0 {
            let Some(ui) = self.ui.clone() else { return };
            let corner = {
                let mut ui = ui.borrow_mut();
                ui.pic.zip(ui.shape(s, (2.0 * r) as u16, (2.0 * r) as u16))
            };
            if let Some((pic, g)) = corner {
                let a = crate::font::UiText::atlas_size() as f32;
                let (u0, v0) = (g.x as f32 / a, g.y as f32 / a);
                let (um, vm) = ((g.x as f32 + r) / a, (g.y as f32 + r) / a);
                let (u1, v1) = ((g.x + g.w) as f32 / a, (g.y + g.h) as f32 / a);
                for (cx, cy, uv) in [
                    (x, y, [u0, v0, um, vm]),
                    (x + w - r, y, [um, v0, u1, vm]),
                    (x, y + h - r, [u0, vm, um, v1]),
                    (x + w - r, y + h - r, [um, vm, u1, v1]),
                ] {
                    self.push(Draw2D::Pic { x: cx, y: cy, w: r, h: r, pic, uv, color });
                }
            }
        }
        match edge {
            None => {
                fill_px(self, x + r, y, w - 2.0 * r, r);
                fill_px(self, x, y + r, w, h - 2.0 * r);
                fill_px(self, x + r, y + h - r, w - 2.0 * r, r);
            }
            Some(t) => {
                fill_px(self, x + r, y, w - 2.0 * r, t);
                fill_px(self, x + r, y + h - t, w - 2.0 * r, t);
                fill_px(self, x, y + r, t, h - 2.0 * r);
                fill_px(self, x + w - t, y + r, t, h - 2.0 * r);
            }
        }
    }
    /// Whole-screen fade (Draw_FadeScreen).
    pub fn fade_screen(&mut self) {
        self.list.push(Draw2D::Fill { x: 0.0, y: 0.0, w: self.w, h: self.h, color: [0.0, 0.0, 0.0, 0.5] });
    }
}
