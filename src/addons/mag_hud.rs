//! The Magazine HUD element: rounds loaded in the held add-on gun, per gun,
//! with a bar while reloading. Every add-on gun shares it.
use crate::draw::Canvas;
use crate::hud::draw::{BACKING, Ctx, draw_text, text_w};
use crate::hud::layout::Elem;
use crate::sbar::PicFn;
use qclient::guns::MagPart;

/// Guns there is room for side by side.
const GUNS: usize = 2;

/// A readout's plate, and the gap between two (a 320-unit screen still has the modern HUD's health beside them).
fn plate(c: &Canvas, e: &Elem) -> (f32, f32, f32) {
    let k = e.size / 8.0;
    (text_w(c, e.font, e.size, "888 / 888") + 8.0 * k, 18.0 * k, 26.0 * k)
}

pub fn size(c: &Canvas, e: &Elem) -> (f32, f32) {
    let (w, h, gap) = plate(c, e);
    (w * GUNS as f32 + gap * (GUNS - 1) as f32, h)
}

/// Draws the magazines of the gun `ctx`'s player holds (the editor: a sample).
pub fn draw(c: &mut Canvas, pf: &mut PicFn, e: &Elem, ctx: &Ctx) {
    let owner = ctx.cl.viewentity.saturating_sub(1);
    let held = ctx.held_addon.and_then(|t| ctx.cl.mags.iter().find(|m| m.owner as usize == owner && m.toy == t.id()));
    let sample = [MagPart { rounds: 9, reload_end: None, reload_secs: 0.0 }, MagPart { rounds: 2, reload_end: Some(ctx.cl.time + 0.6), reload_secs: 1.5 }];
    let (cap, parts): (u32, &[MagPart]) = match held {
        Some(m) => (m.cap, &m.parts),
        None if ctx.preview => (12, &sample),
        None => return,
    };
    let (w, h, gap) = plate(c, e);
    // a lone gun's goes beside the middle, where a centred view model is
    let first = if e.anchor.col() == 0 { 0 } else { GUNS.saturating_sub(parts.len()) };
    let k = e.size / 8.0;
    for (i, p) in parts.iter().enumerate() {
        let x = (first + i) as f32 * (w + gap);
        let color = if p.rounds == 0 {
            [1.0, 0.25, 0.2, 1.0]
        } else if p.rounds * 4 <= cap {
            [1.0, 0.8, 0.3, 1.0]
        } else {
            crate::hud::draw::palette(e.color, [0.95, 0.95, 0.95, 1.0])
        };
        let left = p.reload_end.map(|t| t - ctx.cl.time).filter(|&l| l > 0.0);
        if e.bg {
            // a plate behind, as guns reach this far up
            c.round_box(x, 0.0, w, if left.is_some() { h } else { 13.0 * k }, 3.0 * k, BACKING, None);
        }
        let text = format!("{} / {cap}", p.rounds);
        let tw = text_w(c, e.font, e.size, &text);
        draw_text(c, pf, e.font, e.size, (x + (w - tw) / 2.0, 2.0 * k), &text, color, false);
        if let Some(left) = left {
            let done = 1.0 - (left as f32 / p.reload_secs.max(0.1)).clamp(0.0, 1.0);
            let bw = w - 8.0 * k;
            c.fill(x + 4.0 * k, 12.0 * k, bw, 3.0 * k, [0.0, 0.0, 0.0, 0.6]);
            c.fill(x + 4.0 * k, 12.0 * k, bw * done, 3.0 * k, [0.9, 0.8, 0.5, 0.9]);
        }
    }
}
