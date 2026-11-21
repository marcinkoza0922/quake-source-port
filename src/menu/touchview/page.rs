//! Touch options page input: taps on rows and their controls, slider
//! drags, the sheet listing a row's values, and the footer that explains
//! the last row tapped (with Reset when it differs from its default).

use super::rows::{Geom, Kind};
use super::*;

pub(super) const SHEET_ROW: f32 = 52.0;
const SHEET_TITLE: f32 = 56.0;
const SHEET_W: f32 = 520.0;
const RESET_W: f32 = 104.0;

impl Menu {
    pub(super) fn tv_page_click(&mut self, p: Page, pos: (f32, f32), cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) {
        let g = self.tv_geom(p);
        if let Some((bar, reset)) = self.tv_footer_rects(cv, p, &g) {
            if reset.is_some_and(|r| inside(r, pos)) {
                let item = self.page_item(p, self.tv.footer.unwrap_or(0));
                if self.reset_row(item, cv, out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
                return;
            }
            if inside(bar, pos) {
                return;
            }
        }
        let Some(i) = self.tv_row_at(p, pos) else {
            self.tv.footer = None;
            self.tv.text_focus = None;
            return;
        };
        let item = self.page_item(p, i);
        self.opt_cursor = i;
        self.tv.footer = Some(i);
        self.tv.text_focus = matches!(item, Item::Text { .. }).then_some(i);
        let kind = self.tv_kind(item, &g);
        let disabled = self.item_disabled(item, cv) || self.game_locked(item, cv);
        if disabled && kind != Kind::Sub {
            out.push(Action::Sound("misc/menu3.wav"));
            return;
        }
        let ctrl = self.tv_ctrl_rect(&g, self.tv_row_rect(p, i), kind);
        let enter = |m: &mut Self, cv: &mut Cvars, out: &mut Vec<Action>| {
            out.extend(m.key(K_ENTER, cv, server_active));
        };
        match kind {
            Kind::Heading | Kind::Value => {}
            Kind::Slider | Kind::Stepper => {
                let (minus, plus, _) = Self::tv_steps(ctrl, kind);
                let dir = if inside(minus, pos) { -1 } else if inside(plus, pos) { 1 } else { 0 };
                if dir != 0 {
                    if self.adjust(item, dir, false, cv, out) {
                        out.push(Action::Sound("misc/menu3.wav"));
                    }
                } else if kind == Kind::Slider && inside(Self::tv_track(ctrl), pos) {
                    self.slider_drag = Some(i);
                    self.tv_set_slider(p, i, pos.0, cv);
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Kind::Toggle => {
                if self.adjust(item, 1, true, cv, out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Kind::Segments(n) => {
                let sw = ctrl[2] / n as f32;
                let k = if inside(ctrl, pos) {
                    (((pos.0 - ctrl[0]) / sw) as usize).min(n - 1)
                } else {
                    // a tap on the label steps to the next value
                    (self.option_index(item, cv) + 1) % n
                };
                self.set_option(item, k, cv);
                out.push(Action::Sound("misc/menu3.wav"));
            }
            Kind::List => {
                let cur = self.option_index(item, cv) as f32;
                let mut s = Sheet { row: i, scroll: 0.0 };
                let (_, _, vis) = self.tv_sheet_geom(s);
                s.scroll = ((cur - (vis as f32 - 1.0) / 2.0) * SHEET_ROW).clamp(0.0, self.tv_sheet_max_scroll(s));
                self.tv.sheet = Some(s);
                out.push(Action::Sound("misc/menu2.wav"));
            }
            // text rows take the OS keyboard while they are the cursor row
            Kind::Text => {}
            _ => enter(self, cv, out),
        }
    }

    /// Where a slider's knob can go: the track minus the knob's radius.
    fn tv_track(ctrl: Rect) -> Rect {
        let (_, _, t) = Self::tv_steps(ctrl, Kind::Slider);
        t
    }

    /// Sets slider row `i` from a pointer x, snapped to its step.
    pub(super) fn tv_set_slider(&mut self, p: Page, i: usize, x: f32, cv: &mut Cvars) {
        let item = self.page_item(p, i);
        if self.item_disabled(item, cv) {
            return;
        }
        let g = self.tv_geom(p);
        let t = Self::tv_track(self.tv_ctrl_rect(&g, self.tv_row_rect(p, i), Kind::Slider));
        let f = ((x - t[0] - 12.0) / (t[2] - 24.0).max(1.0)).clamp(0.0, 1.0);
        match item {
            Item::Slider { cvar, min, max, step, invert, .. } => {
                let f = if invert { 1.0 - f } else { f };
                let v = min + ((f * (max - min)) / step).round() * step;
                set_num(cv, cvar, v.clamp(min, max));
            }
            Item::ShaderParam(k) => {
                if let Some(sp) = self.shader_params.get(k) {
                    let v = sp.min + f * (sp.max - sp.min);
                    self.set_shader_param(k, v, cv);
                }
            }
            _ => {}
        }
    }

    /// Slider row whose track is under the point (a drag there moves it).
    pub(in crate::menu) fn tv_slider_at(&self, pos: (f32, f32), cv: &Cvars) -> Option<usize> {
        let Screen::OptionsPage(p) = self.screen else { return None };
        if self.tv_sheet().is_some() || self.tv_dialog_open() {
            return None;
        }
        let g = self.tv_geom(p);
        let i = self.tv_row_at(p, pos)?;
        let item = self.page_item(p, i);
        let kind = self.tv_kind(item, &g);
        let ctrl = self.tv_ctrl_rect(&g, self.tv_row_rect(p, i), kind);
        let [x, y, w, h] = Self::tv_track(ctrl);
        (kind == Kind::Slider && !self.item_disabled(item, cv) && inside([x - 6.0, y - 6.0, w + 12.0, h + 12.0], pos)).then_some(i)
    }

    /// Footer bar and its Reset button, while the tapped row has
    /// something to explain or reset.
    pub(super) fn tv_footer_rects(&self, cv: &Cvars, p: Page, g: &Geom) -> Option<(Rect, Option<Rect>)> {
        let i = self.tv_footer().filter(|&i| i < self.page_len(p))?;
        let item = self.page_item(p, i);
        let reset = self.changed(item, cv) && !self.item_disabled(item, cv) && !self.game_locked(item, cv);
        if !reset && self.tooltip_text(item, cv).is_none() {
            return None;
        }
        let (_, h) = self.tv_size();
        let bar = [g.x0, h - FOOT_H - 6.0, g.cw, FOOT_H];
        let btn = reset.then(|| [bar[0] + bar[2] - RESET_W - 10.0, bar[1] + 9.0, RESET_W, bar[3] - 18.0]);
        Some((bar, btn))
    }

    pub(super) fn tv_draw_footer(&self, c: &mut Canvas, cv: &Cvars, p: Page, g: &Geom) {
        let Some((bar, reset)) = self.tv_footer_rects(cv, p, g) else { return };
        let item = self.page_item(p, self.tv_footer().unwrap_or(0));
        c.round_box(bar[0], bar[1], bar[2], bar[3], 8.0, DIALOG_BG, Some(BORDER));
        let room = bar[2] - 28.0 - reset.map_or(0.0, |_| RESET_W + 10.0);
        if let Some(s) = self.tooltip_text(item, cv) {
            let mut size = SMALL;
            let mut lines: Vec<String> = s.lines().flat_map(|l| navigation::wrap_tooltip_lines(c, l, room, size)).collect();
            if lines.len() > 2 {
                size = 12.0;
                lines = s.lines().flat_map(|l| navigation::wrap_tooltip_lines(c, l, room, size)).collect();
            }
            let n = lines.len().min(3);
            let lh = size + 4.0;
            let y0 = bar[1] + (bar[3] - n as f32 * lh) / 2.0 + 1.0;
            for (k, line) in lines.iter().take(n).enumerate() {
                c.text_raw(bar[0] + 14.0, y0 + k as f32 * lh, line, size, [0.78, 0.75, 0.70, 1.0]);
            }
        }
        if let Some(r) = reset {
            tv_button(c, r, qcore::t!("Reset"), if self.tv_pressed(r) { Look::Pressed } else { Look::Primary });
        }
    }

    /// Sheet panel, its first option row's top and the rows shown at once.
    pub(super) fn tv_sheet_geom(&self, s: Sheet) -> (Rect, f32, usize) {
        let (w, h) = self.tv_size();
        let Screen::OptionsPage(p) = self.screen else { return ([0.0; 4], 0.0, 0) };
        let n = self.options(self.page_item(p, s.row)).len();
        let vis = (((h - 2.0 * MARGIN - SHEET_TITLE - 8.0) / SHEET_ROW).floor() as usize).max(1).min(n.max(1));
        let pw = SHEET_W.min(w - 2.0 * MARGIN);
        let ph = SHEET_TITLE + vis as f32 * SHEET_ROW + 8.0;
        let panel = [(w - pw) / 2.0, (h - ph) / 2.0, pw, ph];
        (panel, panel[1] + SHEET_TITLE, vis)
    }

    pub(super) fn tv_sheet_max_scroll(&self, s: Sheet) -> f32 {
        let Screen::OptionsPage(p) = self.screen else { return 0.0 };
        let n = self.options(self.page_item(p, s.row)).len();
        let (_, _, vis) = self.tv_sheet_geom(s);
        n.saturating_sub(vis) as f32 * SHEET_ROW
    }

    pub(super) fn tv_draw_sheet(&self, c: &mut Canvas, cv: &Cvars, p: Page, s: Sheet) {
        let (w, h) = self.tv_size();
        c.fill(-c.ox, -c.oy, w + 2.0 * c.ox + 1.0, h + 2.0 * c.oy + 1.0, [0.0, 0.0, 0.0, 0.6]);
        let item = self.page_item(p, s.row);
        let (panel, top, vis) = self.tv_sheet_geom(s);
        c.round_box(panel[0], panel[1], panel[2], panel[3], 10.0, DIALOG_BG, Some(BORDER));
        let title = self.tv_label(item);
        fit_left(c, panel[0] + 20.0, panel[1] + (SHEET_TITLE - 20.0) / 2.0, &title, 20.0, panel[2] - 40.0, GOLD_FG);
        c.fill(panel[0] + 12.0, top - 1.0, panel[2] - 24.0, 1.0, [GOLD_FG[0], GOLD_FG[1], GOLD_FG[2], 0.35]);
        let cur = self.option_index(item, cv);
        c.clip_rows(Some((top, top + vis as f32 * SHEET_ROW)));
        for (k, o) in self.options(item).iter().enumerate() {
            let r = [panel[0] + 4.0, top + k as f32 * SHEET_ROW - s.scroll, panel[2] - 8.0, SHEET_ROW];
            if r[1] + r[3] < top || r[1] > top + vis as f32 * SHEET_ROW {
                continue;
            }
            if self.tv_pressed(r) {
                c.fill(r[0], r[1], r[2], r[3], PRESS);
            }
            // radio mark, filled on the current value
            let (cx, cy) = (r[0] + 28.0, r[1] + SHEET_ROW / 2.0);
            c.round_frame([cx - 10.0, cy - 10.0, 20.0, 20.0], 10.0, 2.0, if k == cur { GOLD_FG } else { MUTED });
            if k == cur {
                c.round_box(cx - 5.0, cy - 5.0, 10.0, 10.0, 5.0, GOLD_FG, None);
            }
            let col = if k == cur { GOLD_FG } else { FG };
            fit_left(c, r[0] + 52.0, r[1] + (SHEET_ROW - LABEL) / 2.0, o, LABEL, r[2] - 64.0, col);
        }
        c.clip_rows(None);
        let n = self.options(item).len();
        if n > vis {
            // scroll position along the panel's right edge
            let track = vis as f32 * SHEET_ROW;
            let th = (track * vis as f32 / n as f32).max(24.0);
            let ty = top + (track - th) * s.scroll / self.tv_sheet_max_scroll(s).max(1.0);
            c.round_box(panel[0] + panel[2] - 8.0, ty, 4.0, th, 2.0, [0.85, 0.65, 0.3, 0.8], None);
        }
    }

    pub(super) fn tv_sheet_click(&mut self, cv: &mut Cvars, out: &mut Vec<Action>) {
        let (Some(s), Some(pos), Screen::OptionsPage(p)) = (self.tv.sheet, self.mouse, self.screen) else { return };
        let (panel, top, vis) = self.tv_sheet_geom(s);
        if !inside(panel, pos) {
            self.tv.sheet = None;
            out.push(Action::Sound("misc/menu3.wav"));
            return;
        }
        if pos.1 < top || pos.1 >= top + vis as f32 * SHEET_ROW {
            return;
        }
        let k = ((pos.1 - top + s.scroll) / SHEET_ROW) as usize;
        let item = self.page_item(p, s.row);
        if k < self.options(item).len() {
            self.set_option(item, k, cv);
            self.tv.sheet = None;
            out.push(Action::Sound("misc/menu3.wav"));
        }
    }
}
