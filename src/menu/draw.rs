use super::*;

impl Menu {
    pub fn draw(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        self.sync_autoaim_mapping(cv);
        if self.screen == Screen::None {
            return;
        }
        sync_addon_binds(cv, self.game_running);
        // the HUD editor shows the HUD as it is, under its dialog
        if self.screen == Screen::HudEditor {
            self.hud_edit.draw(c, cv, self.pad_ui, &self.glyphs);
            if let Some(o) = self.osk {
                let base_scale = c.scale;
                c.menu_layout();
                self.draw_osk(c, o);
                c.scale = base_scale;
                c.reset_origin();
            }
            return;
        }
        if self.screen == Screen::TouchEditor {
            c.fade_screen();
            self.canvas_layout(c);
            self.draw_touch_editor(c, cv);
            return;
        }
        c.fade_screen();
        let base_scale = c.scale;
        self.canvas_layout(c);
        if !matches!(self.screen, Screen::Options | Screen::OptionsPage(_) | Screen::Death | Screen::Quit | Screen::Mods | Screen::ModMaps | Screen::ModOnline | Screen::Load | Screen::Save | Screen::Help | Screen::Demos | Screen::DirBrowser | Screen::ServerBrowser) {
            c.ox += CLASSIC.0;
            c.oy += CLASSIC.1;
        }
        let title_c = |c: &mut Canvas, pic: &mut PicFn, name: &str| {
            let p = pic(name);
            if let Some((_, w, _)) = p {
                c.pic(((640 - w as i32) / 2) as f32, 16.0, p);
            }
        };
        let dot = |pic: &mut PicFn| pic(&format!("gfx/menudot{}.lmp", (realtime * 10.0) as i64 % 6 + 1));
        let title = |c: &mut Canvas, pic: &mut PicFn, name: &str| {
            let p = pic(name);
            if let Some((_, w, _)) = p {
                c.pic(((320 - w as i32) / 2) as f32, 4.0, p);
            }
        };
        match self.screen {
            Screen::Main => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_main.lmp");
                let p = pic("gfx/mainmenu.lmp");
                c.pic(72.0, 32.0, p);
                // extra entries under the classic graphic
                let extra: &[&str] = if self.custom_running { &[qcore::t!("Map Packs"), qcore::t!("Demos"), qcore::t!("Customize")] } else { &[qcore::t!("Map Packs"), qcore::t!("Demos")] };
                for (k, &label) in extra.iter().enumerate() {
                    let (y, w) = (32.0 + (5 + k) as f32 * 20.0 + 2.0, 112.0);
                    c.fill(72.0, y, w, 16.0, [0.0, 0.0, 0.0, 0.55]);
                    c.frame(72.0, y, w, 16.0, BORDER);
                    gold(c, 72.0 + (w - c.ptext_w(label)) / 2.0, y + 4.0, label);
                }
                if self.touch_shortcut_index() != Some(self.main_cursor) {
                    let d = dot(pic);
                    c.pic(54.0, 32.0 + self.main_cursor as f32 * 20.0, d);
                }
                self.draw_touch_shortcut(c);
            }
            Screen::Help => self.help.draw(c),
            Screen::Mods => self.mods.draw(c, modmenu::Page::List, self.pad_ui, &self.glyphs),
            Screen::Demos => self.demos.draw(c, self.mouse, self.pad_ui, &self.glyphs),
            Screen::ServerBrowser => self.draw_server_browser(c),
            Screen::DirBrowser => self.dir_browser.draw(c, self.drag.is_some(), self.pad_ui, &self.glyphs),
            Screen::ModMaps => self.mods.draw(c, modmenu::Page::Maps, self.pad_ui, &self.glyphs),
            Screen::ModOnline => self.mods.draw(c, modmenu::Page::Online, self.pad_ui, &self.glyphs),
            Screen::SinglePlayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_sgl.lmp");
                let p = pic("gfx/sp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.sp_cursor as f32 * 20.0, d);
                let (y, w) = (32.0 + 3.0 * 20.0 + 2.0, 112.0);
                c.fill(72.0, y, w, 16.0, [0.0, 0.0, 0.0, 0.55]);
                c.frame(72.0, y, w, 16.0, BORDER);
                gold(c, 72.0 + (w - c.ptext_w("Level Select")) / 2.0, y + 4.0, "Level Select");
            }
            Screen::Multiplayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/p_multi.lmp");
                let p = pic("gfx/mp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.mp_cursor as f32 * 20.0, d);
            }
            Screen::Load | Screen::Save => self.draw_cards(c, pic),
            Screen::Options => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, "gfx/p_option.lmp");
                self.draw_search_bar(c, realtime);
                if self.searching() {
                    self.draw_search_results(c, cv);
                } else {
                    panel(c, ROW0, PAGES.len(), self.game_running);
                    highlight(c, ROW0 + self.page_cursor as f32 * ROW_H);
                    for (i, (label, _)) in PAGES.iter().enumerate() {
                        let y = ROW0 + i as f32 * ROW_H + 2.0;
                        row_label(c, 0.0, y, label, FG);
                    }
                }
            }
            Screen::OptionsPage(page) if matches!(page, Page::Addons | Page::NewGameAddons) => {
                self.draw_addon_browser(c, pic, cv, realtime, page);
            }
            Screen::OptionsPage(Page::WeaponPriority) => {
                self.draw_weapon_priority(c, pic, cv);
            }
            Screen::OptionsPage(Page::Touch) => self.draw_touch_settings(c, cv),
            Screen::OptionsPage(page) => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, if is_net_page(page) { "gfx/p_multi.lmp" } else { "gfx/p_option.lmp" });
                self.draw_page_title(c, page);
                let total = self.page_len(page);
                let height = self.page_row_height();
                let offset = self.page_offset(page) / height;
                let sc = offset.floor() as usize;
                let end = (offset + self.page_vis(page) as f32).ceil() as usize;
                let bottom = self.page_bottom(page);
                panel_height(c, PAGE_ROW0, bottom - PAGE_ROW0, self.game_running);
                c.clip_rows(Some((PAGE_ROW0, bottom)));
                // faint stripes on every other row of a group, to read across to the value
                let mut odd = false;
                for i in 0..end.min(total) {
                    if matches!(self.page_item(page, i), Item::Heading(_)) {
                        odd = false;
                        continue;
                    }
                    if odd && i >= sc {
                        c.fill(PANEL_X + 2.0, self.page_row_y(page, i), PANEL_W - 4.0, height, ROW_STRIPE);
                    }
                    odd = !odd;
                }
                if self.opt_cursor < total
                    && !matches!(self.page_item(page, self.opt_cursor), Item::Run { run: Run::BrowseServers, .. } | Item::Heading(_))
                {
                    c.fill(PANEL_X + 2.0, self.page_row_y(page, self.opt_cursor), PANEL_W - 4.0, height, HILITE);
                }
                self.text_layouts.borrow_mut().clear();
                for i in sc..end.min(total) {
                    let blink = (realtime * 4.0) as i64 & 1 == 0 && i == self.opt_cursor;
                    let top = self.page_row_y(page, i);
                    let y = top + (height - ROW_H) / 2.0 + 2.0;
                    let dep = self.is_dependent(page, i);
                    if dep {
                        let more = i + 1 < total && self.is_dependent(page, i + 1);
                        dep_guide(c, top, height, more, DEP_GUIDE);
                    }
                    match self.page_item(page, i) {
                        Item::Text { .. } => self.draw_text_row(c, cv, page, i, y, blink),
                        item => self.draw_item(c, cv, item, y, blink, dep),
                    }
                }
                c.clip_rows(None);
                if let Some((x, top, h, ty, th)) = self.scrollbar(page) {
                    // scrollbar along the panel's right edge
                    c.round_box(x, top, SCROLL_W, h, SCROLL_W / 2.0, [1.0, 1.0, 1.0, 0.12], None);
                    let a = if self.drag.is_some() { 1.0 } else { 0.9 };
                    c.round_box(x, ty, SCROLL_W, th, SCROLL_W / 2.0, [0.85, 0.65, 0.3, a], None);
                }
                if page == Page::Setup {
                    // M_Setup_Draw: the player in a box, in the chosen colours.
                    let v = cv.value_of("_cl_color") as i32;
                    let y0 = bottom + 12.0;
                    let bx = pic("gfx/bigbox.lmp");
                    let bw = bx.map_or(64.0, |b| b.1 as f32);
                    c.pic(((640.0 - bw) / 2.0).floor(), y0, bx);
                    let p = pic(&format!("gfx/menuplyr.lmp#{},{}", (v >> 4) & 15, v & 15));
                    let pw = p.map_or(48.0, |p| p.1 as f32);
                    c.pic(((640.0 - pw) / 2.0).floor(), y0 + 8.0, p);
                }
                if page == Page::Crosshair { crate::reticle::preview(c, cv); }
                self.draw_custom_notes(c, cv, page);
                if matches!(page, Page::Export | Page::RenameDemo) { self.draw_dialog_notes(c, page); }
                if page == Page::Controls {
                    c.text(VALUE_X, PAGE_ROW0 - 16.0, "Keyboard", TXT, MUTED);
                    c.text(BIND_PAD_X, PAGE_ROW0 - 16.0, "Controller", TXT, MUTED);
                }
                if let Some(d) = self.drop {
                    self.draw_drop(c, cv, self.page_item(page, d.item), d);
                }
            }
            Screen::Quit => {
                let (bx, by, bw, bh) = QUIT_BOX;
                c.fill(bx, by, bw, bh, [0.05, 0.04, 0.03, 0.92]);
                c.frame(bx, by, bw, bh, BORDER);
                let msg = qcore::t!("Really quit?");
                c.text_center(320.0, by + 12.0, msg, TXT * 1.2, FG);
                for (i, (&x, (key, word))) in QUIT_BUTTONS.iter().zip([("Y", qcore::t!("Yes")), ("N", qcore::t!("No"))]).enumerate() {
                    let (w, h) = QUIT_BTN;
                    let on = i == self.quit_cursor;
                    if on {
                        c.fill(x, QUIT_BTN_Y, w, h, HILITE);
                    }
                    c.frame(x, QUIT_BTN_Y, w, h, BORDER);
                    if self.pad_ui {
                        let pad_key = (i == 1).then_some(K_BBUTTON);
                        self.glyphs.button_label(c, [x, QUIT_BTN_Y, w, h], pad_key, word, TXT, if on { GOLD_FG } else { FG });
                    } else {
                        // "(Y)es" in English; other languages keep the Y/N keys: "(Y) Oui"
                        let tail = if qcore::lang::current() == qcore::lang::Lang::English {
                            format!("){}", &word[1..])
                        } else {
                            format!(") {}", qcore::lang::tr(word))
                        };
                        let tx = x + ((w - c.text_w_raw(&format!("({key}{tail}"), TXT)) / 2.0).floor();
                        // gold hotkey letter
                        let ty = QUIT_BTN_Y + 3.0;
                        let mut cx = tx + txt(c, tx, ty, "(");
                        cx += txt_gold(c, cx, ty, key);
                        txt(c, cx, ty, &tail);
                    }
                }
            }
            Screen::Death => {
                let rows = self.death_rows();
                let height = 56.0 + rows as f32 * 24.0;
                c.fill(148.0, 96.0, 344.0, height, [0.05, 0.04, 0.03, 0.95]);
                c.frame(148.0, 96.0, 344.0, height, BORDER);
                c.text_center(320.0, 112.0, "You died", TXT * 1.2, GOLD_FG);
                let labels = [qcore::t!("Continue from most recent save"), qcore::t!("Select save"), qcore::t!("Restart level"), qcore::t!("Return to main menu"), qcore::t!("Quick Load")];
                for (i, label) in labels.into_iter().take(rows).enumerate() {
                    let y = 140.0 + i as f32 * 24.0;
                    if i == self.death_cursor { c.fill(160.0, y, 320.0, 20.0, HILITE); }
                    txt(c, 168.0, y + 4.0, &format!("{}. {}", i + 1, qcore::lang::tr(label)));
                }
            }
            Screen::None | Screen::HudEditor | Screen::TouchEditor => {}
        }
        self.canvas_layout(c);
        if let Some(o) = self.osk {
            self.draw_osk(c, o);
        }
        if !matches!(self.screen, Screen::OptionsPage(Page::Addons | Page::NewGameAddons | Page::WeaponPriority | Page::Touch)) {
            self.draw_tooltip(c, cv);
            self.draw_hints(c, cv);
        }
        self.draw_back(c);
        self.draw_prompt(c, cv);
        c.reset_origin();
        if self.autoaim_prompt { self.draw_autoaim_prompt(c); }
        self.draw_binding_prompt(c);
        c.scale = base_scale;
    }

    pub(super) fn draw_drop(&self, c: &mut Canvas, cv: &Cvars, item: Item, d: Drop) {
        let opts = self.options(item);
        let (x, y, w, vis) = self.drop_geom(d, &opts);
        let row_h = self.drop_row_height();
        let h = vis as f32 * row_h + 2.0;
        c.fill(x, y, w, h, [0.05, 0.04, 0.03, 0.97]);
        c.frame(x, y, w, h, BORDER);
        let cur = self.option_index(item, cv);
        for (k, o) in opts.iter().enumerate().skip(d.scroll).take(vis) {
            let ry = y + 1.0 + (k - d.scroll) as f32 * row_h;
            if k == d.cursor {
                c.fill(x + 1.0, ry, w - 2.0, row_h, HILITE);
            }
            c.text(x + 4.0, ry + (row_h - BOX_H) / 2.0 + 1.0, o, TXT, if k == cur { GOLD_FG } else { FG });
        }
        // scroll hints
        if d.scroll > 0 {
            c.shape(x + w - 10.0, y + 2.0, 7.0, 7.0, crate::font::Shape::ArrowUp, GOLD_FG);
        }
        if d.scroll + vis < opts.len() {
            c.shape(x + w - 10.0, y + h - 9.0, 7.0, 7.0, crate::font::Shape::ArrowDown, GOLD_FG);
        }
    }

    /// One options row; `dep` rows hang off a master toggle above them.
    pub(super) fn draw_item(&self, c: &mut Canvas, cv: &Cvars, item: Item, y: f32, blink: bool, dep: bool) {
        let indent = if dep { DEP_INDENT } else { 0.0 };
        let off = self.item_disabled(item, cv);
        if self.game_locked(item, cv) && let Item::Slider { label, .. } | Item::Choice { label, .. } = item {
            row_label(c, indent, y, label, MUTED);
            c.text(VALUE_X, y, "(fixed for this game)", TXT, MUTED);
            return;
        }
        // disabled rows keep their control, dimmed, so the value stays visible
        let (vc, ac) = if off { (MUTED, MUTED) } else { (FG, GOLD_FG) };
        let label = |c: &mut Canvas, l: &str| row_label(c, indent, y, l, if off { MUTED } else if dep { DEP_FG } else { FG });
        if self.changed(item, cv) {
            changed_mark(c, y, off);
        }
        // values with a dropdown get a combo box; two-way values a ‹ value › box
        let opts = if matches!(item, Item::Toggle { .. }) { 0 } else { self.options(item).len() };
        if opts >= 2 {
            let frame = if off { [BORDER[0], BORDER[1], BORDER[2], 0.45] } else { BORDER };
            c.fill(VALUE_X - 3.0, y - 1.0, DROP_W, BOX_H, [0.0, 0.0, 0.0, 0.5]);
            c.frame(VALUE_X - 3.0, y - 1.0, DROP_W, BOX_H, frame);
            if opts == 2 {
                c.shape(VALUE_X - 1.0, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowLeft, ac);
                c.shape(VALUE_X + DROP_W - 13.0, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowRight, ac);
            } else {
                c.shape(VALUE_X + DROP_W - 13.0, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowDown, ac);
            }
        }
        let vx = if opts == 2 { VALUE_X + 9.0 } else { VALUE_X };
        match item {
            Item::Slider { label: l, cvar, min, max, invert, .. } => {
                label(c, l);
                let v = cv.value_of(cvar);
                let mut f = ((v - min) / (max - min)).clamp(0.0, 1.0);
                if invert {
                    f = 1.0 - f;
                }
                // track, filled part, knob
                let (tx, tw) = (VALUE_X, 92.0);
                let (fill, knob) = if off { ([0.42, 0.38, 0.33, 1.0], MUTED) } else { ([0.80, 0.55, 0.22, 1.0], [0.95, 0.80, 0.55, 1.0]) };
                c.round_box(tx, y + 3.0, tw, 3.0, 1.5, [0.25, 0.22, 0.19, 1.0], None);
                c.round_box(tx, y + 3.0, (tw * f).max(3.0), 3.0, 1.5, fill, None);
                c.round_box(tx + (tw - 8.0) * f, y + 0.5, 8.0, 8.0, 4.0, knob, Some([0.2, 0.12, 0.05, 0.8]));
                c.text_raw(VALUE_X + 100.0, y, &slider_text(cvar, v), TXT, vc);
            }
            Item::Toggle { label: l, cvar } => {
                let locked = self.game_locked(item, cv);
                label(c, l);
                switch(c, VALUE_X, y, cv.value_of(cvar) != 0.0, off);
                if locked {
                    c.text(VALUE_X + SWITCH_W + 6.0, y, "(fixed for this game)", TXT, MUTED);
                }
            }
            Item::Choice { label: l, cvar, choices } => {
                label(c, l);
                let n = self.choice_count(cvar, choices);
                let value = cv.value_of(cvar);
                let choice = if cvar == "cl_autodemo_pb" && value.round() as i32 == 3 {
                    qcore::t!("any% and 100%")
                } else {
                    choices[nearest_choice(value, &choices[..n])].0
                };
                c.text(vx, y, choice, TXT, vc);
            }
            Item::DisplayMode => {
                label(c, "Display Mode");
                { txt(c, vx, y, MODES[self.pending_mode.min(2)]); }
            }
            Item::Resolution => {
                label(c, "Resolution");
                let s = if self.pending_mode == 2 {
                    qcore::tr!("desktop")
                } else {
                    format!("{}x{}", self.pending_res.0, self.pending_res.1)
                };
                { txt(c, vx, y, &s); }
            }
            // drawn by `draw_text_row` (caret and selection)
            Item::Text { .. } => {}
            Item::Color { label: l, high } => {
                label(c, l);
                let v = cv.value_of("_cl_color") as i32;
                let k = if high { v >> 4 } else { v & 15 }.clamp(0, 13) as usize;
                let [r, g, b] = crate::draw::PLAYER_COLORS[k].map(|x| x as f32 / 255.0);
                c.fill(VALUE_X, y - 1.0, 40.0, BOX_H - 1.0, [r, g, b, 1.0]);
                c.frame(VALUE_X, y - 1.0, 40.0, BOX_H - 1.0, BORDER);
                { txt(c, VALUE_X + 48.0, y, &k.to_string()); }
            }
            Item::Language => {
                label(c, "Language");
                txt(c, vx, y, &super::options::language_options()[super::options::language_index(cv)]);
            }
            Item::UserDataDir => {
                label(c, "User Data Directory");
                let bx = VALUE_X - 3.0;
                let bw = PANEL_X + PANEL_W - 14.0 - bx;
                c.fill(bx, y - 1.0, bw, BOX_H, [0.0, 0.0, 0.0, 0.5]);
                c.frame(bx, y - 1.0, bw, BOX_H, BORDER);
                let path = crate::userdata::parent().to_string_lossy().into_owned();
                let shown = dir_browser::fit_path_tail(c, &path, bw - 8.0, TXT);
                c.text_raw(VALUE_X, y, &shown, TXT, if cfg!(target_arch = "wasm32") { MUTED } else { GOLD_FG });
            }
            Item::ShaderPreset => {
                label(c, "Preset");
                let s: String = self.preset_options()[self.preset_index(cv)].chars().take(13).collect();
                { txt(c, vx, y, &s); }
            }
            Item::Soundtrack => {
                label(c, "Soundtrack");
                let opts = self.soundtrack_options();
                let idx = self.soundtrack_index(cv);
                let s: String = opts[idx].chars().take(18).collect();
                if idx == 0 {
                    c.text(vx, y, &s, TXT, FG);
                } else {
                    c.text_raw(vx, y, &s, TXT, FG);
                }
            }
            Item::HudPreset => {
                label(c, "HUD Preset");
                let opts = self.hud_preset_options();
                let idx = self.hud_preset_index(cv);
                let s: String = opts[idx].chars().take(18).collect();
                if idx <= 1 || idx == opts.len() - 1 {
                    c.text(vx, y, &s, TXT, FG);
                } else {
                    c.text_raw(vx, y, &s, TXT, FG);
                }
            }
            Item::ShaderParam(k) => self.draw_shader_param(c, y, k, cv),
            Item::Map => {
                label(c, "Map");
                let m = self.host_map(cv).unwrap_or_else(|| "(none)".into());
                let t = self.net_maps.iter().find(|x| x.name == m).map_or(String::new(), |x| x.title.clone());
                let s: String = format!("{m} {t}").chars().take(13).collect();
                { txt(c, vx, y, &s); }
            }
            Item::Run { label, run: Run::BrowseServers } => {
                self.draw_server_browser_button(c, y, label);
            }
            Item::Run { label: l, .. } => {
                c.text_center(320.0, y, l, TXT, if self.item_disabled(item, cv) { MUTED } else { GOLD_FG });
            }
            Item::Bind { label: l, cmd } => {
                label(c, l);
                let capturing = self.capture.is_some_and(|r| matches!(self.page_item(Page::Controls, r), Item::Bind { cmd: c2, .. } if c2 == cmd));
                self.draw_binding_columns(c, cmd, y - 1.0,
                    [(VALUE_X, BIND_PAD_X - 4.0), (BIND_PAD_X, PANEL_X + PANEL_W - 2.0)], capturing, blink);
            }
            Item::Sub { label: l, page } => {
                // submenu rows: gold label, arrow at the panel's right edge
                let dim = sub_disabled(page, cv);
                let fg = if dim { MUTED } else { GOLD_FG };
                row_label(c, indent, y, l, fg);
                if dim {
                    c.text(VALUE_X, y, "(off)", TXT, MUTED);
                }
                c.shape(SUB_ARROW_X, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowRight, fg);
            }
            Item::ResetBinds => {
                c.text_center(320.0, y, "Reset to Defaults", TXT, GOLD_FG);
            }
            Item::ResetShaderParams => {
                // live only while some parameter differs from the preset
                let dirty = cv.by_name("r_shader_params").is_some_and(|c| !c.string.trim().is_empty());
                c.text_center(320.0, y, "Reset Shader Settings", TXT, if dirty { GOLD_FG } else { MUTED });
            }
            Item::Heading(l) => heading(c, y, l),
            Item::Apply => {
                // a button, live only while there are pending changes
                let col = if self.display_dirty(cv) { GOLD_FG } else { MUTED };
                c.text_center(320.0, y, "Apply Display Changes", TXT, col);
            }
        }
    }
}
