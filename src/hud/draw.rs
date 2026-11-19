//! Sizes and drawing of HUD elements. Each element is drawn in its own
//! units at its rectangle (its scale applied), then faded as a whole.
//! The space an element takes is fixed by its settings (the widest value
//! it can show), so a layout without overlaps stays that way in play.

use qclient::ClientState;
use qnet::protocol::*;
use qrender::Draw2D;

use super::kind::{Font, Kind};
use super::layout::Elem;
use super::place::Rect;
use crate::draw::Canvas;
use crate::sbar::time::{self, LevelInfo};
use crate::sbar::{Pack, PicFn, Sbar};

pub(super) const LABEL: [f32; 4] = [0.85, 0.7, 0.35, 1.0];
pub(super) const VALUE: [f32; 4] = crate::draw::WHITE;
const ALERT: [f32; 4] = [1.0, 0.35, 0.3, 1.0];
pub(super) const BACKING: [f32; 4] = [0.0, 0.0, 0.0, 0.45];
/// Metric labels and the widest values they reserve room for.
const METRICS: [(&str, &str); 6] = [
    ("FPS", "8888"),
    ("Frame", "88.8 ms, max 888.8"),
    ("CPU", "888%"),
    ("RAM", "88888 MB"),
    ("GPU", "88.8 ms"),
    ("VRAM", "88888/88888 MB"),
];
pub(super) const AMMO_PICS: [&str; 4] = ["sb_shells", "sb_nails", "sb_rocket", "sb_cells"];

/// Automap state passed to the `Automap` HUD element.
pub struct AutomapCtx<'a> {
    pub map: &'a crate::automap::Automap,
    pub settings: crate::automap::draw::Settings,
    pub player_pos: qcore::math::Vec3,
    pub player_yaw: f32,
    /// Single player and co-op only (disabled in deathmatch).
    pub allowed: bool,
}

/// What the elements show this frame.
pub struct Ctx<'a> {
    pub cl: &'a ClientState,
    pub pack: Pack,
    pub face_anim: bool,
    pub teamplay: f32,
    pub addon_weapons: &'a [crate::wheel::Entry],
    pub held_addon: Option<qserver::addons::Toy>,
    pub info: &'a LevelInfo,
    /// Values of the metric kinds, `Kind::Fps` order.
    pub metrics: &'a [String; 6],
    pub pb_full: bool,
    pub pb_color: [f32; 4],
    pub show_scores: bool,
    /// The HUD editor: everything drawn, sample text for the messages.
    pub preview: bool,
    pub automap: Option<AutomapCtx<'a>>,
}

/// What an element's size depends on besides its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub pack: Pack,
    /// Add-on weapons that can show in the weapon grid.
    pub addons: usize,
    /// HUD width (messages are no wider).
    pub vw: f32,
}

/// The element's own scale (messages are sized by their text size).
pub fn elem_scale(e: &Elem) -> f32 {
    if e.kind.is_message() { 1.0 } else { e.scale }
}

fn metric_index(k: Kind) -> usize {
    k as usize - Kind::Fps as usize
}

/// Palette colour `i`, or `normal` for the element's own.
pub fn palette(i: u8, normal: [f32; 4]) -> [f32; 4] {
    if i == super::layout::NORMAL {
        return normal;
    }
    crate::reticle::CROSSHAIR_COLORS[usize::from(i).min(crate::reticle::CROSSHAIR_COLORS.len() - 1)]
}

/// The element's value (text) colour.
pub fn value_color(e: &Elem) -> [f32; 4] {
    palette(e.color, VALUE)
}

fn widest(s: String) -> String {
    s.chars().map(|c| if c.is_ascii_digit() { '8' } else { c }).collect()
}

pub fn text_w(c: &Canvas, font: Font, size: f32, s: &str) -> f32 {
    match font {
        Font::Modern => c.text_w_raw(s, size),
        Font::Quake => c.qtext_w(&qcore::text::encode(s)) * size / 8.0,
        Font::Big => s.chars().count() as f32 * size,
    }
}

/// Runs `f` with the canvas origin at (x, y) and units `k` times larger.
fn scoped<R>(c: &mut Canvas, x: f32, y: f32, k: f32, f: impl FnOnce(&mut Canvas) -> R) -> R {
    let saved = (c.scale, c.ox, c.oy);
    c.scale *= k;
    c.ox = (saved.1 + x) / k;
    c.oy = (saved.2 + y) / k;
    let r = f(c);
    (c.scale, c.ox, c.oy) = saved;
    r
}

/// Picture `name` stretched over (x, y, w, h); false when it is missing.
pub(super) fn pic_box(c: &mut Canvas, pf: &mut PicFn, name: &str, [x, y, w, h]: [f32; 4], color: [f32; 4]) -> bool {
    let Some((pic, _, _)) = pf(name) else { return false };
    let s = c.scale;
    c.push(Draw2D::Pic { x: (x + c.ox) * s, y: (y + c.oy) * s, w: w * s, h: h * s, pic, uv: [0.0, 0.0, 1.0, 1.0], color });
    true
}

/// Picture at its own size times `k`; returns its scaled width.
pub(super) fn pic_at(c: &mut Canvas, pf: &mut PicFn, name: &str, x: f32, y: f32, k: f32) -> f32 {
    let Some((_, w, h)) = pf(name) else { return 0.0 };
    let (w, h) = (w as f32 * k, h as f32 * k);
    pic_box(c, pf, name, [x, y, w, h], crate::draw::WHITE);
    w
}

/// Text in `font` with its line top at (x, y); `alert` is the red count.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(c: &mut Canvas, pf: &mut PicFn, font: Font, size: f32, (x, y): (f32, f32), s: &str, color: [f32; 4], alert: bool) -> f32 {
    let color = if alert && font != Font::Big { ALERT } else { color };
    match font {
        Font::Modern => c.text_raw(x, y, s, size, color),
        Font::Quake => scoped(c, x, y, size / 8.0, |c| c.qtext(0.0, 0.0, &qcore::text::encode(s), color)) * size / 8.0,
        Font::Big => {
            let prefix = if alert { "anum_" } else { "num_" };
            for (i, ch) in s.chars().enumerate() {
                let cx = x + i as f32 * size;
                let name = match ch {
                    '0'..='9' => format!("{prefix}{ch}"),
                    '-' => format!("{prefix}minus"),
                    _ => String::new(),
                };
                if name.is_empty() || !pic_box(c, pf, &name, [cx, y, size, size], color) {
                    scoped(c, cx, y, size / 8.0, |c| c.qtext(0.0, 0.0, &[ch as u8], color));
                }
            }
            s.chars().count() as f32 * size
        }
    }
}

/// Multiplies the opacity of `list`'s draws by `a`.
fn fade_list(list: &mut [Draw2D], a: f32) {
    for d in list {
        if let Draw2D::Pic { color, .. } | Draw2D::Char { color, .. } | Draw2D::Fill { color, .. } | Draw2D::Line { color, .. } = d {
            color[3] *= a;
        }
    }
}

/// x of content `w` wide in a box `room` wide, for the anchor column.
pub(super) fn align(col: usize, room: f32, w: f32) -> f32 {
    match col {
        0 => 0.0,
        1 => ((room - w) / 2.0).floor(),
        _ => room - w,
    }
}

type Spans = Vec<(String, [f32; 4])>;

/// Text of a text element; `sample` gives the widest it gets. None when
/// it shows nothing now (deathmatch stats, no personal best).
fn spans(e: &Elem, ctx: Option<&Ctx>, sample: bool) -> Option<Spans> {
    let (value, label) = (value_color(e), palette(e.label, LABEL));
    let recolor = |s: Spans| s.into_iter().map(|(t, c)| (t, if c == VALUE { value } else if c == LABEL { label } else { c })).collect();
    spans_raw(e, ctx, sample).map(recolor)
}

fn spans_raw(e: &Elem, ctx: Option<&Ctx>, sample: bool) -> Option<Spans> {
    let info = ctx.map(|c| c.info);
    let preview = ctx.is_none_or(|c| c.preview);
    let dm = info.is_some_and(|i| i.deathmatch) && !preview;
    let dec = e.a.min(2);
    let v = |s: String| vec![(s, VALUE)];
    Some(match e.kind {
        Kind::Clock => {
            let fmt = e.a.clamp(1, 4);
            if sample { v(widest(time::format_clock((12, 59, 59), fmt))) } else { v(time::format_clock(info?.clock, fmt)) }
        }
        Kind::Timer => v(if sample { widest(time::format_timer(3599.99, dec)) } else { time::format_timer(info?.time, dec) }),
        Kind::Pb => {
            let full = ctx.is_some_and(|c| c.pb_full);
            let color = ctx.map_or(VALUE, |c| c.pb_color);
            let label = if full { "PB 100% " } else { "PB " };
            let t = if sample {
                widest(time::format_timer(3599.99, dec))
            } else {
                let pb = if preview { info?.pb.unwrap_or(None) } else { info?.pb? };
                pb.map_or_else(|| "--:--".to_string(), |t| time::format_timer(f64::from(t), dec))
            };
            if dm {
                return None;
            }
            vec![(label.to_string(), color), (t, color)]
        }
        Kind::Kills | Kind::Secrets => {
            if dm {
                return None;
            }
            let label = if e.kind == Kind::Kills { "K " } else { "S " };
            let n = if sample {
                if e.a == 1 { "100%".into() } else { "888/888".into() }
            } else {
                let i = info?;
                time::count(e.a + 1, if e.kind == Kind::Kills { i.kills } else { i.secrets })
            };
            vec![(label.to_string(), LABEL), (n, VALUE)]
        }
        k if k.is_metric() => {
            let (label, wide) = METRICS[metric_index(k)];
            let value = if sample { wide.to_string() } else { ctx?.metrics[metric_index(k)].clone() };
            vec![(format!("{label} "), LABEL), (value, VALUE)]
        }
        _ => return None,
    })
}

fn spans_w(c: &Canvas, e: &Elem, s: &Spans) -> f32 {
    s.iter().map(|(t, _)| text_w(c, e.font, e.size, t)).sum()
}

fn text_pad(size: f32) -> (f32, f32) {
    ((size * 3.0 / 8.0).round(), (size / 4.0).round())
}

/// The icon and count of a number element; None when it has nothing to show.
fn number(e: &Elem, ctx: &Ctx) -> Option<(Option<String>, i32, bool)> {
    let (cl, st) = (ctx.cl, &ctx.cl.stats);
    let (icon, v, alert) = match e.kind {
        Kind::Health => (Some(Sbar::face_name(cl, ctx.face_anim)), st[STAT_HEALTH], st[STAT_HEALTH] <= 25),
        Kind::Armor => {
            let pic = ctx.pack.armor_pic(cl.items).or(ctx.preview.then_some("sb_armor1"))?;
            if cl.items & crate::sbar::IT_INVULNERABILITY != 0 {
                (Some(pic.to_string()), 666, true)
            } else {
                (Some(pic.to_string()), st[STAT_ARMOR], st[STAT_ARMOR] <= 25)
            }
        }
        Kind::Ammo => {
            let pic = ctx.pack.ammo_pic(cl.items).or(ctx.preview.then_some("sb_shells"))?;
            (Some(pic.to_string()), st[STAT_AMMO], st[STAT_AMMO] <= 10)
        }
        k => {
            let i = k as usize - Kind::Shells as usize;
            (Some(AMMO_PICS[i].to_string()), st[STAT_SHELLS + i], false)
        }
    };
    Some((icon.filter(|_| e.a != 0), v, alert))
}

/// Width of the item row kinds at their fullest.
fn items_w(kind: Kind, pack: Pack) -> f32 {
    let extras = pack.extra_items().len();
    let sigils = if pack == Pack::Rogue { 0 } else { 4 };
    let w = |n: usize| if n == 0 { 0.0 } else { n as f32 * 18.0 - 2.0 };
    let sig_w = if sigils == 0 { 0.0 } else { (sigils - 1) as f32 * 10.0 + 16.0 };
    match kind {
        Kind::Keys => w(2),
        Kind::Powerups => w(4 + extras),
        Kind::Sigils => sig_w,
        _ => sig_w + if sigils > 0 { 2.0 } else { 0.0 } + w(6 + extras),
    }
    .max(8.0)
}

fn weapon_slots(room: &Room) -> usize {
    8 + if room.pack == Pack::Hipnotic { 2 } else { 0 } + room.addons
}

/// The element's size in HUD units (its scale applied).
pub fn size(c: &Canvas, e: &Elem, room: &Room) -> (f32, f32) {
    let z = e.size;
    let k = (z / 8.0).round().clamp(1.0, 3.0);
    let (w, h) = match e.kind {
        // messages are short-lived and may run past their box
        Kind::Center => ((240.0 * k).min(room.vw), 24.0 * k + 6.0),
        Kind::Notify => ((200.0 * k).min(room.vw), 32.0 * k),
        Kind::Captions => ((160.0 * k).min(room.vw), 30.0 * k),
        Kind::Face => (24.0, 24.0),
        Kind::Automap => (120.0, 88.0),
        Kind::StatusBar => (320.0, if e.a == 1 { 48.0 } else { 24.0 }),
        Kind::Weapons => {
            let (n, cols) = (weapon_slots(room), usize::from(e.a.clamp(1, 10)));
            (n.min(cols) as f32 * 20.0 - 2.0, n.div_ceil(cols) as f32 * 14.0 - 2.0)
        }
        Kind::Items | Kind::Keys | Kind::Powerups | Kind::Sigils => (items_w(e.kind, room.pack), 16.0),
        Kind::AmmoList => {
            let name_w = ammo_names().iter().map(|n| text_w(c, e.font, z, n)).fold(0.0, f32::max);
            let lh = (z * 1.125).round();
            (name_w + z + text_w(c, e.font, z, "888"), lh * 3.0 + z)
        }
        k if k.is_bar() => {
            let value = if e.a != 0 { 4.0 + text_w(c, e.font, 8.0, "888") } else { 0.0 };
            (e.len + value, if e.a != 0 { z.max(8.0) } else { z })
        }
        k if k.is_number() => {
            let icon = if e.a != 0 { z + (z / 6.0).round().max(1.0) } else { 0.0 };
            (icon + text_w(c, e.font, z, "888"), z)
        }
        _ => {
            let s = spans(e, None, true).unwrap_or_default();
            let (px, py) = text_pad(z);
            (spans_w(c, e, &s) + 2.0 * px, z + 2.0 * py)
        }
    };
    let s = elem_scale(e);
    ((w * s).ceil(), (h * s).ceil())
}

pub(super) fn ammo_names() -> [&'static str; 4] {
    [qcore::t!("shells"), qcore::t!("nails"), qcore::t!("rockets"), qcore::t!("cells")]
}

/// Draws `e` in `r` at opacity `alpha` (times its own).
pub fn draw(c: &mut Canvas, pf: &mut PicFn, e: &Elem, r: Rect, ctx: &Ctx, alpha: f32) {
    let start = c.list.len();
    if e.kind == Kind::StatusBar && e.b != 0 {
        backtile(c, pf, r.y, r.h);
    }
    let es = elem_scale(e);
    let saved = (c.scale, c.ox, c.oy);
    c.scale *= es;
    c.ox = (saved.1 + r.x) / es;
    c.oy = (saved.2 + r.y) / es;
    let (w, h) = (r.w / es, r.h / es);
    draw_local(c, pf, e, (w, h), ctx);
    (c.scale, c.ox, c.oy) = saved;
    fade_list(&mut c.list[start..], e.opacity * alpha);
}

/// The status bar's backtile across the whole width, rows y..y+h.
fn backtile(c: &mut Canvas, pf: &mut PicFn, y: f32, h: f32) {
    let Some((id, tw, th)) = pf("backtile") else { return };
    let (tw, th) = (tw.max(1) as f32, th.max(1) as f32);
    let vw = c.vw();
    let mut ty = y;
    while ty < y + h {
        let mut tx = 0.0;
        while tx < vw {
            let rows = (y + h - ty).min(th);
            let s = c.scale;
            let uv = [0.0, 0.0, 1.0, rows / th];
            c.push(Draw2D::Pic { x: (tx + c.ox) * s, y: (ty + c.oy) * s, w: tw * s, h: rows * s, pic: id, uv, color: crate::draw::WHITE });
            tx += tw;
        }
        ty += th;
    }
}

fn draw_local(c: &mut Canvas, pf: &mut PicFn, e: &Elem, (w, h): (f32, f32), ctx: &Ctx) {
    let col = e.anchor.col();
    let z = e.size;
    match e.kind {
        Kind::Center | Kind::Notify | Kind::Captions => {
            if ctx.preview {
                super::draw_more::message_sample(c, e, (w, h));
            }
        }
        Kind::Face => {
            pic_box(c, pf, &Sbar::face_name(ctx.cl, ctx.face_anim), [0.0, 0.0, 24.0, 24.0], crate::draw::WHITE);
        }
        Kind::Automap => {
            if let Some(am) = &ctx.automap {
                if ctx.preview {
                    am.map.draw_widget_preview(c, (w, h), am.player_pos, am.player_yaw, am.settings, e.bg);
                } else if am.allowed {
                    am.map.draw_widget(c, (w, h), am.player_pos, am.player_yaw, am.settings, e.bg);
                }
            }
        }
        Kind::StatusBar => {
            let mut sb = Sbar::at(c, pf, ctx.pack, if e.a == 1 { 24.0 } else { 0.0 });
            sb.inventory = e.a == 1;
            sb.teamplay = ctx.teamplay;
            sb.draw(ctx.cl, ctx.show_scores, ctx.face_anim);
        }
        Kind::Weapons => super::draw_more::weapons(c, pf, e, w, ctx),
        Kind::Items | Kind::Keys | Kind::Powerups | Kind::Sigils => super::draw_more::items(c, pf, e, w, ctx),
        Kind::AmmoList => super::draw_more::ammo_list(c, pf, e, w, ctx),
        k if k.is_bar() => super::draw_more::bar(c, pf, e, h, ctx),
        k if k.is_number() => {
            let Some((icon, v, alert)) = number(e, ctx) else { return };
            let gap = (z / 6.0).round().max(1.0);
            let num_w = text_w(c, e.font, z, "888");
            let s = v.to_string();
            let s = if s.len() > 3 { &s[s.len() - 3..] } else { &s[..] };
            let (num_x, icon_x) = match (&icon, col == 2) {
                (Some(_), true) => (0.0, num_w + gap),
                (Some(_), false) => (z + gap, 0.0),
                (None, _) => (0.0, 0.0),
            };
            if let Some(icon) = icon {
                pic_box(c, pf, &icon, [icon_x, 0.0, z, z], crate::draw::WHITE);
            }
            let tw = text_w(c, e.font, z, s);
            draw_text(c, pf, e.font, z, (num_x + num_w - tw, 0.0), s, value_color(e), alert);
        }
        _ => {
            let Some(s) = spans(e, Some(ctx), false) else { return };
            let (px, py) = text_pad(z);
            let tw = spans_w(c, e, &s);
            if e.bg {
                let bw = (tw + 2.0 * px).min(w);
                c.fill(align(col, w, bw), 0.0, bw, h, BACKING);
            }
            let mut x = align(col, w - 2.0 * px, tw) + px;
            for (t, color) in &s {
                x += draw_text(c, pf, e.font, z, (x, py), t, *color, false);
            }
        }
    }
}
