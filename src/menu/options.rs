use super::*;

impl Menu {
    /// Changes one option by `dir` steps. Returns true if something changed.
    pub(super) fn adjust(&mut self, item: Item, dir: i32, enter: bool, cv: &mut Cvars, out: &mut Vec<Action>) -> bool {
        if self.item_disabled(item, cv) { return false; }
        match item {
            Item::Slider { cvar, min, max, step, invert, .. } => {
                let d = if invert { -dir } else { dir } as f32;
                let v = (cv.value_of(cvar) + d * step).clamp(min, max);
                set_num(cv, cvar, v);
            }
            Item::Toggle { cvar, .. } => {
                let v = cv.value_of(cvar);
                cv.set(cvar, if v != 0.0 { "0" } else { "1" });
                if cvar == "assist_autoaim" {
                    if v == 0.0 { self.autoaim_defaults(out); }
                    else { self.autoaim_conflicts.clear(); }
                }
            }
            Item::Choice { cvar, choices, .. } => {
                let n = self.choice_count(cvar, choices);
                let cur = nearest_choice(cv.value_of(cvar), &choices[..n]);
                let i = (cur as i32 + dir).rem_euclid(n as i32) as usize;
                set_num(cv, cvar, choices[i].1);
            }
            Item::DisplayMode => {
                self.pending_mode = (self.pending_mode as i32 + dir).rem_euclid(MODES.len() as i32) as usize;
            }
            Item::Resolution => {
                if self.resolutions.is_empty() {
                    return false;
                }
                let n = self.resolutions.len() as i32;
                let cur = self.resolutions.iter().position(|&r| r == self.pending_res);
                // list is largest first; "right" goes to larger sizes
                let i = match cur {
                    Some(i) => (i as i32 - dir).rem_euclid(n),
                    None => 0,
                };
                self.pending_res = self.resolutions[i as usize];
            }
            Item::Color { high, .. } => {
                let c = cv.value_of("_cl_color") as i32;
                let (mut t, mut b) = (c >> 4, c & 15);
                let v = if high { &mut t } else { &mut b };
                *v = (*v + dir).rem_euclid(14);
                cv.set("_cl_color", &(t * 16 + b).to_string());
            }
            Item::ShaderPreset => {
                let n = self.shader_presets.len() as i32 + 1;
                let i = (self.preset_index(cv) as i32 + dir).rem_euclid(n) as usize;
                self.set_preset(i, cv);
            }
            Item::ShaderParam(k) => return self.adjust_shader_param(k, dir, cv),
            Item::Map => {
                let n = self.net_maps.len() as i32;
                if n == 0 {
                    return false;
                }
                let cur = self.host_map(cv).and_then(|m| self.net_maps.iter().position(|x| x.name == m)).unwrap_or(0);
                let i = (cur as i32 + dir).rem_euclid(n) as usize;
                cv.set("_mp_map", &self.net_maps[i].name.clone());
            }
            Item::Language => {
                let n = qcore::lang::Lang::ALL.len() as i32 + 1;
                let i = (language_index(cv) as i32 + dir).rem_euclid(n) as usize;
                set_language(cv, i);
            }
            Item::Soundtrack => {
                let n = self.soundtracks.len() as i32 + 1;
                let i = (self.soundtrack_index(cv) as i32 + dir).rem_euclid(n) as usize;
                self.set_soundtrack(i, cv);
            }
            Item::HudPreset => {
                let n = self.hud_preset_options().len() as i32;
                let i = (self.hud_preset_index(cv) as i32 + dir).rem_euclid(n) as usize;
                self.set_hud_preset(i, cv);
            }
            Item::Text { .. } | Item::Run { .. } | Item::Bind { .. } | Item::ResetBinds | Item::ResetShaderParams | Item::Sub { .. } | Item::UserDataDir => return false,
            Item::Apply => {
                if !enter || !self.display_dirty(cv) {
                    return false;
                }
                let mut cmd = format!("vid_fullscreen {}\n", self.pending_mode);
                // borderless always uses the desktop size; keep the windowed size
                if self.pending_mode != 2 {
                    let (w, h) = self.pending_res;
                    cmd += &format!("vid_width {w}\nvid_height {h}\n");
                }
                out.push(Action::Command(cmd + "vid_apply\n"));
            }
        }
        true
    }

    /// Display names of an item's discrete values (empty for sliders/Apply).
    /// Items with more than two get a dropdown.
    pub(super) fn options(&self, item: Item) -> Vec<String> {
        match item {
            Item::Toggle { .. } => vec!["off".into(), "on".into()],
            Item::Choice { cvar, choices, .. } => choices[..self.choice_count(cvar, choices)].iter().map(|c| c.0.to_string()).collect(),
            Item::DisplayMode => MODES.iter().map(|m| m.to_string()).collect(),
            Item::Resolution if self.pending_mode != 2 => self.resolutions.iter().map(|(w, h)| format!("{w}x{h}")).collect(),
            Item::Map => self
                .net_maps
                .iter()
                .map(|m| {
                    let t = format!("{:<8} {}", m.name, m.title);
                    t.chars().take(36).collect::<String>().trim_end().to_string()
                })
                .collect(),
            Item::ShaderPreset => self.preset_options(),
            Item::Language => language_options(),
            Item::Soundtrack => self.soundtrack_options(),
            Item::HudPreset => self.hud_preset_options(),
            _ => Vec::new(),
        }
    }

    /// Index of the item's current value in `options`.
    pub(super) fn option_index(&self, item: Item, cv: &Cvars) -> usize {
        match item {
            Item::Toggle { cvar, .. } => (cv.value_of(cvar) != 0.0) as usize,
            Item::Choice { cvar, choices, .. } => nearest_choice(cv.value_of(cvar), &choices[..self.choice_count(cvar, choices)]),
            Item::DisplayMode => self.pending_mode.min(MODES.len() - 1),
            Item::Resolution => self.resolutions.iter().position(|&r| r == self.pending_res).unwrap_or(0),
            Item::Map => self.host_map(cv).and_then(|m| self.net_maps.iter().position(|x| x.name == m)).unwrap_or(0),
            Item::ShaderPreset => self.preset_index(cv),
            Item::Language => language_index(cv),
            Item::Soundtrack => self.soundtrack_index(cv),
            Item::HudPreset => self.hud_preset_index(cv),
            _ => 0,
        }
    }

    pub(super) fn set_option(&mut self, item: Item, i: usize, cv: &mut Cvars) {
        match item {
            Item::Toggle { cvar, .. } => {
                cv.set(cvar, if i != 0 { "1" } else { "0" });
            }
            Item::Choice { cvar, choices, .. } => set_num(cv, cvar, choices[i].1),
            Item::DisplayMode => self.pending_mode = i,
            Item::Resolution => self.pending_res = self.resolutions[i],
            Item::Map => {
                if let Some(m) = self.net_maps.get(i) {
                    cv.set("_mp_map", &m.name.clone());
                }
            }
            Item::ShaderPreset => self.set_preset(i, cv),
            Item::Language => set_language(cv, i),
            Item::Soundtrack => self.set_soundtrack(i, cv),
            Item::HudPreset => self.set_hud_preset(i, cv),
            _ => {}
        }
    }

    /// Dropdown box: (x, y, w) and visible entry count. Opens below its row,
    /// or above when it would run off the bottom.
    pub(super) fn drop_row_height(&self) -> f32 {
        if self.touch_ui { 18.0 } else { BOX_H }
    }

    pub(super) fn drop_geom(&self, d: Drop, opts: &[String]) -> (f32, f32, f32, usize) {
        let vis = opts.len().min(DROP_ROWS);
        let w = opts.iter().map(|o| o.len() as f32 * 8.0 + 20.0).fold(DROP_W, f32::max);
        let row_y = match self.screen {
            Screen::OptionsPage(p) => self.page_row_y(p, d.item),
            _ => PAGE_ROW0 + d.item as f32 * ROW_H,
        };
        let h = vis as f32 * self.drop_row_height() + 2.0;
        let row_h = self.page_row_height();
        let y = if row_y + row_h + h > MENU_H - 4.0 { (row_y + 1.0 - h).max(4.0) } else { row_y + row_h - 1.0 };
        (VALUE_X - 3.0, y, w, vis)
    }

    /// Dropdown entry under the point.
    pub(super) fn drop_at(&self, (x, y): (f32, f32), d: Drop) -> Option<usize> {
        let Screen::OptionsPage(p) = self.screen else { return None };
        let opts = self.options(self.page_item(p, d.item));
        let (bx, by, bw, vis) = self.drop_geom(d, &opts);
        let k = ((y - by - 1.0) / self.drop_row_height()).floor();
        (x >= bx && x < bx + bw && k >= 0.0 && (k as usize) < vis).then(|| d.scroll + k as usize)
    }

    /// Keys while a dropdown is open: navigate, pick, or close.
    pub(super) fn drop_key(&mut self, key: i32, mut d: Drop, item: Item, cv: &mut Cvars, out: &mut Vec<Action>) {
        let n = self.options(item).len();
        if n == 0 {
            self.drop = None;
            return;
        }
        let pick = |m: &mut Self, i: usize, cv: &mut Cvars, out: &mut Vec<Action>| {
            m.set_option(item, i, cv);
            m.drop = None;
            out.push(Action::Sound("misc/menu3.wav"));
        };
        match key {
            K_MOUSE1 => match self.mouse.and_then(|p| self.drop_at(p, d)) {
                Some(i) => pick(self, i, cv, out),
                None => self.drop = None,
            },
            K_ENTER | K_KP_ENTER | K_ABUTTON => pick(self, d.cursor, cv, out),
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => self.drop = None,
            K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN => {
                let up = matches!(key, K_UPARROW | K_DPAD_UP);
                d.cursor = if up { d.cursor.saturating_sub(1) } else { (d.cursor + 1).min(n - 1) };
                clamp_scroll(&mut d);
                self.drop = Some(d);
                out.push(Action::Sound("misc/menu1.wav"));
            }
            K_MWHEELUP | K_MWHEELDOWN => {
                let max = n.saturating_sub(DROP_ROWS);
                d.scroll = if key == K_MWHEELUP { d.scroll.saturating_sub(1) } else { (d.scroll + 1).min(max) };
                // keep the highlight on the entry under the pointer
                if let Some(i) = self.mouse.and_then(|p| self.drop_at(p, d)) {
                    d.cursor = i;
                }
                self.drop = Some(d);
            }
            _ => {}
        }
    }

    /// Choices usable on this device (MSAA limited by the GPU).
    pub(super) fn choice_count(&self, cvar: &str, choices: &[(&str, f32)]) -> usize {
        if cvar == "r_msaa" {
            choices.iter().filter(|c| c.1 as u32 <= self.max_msaa.max(1)).count().max(1)
        } else {
            choices.len()
        }
    }

    /// True when the pending display settings differ from the cvars.
    pub(super) fn display_dirty(&self, cv: &Cvars) -> bool {
        self.pending_mode != cv.value_of("vid_fullscreen") as usize
            || self.pending_mode != 2 && self.pending_res != (cv.value_of("vid_width") as u32, cv.value_of("vid_height") as u32)
    }

    /// Soundtrack dropdown entries: Default (active mod) followed by discovered packs.
    pub(super) fn soundtrack_options(&self) -> Vec<String> {
        std::iter::once(qcore::t!("Default").to_string())
            .chain(self.soundtracks.iter().map(|(_, label)| label.clone()))
            .collect()
    }

    pub(super) fn soundtrack_index(&self, cv: &Cvars) -> usize {
        let cur = cv.by_name("s_soundtrack").map_or("", |c| c.string.trim());
        if cur.is_empty() {
            return 0;
        }
        self.soundtracks
            .iter()
            .position(|(id, _)| id.eq_ignore_ascii_case(cur))
            .map_or(0, |i| i + 1)
    }

    pub(super) fn set_soundtrack(&self, i: usize, cv: &mut Cvars) {
        let id = i
            .checked_sub(1)
            .and_then(|k| self.soundtracks.get(k))
            .map_or("", |(id, _)| id.as_str());
        cv.set("s_soundtrack", id);
    }

    /// HUD Preset dropdown entries: modern, classic, saved user presets, then custom.
    pub(super) fn hud_preset_options(&self) -> Vec<String> {
        let mut out = vec![qcore::t!("modern").to_string(), qcore::t!("classic").to_string()];
        out.extend(self.hud_edit.presets.iter().map(|p| p.name.clone()));
        out.push(qcore::t!("custom").to_string());
        out
    }

    pub(super) fn hud_preset_index(&self, cv: &Cvars) -> usize {
        match cv.value_of("hud_preset") as i32 {
            0 => 0,
            1 => 1,
            _ => {
                let name = cv.by_name(crate::hud::presets::PRESET_NAME).map_or("", |c| c.string.trim());
                if !name.is_empty()
                    && let Some(k) = self.hud_edit.presets.iter().position(|p| p.name.eq_ignore_ascii_case(name))
                {
                    return 2 + k;
                }
                2 + self.hud_edit.presets.len()
            }
        }
    }

    pub(super) fn set_hud_preset(&self, i: usize, cv: &mut Cvars) {
        match i {
            0 => {
                cv.set("hud_preset", "0");
                cv.set(crate::hud::presets::PRESET_NAME, "");
            }
            1 => {
                cv.set("hud_preset", "1");
                cv.set(crate::hud::presets::PRESET_NAME, "");
            }
            k if k >= 2 && k - 2 < self.hud_edit.presets.len() => {
                let p = &self.hud_edit.presets[k - 2];
                cv.set(crate::hud::editor::CVAR, &p.layout);
                cv.set("hud_preset", "2");
                cv.set(crate::hud::presets::PRESET_NAME, &p.name);
            }
            _ => {
                cv.set("hud_preset", "2");
                cv.set(crate::hud::presets::PRESET_NAME, "");
            }
        }
    }
}

/// Language row values: System (auto), then each language by its own name.
pub(super) fn language_options() -> Vec<String> {
    std::iter::once(qcore::t!("System").to_string()).chain(qcore::lang::Lang::ALL.iter().map(|l| l.native().to_string())).collect()
}

/// Index into [`language_options`] of the `language` cvar (0 = auto).
pub(super) fn language_index(cv: &Cvars) -> usize {
    let s = cv.by_name("language").map_or("", |c| c.string.as_str());
    qcore::lang::Lang::from_name(s).and_then(|l| qcore::lang::Lang::ALL.iter().position(|&x| x == l)).map_or(0, |i| i + 1)
}

fn set_language(cv: &mut Cvars, i: usize) {
    let v = i.checked_sub(1).and_then(|k| qcore::lang::Lang::ALL.get(k)).map_or("auto", |l| l.name());
    cv.set("language", v);
}
