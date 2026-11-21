//! Navigation buttons, contextual help and controller prompts.
use super::*;

impl Menu {
    pub(super) fn death_rows(&self) -> usize {
        4 + usize::from(self.death_quickload)
    }

    /// Back follows the current menu's content; drawing and mouse input share its bounds.
    pub(super) fn back_button(&self) -> Option<(f32, f32, f32, f32)> {
        if self.capturing() || self.osk.is_some() { return None; }
        if self.screen == Screen::OptionsPage(Page::Touch) {
            let [x, y, w, h] = touch::BACK;
            return Some((x, y, w, h));
        }
        if self.screen == Screen::Main && self.touch_shortcut {
            let [x, y, w, h] = touch::RESUME;
            return Some((x, y, w, h));
        }
        let (x, y) = match self.screen {
            Screen::SinglePlayer => (CLASSIC.0 + 72.0, CLASSIC.1 + 32.0 + customize::SP_ITEMS as f32 * 20.0 + 8.0),
            Screen::Multiplayer => (CLASSIC.0 + 72.0, CLASSIC.1 + 32.0 + 3.0 * 20.0 + 8.0),
            Screen::Options => (PANEL_X, ROW0 + self.hub_rows() as f32 * ROW_H + 12.0),
            Screen::OptionsPage(Page::Addons) => (20.0, 266.0),
            Screen::OptionsPage(Page::NewGameAddons) => (24.0, 294.0),
            Screen::OptionsPage(Page::WeaponPriority) => {
                if self.wp.is_grabbed() { return None; }
                weapon_priority::WP_BACK
            }
            Screen::OptionsPage(p) => {
                let bottom = self.page_bottom(p);
                // Leave room for contextual help, the player-colour preview
                // and the Customize notes.
                let gap = match p {
                    Page::Setup => 84.0,
                    Page::Customize | Page::NewGameCustomize | Page::Modifiers => 58.0,
                    Page::Gameplay => 38.0,
                    _ => 30.0,
                };
                (PANEL_X, bottom + gap)
            }
            Screen::Load | Screen::Save => (CARD_X, CARD_BOTTOM + 4.0),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => self.mods.back_position(),
            Screen::Demos => demos::BACK,
            Screen::ServerBrowser => self.server_browser_back_position(),
            _ => return None,
        };
        let (width, height) = if self.touch_ui { (84.0, 24.0) } else { (60.0, 16.0) };
        Some((x, y.min(MENU_H - height), width, height))
    }

    pub(super) fn draw_addon_hints(&self, c: &mut Canvas) {
        if self.capturing() {
            self.boxed_hints(c, 344.0, &[(K_START, "Cancel binding")]);
            return;
        }
        let items = if self.browser_binding_focused() {
            vec![(K_ABUTTON, "Remap"), (K_XBUTTON, "Clear pad")]
        } else {
            vec![(K_ABUTTON, self.browser_select_label())]
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
        } else if self.item_disabled(item, cv) || matches!(item, Item::Sub { page, .. } if sub_disabled(page, cv)) {
            disabled_reason(item)
        } else {
            None
        };
        // the export destination in full (the row shows its end)
        let path = match item {
            Item::Text { cvar: "_export_dest", .. } => cv.by_name("_export_dest").map(|c| c.string.clone()),
            Item::UserDataDir => {
                let tip = tooltip(item).map(qcore::lang::tr).unwrap_or_default();
                let root = crate::userdata::root().to_string_lossy().into_owned();
                Some(format!("{tip}\n{}", qcore::tr!("Data folder: {}", root)))
            }
            _ => None,
        };
        let s = match (path, reason.or_else(|| tooltip(item))) {
            (Some(p), _) => std::borrow::Cow::Owned(p),
            (None, Some(s)) => qcore::lang::tr(s),
            (None, None) => return,
        };
        // one fixed bar under the panel: it doesn't change size as the cursor moves
        let (x, w) = TOOLTIP_BAR;
        let lines: Vec<String> = s.lines().flat_map(|part| wrap_tooltip_lines(c, part, w - 16.0, TXT)).collect();
        let h = lines.len() as f32 * 10.0 + 4.0;
        let y = self.page_bottom(page) + 10.0;
        let a = if self.game_running { 0.88 } else { 0.7 };
        c.round_box(x, y - 3.0, w, h, 4.0, [0.0, 0.0, 0.0, a], Some(BORDER));
        for (i, line) in lines.iter().enumerate() {
            c.text_raw(x + 8.0, y + i as f32 * 10.0, line, TXT, MUTED);
        }
    }

    pub(super) fn draw_back(&self, c: &mut Canvas) {
        let Some((x, y, w, h)) = self.back_button() else { return };
        let hover = self.mouse.is_some_and(|(mx, my)| (x..x + w).contains(&mx) && (y..y + h).contains(&my));
        let bg = if hover { HILITE } else { [0.0, 0.0, 0.0, 0.7] };
        c.round_box(x, y, w, h, 4.0, bg, Some(BORDER));
        let fg = if hover { GOLD_FG } else { FG };
        if self.screen == Screen::Main {
            self.glyphs.button_label(c, [x, y, w, h], self.pad_ui.then_some(K_BBUTTON), qcore::t!("Resume Game"), 16.0, fg);
            return;
        }
        let size = if self.screen == Screen::OptionsPage(Page::Touch) { 12.0 } else { TXT };
        if self.pad_ui {
            self.glyphs.button_label(c, [x, y, w, h], Some(K_BBUTTON), qcore::t!("Back"), size, fg);
        } else {
            self.glyphs.button_label(c, [x, y, w, h], None, qcore::t!("< Back"), size, fg);
        }
    }

    /// Menu hint row, centred in a bordered box.
    pub(super) fn boxed_hints(&self, c: &mut Canvas, y: f32, items: &[(i32, &str)]) {
        let w = self.glyphs.hints(c, 0.0, y, items, false);
        let h = crate::glyphs::HINT_H;
        c.round_box(320.0 - w / 2.0 - 8.0, y - 3.0, w + 16.0, h + 6.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        self.glyphs.hints_centered(c, 320.0, y, items);
    }

    pub(super) fn draw_hints(&self, c: &mut Canvas, cv: &Cvars) {
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
            // Shift, Del and Done carry their own controller glyphs on the OSK.
            self.boxed_hints(c, y, &[(K_ABUTTON, "Type")]);
            return;
        }
        if self.screen == Screen::Help {
            self.boxed_hints(c, y, &[(K_DPAD_UP, "Section"), (K_RSHOULDER, "Scroll"), (K_BBUTTON, "Back")]);
            return;
        }
        // Launch and Preset draw their own hints over the dimmed backdrop.
        if matches!(self.prompt, Some(Prompt::Launch { .. } | Prompt::Preset { .. })) {
            return;
        }
        // These lists show controller bindings directly on their buttons.
        let browser_button = self.screen == Screen::OptionsPage(Page::Join)
            && matches!(self.page_item(Page::Join, self.opt_cursor), Item::Run { run: Run::BrowseServers, .. });
        if self.prompt.is_none() && (matches!(self.screen, Screen::Demos | Screen::ServerBrowser) || browser_button) {
            return;
        }
        let action = match self.screen {
            Screen::Save => qcore::t!("Save"),
            Screen::Load => qcore::t!("Load"),
            Screen::OptionsPage(Page::Controls) => qcore::t!("Bind"),
            _ => qcore::t!("Select"),
        };
        let mut items = vec![(sel, action)];
        if self.back_button().is_none() {
            items.push((back, qcore::t!("Back")));
        }
        match &self.prompt {
            Some(Prompt::Confirm { .. }) => items = vec![(sel, qcore::t!("Select"))],
            Some(Prompt::Skill { .. }) => items = vec![(sel, qcore::t!("Select")), (back, qcore::t!("Cancel"))],
            _ => {}
        }
        if self.screen == Screen::Options && self.prompt.is_none() {
            items.push((K_YBUTTON, "Search"));
        }
        if self.screen == Screen::OptionsPage(Page::Controls) && self.drop.is_none() && self.prompt.is_none() {
            items.push((K_XBUTTON, "Clear"));
        } else if let Screen::OptionsPage(p) = self.screen
            && self.drop.is_none()
            && self.prompt.is_none()
            && self.opt_cursor < self.page_len(p)
            && self.changed(self.page_item(p, self.opt_cursor), cv)
            && !self.item_disabled(self.page_item(p, self.opt_cursor), cv)
        {
            items.push((K_XBUTTON, "Reset"));
        }
        if self.screen == Screen::DirBrowser {
            items = if self.dir_browser.has_modal() {
                vec![(sel, qcore::t!("Select"))]
            } else {
                vec![(sel, qcore::t!("Select")), (K_LSHOULDER, qcore::t!("Parent Folder"))]
            };
        }
        if matches!(self.screen, Screen::Quit) {
            items = vec![(sel, qcore::t!("Confirm"))];
        }
        self.boxed_hints(c, y, &items);
    }

}

#[cfg(test)]
mod tests;

pub(super) fn wrap_tooltip_lines(c: &Canvas, s: &str, max_w: f32, size: f32) -> Vec<String> {
    if c.text_w_raw(s, size) <= max_w {
        return vec![s.to_string()];
    }
    qcore::lang::wrap(s, max_w, |t| c.text_w_raw(t, size))
}
