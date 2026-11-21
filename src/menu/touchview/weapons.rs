//! Touch Weapon Priority: the eight weapons as rows with up and down
//! buttons (a drag scrolls on touch, so rows don't drag), then Reset.

use qserver::weaponswitch::{DEFAULT_PRIORITY, read_priority};

use super::*;
use crate::menu::weapon_priority::{WP_COUNT, move_entry, save_order, slot_for_impulse};

const HEAD_H: f32 = 32.0;
const BTN_W: f32 = 56.0;

impl Menu {
    fn tv_wp_top(&self) -> f32 {
        BAR_H + HEAD_H - self.tv_scroll()
    }

    /// Row `i` and its up / down buttons.
    pub(super) fn tv_wp_rects(&self, i: usize) -> (Rect, Rect, Rect) {
        let (x0, cw) = self.tv_col();
        let y = self.tv_wp_top() + i as f32 * ROW;
        let down = [x0 + cw - BTN_W - 6.0, y + 5.0, BTN_W, ROW - 10.0];
        let up = [down[0] - BTN_W - 8.0, y + 5.0, BTN_W, ROW - 10.0];
        ([x0, y, cw, ROW], up, down)
    }

    pub(super) fn tv_wp_reset(&self) -> Rect {
        let (x0, cw) = self.tv_col();
        let bw = (cw - 32.0).min(360.0);
        [x0 + (cw - bw) / 2.0, self.tv_wp_top() + WP_COUNT as f32 * ROW + 12.0, bw, 48.0]
    }

    pub(super) fn tv_wp_h(&self) -> f32 {
        HEAD_H + WP_COUNT as f32 * ROW + 12.0 + 48.0
    }

    pub(super) fn tv_draw_wp(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars) {
        let (_, h) = self.tv_size();
        let (x0, cw) = self.tv_col();
        self.tv_title(c, page_name(Page::WeaponPriority));
        c.clip_rows(Some((BAR_H, h)));
        fit_center(c, x0 + cw / 2.0, self.tv_wp_top() - HEAD_H + (HEAD_H - SMALL) / 2.0 - 2.0, qcore::t!("Highest priority at top"), SMALL, cw, MUTED);
        let order = read_priority(cv);
        let (first, ..) = self.tv_wp_rects(0);
        c.round_box(first[0], first[1], first[2], WP_COUNT as f32 * ROW, 8.0, PANEL_BG, Some(BORDER));
        for (i, &imp) in order.iter().enumerate() {
            let (row, up, down) = self.tv_wp_rects(i);
            if i > 0 {
                c.fill(row[0] + 12.0, row[1], row[2] - 24.0, 1.0, LINE);
            }
            if self.tv.footer == Some(i) {
                c.fill(row[0] + 1.0, row[1], row[2] - 2.0, row[3], SELECT);
            }
            let ty = row[1] + (ROW - LABEL) / 2.0;
            c.text_raw(row[0] + 16.0, ty, &format!("{}.", i + 1), LABEL, MUTED);
            let slot = slot_for_impulse(imp);
            let ix = row[0] + 52.0;
            match slot.icon.and_then(&mut *pic) {
                Some((id, w, ph)) => {
                    let k = (24.0 / ph.max(1) as f32).min(48.0 / w.max(1) as f32);
                    let (dw, dh) = (w as f32 * k, ph as f32 * k);
                    let (px, py) = ((ix + (48.0 - dw) / 2.0 + c.ox) * c.scale, (row[1] + (ROW - dh) / 2.0 + c.oy) * c.scale);
                    c.pic_stretch_px([px, py, dw * c.scale, dh * c.scale], id);
                }
                None => {
                    c.round_box(ix + 8.0, row[1] + 14.0, 32.0, 24.0, 4.0, [0.18, 0.15, 0.12, 0.9], Some(BORDER));
                    c.text_raw(ix + 13.0, row[1] + 18.0, "AX", 14.0, GOLD_FG);
                }
            }
            fit_left(c, row[0] + 116.0, ty, slot.name, LABEL, up[0] - row[0] - 128.0, FG);
            for (r, enabled, shape) in [(up, i > 0, crate::font::Shape::ArrowUp), (down, i + 1 < WP_COUNT, crate::font::Shape::ArrowDown)] {
                let bg = if enabled && self.tv_pressed(r) { PRESS } else { [0.0, 0.0, 0.0, 0.6] };
                c.round_box(r[0], r[1], r[2], r[3], 6.0, bg, Some(if enabled { BORDER } else { MUTED }));
                c.shape(r[0] + r[2] / 2.0 - 8.0, r[1] + r[3] / 2.0 - 7.0, 16.0, 14.0, shape, if enabled { GOLD_FG } else { MUTED });
            }
        }
        let r = self.tv_wp_reset();
        let custom = cv.by_name("cl_weaponpriority").is_some_and(|v| v.string != DEFAULT_PRIORITY);
        tv_button(c, r, qcore::t!("Reset to Defaults"), if !custom { Look::Dim } else if self.tv_pressed(r) { Look::Pressed } else { Look::Primary });
        c.clip_rows(None);
    }

    pub(super) fn tv_wp_click(&mut self, pos: (f32, f32), cv: &mut Cvars, out: &mut Vec<Action>) {
        if pos.1 < BAR_H {
            return;
        }
        if inside(self.tv_wp_reset(), pos) {
            cv.set("cl_weaponpriority", DEFAULT_PRIORITY);
            out.push(Action::Sound("misc/menu2.wav"));
            return;
        }
        for i in 0..WP_COUNT {
            let (row, up, down) = self.tv_wp_rects(i);
            let to = if inside(up, pos) && i > 0 {
                i - 1
            } else if inside(down, pos) && i + 1 < WP_COUNT {
                i + 1
            } else if inside(row, pos) {
                self.tv.footer = Some(i);
                return;
            } else {
                continue;
            };
            let mut order = read_priority(cv);
            move_entry(&mut order, i, to);
            save_order(cv, &order);
            // the moved weapon stays highlighted
            self.tv.footer = Some(to);
            self.wp.cursor = to;
            out.push(Action::Sound("misc/menu1.wav"));
            return;
        }
    }
}
