//! Touch Options hub: a search field over a grid of the option pages, or
//! the search results as rows.

use super::*;

const FIELD_H: f32 = 48.0;
const CARD_H: f32 = 64.0;
const RESULT_H: f32 = 60.0;
const GAP: f32 = 10.0;

impl Menu {
    fn tv_hub_top(&self) -> f32 {
        BAR_H + 4.0 - self.tv_scroll()
    }

    pub(super) fn tv_search_rect(&self) -> Rect {
        let (x0, cw) = self.tv_col();
        [x0, self.tv_hub_top(), cw, FIELD_H]
    }

    /// Clear button at the field's right end (with a query).
    fn tv_search_clear(&self) -> Rect {
        let [x, y, w, h] = self.tv_search_rect();
        [x + w - h, y, h, h]
    }

    fn tv_hub_cols(&self) -> usize {
        let (_, cw) = self.tv_col();
        if cw >= 540.0 { 3 } else if cw >= 300.0 { 2 } else { 1 }
    }

    /// Page cards of the grid.
    pub(super) fn tv_hub_cards(&self) -> Vec<(Page, &'static str, Rect)> {
        let (x0, cw) = self.tv_col();
        let cols = self.tv_hub_cols();
        let tw = (cw - GAP * (cols - 1) as f32) / cols as f32;
        let top = self.tv_hub_top() + FIELD_H + 12.0;
        PAGES
            .iter()
            .enumerate()
            .map(|(k, &(label, p))| {
                let (col, row) = (k % cols, k / cols);
                (p, label, [x0 + col as f32 * (tw + GAP), top + row as f32 * (CARD_H + GAP), tw, CARD_H])
            })
            .collect()
    }

    fn tv_result_rect(&self, k: usize) -> Rect {
        let (x0, cw) = self.tv_col();
        [x0, self.tv_hub_top() + FIELD_H + 12.0 + k as f32 * RESULT_H, cw, RESULT_H]
    }

    pub(super) fn tv_hub_h(&self) -> f32 {
        let below = if self.searching() {
            self.search_results().len().max(1) as f32 * RESULT_H
        } else {
            PAGES.len().div_ceil(self.tv_hub_cols()) as f32 * (CARD_H + GAP)
        };
        4.0 + FIELD_H + 12.0 + below
    }

    pub(super) fn tv_draw_hub(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        let (w, h) = self.tv_size();
        c.clip_rows(Some((BAR_H, h)));
        // search field
        let f = self.tv_search_rect();
        let focus = self.tv_search_focus();
        c.round_box(f[0], f[1], f[2], f[3], 6.0, [0.0, 0.0, 0.0, 0.75], Some(if focus { GOLD_FG } else { BORDER }));
        let ty = f[1] + (FIELD_H - VALUE) / 2.0;
        let q = &self.search.query;
        if q.is_empty() {
            fit_left(c, f[0] + 14.0, ty, "Type to search settings...", VALUE, f[2] - 28.0, MUTED);
        } else {
            let tw = clip_raw(c, f[0] + 14.0, ty, q, VALUE, f[2] - FIELD_H - 20.0, FG);
            if focus && (realtime * 2.0) as i64 & 1 == 0 {
                c.fill(f[0] + 15.0 + tw, ty, 1.5, VALUE, FG);
            }
            let x = self.tv_search_clear();
            c.text_raw(x[0] + (x[2] - c.text_w_raw("×", 22.0)) / 2.0, x[1] + (x[3] - 22.0) / 2.0 - 1.0, "×", 22.0, GOLD_FG);
        }
        if q.is_empty() && focus && (realtime * 2.0) as i64 & 1 == 0 {
            c.fill(f[0] + 14.0, ty, 1.5, VALUE, FG);
        }
        if self.searching() {
            let hits = self.search_results();
            if hits.is_empty() {
                let r = self.tv_result_rect(0);
                fit_center(c, r[0] + r[2] / 2.0, r[1] + (RESULT_H - VALUE) / 2.0, "No matching settings", VALUE, r[2], MUTED);
            }
            if !hits.is_empty() {
                let all = self.tv_result_rect(0);
                c.round_box(all[0], all[1], all[2], hits.len() as f32 * RESULT_H, 8.0, PANEL_BG, Some(BORDER));
            }
            for (k, hit) in hits.iter().enumerate() {
                let r = self.tv_result_rect(k);
                if r[1] > h || r[1] + r[3] < BAR_H {
                    continue;
                }
                if self.tv_pressed(r) {
                    c.fill(r[0] + 1.0, r[1], r[2] - 2.0, r[3], PRESS);
                }
                if k > 0 {
                    c.fill(r[0] + 12.0, r[1], r[2] - 24.0, 1.0, LINE);
                }
                let dim = self.hit_dimmed(hit, cv);
                let col = if dim { MUTED } else if hit.opens() { GOLD_FG } else { FG };
                let trail = hit.trail();
                let ly = if trail.is_empty() { r[1] + (RESULT_H - LABEL) / 2.0 } else { r[1] + 9.0 };
                fit_left(c, r[0] + 16.0, ly, hit.label, LABEL, r[2] - 56.0, col);
                if trail.is_empty() {
                    c.shape(r[0] + r[2] - 28.0, r[1] + RESULT_H / 2.0 - 6.0, 12.0, 12.0, crate::font::Shape::ArrowRight, col);
                } else {
                    let s = trail.iter().map(|p| qcore::lang::tr(p).into_owned()).collect::<Vec<_>>().join("  ›  ");
                    clip_raw(c, r[0] + 16.0, r[1] + 34.0, &s, SMALL, r[2] - 32.0, MUTED);
                }
            }
        } else {
            for (_, label, r) in self.tv_hub_cards() {
                tv_button(c, r, label, if self.tv_pressed(r) { Look::Pressed } else { Look::Primary });
            }
        }
        c.clip_rows(None);
        if let Some(p @ (_, pw, ph)) = pic("gfx/p_option.lmp") {
            let k = ((BAR_H - 14.0) / ph as f32).min(2.0);
            let (dw, dh) = (pw as f32 * k, ph as f32 * k);
            let (x0, cw) = self.tv_col();
            // centred, unless it would reach the Back button
            let x = ((w - dw) / 2.0).max(x0 + 124.0).min(x0 + cw - dw);
            c.pic_stretch_px([(x + c.ox) * c.scale, (7.0 + c.oy) * c.scale, dw * c.scale, dh * c.scale], p.0);
        }
    }

    pub(super) fn tv_hub_click(&mut self, pos: (f32, f32), cv: &mut Cvars, out: &mut Vec<Action>) {
        if pos.1 < BAR_H {
            return;
        }
        if !self.search.query.is_empty() && inside(self.tv_search_clear(), pos) {
            self.clear_search();
            self.tv.search_focus = true;
            out.push(Action::Sound("misc/menu3.wav"));
            return;
        }
        if inside(self.tv_search_rect(), pos) {
            self.tv.search_focus = true;
            return;
        }
        self.tv.search_focus = false;
        if self.searching() {
            if let Some(k) = (0..self.search_results().len()).find(|&k| inside(self.tv_result_rect(k), pos)) {
                self.search.cursor = k;
                self.search_pick(k, cv, out);
            }
            return;
        }
        let Some((k, p)) = self.tv_hub_cards().into_iter().enumerate().find(|(_, (_, _, r))| inside(*r, pos)).map(|(k, (p, ..))| (k, p)) else {
            return;
        };
        self.page_cursor = k;
        out.push(Action::Sound("misc/menu2.wav"));
        if p == Page::Graphics {
            out.push(Action::Command("_vid_menu\n".into()));
        }
        self.opt_cursor = 0;
        self.screen = Screen::OptionsPage(p);
    }
}
