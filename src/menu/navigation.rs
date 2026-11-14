//! Navigation buttons, contextual help and controller prompts.
use super::*;

impl Menu {
    /// Back follows the current menu's content; drawing and mouse input share its bounds.
    pub(super) fn back_button(&self) -> Option<(f32, f32, f32, f32)> {
        if self.capturing() || self.osk.is_some() { return None; }
        let (x, y) = match self.screen {
            Screen::SinglePlayer | Screen::Multiplayer => (CLASSIC.0 + 72.0, CLASSIC.1 + 32.0 + 3.0 * 20.0 + 8.0),
            Screen::Options => (PANEL_X, ROW0 + PAGES.len() as f32 * ROW_H + 12.0),
            Screen::OptionsPage(Page::Addons) => (20.0, 266.0),
            Screen::OptionsPage(Page::NewGameAddons) => (24.0, 294.0),
            Screen::OptionsPage(p) => {
                let bottom = PAGE_ROW0 + self.page_vis(p) as f32 * ROW_H;
                // Leave room for contextual help and the player-colour preview.
                (PANEL_X, bottom + if p == Page::Setup { 84.0 } else { 30.0 })
            }
            Screen::Load | Screen::Save => (CARD_X, CARD_BOTTOM + 4.0),
            Screen::Mods | Screen::ModMaps => self.mods.back_position(),
            _ => return None,
        };
        Some((x, y.min(MENU_H - 16.0), 60.0, 16.0))
    }

    pub(super) fn draw_addon_hints(&self, c: &mut Canvas) {
        if self.capturing() {
            self.boxed_hints(c, 344.0, &[(K_START, "Cancel binding")]);
            return;
        }
        let items = if self.browser_binding_focused() {
            vec![(K_ABUTTON, "Remap"), (K_XBUTTON, "Clear pad"), (K_BBUTTON, "Back")]
        } else {
            vec![(K_ABUTTON, self.browser_select_label()), (K_BBUTTON, "Back")]
        };
        self.boxed_hints(c, 344.0, &items);
    }

    /// One-line explanation of the selected option, just below the panel.
    pub(super) fn draw_tooltip(&self, c: &mut Canvas) {
        let Screen::OptionsPage(page) = self.screen else { return };
        if self.drop.is_some() || self.osk.is_some() || self.capturing() || self.opt_cursor >= self.page_len(page) {
            return;
        }
        let Some(s) = tooltip(self.page_item(page, self.opt_cursor)) else { return };
        let w = c.text_w(s, TXT);
        let y = PAGE_ROW0 + self.page_vis(page) as f32 * ROW_H + 10.0;
        c.round_box(320.0 - w / 2.0 - 8.0, y - 3.0, w + 16.0, 14.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        c.text(320.0 - w / 2.0, y, s, TXT, MUTED);
    }

    pub(super) fn draw_back(&self, c: &mut Canvas) {
        let Some((x, y, w, h)) = self.back_button() else { return };
        let hover = self.mouse.is_some_and(|(mx, my)| (x..x + w).contains(&mx) && (y..y + h).contains(&my));
        let bg = if hover { HILITE } else { [0.0, 0.0, 0.0, 0.7] };
        c.round_box(x, y, w, h, 4.0, bg, Some(BORDER));
        let s = "< Back";
        c.text(x + ((w - c.text_w(s, TXT)) / 2.0).floor(), y + 4.0, s, TXT, if hover { GOLD_FG } else { FG });
    }

    /// Menu hint row, centred in a bordered box.
    pub(super) fn boxed_hints(&self, c: &mut Canvas, y: f32, items: &[(i32, &str)]) {
        let w = self.glyphs.hints(c, 0.0, y, items, false);
        let h = crate::glyphs::HINT_H;
        c.round_box(320.0 - w / 2.0 - 8.0, y - 3.0, w + 16.0, h + 6.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        self.glyphs.hints_centered(c, 320.0, y, items);
    }

    pub(super) fn draw_hints(&self, c: &mut Canvas) {
        if !self.pad_ui || self.screen == Screen::Death { return; }
        let (sel, back) = (K_ABUTTON, K_BBUTTON);
        let y = 344.0;
        if self.capturing() {
            let s = "Press a button or key to bind (Start cancels)";
            let w = c.text_w(s, TXT);
            c.round_box(320.0 - w / 2.0 - 8.0, y - 3.0, w + 16.0, 18.0, 4.0, [0.0, 0.0, 0.0, 0.8], Some(BORDER));
            c.text(320.0 - w / 2.0, y + 2.0, s, TXT, GOLD_FG);
            return;
        }
        if self.osk.is_some() {
            let items = [(K_ABUTTON, "Type"), (K_XBUTTON, "Delete"), (K_YBUTTON, "Shift"), (K_BBUTTON, "Done")];
            self.boxed_hints(c, y, &items);
            return;
        }
        let action = match self.screen {
            Screen::Save => "Save",
            Screen::Load => "Load",
            Screen::OptionsPage(Page::Controls) => "Bind",
            _ => "Select",
        };
        let mut items = vec![(sel, action), (back, "Back")];
        if self.screen == Screen::OptionsPage(Page::Controls) && self.drop.is_none() {
            items.insert(1, (K_XBUTTON, "Clear"));
        }
        if matches!(self.screen, Screen::Quit) {
            items = vec![(sel, "Confirm"), (back, "Cancel")];
        }
        self.boxed_hints(c, y, &items);
    }

}

#[cfg(test)]
mod tests;
