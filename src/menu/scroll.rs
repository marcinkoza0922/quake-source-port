use super::*;

impl Menu {
    pub(super) fn sync_autoaim_mapping(&self, cv: &Cvars) {
        self.autoaim_mapping.set(if cv.value_of("assist_autoaim") == 0.0 { 0 }
            else if cv.value_of("assist_autoaim_mode") == 1.0 { 2 } else { 1 });
    }

    /// Number of options on a page, including contextual rows.
    pub(super) fn page_len(&self, p: Page) -> usize {
        page_items(p).len() + usize::from(p == Page::Controls && self.autoaim_mapping.get() != 0)
            + match p {
                Page::ScreenShader => self.shader_params.len(),
                _ => 0,
            }
    }

    /// Visible rows of a page.
    pub(super) fn page_vis(&self, p: Page) -> usize {
        if p == Page::Touch {
            return self.page_len(p);
        }
        let rows = if self.touch_ui {
            match p {
                Page::Setup => 9,
                Page::Customize | Page::NewGameCustomize | Page::Modifiers | Page::Export | Page::RenameDemo => 10,
                _ => 11,
            }
        } else {
            PAGE_ROWS
        };
        self.page_len(p).min(rows)
    }

    pub(super) fn page_row_height(&self) -> f32 {
        if self.touch_ui { 20.0 } else { ROW_H }
    }

    pub(super) fn page_bottom(&self, p: Page) -> f32 {
        PAGE_ROW0 + self.page_vis(p) as f32 * self.page_row_height()
    }

    /// First visible row, moved just enough to keep the cursor on screen.
    pub(super) fn page_scroll(&self, p: Page) -> usize {
        let n = self.page_len(p);
        let vis = self.page_vis(p);
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

    pub(super) fn page_offset(&self, p: Page) -> f32 {
        self.touch_scroll.filter(|(page, _)| *page == p)
            .map_or_else(|| self.page_scroll(p) as f32 * self.page_row_height(), |(_, y)| y)
    }

    /// Pixel scrolling for the shared options panel and save cards.
    /// Other list widgets already own row-based scrolling.
    pub fn scroll_touch(&mut self, dy: f32) -> Option<bool> {
        if self.tv_input() {
            return Some(self.tv_scroll_by(dy));
        }
        if self.prompt.is_some() || self.osk.is_some() || self.capturing() || self.autoaim_prompt {
            return Some(false);
        }
        if self.drop.is_some() {
            return None;
        }
        match self.screen {
            Screen::OptionsPage(p) if !matches!(p, Page::Touch | Page::WeaponPriority | Page::Addons | Page::NewGameAddons) => {
                let max = self.page_len(p) - self.page_vis(p);
                let height = self.page_row_height();
                let before = self.page_offset(p);
                let next = (before + dy).clamp(0.0, max as f32 * height);
                self.set_page_scroll(p, (next / height) as usize);
                self.touch_scroll = Some((p, next));
                Some(before != next)
            }
            Screen::Load | Screen::Save => {
                let before = self.card_scroll();
                self.scroll_cards(dy);
                Some(before != self.card_scroll())
            }
            Screen::Mods | Screen::ModMaps | Screen::ModOnline | Screen::Demos | Screen::DirBrowser
                | Screen::ServerBrowser | Screen::Help => None,
            Screen::Options if self.searching() => None,
            _ => Some(false),
        }
    }

    pub fn set_touch_ui(&mut self, active: bool) {
        if active != self.touch_ui || !self.touch_scroll.is_some_and(|(p, _)| self.screen == Screen::OptionsPage(p)) {
            self.touch_scroll = None;
        }
        if !active {
            self.tv_leave();
        }
        self.touch_ui = active;
    }

    /// Scrollbar of a long options page: (x, track top, track h, thumb y, thumb h).
    pub(super) fn scrollbar(&self, p: Page) -> Option<(f32, f32, f32, f32, f32)> {
        let (total, n) = (self.page_len(p), self.page_vis(p));
        if total <= n {
            return None;
        }
        let sc = self.page_offset(p) / self.page_row_height();
        let (x, top, h) = (PANEL_X + PANEL_W - SCROLL_W - 2.0, PAGE_ROW0, n as f32 * self.page_row_height());
        let th = (h * n as f32 / total as f32).max(8.0);
        let ty = top + (h - th) * sc / (total - n) as f32;
        Some((x, top, h, ty, th))
    }

    /// Sets the first visible row, pulling the cursor into view.
    pub(super) fn set_page_scroll(&mut self, p: Page, sc: usize) {
        self.touch_scroll = None;
        let (total, vis) = (self.page_len(p), self.page_vis(p));
        if vis == 0 {
            return;
        }
        let sc = sc.min(total - vis);
        self.opt_scroll.set(sc);
        self.opt_cursor = self.opt_cursor.clamp(sc, sc + vis - 1);
        // off a heading, staying in view
        self.skip_headings(p, 1);
        if self.opt_cursor >= sc + vis {
            self.opt_cursor = sc + vis - 1;
            self.skip_headings(p, -1);
        }
    }

    /// Moves the cursor off heading rows in direction `dir` (wrapping).
    pub(super) fn skip_headings(&mut self, p: Page, dir: isize) {
        let n = self.page_len(p);
        for _ in 0..n {
            if !matches!(self.page_item(p, self.opt_cursor.min(n - 1)), Item::Heading(_)) {
                return;
            }
            self.opt_cursor = (self.opt_cursor as isize + dir).rem_euclid(n as isize) as usize;
        }
    }

    /// Keeps the options cursor on a selectable row after any input.
    pub(super) fn settle_cursor(&mut self) {
        if let Screen::OptionsPage(p) = self.screen {
            self.skip_headings(p, 1);
        }
    }

    /// Scrollbar drag: thumb top follows the pointer.
    pub(super) fn drag_to(&mut self, p: Page, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.scrollbar(p) else { return };
        let max = self.page_len(p) - self.page_vis(p);
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.set_page_scroll(p, (f * max as f32).round() as usize);
    }

    pub fn mouse_up(&mut self) {
        self.hud_edit.cancel_dialog_drag();
        self.wp.mouse_up();
        self.touch_editor.release();
        self.drag = None;
        self.slider_drag = None;
        self.text_drag = None;
    }

    /// Sets a slider from a pointer x (the knob centres on it), snapped to its step.
    pub(super) fn set_slider(&self, item: Item, x: f32, cv: &mut Cvars) {
        if self.item_disabled(item, cv) { return; }
        if let Item::ShaderParam(k) = item {
            return self.drag_shader_param(k, x, cv);
        }
        let Item::Slider { cvar, min, max, step, invert, .. } = item else { return };
        let (start, end) = if self.screen == Screen::OptionsPage(Page::Touch) {
            touch::SLIDER
        } else {
            (SLIDER_X.0 + 4.0, SLIDER_X.1 - 4.0)
        };
        let mut f = ((x - start) / (end - start)).clamp(0.0, 1.0);
        if invert {
            f = 1.0 - f;
        }
        let v = min + ((f * (max - min)) / step).round() * step;
        set_num(cv, cvar, v.clamp(min, max));
    }

    /// Menu y of a page row (may be off the visible window).
    pub(super) fn page_row_y(&self, p: Page, i: usize) -> f32 {
        PAGE_ROW0 + i as f32 * self.page_row_height() - self.page_offset(p)
    }

    pub(super) fn page_item(&self, p: Page, i: usize) -> Item {
        let items = page_items(p);
        // Keep the contextual action beside Attack, before the weapon and movement binds.
        let i = if p == Page::Controls && self.autoaim_mapping.get() != 0 {
            if i == 1 {
                return bind(if self.autoaim_mapping.get() == 2 { qcore::t!("Auto-aim Toggle") } else { qcore::t!("Lock-on") }, "+lockon");
            }
            if i > 1 { i - 1 } else { i }
        } else { i };
        items.get(i).copied().unwrap_or(Item::ShaderParam(i.saturating_sub(items.len())))
    }
}

/// Save/load card list: on Load (not Autosaves) the newest save is pinned at the top and the
/// rest scroll in pixels below a divider.
impl Menu {
    pub(super) fn card_sticky(&self) -> usize {
        usize::from(self.screen == Screen::Load && !self.autosaves)
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
