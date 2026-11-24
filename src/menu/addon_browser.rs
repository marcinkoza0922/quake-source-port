//! Add-on browser: checklist, looping demonstration, description and contextual controls.
use super::*;

mod list;
use list::ADDON;

#[derive(Default)]
pub(super) struct Browser {
    selected: usize,
    list_scroll: usize,
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
    /// Uses the shared reload action.
    reload: bool,
}
// Checklist ends before the dedicated help rows below it.
const LIST_TOP: f32 = 58.0;
const LIST_ROW: f32 = 20.0;
const LIST_VISIBLE: usize = 8;
const LIST_BOTTOM: f32 = LIST_TOP + LIST_ROW * LIST_VISIBLE as f32;
/// Clicks or taps left of this (the checkbox column) toggle an add-on.
const CHECK_RIGHT: f32 = 42.0;

/// One add-on as the touch browser shows it.
pub(super) struct AddonInfo {
    pub(super) name: &'static str,
    pub(super) id: &'static str,
    pub(super) cvar: &'static str,
    pub(super) description: &'static str,
    /// Its parameters page.
    pub(super) page: Option<Page>,
    /// (weapon name, slot cvar) per weapon.
    pub(super) slots: Vec<(&'static str, &'static str)>,
    /// (label, bound command): Fire / Alt Fire, or its equipment action.
    pub(super) binds: Vec<(&'static str, &'static str)>,
}

pub(super) const ADDON_COUNT: usize = ADDON.len();

pub(super) fn addon_info(i: usize) -> AddonInfo {
    let a = &ADDON[i.min(ADDON.len() - 1)];
    let mut binds = Vec::new();
    if a.weapons.is_empty() {
        binds.extend(a.equipment_action());
    } else {
        binds.push((qcore::t!("Fire"), "+attack"));
        if a.alt {
            binds.push((qcore::t!("Alt Fire"), "+attack2"));
        }
        if a.reload {
            binds.push((qcore::t!("Reload"), "+reload"));
        }
    }
    AddonInfo {
        name: a.name,
        id: a.id,
        cvar: a.cvar,
        description: a.description,
        page: a.page,
        slots: a.weapons.iter().map(|w| (w.name, w.slot)).collect(),
        binds,
    }
}

/// Every add-on's (name, on/off cvar), in browser order.
pub(super) fn toggles() -> impl Iterator<Item = (&'static str, &'static str)> {
    ADDON.iter().map(|a| (a.name, a.cvar))
}

impl Addon {
    fn equipment_action(&self) -> Option<(&'static str, &'static str)> {
        match self.id {
            "dodge" => Some((qcore::t!("Dodge / Dive"), "+dodge")),
            "goggles" => Some((qcore::t!("Change Goggles"), "goggles cycle")),
            "bullettime" => Some((qcore::t!("Bullet Time"), "+bullettime")),
            _ => None,
        }
    }
    fn row_count(&self) -> usize {
        if self.weapons.is_empty() { 1 + usize::from(self.equipment_action().is_some()) }
        else { self.weapons.len() + 2 + usize::from(self.alt) + usize::from(self.reload) }
    }
    fn command(&self, row: usize) -> Option<&'static str> {
        match row.checked_sub(self.weapons.len())? {
            _ if self.weapons.is_empty() => (row == 0).then(|| self.equipment_action().map(|a| a.1)).flatten(),
            0 => Some("+attack"),
            1 if self.alt => Some("+attack2"),
            n if self.reload && n == 1 + usize::from(self.alt) => Some("+reload"),
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
    /// Selects add-on `i` (touch layout), as clicking its name does.
    pub(super) fn addon_ui_select(&mut self, i: usize) {
        if i != self.addon_ui.selected {
            self.addon_ui.scroll = 0;
            self.addon_ui.settings_scroll = 0.0;
        }
        self.addon_ui.selected = i.min(ADDON.len() - 1);
    }

    /// Add-on the classic browser has selected.
    pub(super) fn addon_ui_selected(&self) -> usize {
        self.addon_ui.selected
    }

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
        if self.addon_ui.focus == 0 { qcore::t!("Toggle") }
        else if self.addon_ui.focus == 1 { qcore::t!("Settings") }
        else if self.addon_ui.focus > self.browser_rows() { qcore::t!("Play") }
        else if self.browser_slot().is_some() { qcore::t!("Change slot") }
        else { qcore::t!("Configure") }
    }
    fn browser_slot(&self) -> Option<&'static str> {
        self.addon_ui.focus.checked_sub(2)
            .and_then(|row| self.selected_addon().weapons.get(row)).map(|weapon| weapon.slot)
    }
    fn list_offset(&self) -> usize {
        self.addon_ui.list_scroll.min(ADDON.len().saturating_sub(LIST_VISIBLE))
    }
    fn reveal_addon_selection(&mut self) {
        let offset = self.list_offset();
        self.addon_ui.list_scroll = if self.addon_ui.selected < offset {
            self.addon_ui.selected
        } else if self.addon_ui.selected >= offset + LIST_VISIBLE {
            self.addon_ui.selected + 1 - LIST_VISIBLE
        } else { offset };
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
        if (20.0..200.0).contains(&x) && (LIST_TOP..LIST_BOTTOM).contains(&y) {
            let index = self.list_offset() + ((y - LIST_TOP) / LIST_ROW) as usize;
            return (index < ADDON.len()).then_some((index, 0));
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
            // back to the Customize page the browser was opened from
            let hub = if self.screen == Screen::OptionsPage(Page::NewGameAddons) { Page::NewGameCustomize } else { Page::Customize };
            self.screen = Screen::OptionsPage(hub);
            self.opt_cursor = 1;
            return true;
        }
        if key == K_MOUSE1 {
            let Some((x, y)) = self.mouse else { return true };
            if self.back_button().is_some_and(|(bx, by, bw, bh)| (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y)) {
                return self.addon_key(K_ESCAPE, cv, out);
            }
            let Some((selected, focus)) = self.browser_hit((x, y)) else { return true };
            if selected != self.addon_ui.selected {
                self.addon_ui.scroll = 0;
                self.addon_ui.settings_scroll = 0.0;
            }
            self.addon_ui.selected = selected;
            self.addon_ui.focus = focus;
            // a name only selects (so its details can be read); the box toggles
            if focus == 0 && x >= CHECK_RIGHT {
                return true;
            }
        }
        if self.screen == Screen::OptionsPage(Page::NewGameAddons) && (key == K_YBUTTON || key == K_F1 + 9) {
            self.launch_play(cv, out);
            return true;
        }
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        let wheel = matches!(key, K_MWHEELUP | K_MWHEELDOWN);
        let paging = matches!(key, K_PGUP | K_PGDN);
        let over_list = self.mouse.is_some_and(|(x, y)| (20.0..200.0).contains(&x) && (LIST_TOP..LIST_BOTTOM).contains(&y));
        if (wheel && over_list) || (paging && self.addon_ui.focus == 0) {
            let step = if paging { LIST_VISIBLE } else { 1 };
            let max = ADDON.len().saturating_sub(LIST_VISIBLE);
            self.addon_ui.list_scroll = if matches!(key, K_MWHEELUP | K_PGUP) {
                self.list_offset().saturating_sub(step)
            } else { (self.list_offset() + step).min(max) };
            if paging {
                self.addon_ui.selected = self.addon_ui.selected.clamp(self.list_offset(), (self.list_offset() + LIST_VISIBLE - 1).min(ADDON.len() - 1));
                self.addon_ui.scroll = 0;
                self.addon_ui.settings_scroll = 0.0;
            }
            return true;
        }
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
                self.launch_play(cv, out);
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
            self.reveal_addon_selection();
        }
        true
    }
    fn description_lines(&self) -> Vec<String> {
        let mut lines = vec![String::new()];
        for word in qcore::lang::tr(self.selected_addon().description).split_whitespace() {
            let line = lines.last_mut().unwrap();
            if line.chars().count() + word.chars().count() + 1 > 78 {
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

}

mod draw;

#[cfg(test)]
mod tests;
