//! The demo playback bar along the bottom of the screen: state and speed,
//! level, time / length, a timeline (map boundaries, clip marks, play head)
//! and a row of buttons. `cl_demobar` 2 (default) keeps it up; 1 is hidden:
//! a control key brings it back for a few seconds and the end of the demo
//! shows it; 0 never. H, View or the Hide button hides a showing bar and
//! shows a hidden one (`toggle_demo_bar`); that choice holds for the session
//! over both modes, so a hidden bar stays hidden, until the player shows it
//! or changes `cl_demobar`. While hidden, the pointer at the bottom edge
//! shows a strip that brings it back (`Peek`). Never while exporting. The HUD is lifted above it (`lift`). Mouse: `demo_mouse.rs`.

use super::*;
use crate::demo::playback::format_time;
use crate::menu::{BORDER, GOLD_FG, MUTED, TXT};
use qinput::keys::*;

/// Panel height (normal, and extra height in controller mode for button glyphs)
/// and the gap under it (virtual units).
const H: f32 = 46.0;
const PAD_EXTRA: f32 = 11.0;
const MARGIN: f32 = 6.0;
const BTN_H: f32 = 13.0;
const GLYPH_H: f32 = crate::glyphs::H;
const ICON_W: f32 = 16.0;
/// Widest text button; narrower when the panel is.
const TEXT_W: f32 = 46.0;
const MIN_TEXT_W: f32 = 30.0;

fn panel_h(pad: bool) -> f32 {
    if pad { H + PAD_EXTRA } else { H }
}

fn btn_h(pad: bool) -> f32 {
    if pad { BTN_H + PAD_EXTRA } else { BTN_H }
}

fn right_buttons(auto: bool) -> &'static [Btn] {
    if auto {
        &[Btn::Hide, Btn::Keep, Btn::Export, Btn::ClearMarks]
    } else {
        &[Btn::Hide, Btn::Export, Btn::ClearMarks]
    }
}

/// x, y, w, h in virtual units.
pub(super) type Rect = (f32, f32, f32, f32);

pub(super) fn inside(r: Rect, (x, y): (f32, f32)) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Btn {
    Start,
    Back10,
    Rewind,
    PlayPause,
    Forward,
    Fwd10,
    NextMap,
    Slower,
    Faster,
    MarkIn,
    MarkOut,
    ClearMarks,
    Export,
    Keep,
    Hide,
}

impl Btn {
    /// Gamepad button that triggers this control (matches `Host::demo_key`).
    pub(super) fn pad_key(self) -> i32 {
        match self {
            Btn::Start => K_LTHUMB,
            Btn::Back10 => K_LSHOULDER,
            Btn::Rewind => K_DPAD_LEFT,
            Btn::PlayPause => K_ABUTTON,
            Btn::Forward => K_DPAD_RIGHT,
            Btn::Fwd10 => K_RSHOULDER,
            Btn::NextMap => K_RTHUMB,
            Btn::Slower => K_DPAD_DOWN,
            Btn::Faster => K_DPAD_UP,
            Btn::MarkIn => K_LTRIGGER,
            Btn::MarkOut => K_RTRIGGER,
            Btn::ClearMarks => K_BBUTTON,
            Btn::Export => K_XBUTTON,
            Btn::Keep => K_YBUTTON,
            Btn::Hide => K_BACK,
        }
    }

    fn text_label(self) -> &'static str {
        match self {
            Btn::ClearMarks => qcore::t!("Clear"),
            Btn::Export => qcore::t!("Export"),
            Btn::Keep => qcore::t!("Keep"),
            Btn::Hide => qcore::t!("Hide"),
            _ => "",
        }
    }
}

/// Where everything sits for a canvas of `vw`×`vh`; drawing and the mouse
/// share it.
pub(super) struct Layout {
    pub panel: Rect,
    /// The drawn track; clicks and drags use `hit`.
    pub track: Rect,
    pub hit: Rect,
    pub buttons: Vec<(Btn, Rect)>,
}

pub(super) fn layout(vw: f32, vh: f32, auto: bool, pad: bool) -> Layout {
    let ph = panel_h(pad);
    let bh = btn_h(pad);
    let panel = (8.0, (vh - ph - MARGIN).floor(), (vw - 16.0).max(1.0), ph);
    let (x, y, w, _) = panel;
    let track = (x + 8.0, y + 17.0, w - 16.0, 4.0);
    let hit = (track.0 - 2.0, y + 12.0, track.2 + 4.0, 14.0);
    let by = y + 28.0;
    let mut buttons = Vec::new();
    let mut bx = x + 8.0;
    for b in [Btn::Start, Btn::Back10, Btn::Rewind, Btn::PlayPause, Btn::Forward, Btn::Fwd10, Btn::NextMap] {
        buttons.push((b, (bx, by, ICON_W, bh)));
        bx += ICON_W + 2.0;
    }
    bx += 6.0;
    buttons.push((Btn::Slower, (bx, by, 14.0, bh)));
    buttons.push((Btn::Faster, (bx + 34.0, by, 14.0, bh)));
    bx += 54.0;
    buttons.push((Btn::MarkIn, (bx, by, ICON_W, bh)));
    buttons.push((Btn::MarkOut, (bx + ICON_W + 2.0, by, ICON_W, bh)));
    let icons_end = bx + 2.0 * ICON_W + 8.0;
    // text buttons from the right edge, sharing what's left; the last ones
    // are dropped when even the narrowest wouldn't fit
    let right = right_buttons(auto);
    let room = x + w - 8.0 - icons_end;
    let n = right.len().min(((room + 2.0) / (MIN_TEXT_W + 2.0)) as usize);
    let tw = if n > 0 { ((room + 2.0) / n as f32 - 2.0).min(TEXT_W) } else { 0.0 };
    let mut rx = x + w - 8.0;
    for &b in &right[..n] {
        rx -= tw;
        buttons.push((b, (rx, by, tw, bh)));
        rx -= 2.0;
    }
    Layout { panel, track, hit, buttons }
}

/// Height of the bottom band where the hidden bar's strip appears.
pub(super) const PEEK_ZONE: f32 = 26.0;

/// The "Show controls" tab of the strip.
pub(super) fn peek_tab(vw: f32, vh: f32, pad: bool) -> Rect {
    let w = (if pad { 142.0f32 } else { 120.0f32 }).min(vw - 16.0);
    (((vw - w) / 2.0).floor(), vh - 17.0, w, 13.0)
}

pub(super) struct Peek {
    hover: bool,
    progress: Option<f32>,
    pad: bool,
}

/// The strip: progress along the bottom edge and a tab to show the bar.
pub(super) fn draw_peek(c: &mut Canvas, p: &Peek, glyphs: &crate::glyphs::Glyphs) {
    c.reset_origin();
    let (vw, vh) = (c.vw(), c.vh());
    c.fill(0.0, vh - 3.0, vw, 3.0, [0.0, 0.0, 0.0, 0.6]);
    if let Some(f) = p.progress {
        c.fill(0.0, vh - 3.0, vw * f.clamp(0.0, 1.0), 3.0, [0.85, 0.65, 0.30, 0.95]);
    }
    let r = peek_tab(vw, vh, p.pad);
    c.round_box(r.0, r.1, r.2, r.3, 3.0, if p.hover { HOVER } else { [0.0, 0.0, 0.0, 0.72] }, Some(BORDER));
    // up chevron, then the label (with the View/Share glyph in controller mode)
    let (cx, cy) = (r.0 + 9.0, r.1 + r.3 / 2.0);
    for i in 0..4 {
        let fi = i as f32;
        c.fill(cx - fi, cy - 2.0 + fi, 1.0, 1.0, ICON);
        c.fill(cx + fi, cy - 2.0 + fi, 1.0, 1.0, ICON);
    }
    glyphs.button_label(c, [r.0 + 14.0, r.1, r.2 - 16.0, r.3], p.pad.then_some(K_BACK), qcore::t!("Show controls"), 8.0, ICON);
}

/// How far the HUD moves up while the bar shows.
pub(super) fn lift(pad: bool) -> f32 {
    panel_h(pad) + MARGIN + 2.0
}

impl Host {
    /// Whether the bar may show at all (a user demo in game, no menu).
    fn demo_bar_allowed(&self) -> bool {
        self.demo.is_some() && self.cv("cl_demobar").round() as i32 != 0 && !self.exporting() && self.timedemo.is_none() && !self.menu.active() && self.cl.signon == 4 && self.demonum < 0
    }

    pub(super) fn demo_bar_visible(&self) -> bool {
        let Some(d) = &self.demo else { return false };
        let mode = self.cv("cl_demobar");
        // the player's Hide/Show, while the setting is what it was then
        let shown = match self.demo_ui.choice {
            Some((show, under)) if under == mode => show,
            _ => mode >= 2.0 || self.realtime < d.play.shown_until,
        };
        self.demo_bar_allowed() && (shown || self.demo_ui.drag.is_some())
    }

    /// The hidden bar's hint strip: the pointer is at the bottom edge.
    pub(super) fn demo_peek(&self) -> Option<Peek> {
        if !self.demo_bar_allowed() || self.demo_bar_visible() {
            return None;
        }
        let (x, y) = self.demo_ui.pos?;
        let (vw, vh) = self.demo_ui.size;
        if y < vh - PEEK_ZONE {
            return None;
        }
        let pad = self.pad_ui();
        let progress = self.demo_clock().zip(self.demo.as_ref()?.play.duration()).map(|(t, len)| (t / len.max(0.001)) as f32);
        Some(Peek { hover: inside(peek_tab(vw, vh, pad), (x, y)), progress, pad })
    }

    /// What the bar shows this frame, or None when hidden.
    pub(super) fn demo_bar(&mut self) -> Option<Bar> {
        self.menu.demos.poll();
        if !self.demo_bar_visible() {
            return None;
        }
        let speed = self.demo_speed();
        let now = self.demo_clock();
        let d = self.demo.as_ref()?;
        let at_end = d.paused && d.play.shown_until.is_infinite();
        let state = if at_end {
            qcore::tr!("End of demo")
        } else if speed == 0.0 {
            qcore::tr!("Paused")
        } else if speed < 0.0 {
            qcore::tr!("Rewind {}x", trim(-speed))
        } else if d.play.scrub != 0 {
            qcore::tr!("Fast forward {}x", trim(speed))
        } else {
            qcore::tr!("Play {}x", trim(speed))
        };
        let mut map = if self.cl.levelname.trim().is_empty() { self.level.mapname.clone() } else { format!("{} ({})", self.cl.levelname.trim(), self.level.mapname) };
        if self.freecam.cam.is_some() {
            map = format!("{map} [Free Cam]");
        } else if let Some(pov) = d.mvd_pov_name() {
            map = format!("{map} [POV: {pov}]");
        }
        let mut marks = d.play.marks;
        // a drag in progress shows the segment it would mark
        if let (Some(a), Some(b)) = (self.demo_ui.drag, self.demo_ui.drag_to) {
            marks = [Some(a.min(b)), Some(a.max(b))];
        }
        let timeline = match (&d.play.index, now) {
            (Some(idx), Some(t)) => Some(Timeline { now: t, len: idx.duration.max(0.001), maps: idx.boundaries().collect(), marks }),
            _ => None,
        };
        let scrub = match d.play.scrub {
            s if s < 0 => Some(Btn::Rewind),
            s if s > 0 => Some(Btn::Forward),
            _ => None,
        };
        let held = self.demo_ui.held.or(scrub);
        Some(Bar {
            state,
            map,
            timeline,
            auto: d.play.auto,
            playing: speed > 0.0 && !d.paused,
            speed: trim(d.play.base_speed),
            pointer: self.demo_ui.pos,
            held,
            pad: self.pad_ui(),
        })
    }
}

pub(super) struct Timeline {
    now: f64,
    len: f64,
    maps: Vec<f64>,
    marks: [Option<f64>; 2],
}

pub(super) struct Bar {
    state: String,
    map: String,
    timeline: Option<Timeline>,
    auto: bool,
    playing: bool,
    speed: String,
    pointer: Option<(f32, f32)>,
    held: Option<Btn>,
    pub(super) pad: bool,
}

const HOVER: [f32; 4] = [0.6, 0.38, 0.12, 0.55];
const BTN_BG: [f32; 4] = [1.0, 1.0, 1.0, 0.08];
const ICON: [f32; 4] = [0.92, 0.88, 0.80, 1.0];

pub(super) fn draw(c: &mut Canvas, bar: &Bar, glyphs: &crate::glyphs::Glyphs) {
    c.reset_origin();
    let (vw, vh) = (c.vw(), c.vh());
    let l = layout(vw, vh, bar.auto, bar.pad);
    let (x, y, w, h) = l.panel;
    c.round_box(x, y, w, h, 3.0, [0.0, 0.0, 0.0, 0.72], Some(BORDER));
    c.text(x + 8.0, y + 3.0, &bar.state, TXT, GOLD_FG);
    c.text_center(x + w / 2.0, y + 3.0, &crate::modmenu::clip(&bar.map, ((w - 180.0) / 5.0).max(8.0) as usize), TXT, MUTED);
    let time = match &bar.timeline {
        Some(t) => format!("{} / {}", format_time(t.now), format_time(t.len)),
        None => qcore::tr!("Indexing..."),
    };
    c.text_right(x + w - 8.0, y + 3.0, &time, TXT, crate::menu::FG);
    draw_track(c, &l, bar);
    for &(b, r) in &l.buttons {
        let hover = bar.pointer.is_some_and(|p| inside(r, p));
        c.fill(r.0, r.1, r.2, r.3, if hover || bar.held == Some(b) { HOVER } else { BTN_BG });
        draw_button(c, b, (r.0, r.1, r.2, BTN_H), bar);
        if bar.pad {
            let key = b.pad_key();
            let gw = crate::glyphs::width(c, key, glyphs.family, GLYPH_H);
            glyphs.key_h(c, r.0 + (r.2 - gw) / 2.0, r.1 + BTN_H - 0.5, key, GLYPH_H);
        }
    }
    if let Some(&(_, s)) = l.buttons.iter().find(|(b, _)| *b == Btn::Slower) {
        let sp = format!("{}x", bar.speed);
        let sw = c.text_w_raw(&sp, 8.0);
        c.text_raw(s.0 + 24.0 - sw / 2.0, s.1 + 2.0, &sp, 8.0, crate::menu::FG);
    }
    // key help above the panel: keyboard keys while the pointer is over the
    // bar, or on narrow screens any controller buttons dropped from the bar
    let hy = y - 12.0;
    if bar.pad {
        let items: Vec<(i32, &str)> = right_buttons(bar.auto)
            .iter()
            .rev()
            .filter(|b| !l.buttons.iter().any(|(placed, _)| placed == *b))
            .map(|&b| (b.pad_key(), b.text_label()))
            .collect();
        if !items.is_empty() {
            let mut rows: Vec<Vec<(i32, &str)>> = vec![Vec::new()];
            for it in items {
                let row = rows.last_mut().unwrap();
                row.push(it);
                if row.len() > 1 && glyphs.hints(c, 0.0, 0.0, row, false) > w - 16.0 {
                    row.pop();
                    rows.push(vec![it]);
                }
            }
            let pitch = crate::glyphs::HINT_H + 4.0;
            for (i, row) in rows.iter().enumerate() {
                let ry = hy - 4.0 - (rows.len() - 1 - i) as f32 * pitch;
                let rw = glyphs.hints(c, 0.0, ry, row, false);
                c.fill(vw / 2.0 - rw / 2.0 - 4.0, ry - 2.0, rw + 8.0, pitch, [0.0, 0.0, 0.0, 0.6]);
                glyphs.hints_centered(c, vw / 2.0, ry, row);
            }
        }
    } else if bar.pointer.is_some_and(|p| inside(l.panel, p)) {
        let mut keys = qcore::lang::tr(qcore::t!("Space pause | Left/Right scrub | Up/Down speed | PgUp/PgDn 10 s | I/O mark | E export")).into_owned();
        keys = format!("{keys} | {}", qcore::lang::tr(qcore::t!("H hide")));
        if bar.auto {
            keys = format!("{keys} | {}", qcore::lang::tr(qcore::t!("K keep")));
        }
        let lines = qcore::lang::wrap(&keys, w, |t| c.text_w_raw(t, 8.0));
        let n = lines.len().min(2);
        for (i, line) in lines.iter().take(n).enumerate() {
            let lw = c.text_w_raw(line, 8.0);
            let ly = hy - (n - 1 - i) as f32 * 10.0;
            c.fill(vw / 2.0 - lw / 2.0 - 4.0, ly - 1.0, lw + 8.0, 10.0, [0.0, 0.0, 0.0, 0.6]);
            c.text_raw(vw / 2.0 - lw / 2.0, ly, line, 8.0, MUTED);
        }
    }
}

fn draw_track(c: &mut Canvas, l: &Layout, bar: &Bar) {
    let (bx, by, bw, bh) = l.track;
    let hover = bar.pointer.is_some_and(|p| inside(l.hit, p));
    c.fill(bx, by, bw, bh, [1.0, 1.0, 1.0, if hover { 0.28 } else { 0.18 }]);
    let Some(t) = &bar.timeline else { return };
    let px = |v: f64| bx + (v / t.len).clamp(0.0, 1.0) as f32 * bw;
    if t.marks.iter().any(Option::is_some) {
        let a = t.marks[0].unwrap_or(0.0);
        let b = t.marks[1].unwrap_or(t.len);
        let (xa, xb) = (px(a.min(b)), px(a.max(b)));
        c.fill(xa, by - 3.0, (xb - xa).max(1.0), bh + 6.0, [0.30, 0.55, 0.95, 0.45]);
    }
    c.fill(bx, by, px(t.now) - bx, bh, [0.85, 0.65, 0.30, 0.95]);
    for &m in &t.maps {
        c.fill(px(m).floor(), by - 2.0, 1.0, bh + 4.0, [1.0, 1.0, 1.0, 0.6]);
    }
    for &m in t.marks.iter().flatten() {
        c.fill(px(m).floor() - 1.0, by - 4.0, 2.0, bh + 8.0, [0.45, 0.70, 1.0, 1.0]);
    }
    c.fill(px(t.now).floor() - 1.0, by - 4.0, 3.0, bh + 8.0, [1.0, 0.95, 0.85, 1.0]);
    // the time under the pointer
    if hover && let Some((mx, _)) = bar.pointer {
        let at = ((mx - bx) / bw).clamp(0.0, 1.0) as f64 * t.len;
        let s = format_time(at);
        let sw = c.text_w_raw(&s, 8.0);
        let sx = (mx - sw / 2.0).clamp(bx, bx + bw - sw);
        c.fill(sx - 3.0, by - 14.0, sw + 6.0, 10.0, [0.0, 0.0, 0.0, 0.8]);
        c.text_raw(sx, by - 13.0, &s, 8.0, crate::menu::FG);
    }
}

/// Triangle of `w`×`h` made of thin columns, pointing right or left.
fn tri(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, right: bool) {
    let n = (w * 2.0).ceil() as i32;
    for i in 0..n {
        let f = (i as f32 + 0.5) / n as f32;
        let ch = h * if right { 1.0 - f } else { f };
        c.fill(x + i as f32 * 0.5, y + (h - ch) / 2.0, 0.5, ch, ICON);
    }
}

fn draw_button(c: &mut Canvas, b: Btn, r: Rect, bar: &Bar) {
    let (cx, cy) = (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
    let label = |c: &mut Canvas, s: &str| {
        let s = crate::modmenu::clip(&qcore::lang::tr(s), ((r.2 - 4.0) / 4.0) as usize);
        let w = c.text_w_raw(&s, 8.0);
        c.text_raw(cx - w / 2.0, r.1 + 2.0, &s, 8.0, ICON);
    };
    match b {
        Btn::Start => {
            c.fill(cx - 4.0, cy - 3.5, 1.0, 7.0, ICON);
            tri(c, cx - 3.0, cy - 3.5, 6.0, 7.0, false);
        }
        Btn::NextMap => {
            tri(c, cx - 3.0, cy - 3.5, 6.0, 7.0, true);
            c.fill(cx + 3.0, cy - 3.5, 1.0, 7.0, ICON);
        }
        Btn::Rewind => {
            tri(c, cx - 5.0, cy - 3.0, 5.0, 6.0, false);
            tri(c, cx, cy - 3.0, 5.0, 6.0, false);
        }
        Btn::Forward => {
            tri(c, cx - 5.0, cy - 3.0, 5.0, 6.0, true);
            tri(c, cx, cy - 3.0, 5.0, 6.0, true);
        }
        Btn::PlayPause if bar.playing => {
            c.fill(cx - 3.0, cy - 3.5, 2.0, 7.0, ICON);
            c.fill(cx + 1.0, cy - 3.5, 2.0, 7.0, ICON);
        }
        Btn::PlayPause => tri(c, cx - 3.0, cy - 4.0, 7.0, 8.0, true),
        Btn::Back10 => {
            let w = c.text_w_raw("-10", 8.0);
            c.text_raw(cx - w / 2.0, r.1 + 2.0, "-10", 8.0, ICON);
        }
        Btn::Fwd10 => {
            let w = c.text_w_raw("+10", 8.0);
            c.text_raw(cx - w / 2.0, r.1 + 2.0, "+10", 8.0, ICON);
        }
        Btn::Slower => c.fill(cx - 3.0, cy - 0.5, 6.0, 1.0, ICON),
        Btn::Faster => {
            c.fill(cx - 3.0, cy - 0.5, 6.0, 1.0, ICON);
            c.fill(cx - 0.5, cy - 3.0, 1.0, 6.0, ICON);
        }
        // clip brackets, in the marks' blue
        Btn::MarkIn | Btn::MarkOut => {
            let blue = [0.45, 0.70, 1.0, 1.0];
            let (bx, dir) = if b == Btn::MarkIn { (cx - 2.0, 1.0) } else { (cx + 1.0, -1.0) };
            c.fill(bx, cy - 4.0, 1.0, 8.0, blue);
            let tick = if dir > 0.0 { bx } else { bx - 2.0 };
            c.fill(tick, cy - 4.0, 3.0, 1.0, blue);
            c.fill(tick, cy + 3.0, 3.0, 1.0, blue);
        }
        Btn::ClearMarks => label(c, qcore::t!("Clear")),
        Btn::Export => label(c, qcore::t!("Export")),
        Btn::Keep => label(c, qcore::t!("Keep")),
        Btn::Hide => label(c, qcore::t!("Hide")),
    }
}

/// `2`, `0.5`, `1.25` without trailing zeros.
fn trim(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests;
