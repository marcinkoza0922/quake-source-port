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
        c.fade_screen();
        let base_scale = c.scale;
        c.menu_layout();
        if !matches!(self.screen, Screen::Options | Screen::OptionsPage(_) | Screen::Death | Screen::Quit | Screen::Mods | Screen::ModMaps | Screen::ModOnline | Screen::Load | Screen::Save | Screen::Help | Screen::Demos | Screen::DirBrowser) {
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
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.main_cursor as f32 * 20.0, d);
            }
            Screen::Help => self.help.draw(c),
            Screen::Mods => self.mods.draw(c, modmenu::Page::List, self.pad_ui, &self.glyphs),
            Screen::Demos => self.demos.draw(c, self.mouse, self.pad_ui, &self.glyphs),
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
                    panel(c, ROW0, PAGES.len());
                    highlight(c, ROW0 + self.page_cursor as f32 * ROW_H);
                    for (i, (label, _)) in PAGES.iter().enumerate() {
                        let y = ROW0 + i as f32 * ROW_H + 2.0;
                        c.text_center(320.0, y, label, TXT, FG);
                    }
                }
            }
            Screen::OptionsPage(page) if matches!(page, Page::Addons | Page::NewGameAddons) => {
                self.draw_addon_browser(c, pic, cv, realtime, page);
            }
            Screen::OptionsPage(Page::WeaponPriority) => {
                self.draw_weapon_priority(c, pic, cv);
            }
            Screen::OptionsPage(page) => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, if is_net_page(page) { "gfx/p_multi.lmp" } else { "gfx/p_option.lmp" });
                self.draw_page_title(c, page);
                let total = self.page_len(page);
                let n = self.page_vis(page);
                let sc = self.page_scroll(page);
                panel(c, PAGE_ROW0, n);
                if self.opt_cursor < total {
                    highlight(c, self.page_row_y(page, self.opt_cursor));
                }
                self.text_layouts.borrow_mut().clear();
                for i in sc..sc + n {
                    let blink = (realtime * 4.0) as i64 & 1 == 0 && i == self.opt_cursor;
                    let y = self.page_row_y(page, i) + 2.0;
                    match self.page_item(page, i) {
                        Item::Text { .. } => self.draw_text_row(c, cv, page, i, y, blink),
                        item => self.draw_item(c, cv, item, y, blink),
                    }
                }
                if let Some((x, top, h, ty, th)) = self.scrollbar(page) {
                    // scrollbar along the panel's right edge
                    c.fill(x, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
                    let a = if self.drag.is_some() { 1.0 } else { 0.9 };
                    c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, a]);
                }
                if page == Page::Setup {
                    // M_Setup_Draw: the player in a box, in the chosen colours.
                    let v = cv.value_of("_cl_color") as i32;
                    let y0 = PAGE_ROW0 + n as f32 * ROW_H + 12.0;
                    let bx = pic("gfx/bigbox.lmp");
                    let bw = bx.map_or(64.0, |b| b.1 as f32);
                    c.pic(((640.0 - bw) / 2.0).floor(), y0, bx);
                    let p = pic(&format!("gfx/menuplyr.lmp#{},{}", (v >> 4) & 15, v & 15));
                    let pw = p.map_or(48.0, |p| p.1 as f32);
                    c.pic(((640.0 - pw) / 2.0).floor(), y0 + 8.0, p);
                }
                if page == Page::Crosshair { crate::reticle::preview(c, cv); }
                self.draw_custom_notes(c, cv, page, n);
                if matches!(page, Page::Export | Page::RenameDemo) { self.draw_dialog_notes(c, n); }
                if page == Page::Join && self.servers.is_empty() && !self.searching {
                    let s = "no servers found yet";
                    c.text_center(320.0, PAGE_ROW0 + n as f32 * ROW_H + 8.0, s, TXT, MUTED);
                }
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
                c.fill(148.0, 96.0, 344.0, 152.0, [0.05, 0.04, 0.03, 0.95]);
                c.frame(148.0, 96.0, 344.0, 152.0, BORDER);
                c.text_center(320.0, 112.0, "You died", TXT * 1.2, GOLD_FG);
                for (i, label) in [qcore::t!("Continue from most recent save"), qcore::t!("Select save"), qcore::t!("Restart level"), qcore::t!("Return to main menu")].iter().enumerate() {
                    let y = 140.0 + i as f32 * 24.0;
                    if i == self.death_cursor { c.fill(160.0, y, 320.0, 20.0, HILITE); }
                    txt(c, 168.0, y + 4.0, &format!("{}. {}", i + 1, qcore::lang::tr(label)));
                }
            }
            Screen::None | Screen::HudEditor => {}
        }
        c.menu_layout();
        if let Some(o) = self.osk {
            self.draw_osk(c, o);
        }
        if !matches!(self.screen, Screen::OptionsPage(Page::Addons | Page::NewGameAddons | Page::WeaponPriority)) {
            self.draw_tooltip(c, cv);
            self.draw_hints(c);
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
        let h = vis as f32 * BOX_H + 2.0;
        c.fill(x, y, w, h, [0.05, 0.04, 0.03, 0.97]);
        c.frame(x, y, w, h, BORDER);
        let cur = self.option_index(item, cv);
        for (k, o) in opts.iter().enumerate().skip(d.scroll).take(vis) {
            let ry = y + 1.0 + (k - d.scroll) as f32 * BOX_H;
            if k == d.cursor {
                c.fill(x + 1.0, ry, w - 2.0, BOX_H, HILITE);
            }
            c.text(x + 4.0, ry + 1.0, o, TXT, if k == cur { GOLD_FG } else { FG });
        }
        // scroll hints
        if d.scroll > 0 {
            c.shape(x + w - 10.0, y + 2.0, 7.0, 7.0, crate::font::Shape::ArrowUp, GOLD_FG);
        }
        if d.scroll + vis < opts.len() {
            c.shape(x + w - 10.0, y + h - 9.0, 7.0, 7.0, crate::font::Shape::ArrowDown, GOLD_FG);
        }
    }

    pub(super) fn draw_item(&self, c: &mut Canvas, cv: &Cvars, item: Item, y: f32, blink: bool) {
        if self.item_disabled(item, cv) && let Item::Slider { label, .. } | Item::Choice { label, .. } = item {
            c.text_right(LABEL_R, y, label, TXT, MUTED);
            c.text(VALUE_X, y, if self.game_locked(item, cv) { "(fixed for this game)" } else { "(disabled)" }, TXT, MUTED);
            return;
        }
        let label = |c: &mut Canvas, l: &str| c.text_right(LABEL_R, y, l, TXT, FG);
        // values with a dropdown get a combo box
        if self.options(item).len() > 2 {
            c.fill(VALUE_X - 3.0, y - 1.0, DROP_W, BOX_H, [0.0, 0.0, 0.0, 0.5]);
            c.frame(VALUE_X - 3.0, y - 1.0, DROP_W, BOX_H, BORDER);
            c.shape(VALUE_X + DROP_W - 13.0, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowDown, GOLD_FG);
        }
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
                c.round_box(tx, y + 3.0, tw, 3.0, 1.5, [0.25, 0.22, 0.19, 1.0], None);
                c.round_box(tx, y + 3.0, (tw * f).max(3.0), 3.0, 1.5, [0.80, 0.55, 0.22, 1.0], None);
                c.round_box(tx + (tw - 8.0) * f, y + 0.5, 8.0, 8.0, 4.0, [0.95, 0.80, 0.55, 1.0], Some([0.2, 0.12, 0.05, 0.8]));
                txt(c, VALUE_X + 100.0, y, &qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
            }
            Item::Toggle { label: l, cvar } => {
                let locked = self.game_locked(item, cv);
                if locked {
                    c.text_right(LABEL_R, y, l, TXT, MUTED);
                } else {
                    label(c, l);
                }
                let on = cv.value_of(cvar) != 0.0;
                c.round_box(VALUE_X, y - 0.5, 9.0, 9.0, 1.5, [0.0, 0.0, 0.0, 0.5], Some(BORDER));
                if on {
                    c.round_box(VALUE_X + 2.0, y + 1.5, 5.0, 5.0, 1.0, if locked { MUTED } else { GOLD_FG }, None);
                }
                if locked {
                    c.text(VALUE_X + 14.0, y, "(fixed for this game)", TXT, MUTED);
                }
            }
            Item::Choice { label: l, cvar, choices } => {
                label(c, l);
                let n = self.choice_count(cvar, choices);
                { txt(c, VALUE_X, y, choices[nearest_choice(cv.value_of(cvar), &choices[..n])].0); }
            }
            Item::DisplayMode => {
                label(c, "Display Mode");
                { txt(c, VALUE_X, y, MODES[self.pending_mode.min(2)]); }
            }
            Item::Resolution => {
                label(c, "Resolution");
                let s = if self.pending_mode == 2 {
                    qcore::tr!("desktop")
                } else {
                    format!("{}x{}", self.pending_res.0, self.pending_res.1)
                };
                { txt(c, VALUE_X, y, &s); }
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
                txt(c, VALUE_X, y, &super::options::language_options()[super::options::language_index(cv)]);
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
                { txt(c, VALUE_X, y, &s); }
            }
            Item::Soundtrack => {
                label(c, "Soundtrack");
                let opts = self.soundtrack_options();
                let idx = self.soundtrack_index(cv);
                let s: String = opts[idx].chars().take(18).collect();
                if idx == 0 {
                    c.text(VALUE_X, y, &s, TXT, FG);
                } else {
                    c.text_raw(VALUE_X, y, &s, TXT, FG);
                }
            }
            Item::HudPreset => {
                label(c, "HUD Preset");
                let opts = self.hud_preset_options();
                let idx = self.hud_preset_index(cv);
                let s: String = opts[idx].chars().take(18).collect();
                if idx <= 1 || idx == opts.len() - 1 {
                    c.text(VALUE_X, y, &s, TXT, FG);
                } else {
                    c.text_raw(VALUE_X, y, &s, TXT, FG);
                }
            }
            Item::ShaderParam(k) => self.draw_shader_param(c, y, k, cv),
            Item::Map => {
                label(c, "Map");
                let m = self.host_map(cv).unwrap_or_else(|| "(none)".into());
                let t = self.net_maps.iter().find(|x| x.name == m).map_or(String::new(), |x| x.title.clone());
                let s: String = format!("{m} {t}").chars().take(13).collect();
                { txt(c, VALUE_X, y, &s); }
            }
            Item::Run { label: l, run } => {
                let l = if run == Run::Search && self.searching { qcore::t!("Searching...") } else { l };
                c.text_center(320.0, y, l, TXT, if self.item_disabled(item, cv) { MUTED } else { GOLD_FG });
            }
            Item::Server(k) => {
                if let Some(s) = self.servers.get(k) {
                    let name: String = s.hostname.chars().take(20).collect();
                    label(c, &name);
                    let m: String = s.map.chars().take(10).collect();
                    { txt(c, VALUE_X, y, &format!("{m:<10} {}/{}", s.players, s.max_players)); }
                }
            }
            Item::Bind { label: l, cmd } => {
                label(c, l);
                let capturing = self.capture.is_some_and(|r| matches!(self.page_item(Page::Controls, r), Item::Bind { cmd: c2, .. } if c2 == cmd));
                self.draw_binding_columns(c, cmd, y - 1.0,
                    [(VALUE_X, BIND_PAD_X - 4.0), (BIND_PAD_X, PANEL_X + PANEL_W - 2.0)], capturing, blink);
            }
            Item::Sub { label: l, page } => {
                // submenu rows: tinted strip, gold label and arrow
                let dim = sub_disabled(page, cv);
                let (fg, tint) = if dim { (MUTED, 0.04) } else { (GOLD_FG, 0.10) };
                c.fill(PANEL_X + 4.0, y - 2.0, PANEL_W - 14.0, ROW_H, [0.85, 0.60, 0.25, tint]);
                c.text_right(LABEL_R, y, l, TXT, fg);
                c.text(VALUE_X, y, if dim { "(off)" } else { "" }, TXT, MUTED);
                c.shape(VALUE_X + if dim { 30.0 } else { 0.0 }, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowRight, fg);
            }
            Item::ResetBinds => {
                c.text_center(320.0, y, "Reset to Defaults", TXT, GOLD_FG);
            }
            Item::ResetShaderParams => {
                // live only while some parameter differs from the preset
                let dirty = cv.by_name("r_shader_params").is_some_and(|c| !c.string.trim().is_empty());
                c.text_center(320.0, y, "Reset Shader Settings", TXT, if dirty { GOLD_FG } else { MUTED });
            }
            Item::Apply => {
                // a button, live only while there are pending changes
                let col = if self.display_dirty(cv) { GOLD_FG } else { MUTED };
                c.text_center(320.0, y, "Apply Display Changes", TXT, col);
            }
        }
    }
}
