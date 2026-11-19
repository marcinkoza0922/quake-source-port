//! Navigation buttons, contextual help and controller prompts.
use super::*;

impl Menu {
    /// Back follows the current menu's content; drawing and mouse input share its bounds.
    pub(super) fn back_button(&self) -> Option<(f32, f32, f32, f32)> {
        if self.capturing() || self.osk.is_some() { return None; }
        let (x, y) = match self.screen {
            Screen::SinglePlayer => (CLASSIC.0 + 72.0, CLASSIC.1 + 32.0 + customize::SP_ITEMS as f32 * 20.0 + 8.0),
            Screen::Multiplayer => (CLASSIC.0 + 72.0, CLASSIC.1 + 32.0 + 3.0 * 20.0 + 8.0),
            Screen::Options => (PANEL_X, ROW0 + PAGES.len() as f32 * ROW_H + 12.0),
            Screen::OptionsPage(Page::Addons) => (20.0, 266.0),
            Screen::OptionsPage(Page::NewGameAddons) => (24.0, 294.0),
            Screen::OptionsPage(p) => {
                let bottom = PAGE_ROW0 + self.page_vis(p) as f32 * ROW_H;
                // Leave room for contextual help, the player-colour preview
                // and the Customize notes.
                let gap = match p {
                    Page::Setup => 84.0,
                    Page::Customize | Page::NewGameCustomize | Page::Modifiers => 58.0,
                    _ => 30.0,
                };
                (PANEL_X, bottom + gap)
            }
            Screen::Load | Screen::Save => (CARD_X, CARD_BOTTOM + 4.0),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => self.mods.back_position(),
            Screen::Demos => demos::BACK,
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

    /// Explanation of the selected option, just below the panel (wraps if needed).
    pub(super) fn draw_tooltip(&self, c: &mut Canvas, cv: &Cvars) {
        let Screen::OptionsPage(page) = self.screen else { return };
        if self.drop.is_some() || self.osk.is_some() || self.capturing() || self.opt_cursor >= self.page_len(page) {
            return;
        }
        let item = self.page_item(page, self.opt_cursor);
        let reason = if self.game_locked(item, cv) {
            Some(qcore::t!("Fixed for this game: it was started without Customize"))
        } else if self.item_disabled(item, cv) {
            disabled_reason(item)
        } else {
            None
        };
        // the export destination in full (the row shows its end)
        let path = match item {
            Item::Text { cvar: "_export_dest", .. } => cv.by_name("_export_dest").map(|c| c.string.clone()),
            _ => None,
        };
        let s = match (path, reason.or_else(|| tooltip(item))) {
            (Some(p), _) => std::borrow::Cow::Owned(p),
            (None, Some(s)) => qcore::lang::tr(s),
            (None, None) => return,
        };
        let lines = wrap_tooltip_lines(c, &s, 588.0, TXT);
        let w = lines.iter().map(|l| c.text_w(l, TXT)).fold(0.0, f32::max);
        let h = lines.len() as f32 * 10.0 + 4.0;
        let y = PAGE_ROW0 + self.page_vis(page) as f32 * ROW_H + 10.0;
        c.round_box(320.0 - w / 2.0 - 8.0, y - 3.0, w + 16.0, h, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        for (i, line) in lines.iter().enumerate() {
            c.text_center(320.0, y + i as f32 * 10.0, line, TXT, MUTED);
        }
    }

    pub(super) fn draw_back(&self, c: &mut Canvas) {
        let Some((x, y, w, h)) = self.back_button() else { return };
        let hover = self.mouse.is_some_and(|(mx, my)| (x..x + w).contains(&mx) && (y..y + h).contains(&my));
        let bg = if hover { HILITE } else { [0.0, 0.0, 0.0, 0.7] };
        c.round_box(x, y, w, h, 4.0, bg, Some(BORDER));
        let s = qcore::t!("< Back");
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
            let s = qcore::t!("Press a button or key to bind (Start cancels)");
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
            Screen::Save => qcore::t!("Save"),
            Screen::Load => qcore::t!("Load"),
            Screen::OptionsPage(Page::Controls) => qcore::t!("Bind"),
            _ => qcore::t!("Select"),
        };
        let mut items = vec![(sel, action), (back, qcore::t!("Back"))];
        if matches!(self.screen, Screen::Load | Screen::Save) && self.save_at(self.load_cursor).is_some() {
            items.insert(1, (K_XBUTTON, "Delete"));
        }
        if self.prompt.is_some() {
            items = vec![(sel, qcore::t!("Select")), (back, qcore::t!("Cancel"))];
        }
        if self.screen == Screen::OptionsPage(Page::Controls) && self.drop.is_none() {
            items.insert(1, (K_XBUTTON, "Clear"));
        }
        if matches!(self.screen, Screen::Quit) {
            items = vec![(sel, qcore::t!("Confirm")), (back, qcore::t!("Cancel"))];
        }
        if self.screen == Screen::Demos && self.prompt.is_none() {
            items = Vec::new();
            if self.demos.selected().is_some() {
                items.extend([(sel, qcore::t!("Play")), (K_XBUTTON, qcore::t!("Export Video")), (K_YBUTTON, qcore::t!("Rename"))]);
                if self.demos.selected().is_some_and(|d| d.auto) {
                    items.push((K_LTRIGGER, qcore::t!("Keep")));
                }
                items.push((K_BACK, qcore::t!("Delete")));
            }
            items.extend([(K_RTRIGGER, qcore::t!("Sort")), (K_LSHOULDER, qcore::t!("Switch List")), (back, qcore::t!("Back"))]);
        }
        self.boxed_hints(c, y, &items);
    }

}

#[cfg(test)]
mod tests;

fn wrap_tooltip_lines(c: &Canvas, s: &str, max_w: f32, size: f32) -> Vec<String> {
    if c.text_w(s, size) <= max_w {
        return vec![s.to_string()];
    }
    qcore::lang::wrap(s, max_w, |t| c.text_w(t, size))
}
