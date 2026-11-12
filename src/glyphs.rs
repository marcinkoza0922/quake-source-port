//! Button glyphs for prompts and the bindings menu. The label/colour table
//! follows padwight's (`src/info.rs` `button_glyph`): Xbox and PlayStation
//! face buttons are coloured discs, Nintendo's are neutral discs labelled by
//! position (south shows B), other buttons are rounded tags, the D-pad is a
//! small cross with the pressed arm lit. Keyboard keys are square tags.
//!
//! Shapes and labels are rasterised at the drawn pixel size ([`crate::font`]).

use qinput::keys::*;
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
/// Fraction of its slot a glyph fills (leaves a small gap at the edges).
const GLYPH_FILL: f32 = 0.9;

#[derive(Clone, Copy)]
enum Sym {
    Cross,
    Circle,
    Square,
    Triangle,
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

/// Label em size as a share of the glyph height (padwight: 13 px on 24).
const LABEL: f32 = 13.0 / 24.0;
const LABEL_FG: [f32; 4] = [0.79, 0.82, 0.85, 1.0];
/// Hint row: glyph height and label size.
pub const HINT_H: f32 = 13.0;
pub const HINT_TEXT: f32 = 9.0;
pub const HINT_FG: [f32; 4] = [0.86, 0.84, 0.80, 1.0];

/// Top of a text em box that centres capitals in a box `h` tall at `y`.
pub fn center_text_y(y: f32, h: f32, em: f32) -> f32 {
    y + h / 2.0 - 0.435 * em
}

/// Width of a key's glyph `h` tall, in virtual units.
pub fn width(c: &Canvas, key: i32, f: Family, h: f32) -> f32 {
    match shape(key, f) {
        Shape::Face(..) | Shape::Dpad(_) => h,
        Shape::Tag(t) | Shape::Key(t) => (c.text_w(&t, h * LABEL) + h * 14.0 / 24.0).max(h),
    }
}

/// Draws button glyphs (padwight's look: coloured discs for face buttons,
/// rounded tags for the rest), rasterised at the drawn pixel size.
#[derive(Clone, Copy, Default)]
pub struct Glyphs {
    pub family: Family,
}

impl Glyphs {
    /// Filled disc of diameter `s` with its top-left at (x, y).
    pub fn disc(&self, c: &mut Canvas, x: f32, y: f32, s: f32, color: [f32; 4]) {
        c.round_box(x, y, s, s, s / 2.0, color, None);
    }

    /// Draws `key` `H` tall with its top-left at (x, y); returns the width.
    pub fn key(&self, c: &mut Canvas, x: f32, y: f32, key: i32) -> f32 {
        self.key_h(c, x, y, key, H)
    }

    /// Draws `key` in a slot `h` tall; returns the slot width. The glyph is
    /// drawn slightly smaller, centred, so neighbours never touch.
    pub fn key_h(&self, c: &mut Canvas, x: f32, y: f32, key: i32, h: f32) -> f32 {
        let w = width(c, key, self.family, h);
        let ih = h * GLYPH_FILL;
        let iw = width(c, key, self.family, ih);
        self.key_raw(c, x + (w - iw) / 2.0, y + (h - ih) / 2.0, key, ih);
        w
    }

    fn key_raw(&self, c: &mut Canvas, x: f32, y: f32, key: i32, h: f32) -> f32 {
        let em = h * LABEL;
        let ty = center_text_y(y, h, em);
        match shape(key, self.family) {
            Shape::Face(fill, label) => {
                let fg = if fill == NEUTRAL { LABEL_FG } else { [1.0; 4] };
                c.round_box(x, y, h, h, h / 2.0, fill, Some([fg[0], fg[1], fg[2], 0.2]));
                let t = c.upx(h * 0.1).max(1);
                match label {
                    Ok(l) => c.text_center(x + h / 2.0, ty, l, em, fg),
                    Err(sym) => {
                        let sh = match sym {
                            Sym::Cross => crate::font::Shape::Cross { t },
                            Sym::Circle => crate::font::Shape::Ring { t },
                            Sym::Square => crate::font::Shape::Square { t },
                            _ => crate::font::Shape::Triangle { t },
                        };
                        c.shape(x, y, h, h, sh, [1.0; 4]);
                    }
                }
                h
            }
            Shape::Tag(t) | Shape::Key(t) => {
                let is_key = !is_pad_key(key);
                let w = width(c, key, self.family, h);
                let (bg, edge) = if is_key { (KEY_BG, TAG_EDGE) } else { (NEUTRAL, [LABEL_FG[0], LABEL_FG[1], LABEL_FG[2], 0.2]) };
                c.round_box(x, y, w, h, h * 6.0 / 24.0, bg, Some(edge));
                c.text_center(x + w / 2.0, ty, &t, em, LABEL_FG);
                w
            }
            Shape::Dpad(lit) => {
                let a = h / 3.0;
                let r = (h * 1.5 / 24.0).max(0.34);
                let arm = |c: &mut Canvas, cx: f32, cy: f32, on: bool| {
                    let col = if on { [0.85, 0.6, 0.2, 1.0] } else { [0.30, 0.31, 0.35, 0.95] };
                    c.round_box(x + cx * a, y + cy * a, a, a, r, col, Some([0.79, 0.82, 0.85, 0.35]));
                };
                arm(c, 1.0, 0.0, lit[0]);
                arm(c, 1.0, 2.0, lit[1]);
                arm(c, 0.0, 1.0, lit[2]);
                arm(c, 2.0, 1.0, lit[3]);
                arm(c, 1.0, 1.0, false);
                h
            }
        }
    }

    /// A hint row: glyph, label, glyph, label... Returns the total width.
    /// With `draw` false only measures.
    pub fn hints(&self, c: &mut Canvas, x: f32, y: f32, items: &[(i32, &str)], draw: bool) -> f32 {
        let mut cx = x;
        for (i, (k, label)) in items.iter().enumerate() {
            if i > 0 {
                cx += 14.0;
            }
            cx += if draw { self.key_h(c, cx, y, *k, HINT_H) } else { width(c, *k, self.family, HINT_H) };
            cx += 5.0;
            if draw {
                c.text(cx, center_text_y(y, HINT_H, HINT_TEXT), label, HINT_TEXT, HINT_FG);
            }
            cx += c.text_w(label, HINT_TEXT);
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
}
