use super::*;

impl Menu {
    pub fn draw(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        if self.screen == Screen::None {
            return;
        }
        sync_addon_binds(cv);
        c.fade_screen();
        let base_scale = c.scale;
        c.menu_layout();
        if !matches!(self.screen, Screen::Options | Screen::OptionsPage(_) | Screen::Death | Screen::Quit | Screen::Mods | Screen::ModMaps | Screen::Load | Screen::Save) {
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
                for (k, label) in ["Map Packs", "Add-ons"].into_iter().enumerate() {
                    let (y, w) = (32.0 + (5 + k) as f32 * 20.0 + 2.0, 112.0);
                    c.fill(72.0, y, w, 16.0, [0.0, 0.0, 0.0, 0.55]);
                    c.frame(72.0, y, w, 16.0, BORDER);
                    gold(c, 72.0 + (w - 8.0 * label.len() as f32) / 2.0, y + 4.0, label);
                }
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.main_cursor as f32 * 20.0, d);
            }
            Screen::Mods => self.mods.draw(c, modmenu::Page::List),
            Screen::ModMaps => self.mods.draw(c, modmenu::Page::Maps),
            Screen::SinglePlayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_sgl.lmp");
                let p = pic("gfx/sp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.sp_cursor as f32 * 20.0, d);
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
                panel(c, ROW0, PAGES.len());
                highlight(c, ROW0 + self.page_cursor as f32 * ROW_H);
                for (i, (label, _)) in PAGES.iter().enumerate() {
                    let y = ROW0 + i as f32 * ROW_H + 2.0;
                    c.text_center(320.0, y, label, TXT, FG);
                }
            }
            Screen::OptionsPage(page) => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, if is_net_page(page) { "gfx/p_multi.lmp" } else { "gfx/p_option.lmp" });
                let name = page_name(page);
                if page == Page::Controls {
                    c.text_right(LABEL_R, PAGE_ROW0 - 16.0, name, TXT, GOLD_FG);
                } else {
                    c.text_center(320.0, PAGE_ROW0 - 16.0, name, TXT, GOLD_FG);
                }
                let total = self.page_len(page);
                let n = self.page_vis(page);
                let sc = self.page_scroll(page);
                panel(c, PAGE_ROW0, n);
                if self.opt_cursor < total {
                    highlight(c, self.page_row_y(page, self.opt_cursor));
                }
                for i in sc..sc + n {
                    let blink = (realtime * 4.0) as i64 & 1 == 0 && i == self.opt_cursor;
                    self.draw_item(c, cv, self.page_item(page, i), self.page_row_y(page, i) + 2.0, blink);
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
                if let Some(o) = self.osk {
                    self.draw_osk(c, o);
                }
            }
            Screen::Quit => {
                let (bx, by, bw, bh) = QUIT_BOX;
                c.fill(bx, by, bw, bh, [0.05, 0.04, 0.03, 0.92]);
                c.frame(bx, by, bw, bh, BORDER);
                let msg = "Really quit?";
                c.text_center(320.0, by + 12.0, msg, TXT * 1.2, FG);
                for (i, (&x, label)) in QUIT_BUTTONS.iter().zip(["(Y)es", "(N)o"]).enumerate() {
                    let (w, h) = QUIT_BTN;
                    if i == self.quit_cursor {
                        c.fill(x, QUIT_BTN_Y, w, h, HILITE);
                    }
                    c.frame(x, QUIT_BTN_Y, w, h, BORDER);
                    let tx = x + ((w - c.text_w(label, TXT)) / 2.0).floor();
                    // gold hotkey letter
                    let ty = QUIT_BTN_Y + 3.0;
                    let mut cx = tx + txt(c, tx, ty, "(");
                    cx += txt_gold(c, cx, ty, &label[1..2]);
                    txt(c, cx, ty, &label[2..]);
                }
            }
            Screen::Death => {
                c.fill(148.0, 96.0, 344.0, 152.0, [0.05, 0.04, 0.03, 0.95]);
                c.frame(148.0, 96.0, 344.0, 152.0, BORDER);
                c.text_center(320.0, 112.0, "You died", TXT * 1.2, GOLD_FG);
                for (i, label) in ["1. Continue from most recent save", "2. Select save", "3. Restart level", "4. Return to main menu"].iter().enumerate() {
                    let y = 140.0 + i as f32 * 24.0;
                    if i == self.death_cursor { c.fill(160.0, y, 320.0, 20.0, HILITE); }
                    txt(c, 168.0, y + 4.0, label);
                }
            }
            Screen::None => {}
        }
        c.menu_layout();
        self.draw_tooltip(c);
        self.draw_hints(c);
        self.draw_back(c);
        c.reset_origin();
        c.scale = base_scale;
    }

    /// Save/load cards: one column, scrolled so the cursor is in view, with
    /// the next card cut off at the bottom to show the list goes on.
    pub(super) fn draw_cards(&self, c: &mut Canvas, pic: &mut PicFn) {
        let heading = if self.screen == Screen::Save { "Save Game" } else if self.autosaves { "Autosaves" } else { "Load Game" };
        c.text(CARD_X, 20.0, heading, 13.0, GOLD_FG);
        if self.autosaves {
            c.text(CARD_X + c.text_w(heading, 13.0) + 10.0, 23.0, "newest first", TXT, MUTED);
        }
        let n = self.save_rows();
        c.text_right(CARD_X + CARD_W, 23.0, &format!("{} / {}", self.load_cursor + 1, n), TXT, MUTED);
        let small = 8.5;
        let sticky = usize::from(self.screen == Screen::Load);
        if sticky > 0 {
            // divider between the pinned newest save and the scrolling rest
            let y = CARD_Y0 + CARD_H + 3.0;
            c.fill(CARD_X, y, CARD_W, 1.0, [0.55, 0.40, 0.20, 0.8]);
        }
        for i in 0..n {
            if !self.card_visible(i) {
                continue;
            }
            let y = self.card_y(i);
            c.clip_rows(Some(if i < sticky { (CARD_Y0 - 1.0, CARD_Y0 + CARD_H + 1.0) } else { (self.card_top(), CARD_BOTTOM) }));
            let x = CARD_X;
            let selected = i == self.load_cursor;
            let bg = if selected { [0.30, 0.20, 0.09, 0.97] } else { [0.08, 0.07, 0.06, 0.94] };
            let edge = if selected { [0.85, 0.59, 0.22, 0.9] } else { [0.35, 0.27, 0.16, 0.7] };
            c.round_box(x, y, CARD_W, CARD_H, 4.0, bg, Some(edge));
            if selected {
                c.round_box(x + 3.0, y + 6.0, 2.5, CARD_H - 12.0, 1.25, [0.95, 0.66, 0.26, 1.0], None);
            }
            let (lx, rx) = (x + 14.0, x + CARD_W - 12.0);
            let clean = |t: &str| -> String { t.chars().map(|ch| if ch.is_control() { '?' } else { ch }).collect() };
            if self.screen == Screen::Load && !self.autosaves && i == 1 {
                c.text(lx, y + 9.0, "Browse Autosaves", 11.0, FG);
                c.text(lx, y + 27.0, "Every retained autosave, newest first", small, MUTED);
                c.shape(rx - 10.0, y + CARD_H / 2.0 - 5.0, 10.0, 10.0, crate::font::Shape::ArrowRight, GOLD_FG);
                continue;
            }
            let Some(sv) = &self.saves[i] else {
                let label = if self.screen == Screen::Load && self.autosaves { "No autosaves yet".into() }
                    else if self.screen == Screen::Load && i == 0 { "No autosave yet".into() }
                    else { format!("Empty slot {}", if self.screen == Screen::Load { i - 1 } else { i + 1 }) };
                c.text(lx, y + 9.0, &label, 11.0, MUTED);
                if self.screen == Screen::Save {
                    c.text(lx, y + 27.0, "Select to save here", small, MUTED);
                }
                continue;
            };
            let label = if self.autosaves { format!("Autosave {}: {}", i + 1, sv.reason.as_deref().unwrap_or("unknown trigger")) }
                else if self.screen == Screen::Load && i == 0 { format!("Latest autosave: {}", sv.reason.as_deref().unwrap_or("unknown")) }
                else { format!("Slot {}", if self.screen == Screen::Load { i - 1 } else { i + 1 }) };
            c.text_right(rx, y + 7.0, &clean(&label), small, GOLD_FG);
            // title, cut to the space left of the label
            let room = CARD_W - 40.0 - c.text_w(&label, small) - 20.0;
            let mut title = clean(&sv.level);
            while c.text_w(&title, 11.0) > room && title.pop().is_some() {}
            if title.len() < sv.level.len() {
                title.push('…');
            }
            c.text(lx, y + 5.0, &title, 11.0, FG);
            c.text(lx, y + 20.0, &sv.timestamp, small, MUTED);
            c.text(lx, y + 32.0, &format!("Health {}   Armor {}", sv.health, sv.armor), small, FG);
            let stats = format!("Kills {}/{}   Secrets {}/{}", sv.kills.0, sv.kills.1, sv.secrets.0, sv.secrets.1);
            let sw = c.text(lx, y + 44.0, &stats, small, FG);
            // keys after the stats line, weapons in the right half
            let keys = self.save_inventory.keys(sv);
            let mut kx = lx + sw + 12.0;
            for icon in &keys {
                if pic(&icon.name).is_some() {
                    draw_inventory_icon(c, pic, kx, y + 42.0, 24.0, 12.0, icon);
                    kx += 26.0;
                } else {
                    kx += c.text(kx, y + 44.0, &icon.fallback, small, GOLD_FG) + 6.0;
                }
            }
            let icons = self.save_inventory.weapons(sv);
            let wx = x + 250.0;
            if icons.is_empty() {
                c.text(wx, y + 26.0, "No weapons", small, MUTED);
            }
            let columns = if icons.len() <= 20 { 10 } else { 14 };
            let cell = if columns == 10 { 22.0 } else { 16.0 };
            let mut missing = Vec::new();
            for (j, icon) in icons.iter().take(columns * 2).enumerate() {
                if pic(&icon.name).is_none() {
                    missing.push(icon.fallback.as_str());
                    continue;
                }
                draw_inventory_icon(c, pic, wx + (j % columns) as f32 * cell, y + 22.0 + (j / columns) as f32 * 15.0, cell - 2.0, 13.0, icon);
            }
            if !missing.is_empty() {
                c.text(wx, y + 44.0, &missing.join(", "), small, MUTED);
            }
        }
        c.clip = None;
        if let Some((x, top, h, ty, th)) = self.card_scrollbar() {
            c.fill(x, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, if self.drag.is_some() { 1.0 } else { 0.9 }]);
        }
    }

    /// Bottom row of button prompts, for the device in use.
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
        if self.screen == Screen::Death { return; }
        let pad = self.pad_ui;
        let (sel, back) = if pad { (K_ABUTTON, K_BBUTTON) } else { (K_ENTER, K_ESCAPE) };
        let y = 344.0;
        if self.capturing() {
            let s = if pad { "Press a button or key to bind" } else { "Press a key or button to bind" };
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
            items.insert(1, (if pad { K_XBUTTON } else { K_BACKSPACE }, "Clear"));
        }
        if matches!(self.screen, Screen::Quit) {
            items = vec![(sel, "Confirm"), (back, "Cancel")];
        }
        self.boxed_hints(c, y, &items);
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
                label(c, l);
                let on = cv.value_of(cvar) != 0.0;
                c.round_box(VALUE_X, y - 0.5, 9.0, 9.0, 1.5, [0.0, 0.0, 0.0, 0.5], Some(BORDER));
                if on {
                    c.round_box(VALUE_X + 2.0, y + 1.5, 5.0, 5.0, 1.0, GOLD_FG, None);
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
                    "desktop".to_string()
                } else {
                    format!("{}x{}", self.pending_res.0, self.pending_res.1)
                };
                { txt(c, VALUE_X, y, &s); }
            }
            Item::Text { label: l, cvar, max } => {
                label(c, l);
                let v = cv.by_name(cvar).map_or("", |c| c.string.as_str());
                // fits inside the panel: 20 visible characters at most
                let w = (max.min(20) as f32 * 7.0 + 14.0).min(PANEL_X + PANEL_W - VALUE_X - 4.0);
                c.fill(VALUE_X - 3.0, y - 1.0, w, BOX_H, [0.0, 0.0, 0.0, 0.5]);
                let shown: String = v.chars().skip(v.len().saturating_sub(20)).collect();
                let tw = txt(c, VALUE_X, y, &shown);
                if blink {
                    c.fill(VALUE_X + tw + 1.0, y, 1.0, 8.0, FG);
                }
            }
            Item::Color { label: l, high } => {
                label(c, l);
                let v = cv.value_of("_cl_color") as i32;
                let k = if high { v >> 4 } else { v & 15 }.clamp(0, 13) as usize;
                let [r, g, b] = crate::draw::PLAYER_COLORS[k].map(|x| x as f32 / 255.0);
                c.fill(VALUE_X, y - 1.0, 40.0, BOX_H - 1.0, [r, g, b, 1.0]);
                c.frame(VALUE_X, y - 1.0, 40.0, BOX_H - 1.0, BORDER);
                { txt(c, VALUE_X + 48.0, y, &k.to_string()); }
            }
            Item::ShaderPreset => {
                label(c, "Preset");
                let s: String = self.preset_options()[self.preset_index(cv)].chars().take(13).collect();
                { txt(c, VALUE_X, y, &s); }
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
                let l = if run == Run::Search && self.searching { "Searching..." } else { l };
                c.text_center(320.0, y, l, TXT, GOLD_FG);
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
                let keys = keys_for(&self.bindings, cmd);
                let capturing = self.capture.is_some_and(|r| matches!(page_items(Page::Controls).get(r), Some(Item::Bind { cmd: c2, .. }) if *c2 == cmd));
                if capturing {
                    if blink {
                        txt_gold(c, VALUE_X, y, "press a key...");
                    }
                    return;
                }
                let g = self.glyphs;
                let (mut x, mut px) = (VALUE_X, BIND_PAD_X);
                for &k in &keys {
                    if is_pad_key(k) {
                        if px + crate::glyphs::width(c, k, g.family, crate::glyphs::H) < PANEL_X + PANEL_W - 2.0 {
                            px += g.key(c, px, y - 1.0, k) + 3.0;
                        }
                    } else if x + crate::glyphs::width(c, k, g.family, crate::glyphs::H) < BIND_PAD_X - 4.0 {
                        x += g.key(c, x, y - 1.0, k) + 3.0;
                    }
                }
                if x == VALUE_X {
                    { txt(c, VALUE_X, y, "---"); }
                }
                if px == BIND_PAD_X {
                    { txt(c, BIND_PAD_X, y, "---"); }
                }
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
