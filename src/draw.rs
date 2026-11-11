//! 2D drawing helpers over `qrender::Draw2D` (draw.c equivalents) and the
//! picture cache (gfx.wad lumps and gfx/*.lmp files).

use std::collections::HashMap;

use qcore::fs::Fs;
use qformats::Wad;
use qformats::image::{Pic, parse_pic};
use qrender::{Draw2D, PicId, Renderer};

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
}

pub const WHITE: [f32; 4] = [1.0; 4];

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        let scale = (h as f32 / 240.0).floor().clamp(1.0, 8.0);
        Self { list: Vec::new(), w: w as f32, h: h as f32, scale, ox: 0.0, oy: 0.0 }
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
        self.list.push(Draw2D::Pic {
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
        self.list.push(Draw2D::Char { x: px, y: py, size: 8.0 * self.scale, ch: c, color });
    }
    /// Plain string.
    pub fn string(&mut self, x: f32, y: f32, s: &str) {
        for (i, b) in s.bytes().enumerate() {
            self.ch(x + i as f32 * 8.0, y, b);
        }
    }
    pub fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let (px, py) = self.px(x, y);
        self.list.push(Draw2D::Fill { x: px, y: py, w: w * self.scale, h: h * self.scale, color });
    }
    /// Whole-screen fade (Draw_FadeScreen).
    pub fn fade_screen(&mut self) {
        self.list.push(Draw2D::Fill { x: 0.0, y: 0.0, w: self.w, h: self.h, color: [0.0, 0.0, 0.0, 0.5] });
    }
}
