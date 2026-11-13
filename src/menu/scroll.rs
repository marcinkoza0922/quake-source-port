use super::*;

impl Menu {
    /// Rows on a page: its static items, plus found servers on Join.
    pub(super) fn page_len(&self, p: Page) -> usize {
        page_items(p).len()
            + match p {
                Page::Join => self.servers.len(),
                Page::ScreenShader => self.shader_params.len(),
                _ => 0,
            }
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
        self.slider_drag = None;
    }

    /// Sets a slider from a pointer x (the knob centres on it), snapped to its step.
    pub(super) fn set_slider(&self, item: Item, x: f32, cv: &mut Cvars) {
        if let Item::ShaderParam(k) = item {
            return self.drag_shader_param(k, x, cv);
        }
        let Item::Slider { cvar, min, max, step, invert, .. } = item else { return };
        let mut f = ((x - SLIDER_X.0 - 4.0) / (SLIDER_X.1 - SLIDER_X.0 - 8.0)).clamp(0.0, 1.0);
        if invert {
            f = 1.0 - f;
        }
        let v = min + ((f * (max - min)) / step).round() * step;
        set_num(cv, cvar, v.clamp(min, max));
    }

    /// Menu y of a page row (may be off the visible window).
    pub(super) fn page_row_y(&self, p: Page, i: usize) -> f32 {
        PAGE_ROW0 + (i as f32 - self.page_scroll(p) as f32) * ROW_H
    }

    pub(super) fn page_item(&self, p: Page, i: usize) -> Item {
        let items = page_items(p);
        let k = i.saturating_sub(items.len());
        items.get(i).copied().unwrap_or(if p == Page::ScreenShader { Item::ShaderParam(k) } else { Item::Server(k) })
    }
}

/// Save/load card list: on Load the newest save is pinned at the top and the
/// rest scroll in pixels below a divider.
impl Menu {
    fn card_sticky(&self) -> usize {
        usize::from(self.screen == Screen::Load)
    }

    pub(super) fn card_top(&self) -> f32 {
        if self.card_sticky() > 0 { CARD_SCROLL_Y0 } else { CARD_Y0 }
    }

    pub(super) fn card_max_scroll(&self) -> f32 {
        let rows = self.save_rows().saturating_sub(self.card_sticky());
        let content = rows as f32 * CARD_PITCH - (CARD_PITCH - CARD_H);
        (content - (CARD_BOTTOM - self.card_top())).max(0.0)
    }

    pub(super) fn card_scroll(&self) -> f32 {
        self.save_scroll.clamp(0.0, self.card_max_scroll())
    }

    /// Top of card `i` in menu units (may be outside the visible region).
    pub(super) fn card_y(&self, i: usize) -> f32 {
        let s = self.card_sticky();
        if i < s { CARD_Y0 } else { self.card_top() + (i - s) as f32 * CARD_PITCH - self.card_scroll() }
    }

    /// Whether card `i` at least partly shows.
    pub(super) fn card_visible(&self, i: usize) -> bool {
        let y = self.card_y(i);
        i < self.card_sticky() || (y + CARD_H > self.card_top() && y < CARD_BOTTOM)
    }

    /// Card under a menu-space point.
    pub(super) fn card_at(&self, x: f32, y: f32) -> Option<usize> {
        if !(CARD_X..CARD_X + CARD_W).contains(&x) {
            return None;
        }
        let s = self.card_sticky();
        let i = if s > 0 && (CARD_Y0..CARD_Y0 + CARD_H).contains(&y) {
            0
        } else if (self.card_top()..CARD_BOTTOM).contains(&y) {
            let rel = y - self.card_top() + self.card_scroll();
            let row = (rel / CARD_PITCH).floor();
            if rel - row * CARD_PITCH >= CARD_H {
                return None;
            }
            s + row as usize
        } else {
            return None;
        };
        (i < self.save_rows()).then_some(i)
    }

    pub(super) fn scroll_cards(&mut self, dy: f32) {
        self.save_scroll = (self.card_scroll() + dy).clamp(0.0, self.card_max_scroll());
    }

    /// Scrolls just enough to show the cursor card whole.
    pub(super) fn fix_card_scroll(&mut self) {
        let s = self.card_sticky();
        let mut sc = self.card_scroll();
        if self.load_cursor >= s {
            let top = (self.load_cursor - s) as f32 * CARD_PITCH;
            sc = sc.min(top).max(top + CARD_H - (CARD_BOTTOM - self.card_top()));
        }
        self.save_scroll = sc.clamp(0.0, self.card_max_scroll());
    }

    /// Card scrollbar: (x, track top, track h, thumb y, thumb h).
    pub(super) fn card_scrollbar(&self) -> Option<(f32, f32, f32, f32, f32)> {
        let max = self.card_max_scroll();
        if max <= 0.0 {
            return None;
        }
        let (top, h) = (self.card_top(), CARD_BOTTOM - self.card_top());
        let th = (h * h / (h + max)).max(12.0);
        Some((CARD_X + CARD_W + 8.0, top, h, top + (h - th) * self.card_scroll() / max, th))
    }

    pub(super) fn card_drag_to(&mut self, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.card_scrollbar() else { return };
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.save_scroll = f * self.card_max_scroll();
    }
}
