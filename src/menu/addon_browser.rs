//! Add-on browser: checklist, looping demonstration, description and contextual controls.
use super::*;

#[derive(Default)]
pub(super) struct Browser {
    selected: usize,
    /// Zero is the checklist; remaining entries are rows in the details pane.
    focus: usize,
    scroll: usize,
}

struct Addon {
    name: &'static str,
    id: &'static str,
    cvar: &'static str,
    slot: Option<&'static str>,
    page: Option<Page>,
    description: &'static str,
    alt: bool,
}
const ADDON: &[Addon] = &[
    Addon {
        name: "Portal Gun",
        id: "portalgun",
        cvar: "addon_portalgun",
        slot: Some("addon_portalgun_slot"),
        page: Some(Page::PortalGun),
        alt: true,
        description: "Create two linked portals on flat walls. Shared Fire places the first portal; shared Alt Fire places the second. Walk through either portal to emerge from the other. Your momentum carries through. Assign a weapon slot below: pressing that slot repeatedly cycles its ordinary weapon and every enabled add-on assigned there. Portal view quality is configurable below.",
    },
    Addon {
        name: "Boomerang",
        id: "boomerang",
        cvar: "addon_boomerang",
        slot: Some("addon_boomerang_slot"),
        page: Some(Page::Boomerang),
        alt: true,
        description: "Hold shared Fire and sweep the crosshair over monsters to queue targets. Release Fire to throw: the boomerang visits each target, stuns it and returns to you. Shared Alt Fire clears the queue. The weapon slot selects a group, not a separate hotkey: press it again to cycle through the weapons in that slot. Configure stun duration and flight speed below.",
    },
    Addon {
        name: "Stickybomb Launcher",
        id: "sticky",
        cvar: "addon_sticky",
        slot: Some("addon_sticky_slot"),
        page: Some(Page::Sticky),
        alt: true,
        description: "Hold shared Fire to charge a sticky bomb using rocket ammunition; release to launch. A longer charge throws it faster. Bombs cling to walls, doors and monsters. Shared Alt Fire detonates your planted bombs together. Place a trap, retreat, then detonate it when enemies approach. Assign any weapon slot below; repeated presses cycle through the ordinary weapon and add-ons in that slot.",
    },
    Addon {
        name: "Hookshot",
        id: "hookshot",
        cvar: "addon_hookshot",
        slot: Some("addon_hookshot_slot"),
        page: Some(Page::Hookshot),
        alt: true,
        description: "Shared Fire launches a chain hook. It latches onto a permitted surface or enemy and reels you toward the anchor. Fire again or use shared Alt Fire to let go. Choose which materials and enemies can be hooked in Configure Parameters. Assign it to any weapon slot; repeated presses of the slot's existing binding cycle through its weapons.",
    },
    Addon {
        name: "Railgun",
        id: "railgun",
        cvar: "addon_railgun",
        slot: Some("addon_railgun_slot"),
        page: Some(Page::Railgun),
        alt: false,
        description: "A piercing railgun inspired by Quake III. Shared Fire sends a slug through every enemy along its line. Each shot consumes two cells, with a 1.5 second refire interval. Damage and the spiral trail colour are configurable below. Choose a weapon slot: the slot's current binding selects and cycles the weapons assigned to it.",
    },
    Addon {
        name: "Goggles",
        id: "goggles",
        cvar: "addon_goggles",
        slot: None,
        page: Some(Page::Goggles),
        alt: false,
        description: "One equipment add-on includes both night vision and thermal vision. The unique Change Goggles action cycles Normal, Night Vision, Thermal, then Normal again. Night vision brightens dark rooms; thermal vision highlights living enemies. Goggles do not occupy a weapon slot. Configure intensity and sensor noise below. The unique binding affects only this add-on.",
    },
];

impl Menu {
    fn browser_active(&self) -> bool {
        matches!(self.screen, Screen::OptionsPage(Page::Addons | Page::NewGameAddons))
    }
    fn selected_addon(&self) -> &'static Addon {
        &ADDON[self.addon_ui.selected.min(ADDON.len() - 1)]
    }
    fn browser_rows(&self) -> usize {
        if self.selected_addon().slot.is_none() {
            3
        } else if self.selected_addon().alt {
            5
        } else {
            4
        }
    }
    fn browser_last_focus(&self) -> usize {
        self.browser_rows() + usize::from(self.screen == Screen::OptionsPage(Page::NewGameAddons))
    }
    fn browser_command(&self) -> Option<&'static str> {
        let a = self.selected_addon();
        match (a.slot.is_some(), self.addon_ui.focus) {
            (false, 2) => Some("goggles cycle"),
            (true, 3) => Some("+attack"),
            (true, 4) if a.alt => Some("+attack2"),
            _ => None,
        }
    }
    fn browser_row_y(&self, row: usize) -> f32 {
        if self.selected_addon().slot.is_some() {
            if row == 0 { 238.0 } else { 270.0 + (row - 1) as f32 * 18.0 }
        } else {
            250.0 + row as f32 * 18.0
        }
    }
    fn browser_hit(&self, (x, y): (f32, f32)) -> Option<(usize, usize)> {
        if (20.0..200.0).contains(&x) && (58.0..202.0).contains(&y) {
            return Some((((y - 58.0) / 24.0) as usize, 0));
        }
        if (216.0..620.0).contains(&x) {
            if (174.0..232.0).contains(&y) {
                return Some((self.addon_ui.selected, 1));
            }
            for row in 0..self.browser_rows() - 1 {
                let top = self.browser_row_y(row);
                if (top..top + 16.0).contains(&y) {
                    return Some((self.addon_ui.selected, row + 2));
                }
            }
        }
        None
    }
    pub(super) fn addon_mouse(&mut self, pos: (f32, f32)) -> bool {
        if !self.browser_active() {
            return false;
        }
        if let Some((selected, focus)) = self.browser_hit(pos) {
            if selected != self.addon_ui.selected {
                self.addon_ui.scroll = 0;
            }
            self.addon_ui.selected = selected;
            self.addon_ui.focus = focus;
        }
        true
    }
    pub(super) fn addon_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) -> bool {
        if !self.browser_active() {
            return false;
        }
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1);
        if matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2) {
            self.screen = if self.screen == Screen::OptionsPage(Page::NewGameAddons) { Screen::SinglePlayer } else { Screen::Main };
            return true;
        }
        if key == K_MOUSE1 {
            let Some((x, y)) = self.mouse else { return true };
            if (12.0..72.0).contains(&x) && (341.0..357.0).contains(&y) {
                return self.addon_key(K_ESCAPE, cv, out);
            }
            if self.screen == Screen::OptionsPage(Page::NewGameAddons) && (24.0..196.0).contains(&x) && (264.0..286.0).contains(&y) {
                self.start_new_game(out);
                return true;
            }
            let Some((selected, focus)) = self.browser_hit((x, y)) else { return true };
            self.addon_ui.selected = selected;
            self.addon_ui.focus = focus;
        }
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        if key == K_TAB {
            self.addon_ui.focus = (self.addon_ui.focus + 1) % (self.browser_last_focus() + 1);
        } else if matches!(key, K_MWHEELUP | K_MWHEELDOWN | K_PGUP | K_PGDN) || (self.addon_ui.focus == 1 && (up || down)) {
            let decrease = matches!(key, K_MWHEELUP | K_PGUP | K_UPARROW | K_DPAD_UP);
            let max = self.description_lines().len().saturating_sub(4);
            if up && self.addon_ui.scroll == 0 {
                self.addon_ui.focus = 0;
            } else if down && self.addon_ui.scroll == max {
                self.addon_ui.focus = 2;
            } else {
                self.addon_ui.scroll = if decrease { self.addon_ui.scroll.saturating_sub(2) } else { (self.addon_ui.scroll + 2).min(max) };
            }
        } else if self.addon_ui.focus == 0 {
            if up || down {
                self.addon_ui.selected = (self.addon_ui.selected + if up { ADDON.len() - 1 } else { 1 }) % ADDON.len();
                self.addon_ui.scroll = 0;
            }
            if right {
                self.addon_ui.focus = 1;
            }
            if enter || key == b' ' as i32 {
                let a = self.selected_addon();
                self.adjust(toggle(a.name, a.cvar), 1, true, cv, out);
            }
        } else {
            if up {
                self.addon_ui.focus = self.addon_ui.focus.saturating_sub(1);
            }
            if down {
                self.addon_ui.focus = (self.addon_ui.focus + 1).min(self.browser_last_focus());
            }
            if left && self.addon_ui.focus != 2 {
                self.addon_ui.focus = 0;
            }
            let a = self.selected_addon();
            if let Some(cmd) = self.browser_command() {
                if enter {
                    self.addon_capture = Some(cmd);
                }
                if matches!(key, K_DEL | K_BACKSPACE | K_XBUTTON) {
                    self.clear_addon_binding(cmd, key == K_XBUTTON, out);
                }
            } else if self.addon_ui.focus == 2
                && (left || right || enter)
                && let Some(slot) = a.slot
            {
                let v = cv.value_of(slot).round().clamp(1.0, 10.0) as i32;
                set_num(cv, slot, ((v - 1 + if left { -1 } else { 1 }).rem_euclid(10) + 1) as f32);
            } else if self.addon_ui.focus == self.browser_rows()
                && (enter || right)
                && let Some(page) = a.page
            {
                let parent = if self.screen == Screen::OptionsPage(Page::NewGameAddons) { Page::NewGameAddons } else { Page::Addons };
                self.sub_return = Some((parent, page, self.opt_cursor));
                self.screen = Screen::OptionsPage(page);
                self.opt_cursor = 0;
            } else if self.addon_ui.focus > self.browser_rows() && enter {
                self.start_new_game(out);
            }
        }
        if self.screen == Screen::OptionsPage(Page::NewGameAddons) && key == K_F1 + 9 {
            self.start_new_game(out);
        }
        true
    }
    fn description_lines(&self) -> Vec<String> {
        let mut lines = vec![String::new()];
        for word in self.selected_addon().description.split_whitespace() {
            let line = lines.last_mut().unwrap();
            if line.len() + word.len() + 1 > 78 {
                lines.push(word.into());
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        lines
    }
    pub(super) fn draw_addon_browser(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, time: f64, page: Page) {
        c.text(24.0, 25.0, page_name(page), 14.0, GOLD_FG);
        c.round_box(20.0, 50.0, 180.0, 208.0, 3.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        for (i, a) in ADDON.iter().enumerate() {
            let y = 58.0 + i as f32 * 24.0;
            if i == self.addon_ui.selected {
                c.fill(22.0, y, 176.0, 22.0, HILITE);
            }
            c.frame(28.0, y + 6.0, 9.0, 9.0, BORDER);
            if cv.value_of(a.cvar) != 0.0 {
                c.fill(30.0, y + 8.0, 5.0, 5.0, GOLD_FG);
            }
            c.text(44.0, y + 6.0, a.name, TXT, if self.toys_locked(cv) { MUTED } else { FG });
        }
        c.text(28.0, 218.0, "Enter: toggle   Right: details", 8.0, MUTED);
        c.text(28.0, 234.0, "Tab: switch section", 8.0, MUTED);
        if page == Page::NewGameAddons {
            c.round_box(24.0, 264.0, 172.0, 22.0, 2.0, HILITE, Some(if self.addon_ui.focus > self.browser_rows() { GOLD_FG } else { BORDER }));
            c.text_center(110.0, 271.0, "Start New Game [F10]", TXT, GOLD_FG);
        }
        let a = self.selected_addon();
        super::addon_preview::draw(c, pic, a.id, time, [216.0, 50.0, 404.0, 116.0]);
        c.round_box(216.0, 174.0, 404.0, 58.0, 2.0, [0.0, 0.0, 0.0, 0.75], Some(if self.addon_ui.focus == 1 { GOLD_FG } else { BORDER }));
        let lines = self.description_lines();
        let start = self.addon_ui.scroll.min(lines.len().saturating_sub(4));
        for (i, line) in lines.iter().skip(start).take(4).enumerate() {
            c.text(224.0, 180.0 + i as f32 * 11.0, line, 8.5, FG);
        }
        if lines.len() > 4 {
            let h = 46.0 * 4.0 / lines.len() as f32;
            c.fill(614.0, 180.0 + start as f32 / (lines.len() - 4) as f32 * (46.0 - h), 2.0, h, GOLD_FG);
        }
        c.round_box(216.0, 236.0, 404.0, 86.0, 2.0, [0.0, 0.0, 0.0, 0.8], Some(BORDER));
        let header_y = if a.slot.is_some() { 258.0 } else { 238.0 };
        c.text(220.0, header_y, "Input Remapping", 8.0, MUTED);
        c.text(425.0, header_y, "Keyboard", 8.0, MUTED);
        c.text(525.0, header_y, "Controller", 8.0, MUTED);
        let mut rows = Vec::new();
        if let Some(slot) = a.slot {
            rows.push(("Weapon Slot", None, Some(slot)));
            rows.push(("Fire (shared)", Some("+attack"), None));
            if a.alt {
                rows.push(("Alt Fire (shared)", Some("+attack2"), None));
            }
        } else {
            rows.push(("Change Goggles (unique)", Some("goggles cycle"), None));
        }
        rows.push(("Configure Parameters >", None, None));
        for (i, (label, cmd, slot)) in rows.iter().enumerate() {
            let y = self.browser_row_y(i);
            if self.addon_ui.focus == i + 2 {
                c.fill(216.0, y, 404.0, 16.0, HILITE);
            }
            let submenu = cmd.is_none() && slot.is_none();
            if submenu {
                c.fill(216.0, y, 404.0, 16.0, [0.85, 0.60, 0.25, 0.10]);
            }
            c.text(224.0, y + 3.0, label, TXT, if submenu { GOLD_FG } else { FG });
            if let Some(slot) = slot {
                let v = cv.value_of(slot).round().clamp(1.0, 10.0) as i32;
                c.text(425.0, y + 3.0, &format!("< {} >", v % 10), TXT, GOLD_FG);
            }
            if let Some(cmd) = cmd {
                if self.addon_capture == Some(*cmd) {
                    c.text(425.0, y + 3.0, "press a key... (Esc cancels)", TXT, GOLD_FG);
                } else {
                    let keys = keys_for(&self.bindings, cmd);
                    for (pad, mut x) in [(false, 425.0), (true, 525.0)] {
                        let device_keys: Vec<_> = keys.iter().copied().filter(|&key| is_pad_key(key) == pad).collect();
                        let edge = x + 90.0;
                        if device_keys.is_empty() {
                            c.text(x, y + 3.0, "---", TXT, MUTED);
                        }
                        for key in device_keys {
                            if x < edge {
                                x += self.glyphs.key(c, x, y + 2.0, key) + 3.0;
                            }
                        }
                    }
                }
            }
        }
        let note = if self.toys_locked(cv) {
            "This game started without add-ons. Start a new add-on game to enable them."
        } else if self.game_running {
            "Changes are saved with this game (.qsa); these saves require this source port."
        } else {
            "Add-on saves (.qsa) require this source port. Enable Add-ons beside New Game."
        };
        c.text_center(320.0, 327.0, note, 8.0, MUTED);
        c.text_center(350.0, 346.0, "Enter: remap  Del: clear keyboard  X: clear pad  Wheel / PgUp/PgDn: description", 8.0, MUTED);
    }
}

#[cfg(test)]
mod tests;
