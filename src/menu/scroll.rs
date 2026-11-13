use super::*;

impl Menu {
    /// Rows on a page: its static items, plus found servers on Join.
    pub(super) fn page_len(&self, p: Page) -> usize {
        page_items(p).len() + if p == Page::Join { self.servers.len() } else { 0 }
    }

    /// Visible rows of a page.
    pub(super) fn page_vis(&self, p: Page) -> usize {
        self.page_len(p).min(PAGE_ROWS)
    }

    /// First visible row, moved just enough to keep the cursor on screen.
    pub(super) fn page_scroll(&self, p: Page) -> usize {
        let n = self.page_len(p);
        let vis = n.min(PAGE_ROWS);
        let mut sc = self.opt_scroll.get().min(n - vis);
        let cur = self.opt_cursor.min(n.saturating_sub(1));
        if cur < sc {
            sc = cur;
        } else if cur >= sc + vis {
            sc = cur + 1 - vis;
        }
        self.opt_scroll.set(sc);
        sc
    }

    /// Scrollbar of a long options page: (x, track top, track h, thumb y, thumb h).
    pub(super) fn scrollbar(&self, p: Page) -> Option<(f32, f32, f32, f32, f32)> {
        let (total, n) = (self.page_len(p), self.page_vis(p));
        if total <= n {
            return None;
        }
        let sc = self.page_scroll(p);
        let (x, top, h) = (PANEL_X + PANEL_W - 5.0, PAGE_ROW0, n as f32 * ROW_H);
        let th = (h * n as f32 / total as f32).max(8.0);
        let ty = top + (h - th) * sc as f32 / (total - n) as f32;
        Some((x, top, h, ty, th))
    }

    /// Sets the first visible row, pulling the cursor into view.
    pub(super) fn set_page_scroll(&mut self, p: Page, sc: usize) {
        let (total, vis) = (self.page_len(p), self.page_vis(p));
        if vis == 0 {
            return;
        }
        let sc = sc.min(total - vis);
        self.opt_scroll.set(sc);
        self.opt_cursor = self.opt_cursor.clamp(sc, sc + vis - 1);
    }

    /// Scrollbar drag: thumb top follows the pointer.
    pub(super) fn drag_to(&mut self, p: Page, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.scrollbar(p) else { return };
        let max = self.page_len(p) - self.page_vis(p);
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.set_page_scroll(p, (f * max as f32).round() as usize);
    }

    /// Clickable Back button in the bottom-left corner (x, y, w, h).
    pub(super) fn back_button(&self) -> Option<(f32, f32, f32, f32)> {
        let hidden = matches!(self.screen, Screen::None | Screen::Main | Screen::Quit | Screen::Death)
            || self.capturing()
            || self.osk.is_some();
        (!hidden).then_some(BACK_BTN)
    }

    pub fn mouse_up(&mut self) {
        self.drag = None;
    }

    /// Menu y of a page row (may be off the visible window).
    pub(super) fn page_row_y(&self, p: Page, i: usize) -> f32 {
        PAGE_ROW0 + (i as f32 - self.page_scroll(p) as f32) * ROW_H
    }

    pub(super) fn page_item(&self, p: Page, i: usize) -> Item {
        let items = page_items(p);
        items.get(i).copied().unwrap_or(Item::Server(i.saturating_sub(items.len())))
    }
}
