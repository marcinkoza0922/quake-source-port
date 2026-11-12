//! Button glyphs for prompts and the bindings menu. The label/colour table
//! follows padwight's (`src/info.rs` `button_glyph`): Xbox and PlayStation
//! face buttons are coloured discs, Nintendo's are neutral discs labelled by
//! position (south shows B), other buttons are rounded tags, the D-pad is a
//! small cross with the pressed arm lit. Keyboard keys are square tags.
//!
//! Discs and the PlayStation symbols come from a small RGBA atlas rasterised
//! at startup ([`atlas_rgba`]).

use qinput::keys::*;
use qrender::{Draw2D, PicId};

use crate::draw::Canvas;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Family {
    #[default]
    Xbox,
    PlayStation,
    Nintendo,
}

impl Family {
    pub fn from_platform(f: u8) -> Self {
        match f {
            qplatform::family::PLAYSTATION => Family::PlayStation,
            qplatform::family::NINTENDO => Family::Nintendo,
            _ => Family::Xbox,
        }
    }
    /// `joy_glyphs` override: 0 auto (detected), 1 Xbox, 2 PlayStation, 3 Nintendo.
    pub fn with_override(self, v: f32) -> Self {
        match v as i32 {
            1 => Family::Xbox,
            2 => Family::PlayStation,
            3 => Family::Nintendo,
            _ => self,
        }
    }
}

/// Glyph height in virtual units.
pub const H: f32 = 10.0;

/// Atlas cells (CELL×CELL each, in a row).
const CELL: u32 = 64;
const CELLS: u32 = 5;
#[derive(Clone, Copy)]
enum Sym {
    Disc = 0,
    Cross = 1,
    Circle = 2,
    Square = 3,
    Triangle = 4,
}

/// Rasterise the atlas: white shapes on transparent, 4×4 supersampled.
pub fn atlas_rgba() -> (u32, u32, Vec<u8>) {
    let (w, h) = (CELL * CELLS, CELL);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        let (qx, qy) = (a.0 + t * dx - p.0, a.1 + t * dy - p.1);
        (qx * qx + qy * qy).sqrt()
    };
    let inside = |cell: u32, x: f32, y: f32| -> bool {
        // centred coordinates, cell spans -16..16
        let (x, y) = (x * 32.0 / CELL as f32 - 16.0, y * 32.0 / CELL as f32 - 16.0);
        let st = 2.4;
        match cell {
            0 => x * x + y * y <= 15.5 * 15.5,
            1 => seg((x, y), (-7.0, -7.0), (7.0, 7.0)) <= st || seg((x, y), (-7.0, 7.0), (7.0, -7.0)) <= st,
            2 => ((x * x + y * y).sqrt() - 8.0).abs() <= st,
            3 => {
                let m = x.abs().max(y.abs());
                (m - 7.5).abs() <= st
            }
            _ => {
                let (a, b, c) = ((0.0, -9.0), (-9.0, 7.0), (9.0, 7.0));
                seg((x, y), a, b) <= st || seg((x, y), b, c) <= st || seg((x, y), c, a) <= st
            }
        }
    };
    for cell in 0..CELLS {
        for y in 0..CELL {
            for x in 0..CELL {
                let mut n = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        if inside(cell, x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) {
                            n += 1;
                        }
                    }
                }
                let i = ((y * w + cell * CELL + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&[255, 255, 255, (n * 255 / 16) as u8]);
            }
        }
    }
    (w, h, px)
}

fn rgb(c: [u8; 3]) -> [f32; 4] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]
}

const GREEN: [u8; 3] = [0x3c, 0xa0, 0x3c];
const RED: [u8; 3] = [0xc8, 0x3c, 0x3c];
const BLUE: [u8; 3] = [0x2f, 0x6c, 0xc8];
const YELLOW: [u8; 3] = [0xc8, 0xa0, 0x1e];
const PS_BLUE: [u8; 3] = [0x5a, 0x8c, 0xdc];
const PS_RED: [u8; 3] = [0xdc, 0x5a, 0x5a];
const PS_PINK: [u8; 3] = [0xc8, 0x6e, 0xc8];
const PS_GREEN: [u8; 3] = [0x32, 0xb4, 0x96];
const NEUTRAL: [f32; 4] = [0.36, 0.36, 0.40, 0.95];
const TAG_EDGE: [f32; 4] = [0.6, 0.6, 0.62, 0.9];
const KEY_BG: [f32; 4] = [0.12, 0.10, 0.08, 0.95];

/// How one key is drawn.
enum Shape {
    /// Round face button: fill, and a label or PlayStation symbol.
    Face([f32; 4], Result<&'static str, Sym>),
    /// Rounded tag with text (shoulders, triggers, menu buttons).
    Tag(String),
    /// D-pad cross with arms [up, down, left, right] lit.
    Dpad([bool; 4]),
    /// Keyboard/mouse key: square tag.
    Key(String),
}

fn pad_shape(key: i32, f: Family) -> Option<Shape> {
    use Family::*;
    let key = if (K_LTHUMB + K_ALT_OFFSET..K_GAMEPAD_END).contains(&key) { key - K_ALT_OFFSET } else { key };
    let face = |xbox: (&'static str, [u8; 3]), ps: (Sym, [u8; 3]), nin: &'static str| match f {
        Xbox => Shape::Face(rgb(xbox.1), Ok(xbox.0)),
        PlayStation => Shape::Face(rgb(ps.1), Err(ps.0)),
        Nintendo => Shape::Face(NEUTRAL, Ok(nin)),
    };
    let named = |x: &str, p: &str, n: &str| {
        Shape::Tag(match f {
            Xbox => x,
            PlayStation => p,
            Nintendo => n,
        }
        .to_string())
    };
    Some(match key {
        K_ABUTTON => face(("A", GREEN), (Sym::Cross, PS_BLUE), "B"),
        K_BBUTTON => face(("B", RED), (Sym::Circle, PS_RED), "A"),
        K_XBUTTON => face(("X", BLUE), (Sym::Square, PS_PINK), "Y"),
        K_YBUTTON => face(("Y", YELLOW), (Sym::Triangle, PS_GREEN), "X"),
        K_LSHOULDER => named("LB", "L1", "L"),
        K_RSHOULDER => named("RB", "R1", "R"),
        K_LTRIGGER => named("LT", "L2", "ZL"),
        K_RTRIGGER => named("RT", "R2", "ZR"),
        K_BACK => named("View", "Share", "-"),
        K_START => named("Menu", "Options", "+"),
        K_LTHUMB => named("LS", "L3", "LS"),
        K_RTHUMB => named("RS", "R3", "RS"),
        K_DPAD_UP => Shape::Dpad([true, false, false, false]),
        K_DPAD_DOWN => Shape::Dpad([false, true, false, false]),
        K_DPAD_LEFT => Shape::Dpad([false, false, true, false]),
        K_DPAD_RIGHT => Shape::Dpad([false, false, false, true]),
        K_MISC1 => named("Share", "Mic", "Capture"),
        K_PADDLE1 => Shape::Tag("P1".into()),
        K_PADDLE2 => Shape::Tag("P2".into()),
        K_PADDLE3 => Shape::Tag("P3".into()),
        K_PADDLE4 => Shape::Tag("P4".into()),
        K_TOUCHPAD => named("Touch", "Touchpad", "Touch"),
        _ => return None,
    })
}

/// Whether `key` is a gamepad key.
pub fn is_pad_key(key: i32) -> bool {
    (K_GAMEPAD_BEGIN..K_GAMEPAD_END).contains(&key)
}

/// Short readable name of a keyboard/mouse key.
pub fn key_name(key: i32) -> String {
    match key {
        K_ESCAPE => "Esc".into(),
        K_ENTER => "Enter".into(),
        K_SPACE => "Space".into(),
        K_BACKSPACE => "Bksp".into(),
        K_UPARROW => "Up".into(),
        K_DOWNARROW => "Down".into(),
        K_LEFTARROW => "Left".into(),
        K_RIGHTARROW => "Right".into(),
        K_MOUSE1 => "LMB".into(),
        K_MOUSE2 => "RMB".into(),
        K_MOUSE3 => "MMB".into(),
        K_MWHEELUP => "Wheel Up".into(),
        K_MWHEELDOWN => "Wheel Dn".into(),
        k => {
            let s = keynum_to_string(k);
            if s.len() == 1 { s.to_ascii_uppercase() } else { s }
        }
    }
}

fn shape(key: i32, f: Family) -> Shape {
    pad_shape(key, f).unwrap_or_else(|| Shape::Key(key_name(key)))
}

/// Width of a key's glyph in virtual units.
pub fn width(key: i32, f: Family) -> f32 {
    match shape(key, f) {
        Shape::Face(..) => H,
        Shape::Dpad(_) => H,
        Shape::Tag(t) | Shape::Key(t) => t.len() as f32 * 8.0 + 6.0,
    }
}

/// Draws glyphs onto a canvas with the atlas picture.
#[derive(Clone, Copy)]
pub struct Glyphs {
    pub atlas: Option<PicId>,
    pub family: Family,
}

impl Glyphs {
    fn sym(&self, c: &mut Canvas, x: f32, y: f32, s: f32, sym: Sym, color: [f32; 4]) {
        let Some(pic) = self.atlas else {
            c.fill(x, y, s, s, color);
            return;
        };
        let u0 = sym as u32 as f32 / CELLS as f32;
        let uv = [u0, 0.0, u0 + 1.0 / CELLS as f32, 1.0];
        let (px, py) = ((x + c.ox) * c.scale, (y + c.oy) * c.scale);
        c.list.push(Draw2D::Pic { x: px, y: py, w: s * c.scale, h: s * c.scale, pic, uv, color });
    }

    /// Filled disc of diameter `s` with its top-left at (x, y).
    pub fn disc(&self, c: &mut Canvas, x: f32, y: f32, s: f32, color: [f32; 4]) {
        self.sym(c, x, y, s, Sym::Disc, color);
    }

    /// Half disc (left or right) for rounded tag ends.
    fn cap(&self, c: &mut Canvas, x: f32, y: f32, right: bool, color: [f32; 4]) {
        let Some(pic) = self.atlas else {
            c.fill(x, y, H / 2.0, H, color);
            return;
        };
        let cw = 1.0 / CELLS as f32;
        let uv = if right { [cw * 0.5, 0.0, cw, 1.0] } else { [0.0, 0.0, cw * 0.5, 1.0] };
        let (px, py) = ((x + c.ox) * c.scale, (y + c.oy) * c.scale);
        c.list.push(Draw2D::Pic { x: px, y: py, w: H / 2.0 * c.scale, h: H * c.scale, pic, uv, color });
    }

    /// Draws `key` with its top-left at (x, y); returns the width used.
    pub fn key(&self, c: &mut Canvas, x: f32, y: f32, key: i32) -> f32 {
        match shape(key, self.family) {
            Shape::Face(fill, label) => {
                self.sym(c, x, y, H, Sym::Disc, fill);
                match label {
                    Ok(t) => c.string(x + 1.0, y + 1.0, t),
                    Err(s) => self.sym(c, x + 1.0, y + 1.0, H - 2.0, s, [1.0; 4]),
                }
                H
            }
            Shape::Tag(t) => {
                let w = t.len() as f32 * 8.0 + 6.0;
                self.cap(c, x - 2.0, y, false, NEUTRAL);
                c.fill(x + 3.0, y, w - 6.0, H, NEUTRAL);
                self.cap(c, x + w - 3.0, y, true, NEUTRAL);
                c.string(x + 3.0, y + 1.0, &t);
                w
            }
            Shape::Dpad(lit) => {
                let a = 10.0 / 3.0;
                let arm = |c: &mut Canvas, cx: f32, cy: f32, on: bool| {
                    c.fill(x + cx * a, y + cy * a, a, a, if on { [0.85, 0.6, 0.2, 1.0] } else { [0.45, 0.45, 0.48, 1.0] });
                };
                arm(c, 1.0, 0.0, lit[0]);
                arm(c, 1.0, 2.0, lit[1]);
                arm(c, 0.0, 1.0, lit[2]);
                arm(c, 2.0, 1.0, lit[3]);
                arm(c, 1.0, 1.0, false);
                H
            }
            Shape::Key(t) => {
                let w = t.len() as f32 * 8.0 + 6.0;
                c.fill(x, y, w, H, KEY_BG);
                c.frame(x, y, w, H, TAG_EDGE);
                c.string(x + 3.0, y + 1.0, &t);
                w
            }
        }
    }

    /// A hint row: glyph, label, glyph, label... Returns the total width.
    /// With `draw` false only measures.
    pub fn hints(&self, c: &mut Canvas, x: f32, y: f32, items: &[(i32, &str)], draw: bool) -> f32 {
        let mut cx = x;
        for (i, (k, label)) in items.iter().enumerate() {
            if i > 0 {
                cx += 12.0;
            }
            cx += if draw { self.key(c, cx, y, *k) } else { width(*k, self.family) };
            cx += 4.0;
            if draw {
                c.string(cx, y + 1.0, label);
            }
            cx += label.len() as f32 * 8.0;
        }
        cx - x
    }

    /// Hint row centred on `cx`.
    pub fn hints_centered(&self, c: &mut Canvas, cx: f32, y: f32, items: &[(i32, &str)]) {
        let w = self.hints(c, 0.0, y, items, false);
        self.hints(c, (cx - w / 2.0).floor(), y, items, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_follow_family() {
        assert!(matches!(pad_shape(K_ABUTTON, Family::Nintendo), Some(Shape::Face(_, Ok("B")))));
        assert!(matches!(pad_shape(K_ABUTTON, Family::Xbox), Some(Shape::Face(_, Ok("A")))));
        assert!(matches!(pad_shape(K_YBUTTON, Family::PlayStation), Some(Shape::Face(_, Err(Sym::Triangle)))));
        assert!(matches!(pad_shape(K_RTRIGGER, Family::PlayStation), Some(Shape::Tag(t)) if t == "R2"));
        assert!(matches!(pad_shape(K_LTRIGGER, Family::Nintendo), Some(Shape::Tag(t)) if t == "ZL"));
        assert!(pad_shape(b'w' as i32, Family::Xbox).is_none());
        assert_eq!(key_name(b'w' as i32), "W");
    }
    #[test]
    fn atlas_has_shapes() {
        let (w, h, px) = atlas_rgba();
        assert_eq!(px.len(), (w * h * 4) as usize);
        // disc centre opaque, cell corner transparent
        assert_eq!(px[((CELL / 2 * w + CELL / 2) * 4 + 3) as usize], 255);
        assert_eq!(px[3], 0);
    }
}
