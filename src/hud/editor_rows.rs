//! The HUD editor's property rows: which an element has, their text, and
//! stepping their values.

use qcore::cvar::Cvars;

use super::kind::{Field, Kind, TEXT_COLOR, Values};
use super::layout::Elem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    Font,
    Size,
    Scale,
    Opacity,
    Show,
    Background,
    TextColor,
    LabelColor,
    Position,
    X,
    Y,
    /// k-th of `Kind::opts`.
    Opt(usize),
}

const SHOW: &[(&str, f32)] = &[("always", 0.0), ("immersive", 1.0)];
/// Coordinate labels are the same in every language.
pub const AXES: [&str; 2] = ["X", "Y"];

/// The rows `k` has, in order.
pub fn props(k: Kind) -> Vec<Prop> {
    let mut out = Vec::new();
    if k.fonts().len() > 1 {
        out.push(Prop::Font);
    }
    if !k.sizes().is_empty() {
        out.push(Prop::Size);
    }
    if !k.is_message() {
        out.extend([Prop::Scale, Prop::Opacity]);
    }
    if k.can_fade() {
        out.push(Prop::Show);
    }
    if k.has_background() {
        out.push(Prop::Background);
    }
    if k.has_text_color() {
        out.push(Prop::TextColor);
    }
    if k.has_label() {
        out.push(Prop::LabelColor);
    }
    out.extend([Prop::Position, Prop::X, Prop::Y]);
    out.extend((0..k.opts().len()).map(Prop::Opt));
    out
}

pub fn label(p: Prop, k: Kind) -> &'static str {
    match p {
        Prop::Font => qcore::t!("Font"),
        Prop::Size => qcore::t!("Size"),
        Prop::Scale => qcore::t!("Scale"),
        Prop::Opacity => qcore::t!("Opacity"),
        Prop::Show => qcore::t!("Visibility"),
        Prop::Background => qcore::t!("Background"),
        Prop::TextColor => qcore::t!("Text Color"),
        Prop::LabelColor => qcore::t!("Label Color"),
        Prop::Position => qcore::t!("Position"),
        Prop::X => AXES[0],
        Prop::Y => AXES[1],
        Prop::Opt(i) => k.opts()[i].label,
    }
}

fn num(v: f32) -> String {
    qcore::cvar::format_float((v * 100.0).round() / 100.0)
}

fn choice_text(choices: &[(&'static str, f32)], v: f32) -> String {
    choices.iter().min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs())).map_or_else(String::new, |c| c.0.to_string())
}

fn opt_value(f: Field, e: &Elem, cv: &Cvars) -> f32 {
    match f {
        Field::A => f32::from(e.a),
        Field::B => f32::from(e.b),
        Field::Len => e.len,
        Field::Cvar(name) => cv.value_of(name),
    }
}

/// The value as shown (choice names are translated where drawn).
pub fn value_text(p: Prop, e: &Elem, cv: &Cvars) -> String {
    match p {
        Prop::Font => e.font.label().to_string(),
        Prop::Size => num(e.size),
        Prop::Scale => format!("{}x", num(e.scale)),
        Prop::Opacity => format!("{}%", (e.opacity * 100.0).round()),
        Prop::Show => SHOW[usize::from(e.fade)].0.to_string(),
        Prop::Background => if e.bg { "on" } else { "off" }.to_string(),
        Prop::TextColor => choice_text(TEXT_COLOR, f32::from(e.color)),
        Prop::LabelColor => choice_text(TEXT_COLOR, f32::from(e.label)),
        Prop::Position => e.anchor.label().to_string(),
        Prop::X => num(e.x),
        Prop::Y => num(e.y),
        Prop::Opt(i) => {
            let o = e.kind.opts()[i];
            let v = opt_value(o.field, e, cv);
            match o.values {
                Values::Choice(ch) => choice_text(ch, v),
                Values::Range(..) => num(v),
            }
        }
    }
}

/// Next (`dir` 1) or previous (-1) of `choices` after `v`, wrapping.
fn cycle(choices: &[f32], v: f32, dir: i32) -> f32 {
    let i = choices.iter().enumerate().min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs())).map_or(0, |(i, _)| i);
    let n = choices.len() as i32;
    choices[(i as i32 + dir).rem_euclid(n) as usize]
}

fn range(v: f32, (min, max, step): (f32, f32, f32), dir: i32) -> f32 {
    ((v / step).round() * step + step * dir as f32).clamp(min, max)
}

/// Steps the value; true when the layout (not a cvar) changed.
/// Position is picked from its grid instead.
pub fn step(p: Prop, e: &mut Elem, cv: &mut Cvars, dir: i32) -> bool {
    match p {
        Prop::Font => {
            let fonts = e.kind.fonts();
            let i = fonts.iter().position(|&f| f == e.font).unwrap_or(0) as i32;
            e.font = fonts[(i + dir).rem_euclid(fonts.len() as i32) as usize];
        }
        Prop::Size => {
            let sizes: Vec<f32> = e.kind.sizes().iter().map(|s| s.1).collect();
            e.size = cycle(&sizes, e.size, dir);
        }
        Prop::Scale => e.scale = range(e.scale, (0.25, 4.0, 0.25), dir),
        Prop::Opacity => e.opacity = range(e.opacity, (0.1, 1.0, 0.1), dir),
        Prop::Show => e.fade = !e.fade,
        Prop::Background => e.bg = !e.bg,
        Prop::TextColor | Prop::LabelColor => {
            let field = if p == Prop::TextColor { &mut e.color } else { &mut e.label };
            let values: Vec<f32> = TEXT_COLOR.iter().map(|c| c.1).collect();
            *field = cycle(&values, f32::from(*field), dir) as u8;
        }
        Prop::Position => return false,
        Prop::X => e.x += dir as f32,
        Prop::Y => e.y += dir as f32,
        Prop::Opt(i) => {
            let o = e.kind.opts()[i];
            let v = opt_value(o.field, e, cv);
            let nv = match o.values {
                Values::Choice(ch) => cycle(&ch.iter().map(|c| c.1).collect::<Vec<_>>(), v, dir),
                Values::Range(min, max, st) => range(v, (min, max, st), dir),
            };
            match o.field {
                Field::A => e.a = nv as u8,
                Field::B => e.b = nv as u8,
                Field::Len => e.len = nv,
                Field::Cvar(name) => {
                    cv.set(name, &qcore::cvar::format_float(nv));
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::layout::Anchor;

    #[test]
    fn rows_follow_the_kind() {
        let p = props(Kind::Health);
        assert_eq!(&p[..5], [Prop::Font, Prop::Size, Prop::Scale, Prop::Opacity, Prop::Show]);
        assert_eq!(*p.last().unwrap(), Prop::Opt(0));
        let m = props(Kind::Center);
        assert!(!m.contains(&Prop::Scale) && m.contains(&Prop::Background) && m.contains(&Prop::Position));
        assert!(!props(Kind::Weapons).contains(&Prop::Font));
    }

    #[test]
    fn values_step_and_wrap() {
        let mut cv = Cvars::new();
        cv.register("con_notifytime", "3", 0);
        let mut e = Elem::new(Kind::Clock, Anchor::TopRight);
        assert!(step(Prop::Opt(0), &mut e, &mut cv, -1));
        assert_eq!(e.a, 4);
        assert_eq!(value_text(Prop::Opt(0), &e, &cv), "12-hour + seconds");
        assert!(step(Prop::Scale, &mut e, &mut cv, 1));
        assert_eq!(value_text(Prop::Scale, &e, &cv), "1.25x");
        for _ in 0..20 {
            step(Prop::Opacity, &mut e, &mut cv, -1);
        }
        assert_eq!(value_text(Prop::Opacity, &e, &cv), "10%");
        let mut n = Elem::new(Kind::Notify, Anchor::TopLeft);
        assert!(!step(Prop::Opt(0), &mut n, &mut cv, 1));
        assert_eq!(cv.value_of("con_notifytime"), 4.0);
        // text colours: the element's own first, then the palette
        assert_eq!(value_text(Prop::TextColor, &e, &cv), "normal");
        step(Prop::TextColor, &mut e, &mut cv, 1);
        assert_eq!((e.color, value_text(Prop::TextColor, &e, &cv).as_str()), (6, "grey"));
        assert!(props(Kind::Fps).contains(&Prop::LabelColor) && !props(Kind::Clock).contains(&Prop::LabelColor));
        assert!(!props(Kind::Pb).contains(&Prop::TextColor) && !props(Kind::Face).contains(&Prop::TextColor));
        let mut h = Elem::new(Kind::Health, Anchor::BottomLeft);
        step(Prop::Font, &mut h, &mut cv, 1);
        assert_eq!(value_text(Prop::Font, &h, &cv), "Quake");
    }
}
