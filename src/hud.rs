//! The HUD: a free layout of elements (`hud_layout`), drawn over the 3D
//! view, and its editor (Options → HUD → Edit HUD).
//!
//! Every element sits at one of nine anchor points plus an offset, with
//! its own scale, opacity, font and text size; elements never overlap
//! (one that would moves to the nearest free spot). The center message,
//! notify lines and sound captions are permanent elements: they can be
//! moved but not removed.

pub mod draw;
mod draw_more;
pub mod editor;
mod editor_draw;
mod editor_mouse;
mod editor_pad;
mod editor_popup;
mod editor_rows;
pub mod kind;
pub mod layout;
pub mod place;

use qclient::ClientState;
use qnet::protocol::*;

use crate::draw::Canvas;
use crate::sbar::{PicFn, Sbar};
use kind::Kind;
use layout::{Elem, Layout};
use place::Rect;

/// Seconds an immersive element stays fully visible, then fades.
const IMMERSIVE_HOLD: f64 = 3.0;
const IMMERSIVE_FADE: f64 = 0.5;

/// Change tracking for immersive elements, per layout element.
#[derive(Default)]
pub struct Fade {
    kinds: Vec<Kind>,
    last: Vec<Option<i64>>,
    changed: Vec<f64>,
    addons: Option<(Vec<crate::wheel::Entry>, Option<qserver::addons::Toy>)>,
}

impl Fade {
    fn signature(cl: &ClientState, k: Kind) -> i64 {
        let st = &cl.stats;
        let it = i64::from(cl.items) & 0xffff_ffff;
        match k {
            Kind::Health | Kind::HealthBar | Kind::Face => i64::from(st[STAT_HEALTH]),
            Kind::Armor | Kind::ArmorBar => i64::from(st[STAT_ARMOR]) << 32 | it,
            Kind::Ammo | Kind::AmmoBar => i64::from(st[STAT_AMMO]) << 32 | i64::from(st[STAT_ACTIVEWEAPON]) & 0xffff_ffff,
            Kind::AmmoList => (0..4).fold(0i64, |h, i| h << 16 | (i64::from(st[STAT_SHELLS + i]) & 0xffff)),
            Kind::Shells | Kind::Nails | Kind::Rockets | Kind::Cells => i64::from(st[STAT_SHELLS + (k as usize - Kind::Shells as usize)]),
            Kind::Weapons => it << 32 ^ i64::from(st[STAT_ACTIVEWEAPON]),
            _ => it,
        }
    }

    /// Records value changes; the first sighting counts as a change.
    pub fn update(&mut self, layout: &Layout, cl: &ClientState, addons: (&[crate::wheel::Entry], Option<qserver::addons::Toy>), now: f64) {
        let kinds: Vec<Kind> = layout.elems.iter().map(|e| e.kind).collect();
        if kinds != self.kinds {
            self.last = vec![None; kinds.len()];
            self.changed = vec![now; kinds.len()];
            self.kinds = kinds;
        }
        let addons_changed = self.addons.as_ref().is_none_or(|(a, h)| a.as_slice() != addons.0 || *h != addons.1);
        if addons_changed {
            self.addons = Some((addons.0.to_vec(), addons.1));
        }
        for (i, &k) in self.kinds.iter().enumerate() {
            let s = Self::signature(cl, k);
            if self.last[i] != Some(s) || (addons_changed && k == Kind::Weapons) {
                self.last[i] = Some(s);
                self.changed[i] = now;
            }
        }
    }

    /// Opacity of element `i` (`e`) now.
    pub fn alpha(&self, i: usize, e: &Elem, cl: &ClientState, now: f64) -> f32 {
        if !e.fade || !e.kind.can_fade() {
            return 1.0;
        }
        // low health always shows
        if matches!(e.kind, Kind::Health | Kind::HealthBar | Kind::Face) && cl.stats[STAT_HEALTH] <= 25 {
            return 1.0;
        }
        let t = now - self.changed.get(i).copied().unwrap_or(now);
        if t < IMMERSIVE_HOLD { 1.0 } else { (1.0 - (t - IMMERSIVE_HOLD) / IMMERSIVE_FADE).max(0.0) as f32 }
    }
}

/// Layout text, element sizes, screen size and the element that gives way.
type PlaceKey = (String, Vec<(f32, f32)>, (f32, f32), Option<usize>);

/// The parsed `hud_layout` and where its elements went last frame.
#[derive(Default)]
pub struct Hud {
    /// The cvar text `layout` was read from.
    pub src: String,
    pub layout: Layout,
    pub fade: Fade,
    /// Placement inputs of the main view's `rects`.
    key: Option<PlaceKey>,
    pub sizes: Vec<(f32, f32)>,
    pub rects: Vec<Rect>,
    /// `hud_preset` as last applied (None until the configs have run).
    pub preset: Option<f32>,
    /// The loaded layout was checked against its preset.
    pub preset_checked: bool,
}

impl Hud {
    /// Re-reads the layout when the cvar text changed. Returns false when
    /// it is empty, "default" or "classic", which the host turns into a
    /// layout first.
    pub fn sync(&mut self, text: &str) -> bool {
        if matches!(text, "" | "default" | "classic") {
            return false;
        }
        if text != self.src {
            self.layout = Layout::parse(text);
            self.src = text.to_string();
        }
        true
    }

    /// Places the elements (cached); `last` gives way to the others.
    pub fn place(&mut self, sizes: Vec<(f32, f32)>, vw: f32, vh: f32, last: Option<usize>) -> &[Rect] {
        let key = (self.src.clone(), sizes, (vw, vh), last);
        if self.key.as_ref() != Some(&key) {
            self.rects = place::place_all(&self.layout.elems, &key.1, vw, vh, last);
            self.sizes.clone_from(&key.1);
            self.key = Some(key);
        }
        &self.rects
    }

    /// Element offsets that put them where they were placed: written back
    /// while editing, so the editor shows real coordinates. None when
    /// nothing moved.
    pub fn settled(&self, vw: f32, vh: f32) -> Option<Layout> {
        let mut l = self.layout.clone();
        let mut moved = false;
        for (e, r) in l.elems.iter_mut().zip(&self.rects) {
            let (x, y) = place::offset_for(e, *r, vw, vh);
            if (x, y) != (e.x, e.y) {
                (e.x, e.y) = (x, y);
                moved = true;
            }
        }
        moved.then_some(l)
    }

    /// See [`reserve_bottom`].
    pub fn reserve_bottom(&self, vh: f32) -> f32 {
        reserve_bottom(&self.layout, &self.rects, vh)
    }

    /// The rectangle of the permanent element `k`.
    pub fn rect_of(&self, k: Kind) -> Option<(&Elem, Rect)> {
        self.layout.elems.iter().zip(&self.rects).find(|(e, _)| e.kind == k).map(|(e, r)| (e, *r))
    }
}

/// Rows taken by a full-width status bar at the bottom of the screen
/// (the 3D view ends above it), in HUD units.
pub fn reserve_bottom(l: &Layout, rects: &[Rect], vh: f32) -> f32 {
    l.elems
        .iter()
        .zip(rects)
        .filter(|(e, r)| e.kind == Kind::StatusBar && e.b != 0 && r.bottom() >= vh - 0.5)
        .map(|(_, r)| vh - r.y)
        .fold(0.0, f32::max)
}

/// Where the console draws the center message or notify lines: a
/// rectangle in HUD units, aligned to the element's anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MsgPlace {
    pub rect: Rect,
    /// 0 left, 1 centre, 2 right.
    pub align: usize,
    /// Lines grow upward from the bottom (bottom anchors).
    pub up: bool,
    /// Text size multiplier (1..=3).
    pub scale: f32,
    pub bg: bool,
    pub color: [f32; 4],
}

impl Default for MsgPlace {
    fn default() -> Self {
        MsgPlace { rect: Rect::default(), align: 0, up: false, scale: 1.0, bg: false, color: crate::draw::WHITE }
    }
}

impl MsgPlace {
    pub fn of(e: &Elem, rect: Rect) -> Self {
        let scale = (e.size / 8.0).round().clamp(1.0, 3.0);
        MsgPlace { rect, align: e.anchor.col(), up: e.anchor.row() == 2, scale, bg: e.bg, color: draw::value_color(e) }
    }
}

/// Draws the elements `keep` accepts. Player elements hide with the
/// scoreboard and at death (the scoreboard then shows at the top unless a
/// status bar shows it), Time & Stats with the scoreboard.
#[allow(clippy::too_many_arguments)]
pub fn draw_layout(c: &mut Canvas, pf: &mut PicFn, l: &Layout, rects: &[Rect], ctx: &draw::Ctx, fade: Option<&Fade>, now: f64, keep: impl Fn(Kind) -> bool) {
    c.reset_origin();
    let hide_player = !ctx.preview && (ctx.show_scores || ctx.cl.stats[STAT_HEALTH] <= 0);
    let mut status_bar = false;
    for (i, (e, r)) in l.elems.iter().zip(rects).enumerate() {
        if !keep(e.kind) {
            continue;
        }
        if e.kind == Kind::StatusBar {
            status_bar = true;
        } else if (e.kind.is_player() && hide_player) || (ctx.show_scores && !e.kind.is_player() && !e.kind.is_metric()) {
            continue;
        }
        let a = if ctx.preview { 1.0 } else { fade.map_or(1.0, |f| f.alpha(i, e, ctx.cl, now)) };
        if a > 0.0 {
            draw::draw(c, pf, e, *r, ctx, a);
        }
    }
    if hide_player && !status_bar && keep(Kind::StatusBar) {
        Sbar::new(c, pf, ctx.pack).draw_scores_top(ctx.cl);
    }
}

/// A player with a bit of everything, for the editor's preview.
pub fn sample_client(cl: &ClientState) -> ClientState {
    let mut s = ClientState::default();
    (s.time, s.maxclients) = (cl.time, 1);
    let st = &mut s.stats;
    st[STAT_HEALTH] = 100;
    st[STAT_ARMOR] = 150;
    st[STAT_AMMO] = 25;
    st[STAT_ACTIVEWEAPON] = 2;
    for (i, n) in [25, 120, 15, 60].into_iter().enumerate() {
        st[STAT_SHELLS + i] = n;
    }
    s.items = 4096 | 127 | crate::sbar::IT_SHELLS | crate::sbar::IT_ARMOR2 | 1 << 17 | 1 << 18 | crate::sbar::IT_QUAD | 1 << 28 | 1 << 29;
    s
}
