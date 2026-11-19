//! The HUD layout: a list of elements, each at an anchor point of the
//! screen plus an offset, kept in the archived `hud_layout` cvar.
//!
//! Text form: elements separated by spaces (`|` would start a shell pipe
//! on an unquoted console line); each is `kind@anchor` followed by
//! `,<key><value>` fields that differ from the kind's defaults:
//! `x`/`y` offset, `s` scale, `o` opacity, `f` font (b/q/m), `z` size,
//! `i` immersive, `g` background (0/1), `t`/`k` text and label colour
//! (crosshair palette index), `a`/`b` kind options, `l`
//! bar length. Example: `health@bl,x8,y-8 clock@tr,a3`. Unknown kinds and
//! fields are skipped, so older builds read newer layouts.

use super::kind::{Font, Kind};

/// One of nine screen points; the element's matching corner, edge
/// midpoint or centre sits there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft, Anchor::TopCenter, Anchor::TopRight, Anchor::MiddleLeft, Anchor::Center,
        Anchor::MiddleRight, Anchor::BottomLeft, Anchor::BottomCenter, Anchor::BottomRight,
    ];
    const IDS: [&'static str; 9] = ["tl", "tc", "tr", "ml", "mc", "mr", "bl", "bc", "br"];

    pub fn id(self) -> &'static str {
        Self::IDS[self as usize]
    }
    pub fn from_id(s: &str) -> Option<Self> {
        Self::IDS.iter().position(|&i| i == s).map(|i| Self::ALL[i])
    }
    /// 0 left, 1 centre, 2 right.
    pub fn col(self) -> usize {
        self as usize % 3
    }
    /// 0 top, 1 middle, 2 bottom.
    pub fn row(self) -> usize {
        self as usize / 3
    }
    /// The old 8-way `HudAnchor` cvar value (`hud_*_pos`).
    pub fn from_legacy(v: f32) -> Self {
        use Anchor::*;
        [BottomLeft, BottomCenter, BottomRight, TopLeft, TopCenter, TopRight, MiddleLeft, MiddleRight][(v.max(0.0) as usize).min(7)]
    }
    pub fn label(self) -> &'static str {
        ["top left", "top center", "top right", "middle left", "center", "middle right", "bottom left", "bottom center", "bottom right"]
            [self as usize]
    }
}

/// The element's own text colours (no palette colour picked).
pub const NORMAL: u8 = 255;

#[derive(Clone, Debug, PartialEq)]
pub struct Elem {
    pub kind: Kind,
    pub anchor: Anchor,
    /// Offset from the anchor in HUD units.
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub font: Font,
    /// Text size in units (bars: thickness).
    pub size: f32,
    /// Immersive: shown for a few seconds after the value changes.
    pub fade: bool,
    /// Dark backing behind text.
    pub bg: bool,
    /// Text and label colours: crosshair palette indices, `NORMAL` keeps
    /// the element's own colours.
    pub color: u8,
    pub label: u8,
    /// Kind options (see `Kind::opts`).
    pub a: u8,
    pub b: u8,
    /// Bar length in units.
    pub len: f32,
}

impl Elem {
    /// A new element with the kind's defaults.
    pub fn new(kind: Kind, anchor: Anchor) -> Self {
        use Kind::*;
        let font = kind.fonts().first().copied().unwrap_or(Font::Quake);
        let size = if kind.is_number() { 24.0 } else { 8.0 };
        let a = match kind {
            Health | Armor | Ammo | Shells | Nails | Rockets | Cells | HealthBar | ArmorBar | AmmoBar => 1,
            Weapons => 4,
            StatusBar | Clock => 1,
            Timer | Pb => 2,
            _ => 0,
        };
        let b = u8::from(kind == StatusBar);
        // a bottom status bar sits on the screen edge
        let y = if kind == StatusBar && anchor.row() == 2 { super::place::MARGIN } else { 0.0 };
        let bg = kind.has_background() && !kind.is_message();
        Elem { kind, anchor, x: 0.0, y, scale: 1.0, opacity: 1.0, font, size, fade: false, bg, color: NORMAL, label: NORMAL, a, b, len: 96.0 }
    }

    fn write(&self, out: &mut String) {
        let d = Elem::new(self.kind, self.anchor);
        out.push_str(self.kind.id());
        out.push('@');
        out.push_str(self.anchor.id());
        let mut num = |k: char, v: f32, dv: f32| {
            if v != dv {
                out.push(',');
                out.push(k);
                out.push_str(&qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
            }
        };
        num('x', self.x, d.x);
        num('y', self.y, d.y);
        num('s', self.scale, d.scale);
        num('o', self.opacity, d.opacity);
        num('z', self.size, d.size);
        num('g', f32::from(u8::from(self.bg)), f32::from(u8::from(d.bg)));
        num('a', f32::from(self.a), f32::from(d.a));
        num('b', f32::from(self.b), f32::from(d.b));
        num('l', self.len, d.len);
        num('t', f32::from(self.color), f32::from(d.color));
        num('k', f32::from(self.label), f32::from(d.label));
        if self.font != d.font {
            out.push_str(",f");
            out.push(self.font.id());
        }
        if self.fade {
            out.push_str(",i");
        }
    }

    fn parse(s: &str) -> Option<Self> {
        let mut fields = s.split(',');
        let (kind, anchor) = fields.next()?.trim().split_once('@')?;
        let mut e = Elem::new(Kind::from_id(kind)?, Anchor::from_id(anchor)?);
        for f in fields {
            let f = f.trim();
            let Some(k) = f.chars().next() else { continue };
            let v = &f[k.len_utf8()..];
            let n = v.parse::<f32>().ok().filter(|n| n.is_finite());
            let byte = |n: Option<f32>, old: u8| n.map_or(old, |n| n.clamp(0.0, 255.0) as u8);
            match k {
                'x' => e.x = n.map_or(e.x, |n| n.clamp(-4096.0, 4096.0)),
                'y' => e.y = n.map_or(e.y, |n| n.clamp(-4096.0, 4096.0)),
                's' => e.scale = n.map_or(e.scale, |n| n.clamp(0.25, 8.0)),
                'o' => e.opacity = n.map_or(e.opacity, |n| n.clamp(0.0, 1.0)),
                'z' => e.size = n.map_or(e.size, |n| n.clamp(2.0, 64.0)),
                'g' => e.bg = n.map_or(e.bg, |n| n != 0.0),
                'a' => e.a = byte(n, e.a),
                'b' => e.b = byte(n, e.b),
                'l' => e.len = n.map_or(e.len, |n| n.clamp(8.0, 1024.0)),
                't' => e.color = byte(n, e.color),
                'k' => e.label = byte(n, e.label),
                'f' => {
                    if let Some(font) = v.chars().next().and_then(Font::from_id).filter(|f| e.kind.fonts().contains(f)) {
                        e.font = font;
                    }
                }
                'i' => e.fade = e.kind.can_fade(),
                _ => {}
            }
        }
        Some(e)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub elems: Vec<Elem>,
}

impl Layout {
    pub fn parse(s: &str) -> Self {
        let mut l = Layout { elems: s.split_whitespace().filter_map(Elem::parse).collect() };
        l.ensure_permanent();
        l
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (i, e) in self.elems.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            e.write(&mut out);
        }
        out
    }

    /// Adds the messages when missing, and keeps one of each.
    pub fn ensure_permanent(&mut self) {
        for k in Kind::PERMANENT {
            let mut seen = false;
            self.elems.retain(|e| e.kind != k || !std::mem::replace(&mut seen, true));
            if !seen {
                self.elems.push(message(k, 0.0, 1.0, false));
            }
        }
    }

    /// The layout the old HUD settings describe (`hud_*`, `scr_hudstyle`,
    /// Time & Stats, metrics and message cvars). Elements sharing an
    /// anchor come out at offset zero; `place::stack` spreads them.
    pub fn from_legacy(cv: &dyn Fn(&str) -> f32) -> Self {
        let mut elems = Vec::new();
        let at = |name: &str| Anchor::from_legacy(cv(name));
        if cv("scr_hudstyle") == 0.0 {
            elems.push(Elem::new(Kind::StatusBar, Anchor::BottomCenter));
        } else {
            for (kind, name) in [
                (Kind::Health, "health"),
                (Kind::Armor, "armor"),
                (Kind::Ammo, "ammo"),
                (Kind::AmmoList, "ammolist"),
                (Kind::Weapons, "weapons"),
                (Kind::Items, "items"),
            ] {
                let mode = cv(&format!("hud_{name}")) as i32;
                if mode == 0 {
                    continue;
                }
                let mut e = Elem::new(kind, at(&format!("hud_{name}_pos")));
                e.fade = mode == 2;
                if kind == Kind::Health {
                    e.a = u8::from(cv("hud_face") != 0.0);
                }
                elems.push(e);
            }
        }
        // metrics nearest the edge, then Time & Stats, as before
        for (kind, name) in [
            (Kind::Fps, "scr_showfps"),
            (Kind::FrameTime, "scr_showframetime"),
            (Kind::Cpu, "scr_showcpu"),
            (Kind::Ram, "scr_showram"),
            (Kind::Gpu, "scr_showgpu"),
            (Kind::Vram, "scr_showvram"),
        ] {
            if cv(name) != 0.0 {
                elems.push(Elem::new(kind, at("scr_metrics_pos")));
            }
        }
        let clock = cv("hud_clock").clamp(0.0, 4.0) as u8;
        if clock > 0 {
            let mut e = Elem::new(Kind::Clock, at("hud_clock_pos"));
            e.a = clock;
            elems.push(e);
        }
        let timer = cv("hud_timer").clamp(0.0, 3.0) as u8;
        if timer > 0 {
            let mut e = Elem::new(Kind::Timer, at("hud_timer_pos"));
            e.a = timer - 1;
            elems.push(e);
            if cv("hud_pb") != 0.0 {
                let mut e = Elem::new(Kind::Pb, at("hud_timer_pos"));
                e.a = timer - 1;
                elems.push(e);
            }
        }
        for (kind, name) in [(Kind::Kills, "hud_kills"), (Kind::Secrets, "hud_secrets")] {
            let v = cv(name).clamp(0.0, 2.0) as u8;
            if v > 0 {
                let mut e = Elem::new(kind, at("hud_stats_pos"));
                e.a = v - 1;
                elems.push(e);
            }
        }
        let size = (cv("scr_textscale").clamp(1.0, 3.0).round() * 8.0).max(8.0);
        let bg = cv("scr_textbg") != 0.0;
        let center_y = match cv("scr_centerpos") as i32 {
            1 => 0.0,
            2 => 40.0,
            _ => -24.0,
        };
        let mut center = message(Kind::Center, center_y, size / 8.0, bg);
        center.anchor = Anchor::Center;
        let mut notify = message(Kind::Notify, 0.0, size / 8.0, bg);
        notify.anchor = match cv("con_notifypos") as i32 {
            1 => Anchor::TopCenter,
            2 => Anchor::MiddleLeft,
            _ => Anchor::TopLeft,
        };
        if notify.anchor == Anchor::MiddleLeft {
            notify.y = 24.0;
        }
        elems.extend([center, notify, message(Kind::Captions, 0.0, size / 8.0, false)]);
        Layout { elems }
    }

    /// The layout of a fresh install (the modern preset).
    pub fn default_layout() -> Self {
        Self::from_legacy(&legacy_default)
    }

    /// The classic preset: the full-width Quake status bar and the messages.
    pub fn classic_layout() -> Self {
        Self::from_legacy(&|n| if n == "scr_hudstyle" { 0.0 } else { legacy_default(n) })
    }
}

/// A message element; `scale` is the old text size multiplier.
fn message(kind: Kind, y: f32, scale: f32, bg: bool) -> Elem {
    let anchor = match kind {
        Kind::Center => Anchor::Center,
        Kind::Notify => Anchor::TopLeft,
        _ => Anchor::BottomCenter,
    };
    let mut e = Elem::new(kind, anchor);
    e.y = if kind == Kind::Captions { -36.0 } else { y };
    e.size = 8.0 * scale.round().clamp(1.0, 3.0);
    e.bg = bg;
    e
}

/// Defaults of the old HUD cvars (no longer archived; read once).
pub fn legacy_default(name: &str) -> f32 {
    match name {
        "scr_hudstyle" | "hud_health" | "hud_armor" | "hud_ammo" | "hud_ammolist" | "hud_weapons" | "hud_items"
        | "hud_face" | "hud_pb" | "scr_textscale" => 1.0,
        "hud_ammo_pos" | "hud_ammolist_pos" | "hud_weapons_pos" | "hud_items_pos" => 2.0,
        "hud_clock_pos" | "hud_timer_pos" | "hud_stats_pos" | "scr_metrics_pos" => 5.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_round_trips_and_skips_what_it_does_not_know() {
        let mut l = Layout::default_layout();
        let mut e = Elem::new(Kind::HealthBar, Anchor::TopRight);
        (e.x, e.y, e.scale, e.opacity, e.len, e.a, e.fade) = (-12.0, 30.5, 1.5, 0.6, 160.0, 0, true);
        e.font = Font::Modern;
        (e.color, e.label) = (3, 0);
        l.elems.push(e);
        let text = l.to_text();
        assert_eq!(Layout::parse(&text), l);
        let odd = format!("{text} bogus@tl,x3 clock@zz clock@tr,q9,s,a3");
        let back = Layout::parse(&odd);
        assert_eq!(back.elems.len(), l.elems.len() + 1);
        assert_eq!(back.elems.last().unwrap().a, 3);
        // fonts the kind can't use are refused
        assert_eq!(Layout::parse("face@tl,fm").elems[0].font, Elem::new(Kind::Face, Anchor::TopLeft).font);
    }

    #[test]
    fn messages_are_always_there_once() {
        let l = Layout::parse("center@tl center@br health@bl");
        let kinds: Vec<_> = l.elems.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [Kind::Center, Kind::Health, Kind::Notify, Kind::Captions]);
        assert_eq!(l.elems[0].anchor, Anchor::TopLeft);
    }

    #[test]
    fn legacy_settings_become_elements() {
        let d = Layout::default_layout();
        let kinds: Vec<_> = d.elems.iter().map(|e| e.kind).collect();
        use Kind::*;
        assert_eq!(kinds, [Health, Armor, Ammo, AmmoList, Weapons, Items, Center, Notify, Captions]);
        assert_eq!(d.elems[2].anchor, Anchor::BottomRight);
        let classic: Vec<_> = Layout::classic_layout().elems.iter().map(|e| e.kind).collect();
        assert_eq!(classic, [StatusBar, Center, Notify, Captions]);
        let cv = |n: &str| match n {
            "scr_hudstyle" => 0.0,
            "hud_timer" => 3.0,
            "hud_timer_pos" => 3.0,
            "hud_kills" => 2.0,
            "scr_showfps" => 1.0,
            "scr_textscale" => 2.0,
            "con_notifypos" => 1.0,
            n => legacy_default(n),
        };
        let l = Layout::from_legacy(&cv);
        let kinds: Vec<_> = l.elems.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [StatusBar, Fps, Timer, Pb, Kills, Center, Notify, Captions]);
        assert_eq!((l.elems[2].anchor, l.elems[2].a), (Anchor::TopLeft, 2));
        assert_eq!(l.elems[4].a, 1);
        assert_eq!(l.elems[5].size, 16.0);
        assert_eq!(l.elems[6].anchor, Anchor::TopCenter);
    }
}
