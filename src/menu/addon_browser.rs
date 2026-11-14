//! Add-on browser: checklist, looping demonstration, description and contextual controls.
use super::*;

#[derive(Default)]
pub(super) struct Browser {
    selected: usize,
    /// Checklist, description, settings rows, then the new-game Start button.
    focus: usize,
    scroll: usize,
    settings_scroll: f32,
}

struct Weapon {
    name: &'static str,
    slot: &'static str,
}

struct Addon {
    name: &'static str,
    id: &'static str,
    cvar: &'static str,
    weapons: &'static [Weapon],
    page: Option<Page>,
    description: &'static str,
    alt: bool,
}
const ADDON: &[Addon] = &[
    Addon {
        name: "Portal Gun",
        id: "portalgun",
        cvar: "addon_portalgun",
        weapons: &[Weapon { name: "Portal Gun", slot: "addon_portalgun_slot" }],
        page: Some(Page::PortalGun),
        alt: true,
        description: "Create two linked portals on flat walls. Fire places the first portal; Alt Fire places the second. Walk through either portal to emerge from the other. Your momentum carries through. Assign a weapon slot below: pressing that slot repeatedly cycles its ordinary weapon and every enabled add-on assigned there. Portal view quality is configurable below.",
    },
    Addon {
        name: "Boomerang",
        id: "boomerang",
        cvar: "addon_boomerang",
        weapons: &[Weapon { name: "Boomerang", slot: "addon_boomerang_slot" }],
        page: Some(Page::Boomerang),
        alt: true,
        description: "Hold Fire and sweep the crosshair over monsters to queue targets. Release Fire to throw: the boomerang visits each target, stuns it and returns to you. Alt Fire clears the queue. The weapon slot selects a group, not a separate hotkey: press it again to cycle through the weapons in that slot. Configure stun duration and flight speed below.",
    },
    Addon {
        name: "Stickybomb Launcher",
        id: "sticky",
        cvar: "addon_sticky",
        weapons: &[Weapon { name: "Stickybomb Launcher", slot: "addon_sticky_slot" }],
        page: Some(Page::Sticky),
        alt: true,
        description: "Hold Fire to charge a sticky bomb using rocket ammunition; release to launch. A longer charge throws it faster. Bombs cling to walls, doors and monsters. Alt Fire detonates your planted bombs together. Place a trap, retreat, then detonate it when enemies approach. Assign any weapon slot below; repeated presses cycle through the ordinary weapon and add-ons in that slot.",
    },
    Addon {
        name: "Hookshot",
        id: "hookshot",
        cvar: "addon_hookshot",
        weapons: &[Weapon { name: "Hookshot", slot: "addon_hookshot_slot" }],
        page: Some(Page::Hookshot),
        alt: true,
        description: "Fire launches a chain hook. It latches onto a permitted surface or enemy and reels you toward the anchor. Fire again or use Alt Fire to let go. Choose which materials and enemies can be hooked in Configure Parameters. Assign it to any weapon slot; repeated presses of the slot's existing binding cycle through its weapons.",
    },
    Addon {
        name: "Railgun",
        id: "railgun",
        cvar: "addon_railgun",
        weapons: &[Weapon { name: "Railgun", slot: "addon_railgun_slot" }],
        page: Some(Page::Railgun),
        alt: false,
        description: "A piercing railgun inspired by Quake III. Fire sends a slug through every enemy along its line. Each shot consumes two cells, with a 1.5 second refire interval. Damage and the spiral trail colour are configurable below. Choose a weapon slot: the slot's current binding selects and cycles the weapons assigned to it.",
    },
    Addon {
        name: "Goggles",
        id: "goggles",
        cvar: "addon_goggles",
        weapons: &[],
        page: Some(Page::Goggles),
        alt: false,
        description: "One equipment add-on includes both night vision and thermal vision. The Change Goggles action cycles Normal, Night Vision, Thermal, then Normal again. Night vision brightens dark rooms; thermal vision highlights living enemies. Goggles do not occupy a weapon slot. Configure intensity and sensor noise below. The binding affects only this add-on.",
    },
    Addon {
        name: "Projectile Shotguns", id: "projectileshotguns", cvar: "addon_projectile_shotguns",
        weapons: &[], page: Some(Page::ProjectileShotguns), alt: false,
        description: "Both shotguns and enemy grunts fire physical pellets with their original spread and damage. Fast default projectiles feel immediate; lower their speed to dodge incoming fire during bullet time. Configure projectile speed below, or reset it to the default of 10000 units per second.",
    },
    Addon {
        name: "Bullet Time", id: "bullettime", cvar: "addon_bullet_time",
        weapons: &[], page: Some(Page::BulletTime), alt: false,
        description: "Use the dedicated Bullet Time action to slow the whole local game to 20% speed. Hold is the default; Toggle switches on each press. The optional meter limits use in real seconds and regenerates while inactive. Disable the meter for unlimited use. Remap the action and configure duration and regeneration below.",
    },
];

impl Addon {
    fn equipment_action(&self) -> Option<(&'static str, &'static str)> {
        match self.id {
            "goggles" => Some(("Change Goggles", "goggles cycle")),
            "bullettime" => Some(("Bullet Time", "+bullettime")),
            _ => None,
        }
    }
    fn row_count(&self) -> usize {
        if self.weapons.is_empty() { 1 + usize::from(self.equipment_action().is_some()) }
        else { self.weapons.len() + 2 + usize::from(self.alt) }
    }
    fn command(&self, row: usize) -> Option<&'static str> {
        match row.checked_sub(self.weapons.len())? {
            _ if self.weapons.is_empty() => (row == 0).then(|| self.equipment_action().map(|a| a.1)).flatten(),
            0 => Some("+attack"),
            1 if !self.weapons.is_empty() && self.alt => Some("+attack2"),
            _ => None,
        }
    }
    fn header_y(&self) -> f32 {
        if self.weapons.is_empty() { 238.0 } else { 252.0 + self.weapons.len() as f32 * 18.0 }
    }
    fn row_y(&self, row: usize) -> f32 {
        if row < self.weapons.len() {
            250.0 + row as f32 * 18.0
        } else {
            self.header_y() + 12.0 + (row - self.weapons.len()) as f32 * 18.0
        }
    }
}

impl Menu {
    fn browser_active(&self) -> bool {
        matches!(self.screen, Screen::OptionsPage(Page::Addons | Page::NewGameAddons))
    }
    fn selected_addon(&self) -> &'static Addon {
        &ADDON[self.addon_ui.selected.min(ADDON.len() - 1)]
    }
    fn browser_rows(&self) -> usize {
        self.selected_addon().row_count() + 1
    }
    fn browser_last_focus(&self) -> usize {
        self.browser_rows() + usize::from(self.screen == Screen::OptionsPage(Page::NewGameAddons))
    }
    fn browser_command(&self) -> Option<&'static str> {
        self.addon_ui.focus.checked_sub(2).and_then(|row| self.selected_addon().command(row))
    }
    pub(super) fn browser_binding_focused(&self) -> bool {
        self.browser_command().is_some()
    }
    pub(super) fn browser_select_label(&self) -> &'static str {
        if self.addon_ui.focus == 0 { "Toggle" }
        else if self.addon_ui.focus == 1 { "Settings" }
        else if self.addon_ui.focus > self.browser_rows() { "Start game" }
        else if self.browser_slot().is_some() { "Change slot" }
        else { "Configure" }
    }
    fn browser_slot(&self) -> Option<&'static str> {
        self.addon_ui.focus.checked_sub(2)
            .and_then(|row| self.selected_addon().weapons.get(row)).map(|weapon| weapon.slot)
    }
    fn browser_max_offset(&self) -> f32 {
        let a = self.selected_addon();
        (a.row_y(a.row_count() - 1) + 16.0 - 322.0).max(0.0)
    }
    fn browser_offset(&self) -> f32 {
        self.addon_ui.settings_scroll.min(self.browser_max_offset())
    }
    fn reveal_browser_focus(&mut self) {
        let Some(row) = self.addon_ui.focus.checked_sub(2) else { return };
        if row >= self.selected_addon().row_count() { return; }
        let y = self.selected_addon().row_y(row);
        let offset = self.browser_offset();
        self.addon_ui.settings_scroll = if y < 236.0 + offset {
            (y - 236.0).max(0.0)
        } else if y + 16.0 > 322.0 + offset {
            y + 16.0 - 322.0
        } else { offset };
    }
    fn browser_row_y(&self, row: usize) -> f32 {
        self.selected_addon().row_y(row) - self.browser_offset()
    }
    fn browser_hit(&self, (x, y): (f32, f32)) -> Option<(usize, usize)> {
        if self.screen == Screen::OptionsPage(Page::NewGameAddons)
            && (24.0..196.0).contains(&x) && (264.0..286.0).contains(&y) {
            return Some((self.addon_ui.selected, self.browser_last_focus()));
        }
        if (20.0..200.0).contains(&x) && (58.0..218.0).contains(&y) {
            return Some((((y - 58.0) / 20.0) as usize, 0));
        }
        if (216.0..620.0).contains(&x) {
            if (174.0..232.0).contains(&y) {
                return Some((self.addon_ui.selected, 1));
            }
            for row in 0..self.browser_rows() - 1 {
                let top = self.browser_row_y(row);
                if top >= 236.0 && top + 16.0 <= 322.0 && (top..top + 16.0).contains(&y) {
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
                self.addon_ui.settings_scroll = 0.0;
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
        self.addon_ui.focus = self.addon_ui.focus.min(self.browser_last_focus());
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1);
        if matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2) {
            self.screen = if self.screen == Screen::OptionsPage(Page::NewGameAddons) { Screen::SinglePlayer } else { Screen::Main };
            return true;
        }
        if key == K_MOUSE1 {
            let Some((x, y)) = self.mouse else { return true };
            if self.back_button().is_some_and(|(bx, by, bw, bh)| (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y)) {
                return self.addon_key(K_ESCAPE, cv, out);
            }
            let Some((selected, focus)) = self.browser_hit((x, y)) else { return true };
            self.addon_ui.selected = selected;
            self.addon_ui.focus = focus;
        }
        if self.screen == Screen::OptionsPage(Page::NewGameAddons) && (key == K_YBUTTON || key == K_F1 + 9) {
            self.start_new_game(out);
            return true;
        }
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        let wheel = matches!(key, K_MWHEELUP | K_MWHEELDOWN);
        let paging = matches!(key, K_PGUP | K_PGDN);
        let over_settings = self.mouse.is_some_and(|(x, y)| (216.0..620.0).contains(&x) && (236.0..322.0).contains(&y));
        if (wheel && over_settings) || (paging && self.addon_ui.focus >= 2 && self.addon_ui.focus <= self.browser_rows()) {
            let step = if wheel { 18.0 } else { 64.0 };
            let delta = if matches!(key, K_MWHEELUP | K_PGUP) { -step } else { step };
            self.addon_ui.settings_scroll = (self.browser_offset() + delta).clamp(0.0, self.browser_max_offset());
            return true;
        }
        if matches!(key, K_LSHOULDER | K_RSHOULDER) {
            let sections = [0, 1, 2, self.browser_last_focus()];
            let count = if self.screen == Screen::OptionsPage(Page::NewGameAddons) { 4 } else { 3 };
            let section = if self.addon_ui.focus > self.browser_rows() { 3 } else { self.addon_ui.focus.min(2) };
            let next = (section + if key == K_LSHOULDER { count - 1 } else { 1 }) % count;
            self.addon_ui.focus = sections[next];
        } else if self.addon_ui.focus == 1 && (enter || right) {
            self.addon_ui.focus = 2;
        } else if key == K_TAB {
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
                if down && self.addon_ui.selected == ADDON.len() - 1
                    && self.screen == Screen::OptionsPage(Page::NewGameAddons) {
                    self.addon_ui.focus = self.browser_last_focus();
                } else {
                    self.addon_ui.selected = (self.addon_ui.selected + if up { ADDON.len() - 1 } else { 1 }) % ADDON.len();
                }
                self.addon_ui.scroll = 0;
                self.addon_ui.settings_scroll = 0.0;
            }
            if right {
                self.addon_ui.focus = 1;
            }
            if enter || key == b' ' as i32 {
                let a = self.selected_addon();
                self.adjust(toggle(a.name, a.cvar), 1, true, cv, out);
            }
        } else if self.addon_ui.focus > self.browser_rows() {
            if enter {
                self.start_new_game(out);
            } else if up || down || right {
                self.addon_ui.focus = if right { 1 } else { 0 };
                if up || down { self.addon_ui.selected = if up { ADDON.len() - 1 } else { 0 }; }
            }
        } else {
            if up {
                self.addon_ui.focus = self.addon_ui.focus.saturating_sub(1);
            }
            if down {
                self.addon_ui.focus = (self.addon_ui.focus + 1).min(self.browser_rows());
            }
            if left && self.browser_slot().is_none() {
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
            } else if (left || right || enter)
                && let Some(slot) = self.browser_slot()
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
            }
        }
        if matches!(key, K_TAB | K_UPARROW | K_DOWNARROW | K_DPAD_UP | K_DPAD_DOWN | K_RIGHTARROW | K_DPAD_RIGHT | K_LSHOULDER | K_RSHOULDER | K_ABUTTON | K_ENTER | K_KP_ENTER) {
            self.reveal_browser_focus();
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
            let y = 58.0 + i as f32 * 20.0;
            if i == self.addon_ui.selected {
                c.fill(22.0, y, 176.0, 19.0, HILITE);
            }
            c.frame(28.0, y + 6.0, 9.0, 9.0, BORDER);
            if cv.value_of(a.cvar) != 0.0 {
                c.fill(30.0, y + 8.0, 5.0, 5.0, GOLD_FG);
            }
            c.text(44.0, y + 6.0, a.name, TXT, if self.toys_locked(cv) { MUTED } else { FG });
        }
        if self.pad_ui {
            self.glyphs.hints(c, 28.0, 224.0, &[(K_ABUTTON, "Toggle")], true);
        } else {
            c.text(28.0, 224.0, "Enter: toggle   Right: details", 8.0, MUTED);
            c.text(28.0, 238.0, "Tab: switch section", 8.0, MUTED);
        }
        if page == Page::NewGameAddons {
            c.round_box(24.0, 264.0, 172.0, 22.0, 2.0, HILITE, Some(BORDER));
            if self.pad_ui {
                self.glyphs.hints_centered(c, 110.0, 268.0, &[(K_YBUTTON, "Start New Game")]);
            } else {
                c.text_center(110.0, 271.0, "Start New Game", TXT, GOLD_FG);
            }
        }
        let a = self.selected_addon();
        super::addon_preview::draw(c, pic, a.id, time, [216.0, 50.0, 404.0, 116.0]);
        c.round_box(216.0, 174.0, 404.0, 58.0, 2.0, [0.0, 0.0, 0.0, 0.75], Some(BORDER));
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
        let max_offset = self.browser_max_offset();
        if max_offset > 0.0 {
            let h = 80.0 * 86.0 / (86.0 + max_offset);
            c.fill(616.0, 239.0 + self.browser_offset() / max_offset * (80.0 - h), 2.0, h, GOLD_FG);
        }
        let slot_header_y = 238.0 - self.browser_offset();
        if !a.weapons.is_empty() && slot_header_y >= 236.0 {
            c.text(220.0, slot_header_y, "Weapon Slot", 8.0, MUTED);
        }
        let header_y = a.header_y() - self.browser_offset();
        if header_y >= 236.0 && header_y + 8.0 <= 322.0
            && (!a.weapons.is_empty() || a.equipment_action().is_some()) {
            c.text(220.0, header_y, "Input Remapping", 8.0, MUTED);
            c.text(425.0, header_y, "Keyboard", 8.0, MUTED);
            c.text(525.0, header_y, "Controller", 8.0, MUTED);
        }
        let mut rows = Vec::new();
        for weapon in a.weapons {
            rows.push((weapon.name, None, Some(weapon.slot)));
        }
        if a.weapons.is_empty() {
            if let Some((label, cmd)) = a.equipment_action() { rows.push((label, Some(cmd), None)); }
        } else {
            rows.push(("Fire", Some("+attack"), None));
            if a.alt {
                rows.push(("Alt Fire", Some("+attack2"), None));
            }
        }
        rows.push(("Configure Parameters >", None, None));
        for (i, (label, cmd, slot)) in rows.iter().enumerate() {
            let y = self.browser_row_y(i);
            if y < 236.0 || y + 16.0 > 322.0 { continue; }
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
                self.draw_binding_columns(c, cmd, y + 2.0, [(425.0, 515.0), (525.0, 615.0)],
                    self.addon_capture == Some(*cmd), (time * 4.0) as i64 & 1 == 0);
            }
        }
        let (rect, radius) = match self.addon_ui.focus {
            0 => ([20.0, 50.0, 180.0, 208.0], 3.0),
            1 => ([216.0, 174.0, 404.0, 58.0], 2.0),
            f if f <= self.browser_rows() => ([216.0, 236.0, 404.0, 86.0], 2.0),
            _ => ([24.0, 264.0, 172.0, 22.0], 2.0),
        };
        c.round_frame(rect, radius, 2.0, GOLD_FG);
        let note = if self.toys_locked(cv) {
            "This game started without add-ons. Start a new add-on game to enable them."
        } else if self.game_running {
            "Changes are saved with this game (.qsa); these saves require this source port."
        } else {
            "Add-on saves (.qsa) require this source port. Enable Add-ons beside New Game."
        };
        c.text_center(320.0, 327.0, note, 8.0, MUTED);
        if self.pad_ui {
            self.draw_addon_hints(c);
        }
    }
}

#[cfg(test)]
mod tests;
