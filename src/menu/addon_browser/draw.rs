//! Browser panels; the checklist viewport leaves dedicated space for prompts.
use super::*;

impl Menu {
    pub(in crate::menu) fn draw_addon_browser(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, time: f64, page: Page) {
        c.text(24.0, 25.0, page_name(page), 14.0, GOLD_FG);
        c.round_box(20.0, 50.0, 180.0, 208.0, 3.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        for (row, (i, a)) in ADDON.iter().enumerate().skip(self.list_offset()).take(LIST_VISIBLE).enumerate() {
            let y = LIST_TOP + row as f32 * LIST_ROW;
            if i == self.addon_ui.selected {
                c.fill(22.0, y, 176.0, 19.0, HILITE);
            }
            c.frame(28.0, y + 6.0, 9.0, 9.0, BORDER);
            if cv.value_of(a.cvar) != 0.0 {
                c.fill(30.0, y + 8.0, 5.0, 5.0, GOLD_FG);
            }
            c.text(44.0, y + 6.0, a.name, TXT, if self.toys_locked(cv) { MUTED } else { FG });
        }
        if ADDON.len() > LIST_VISIBLE {
            let height = LIST_BOTTOM - LIST_TOP;
            let thumb = height * LIST_VISIBLE as f32 / ADDON.len() as f32;
            let y = LIST_TOP + self.list_offset() as f32 / (ADDON.len() - LIST_VISIBLE) as f32 * (height - thumb);
            c.fill(194.0, LIST_TOP, 2.0, height, [1.0, 1.0, 1.0, 0.12]);
            c.fill(194.0, y, 2.0, thumb, GOLD_FG);
        }
        if self.pad_ui {
            self.glyphs.hints(c, 28.0, 224.0, &[(K_ABUTTON, "Toggle")], true);
        } else {
            c.text(28.0, 224.0, "Enter: toggle", 8.0, MUTED);
            c.text(28.0, 238.0, "Right / Tab: details", 8.0, MUTED);
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
