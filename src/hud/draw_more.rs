//! Drawing of the HUD's grid, row, list and bar elements, and the sample
//! text the editor shows for the messages.

use qnet::protocol::*;

use super::draw::{Ctx, align, value_color, ammo_names, draw_text, pic_at, pic_box, text_w};
use super::kind::{Font, Kind};
use super::layout::Elem;
use crate::draw::Canvas;
use crate::sbar::{PicFn, Sbar};

const GOLD: [f32; 4] = [0.95, 0.7, 0.34, 1.0];
const BAR_BACK: [f32; 4] = [0.0, 0.0, 0.0, 0.5];
const BAR_EDGE: [f32; 4] = [0.55, 0.42, 0.25, 0.9];

/// The weapon grid: `e.a` columns of 20×14 cells.
pub(super) fn weapons(c: &mut Canvas, pf: &mut PicFn, e: &Elem, width: f32, ctx: &Ctx) {
    let icons = {
        let mut sb = Sbar::at(c, pf, ctx.pack, 0.0);
        sb.addon_weapons = ctx.addon_weapons.to_vec();
        sb.held_addon = ctx.held_addon;
        sb.weapon_row(ctx.cl)
    };
    let cols = usize::from(e.a.clamp(1, 10));
    let n = icons.len();
    for (i, w) in icons.iter().enumerate() {
        // rows line up with the anchor's side
        let in_row = (n - i / cols * cols).min(cols) as f32 * 20.0 - 2.0;
        let x0 = align(e.anchor.col(), width, in_row);
        let (wx, wy) = (x0 + (i % cols) as f32 * 20.0, (i / cols) as f32 * 14.0);
        let color = if w.active { [1.0, 0.7, 0.3, 1.0] } else { [1.0, 1.0, 1.0, 0.65] };
        if w.active {
            c.fill(wx, wy, 18.0, 12.0, [0.35, 0.22, 0.07, 0.7]);
        }
        match w.pic.as_ref().and_then(|p| pf(p).map(|s| (p, s))) {
            Some((name, (_, pw, ph))) => {
                let k = (18.0 / pw as f32).min(12.0 / ph as f32).min(0.75);
                let (pw, ph) = (pw as f32 * k, ph as f32 * k);
                pic_box(c, pf, name, [wx, wy + (12.0 - ph) / 2.0, pw, ph], color);
            }
            None => {
                c.qtext(wx + 1.0, wy + 2.0, w.label.as_bytes(), color);
            }
        }
    }
}

fn bullet_time_label(e: &Elem) -> String {
    let s = qcore::lang::tr("BULLET TIME");
    if e.font == Font::Quake { qcore::lang::quake_encode(&s).to_string() } else { s.into_owned() }
}

/// The bar's length and top (under the label).
fn bullet_time_bar(e: &Elem) -> (f32, f32) {
    (e.len, if e.a != 0 { (e.size * 1.5).round() } else { 0.0 })
}

const BULLET_TIME_BAR_H: f32 = 6.0;

pub(super) fn bullet_time_size(c: &Canvas, e: &Elem) -> (f32, f32) {
    let (len, top) = bullet_time_bar(e);
    let label = if e.a != 0 { text_w(c, e.font, e.size, &bullet_time_label(e)) } else { 0.0 };
    (len.max(label), top + BULLET_TIME_BAR_H)
}

/// Bullet time's charge (the editor: a sample).
pub(super) fn bullet_time(c: &mut Canvas, pf: &mut PicFn, e: &Elem, w: f32, ctx: &Ctx) {
    let Some((charge, on)) = ctx.bullet_time.or(ctx.preview.then_some((0.7, true))) else { return };
    let (len, top) = bullet_time_bar(e);
    if e.a != 0 {
        let s = bullet_time_label(e);
        let x = align(e.anchor.col(), w, text_w(c, e.font, e.size, &s));
        draw_text(c, pf, e.font, e.size, (x, 0.0), &s, value_color(e), false);
    }
    let x = align(e.anchor.col(), w, len);
    c.fill(x, top, len, BULLET_TIME_BAR_H, [0.0, 0.0, 0.0, 0.7]);
    let color = if on { [0.4, 0.8, 1.0, 0.9] } else { [0.8, 0.7, 0.4, 0.8] };
    c.fill(x + 1.0, top + 1.0, (len - 2.0) * charge, BULLET_TIME_BAR_H - 2.0, color);
}

/// Keys, powerups and sigils (all of them, or one group), in a row.
pub(super) fn items(c: &mut Canvas, pf: &mut PicFn, e: &Elem, w: f32, ctx: &Ctx) {
    let row = Sbar::at(c, pf, ctx.pack, 0.0).item_row(ctx.cl);
    let keep = |p: &str| {
        let (key, sigil) = (p.starts_with("sb_key"), p.starts_with("sb_sigil"));
        match e.kind {
            Kind::Keys => key,
            Kind::Sigils => sigil,
            Kind::Powerups => !key && !sigil,
            _ => true,
        }
    };
    let mut row: Vec<_> = row.into_iter().filter(|(p, ..)| keep(p)).collect();
    if let Some(last) = row.last_mut() {
        last.1 = 16.0;
    }
    let total: f32 = row.iter().map(|r| r.1).sum();
    let mut x = align(e.anchor.col(), w, total);
    for (p, adv, dx) in &row {
        pic_at(c, pf, p, x + dx, 0.0, 1.0);
        x += adv;
    }
}

/// Shells, nails, rockets and cells with their counts, gold when the
/// matching weapon is owned.
pub(super) fn ammo_list(c: &mut Canvas, pf: &mut PicFn, e: &Elem, w: f32, ctx: &Ctx) {
    let z = e.size;
    let bits = ctx.pack.ammo_bits();
    let names = ammo_names();
    let name_w = names.iter().map(|n| text_w(c, e.font, z, n)).fold(0.0, f32::max);
    let num_w = text_w(c, e.font, z, "888");
    let lh = (z * 1.125).round();
    let right = e.anchor.col() == 2;
    let x0 = if right { w - name_w - z - num_w } else { 0.0 };
    for (i, name) in names.iter().enumerate() {
        let y = i as f32 * lh;
        let color = if ctx.cl.items & bits[i] != 0 { GOLD } else { value_color(e) };
        let nx = if right { x0 + name_w - text_w(c, e.font, z, name) } else { x0 };
        draw_text(c, pf, e.font, z, (nx, y), name, color, false);
        let n = ctx.cl.stats[STAT_SHELLS + i].clamp(0, 999).to_string();
        let tx = x0 + name_w + z + num_w - text_w(c, e.font, z, &n);
        draw_text(c, pf, e.font, z, (tx, y), &n, color, false);
    }
}

/// Value, full value and colour of a bar; None when it has nothing to show.
fn bar_value(kind: Kind, ctx: &Ctx) -> Option<(i32, f32, [f32; 4])> {
    let (cl, st) = (ctx.cl, &ctx.cl.stats);
    Some(match kind {
        Kind::HealthBar => (st[STAT_HEALTH], 100.0, [0.85, 0.2, 0.15, 1.0]),
        Kind::ArmorBar => {
            let pic = ctx.pack.armor_pic(cl.items).or(ctx.preview.then_some("sb_armor1"))?;
            let (full, color) = match pic {
                "sb_armor3" => (200.0, [0.85, 0.25, 0.15, 1.0]),
                "sb_armor2" => (150.0, [0.95, 0.8, 0.2, 1.0]),
                "disc" => return Some((666, 1.0, GOLD)),
                _ => (100.0, [0.3, 0.75, 0.3, 1.0]),
            };
            (st[STAT_ARMOR], full, color)
        }
        _ => {
            let pic = ctx.pack.ammo_pic(cl.items).or(ctx.preview.then_some("sb_shells"))?;
            let full = if matches!(pic, "sb_nails" | "r_ammolava") { 200.0 } else { 100.0 };
            (st[STAT_AMMO], full, [0.95, 0.65, 0.2, 1.0])
        }
    })
}

/// A gauge `e.len` long and `e.size` thick, the value beside it.
pub(super) fn bar(c: &mut Canvas, pf: &mut PicFn, e: &Elem, h: f32, ctx: &Ctx) {
    let Some((v, full, color)) = bar_value(e.kind, ctx) else { return };
    let z = e.size;
    let show = e.a != 0;
    let num_w = if show { text_w(c, e.font, 8.0, "888") } else { 0.0 };
    let right = e.anchor.col() == 2;
    let bx = if right && show { num_w + 4.0 } else { 0.0 };
    let by = ((h - z) / 2.0).floor();
    c.fill(bx, by, e.len, z, BAR_BACK);
    let f = (v as f32 / full).clamp(0.0, 1.0);
    c.fill(bx + 1.0, by + 1.0, ((e.len - 2.0) * f).max(0.0), (z - 2.0).max(1.0), color);
    c.frame(bx, by, e.len, z, BAR_EDGE);
    if show {
        let s = v.clamp(-99, 999).to_string();
        let tw = text_w(c, e.font, 8.0, &s);
        let tx = if right { num_w - tw } else { e.len + 4.0 };
        let alert = e.kind != Kind::ArmorBar && v <= if e.kind == Kind::AmmoBar { 10 } else { 25 };
        draw_text(c, pf, e.font, 8.0, (tx, ((h - 8.0) / 2.0).floor()), &s, value_color(e), alert);
    }
}

/// The editor's stand-in for a message: its name where the text goes.
pub(super) fn message_sample(c: &mut Canvas, e: &Elem, (w, h): (f32, f32)) {
    c.fill(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.3]);
    let k = (e.size / 8.0).round().clamp(1.0, 3.0);
    let text = qcore::lang::tr(e.kind.label());
    let text = text.as_bytes();
    let col = e.anchor.col();
    let saved = (c.scale, c.ox, c.oy);
    c.scale *= k;
    (c.ox, c.oy) = (saved.1 / k, saved.2 / k);
    let tw = c.qtext_w(text);
    let lines = if e.kind == Kind::Captions { 1 } else { 2 };
    for i in 0..lines {
        let y = 2.0 + i as f32 * 10.0;
        let [r, g, b, _] = value_color(e);
        let color = [r, g, b, if i == 0 { 0.9 } else { 0.45 }];
        c.qtext(align(col, w / k, tw), y, text, color);
    }
    (c.scale, c.ox, c.oy) = saved;
}

/// A two-tone skull on a 14 × 14 pixel grid.
pub(super) fn damage_skull(c: &mut Canvas) {
    let outline = [0.16, 0.05, 0.04, 1.0];
    let bone = [1.0, 0.83, 0.69, 1.0];
    c.fill(2.0, 0.0, 10.0, 11.0, outline);
    c.fill(0.0, 2.0, 14.0, 8.0, outline);
    c.fill(3.0, 10.0, 8.0, 4.0, outline);
    c.fill(3.0, 1.0, 8.0, 9.0, bone);
    c.fill(1.0, 3.0, 12.0, 6.0, bone);
    c.fill(4.0, 9.0, 6.0, 4.0, bone);
    c.fill(3.0, 5.0, 3.0, 3.0, outline);
    c.fill(8.0, 5.0, 3.0, 3.0, outline);
    c.fill(6.0, 8.0, 2.0, 2.0, outline);
    c.fill(6.0, 11.0, 2.0, 2.0, outline);
}
