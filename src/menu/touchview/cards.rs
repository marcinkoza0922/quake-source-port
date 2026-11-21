//! Touch Load/Save screens: the classic save cards, scaled up to fill the
//! column, in one scrolling list (the newest save first, not pinned), each
//! own save with a Delete button beside it. A tap on a card loads or
//! saves, as a click does.

use super::*;

/// Delete button width, and the gap before it.
const DEL_W: f32 = 76.0;
const DEL_GAP: f32 = 8.0;
/// "Saves: n" line between the bar and the cards.
const HEAD_H: f32 = 30.0;

impl Menu {
    /// Card scale (classic units to layout units) and the card width.
    fn tv_card_scale(&self) -> (f32, f32) {
        let (_, cw) = self.tv_col();
        let w = cw - DEL_W - DEL_GAP;
        ((w / CARD_W).min(1.8), w)
    }

    fn tv_card_pitch(&self) -> f32 {
        let (k, _) = self.tv_card_scale();
        CARD_PITCH * k
    }

    /// Card `i` and its Delete button (own saves only).
    pub(super) fn tv_card_rects(&self, i: usize) -> (Rect, Option<Rect>) {
        let (x0, _) = self.tv_col();
        let (k, w) = self.tv_card_scale();
        let y = BAR_H + HEAD_H - self.tv_scroll() + i as f32 * self.tv_card_pitch();
        let card = [x0, y, w, CARD_H * k];
        let del = self.save_at(i).filter(|s| s.source == SaveSource::Own).map(|_| [x0 + w + DEL_GAP, y, DEL_W, CARD_H * k]);
        (card, del)
    }

    pub(super) fn tv_cards_h(&self) -> f32 {
        HEAD_H + self.save_rows() as f32 * self.tv_card_pitch()
    }

    pub(super) fn tv_draw_cards(&self, c: &mut Canvas, pic: &mut PicFn) {
        let (_, h) = self.tv_size();
        let (x0, cw) = self.tv_col();
        self.tv_title(c, self.cards_heading());
        let head = format!("{}  ·  {}", qcore::lang::tr("newest first"), qcore::tr!("Saves: {}", self.save_count()));
        clip_raw(c, x0 + 4.0, BAR_H + (HEAD_H - SMALL) / 2.0 - 2.0 - self.tv_scroll(), &head, SMALL, cw, MUTED);
        c.clip_rows(Some((BAR_H, h)));
        let (k, _) = self.tv_card_scale();
        let now = qcore::time::SystemTime::now();
        for i in 0..self.save_rows() {
            let (card, del) = self.tv_card_rects(i);
            if card[1] > h || card[1] + card[3] < BAR_H {
                continue;
            }
            // the classic card, drawn at its menu position through a scaled origin
            let (scale, ox, oy) = (c.scale, c.ox, c.oy);
            c.scale = scale * k;
            c.ox = (card[0] + ox) / k - CARD_X;
            c.oy = (card[1] + oy) / k;
            self.draw_card(c, pic, i, 0.0, self.tv_pressed(card), now);
            (c.scale, c.ox, c.oy) = (scale, ox, oy);
            if let Some(d) = del {
                c.round_box(d[0], d[1], d[2], d[3], 6.0, if self.tv_pressed(d) { [0.55, 0.12, 0.08, 0.95] } else { [0.0, 0.0, 0.0, 0.7] }, Some(BORDER));
                fit_center(c, d[0] + d[2] / 2.0, d[1] + (d[3] - VALUE) / 2.0, "Delete", VALUE, d[2] - 8.0, GOLD_FG);
            }
        }
        c.clip_rows(None);
    }

    pub(super) fn tv_cards_click(&mut self, pos: (f32, f32), cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) {
        if pos.1 < BAR_H {
            return;
        }
        for i in 0..self.save_rows() {
            let (card, del) = self.tv_card_rects(i);
            if del.is_some_and(|d| inside(d, pos)) {
                if let Some(sv) = self.save_at(i).cloned() {
                    self.load_cursor = i;
                    self.ask_delete(sv);
                }
                return;
            }
            if inside(card, pos) {
                self.load_cursor = i;
                out.extend(self.key(K_ENTER, cv, server_active));
                return;
            }
        }
    }
}
