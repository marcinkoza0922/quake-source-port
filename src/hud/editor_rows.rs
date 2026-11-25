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

/// Coordinate labels are the same in every language.
pub const AXES: [&str; 2] = ["X", "Y"];

/// True when `e` is still at its chosen anchor's default offset.
pub fn is_at_anchor(e: &Elem) -> bool {
    let d = Elem::new(e.kind, e.anchor);
    (e.x - d.x).abs() < 0.5 && (e.y - d.y).abs() < 0.5
}

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
    if k.has_visibility() {
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
        Prop::Show => {
            if e.hidden {
                "hidden".to_string()
            } else if e.fade && e.kind.can_fade() {
                "appear on change".to_string()
            } else {
                "always".to_string()
            }
        }
        Prop::Background => if e.bg { "on" } else { "off" }.to_string(),
        Prop::TextColor => choice_text(TEXT_COLOR, f32::from(e.color)),
        Prop::LabelColor => choice_text(TEXT_COLOR, f32::from(e.label)),
        Prop::Position => {
            if is_at_anchor(e) {
                e.anchor.label().to_string()
            } else {
                format!("({}, {})", num(e.x), num(e.y))
            }
        }
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
        Prop::Show => {
            if e.kind.can_fade() {
                let mode = if e.hidden { 2 } else if e.fade { 1 } else { 0 };
                let next = (mode + dir).rem_euclid(3);
                e.fade = next == 1;
                e.hidden = next == 2;
            } else {
                e.hidden = !e.hidden;
                e.fade = false;
            }
        }
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

impl super::editor::Editor {
    /// Typed text: preset name in Save As popup, or coordinate digits.
    pub fn text(&mut self, ch: char) {
        if let Some(super::editor::Popup::SaveAs { name, confirm_overwrite }) = &mut self.popup {
            if !ch.is_control() && ch != '\t' && name.chars().count() < super::presets::MAX_NAME_LEN {
                name.push(ch);
                *confirm_overwrite = false;
            }
            return;
        }
        if let Some((_, s)) = &mut self.typing
            && (ch.is_ascii_digit() || (ch == '-' && s.is_empty()))
            && s.len() < 6
        {
            s.push(ch);
        }
    }

    pub(super) fn type_key(&mut self, key: i32, (p, mut s): (Prop, String), l: &mut super::layout::Layout, cv: &mut Cvars) -> super::editor::Out {
        use qinput::keys::*;
        match key {
            K_BACKSPACE | K_XBUTTON => {
                s.pop();
                self.typing = Some((p, s));
                super::editor::Out::default()
            }
            K_ESCAPE | K_BBUTTON => super::editor::sound("misc/menu3.wav"),
            K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1 => {
                if let (Ok(v), Some(e)) = (s.parse::<f32>(), self.sel.and_then(|i| l.elems.get_mut(i))) {
                    let old = if p == Prop::X { e.x } else { e.y };
                    if old != v {
                        self.push_undo_now(cv);
                        if p == Prop::X { e.x = v } else { e.y = v }
                        self.save(cv, l);
                    }
                }
                if key == K_MOUSE1 {
                    return self.key(key, cv);
                }
                super::editor::sound("misc/menu2.wav")
            }
            _ => {
                self.typing = Some((p, s));
                super::editor::Out::default()
            }
        }
    }

    pub(super) fn step(&mut self, p: Prop, dir: i32, l: &mut super::layout::Layout, cv: &mut Cvars) -> super::editor::Out {
        let Some(i) = self.sel else { return super::editor::Out::default() };
        let snap = self.snapshot(cv);
        let Some(e) = l.elems.get_mut(i) else { return super::editor::Out::default() };
        if step(p, e, cv, dir) {
            self.push_undo(snap);
            self.save(cv, l);
        }
        super::editor::sound("misc/menu3.wav")
    }

    /// Enter or a click on a property row.
    pub(super) fn activate(&mut self, p: Prop, dir: i32, l: &mut super::layout::Layout, cv: &mut Cvars) -> super::editor::Out {
        match p {
            Prop::Position => self.open_grid(l),
            Prop::X | Prop::Y => {
                self.typing = Some((p, String::new()));
                super::editor::sound("misc/menu2.wav")
            }
            _ => self.step(p, dir, l, cv),
        }
    }
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
        // Visibility cycles always -> appear on change -> hidden -> always on player elements
        assert_eq!(value_text(Prop::Show, &h, &cv), "always");
        step(Prop::Show, &mut h, &mut cv, 1);
        assert_eq!(value_text(Prop::Show, &h, &cv), "appear on change");
        assert!(h.fade && !h.hidden);
        step(Prop::Show, &mut h, &mut cv, 1);
        assert_eq!(value_text(Prop::Show, &h, &cv), "hidden");
        assert!(!h.fade && h.hidden);
        step(Prop::Show, &mut h, &mut cv, 1);
        assert_eq!(value_text(Prop::Show, &h, &cv), "always");
        // Non-fade elements cycle always <-> hidden
        assert_eq!(value_text(Prop::Show, &e, &cv), "always");
        step(Prop::Show, &mut e, &mut cv, 1);
        assert_eq!(value_text(Prop::Show, &e, &cv), "hidden");
        // Position shows anchor name at default offset, and (x, y) once moved
        assert_eq!(value_text(Prop::Position, &h, &cv), "bottom left");
        h.x = 12.0;
        h.y = -8.0;
        assert_eq!(value_text(Prop::Position, &h, &cv), "(12, -8)");
    }
}
