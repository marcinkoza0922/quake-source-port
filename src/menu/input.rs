use super::*;

impl Menu {
    /// Mouse hover: moves the cursor onto the item under the pointer.
    /// Returns a sound to play when the selection changed.
    pub fn mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) -> Option<&'static str> {
        self.sync_autoaim_mapping(cv);
        self.mouse = Some(pos);
        if self.screen == Screen::TouchEditor {
            self.move_touch_button(pos, cv);
            return None;
        }
        if self.screen == Screen::HudEditor {
            self.hud_edit.mouse_move(pos, cv);
            return None;
        }
        if self.autoaim_prompt || self.capturing() { return None; }
        if matches!(self.prompt, Some(Prompt::Launch { .. })) { return self.launch_hover(pos, cv); }
        if matches!(self.prompt, Some(Prompt::Preset { .. })) { return self.preset_hover(pos); }
        if self.prompt.is_some() { return self.prompt_hover(pos); }
        if self.addon_mouse(pos) { return None; }
        if self.screen == Screen::OptionsPage(Page::WeaponPriority) {
            return self.wp_mouse_move(pos, cv);
        }
        if let (Some(i), Screen::OptionsPage(p)) = (self.slider_drag, self.screen) {
            self.set_slider(self.page_item(p, i), pos.0, cv);
            return None;
        }
        if let Some(row) = self.text_drag {
            self.text_drag_to(row, pos.0, cv);
            return None;
        }
        if let (Some(g), Screen::OptionsPage(p)) = (self.drag, self.screen) {
            self.drag_to(p, pos.1, g);
            return None;
        }
        if let (Some(g), Screen::Mods | Screen::ModMaps | Screen::ModOnline) = (self.drag, self.screen) {
            self.mods.drag_to(self.mods_page(), pos.1, g);
            return None;
        }
        if let (Some(g), Screen::Load | Screen::Save) = (self.drag, self.screen) {
            self.card_drag_to(pos.1, g);
            return None;
        }
        if let Some(d) = self.drop {
            let i = self.drop_at(pos, d)?;
            let dc = &mut self.drop.as_mut()?.cursor;
            return (*dc != i).then(|| {
                *dc = i;
                "misc/menu1.wav"
            });
        }
        if let (Some(g), Screen::Demos) = (self.drag, self.screen) {
            self.demos.drag_to(pos.1, g);
            return None;
        }
        if let (Some(g), Screen::DirBrowser) = (self.drag, self.screen) {
            self.dir_browser.drag_to(pos.1, g);
            return None;
        }
        if self.screen == Screen::ServerBrowser {
            return self.server_browser_hover(pos);
        }
        if self.screen == Screen::DirBrowser {
            return self.dir_browser.mouse_move(pos);
        }
        // the Demos list selects on click; hovering only lights the row
        if self.screen == Screen::Demos {
            return None;
        }
        let i = self.item_at(pos)?;
        let c = self.cursor_mut()?;
        (*c != i).then(|| {
            *c = i;
            "misc/menu1.wav"
        })
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let out = self.key_event(key, cv, server_active);
        self.settle_cursor();
        out
    }

    fn key_event(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        if key != K_MOUSE1 && self.drop.is_none() {
            self.touch_scroll = None;
        }
        self.game_running = server_active;
        self.custom_running = server_active && cv.value_of("custom_active") != 0.0;
        sync_addon_binds(cv, server_active);
        self.sync_autoaim_mapping(cv);
        let mut out = Vec::new();
        if self.binding_key(key, &mut out) { return out; }
        if self.addon_key(key, cv, &mut out) { return out; }
        if self.screen == Screen::OptionsPage(Page::WeaponPriority) {
            self.wp_key(key, cv, &mut out);
            return out;
        }
        if self.autoaim_prompt {
            self.autoaim_prompt_key(key, cv, &mut out);
            return out;
        }
        if matches!(self.prompt, Some(Prompt::Launch { .. })) {
            self.launch_key(key, cv, &mut out);
            return out;
        }
        if matches!(self.prompt, Some(Prompt::Preset { .. })) {
            self.preset_key(key, &mut out);
            return out;
        }
        if self.prompt.is_some() {
            self.prompt_key(key, &mut out);
            return out;
        }
        if self.screen == Screen::TouchEditor {
            self.touch_editor_key(key, cv);
            return out;
        }
        if self.screen == Screen::HudEditor {
            if let Some(o) = self.osk {
                self.osk_key(key, o, cv, &mut out);
                return out;
            }
            if key == K_YBUTTON && self.hud_edit.saving_preset() {
                self.osk = Some(Osk::default());
                out.push(Action::Sound("misc/menu2.wav"));
                return out;
            }
            let r = self.hud_edit.key(key, cv);
            out.extend(r.sound.map(Action::Sound));
            if r.exit {
                self.open(Screen::OptionsPage(Page::Hud));
                // back on the Edit HUD row
                self.opt_cursor = HUD.iter().position(|i| matches!(i, Item::Run { run: Run::EditHud, .. })).unwrap_or(0);
            }
            return out;
        }
        if let Some(row) = self.capture.take() {
            if self.screen != Screen::OptionsPage(Page::Controls) || key == K_ESCAPE {
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
            if let Item::Bind { cmd, .. } = self.page_item(Page::Controls, row) {
                self.request_binding(key, cmd, &mut out);
            }
            return out;
        }
        if let Some(o) = self.osk {
            self.osk_key(key, o, cv, &mut out);
            return out;
        }
        if self.screen == Screen::ServerBrowser {
            self.server_browser_key(key, cv, &mut out);
            return out;
        }
        if let (Some(d), Screen::OptionsPage(p)) = (self.drop, self.screen) {
            let item = self.page_item(p, d.item);
            self.drop_key(key, d, item, cv, &mut out);
            return out;
        }
        if self.screen == Screen::DirBrowser {
            if matches!(key, K_MWHEELUP | K_MWHEELDOWN) {
                self.dir_browser.wheel(key == K_MWHEELUP);
                return out;
            }
            if key == K_MOUSE1
                && !self.dir_browser.has_modal()
                && let Some(pos) = self.mouse
                && let Some((x, top, h, ty, th)) = self.dir_browser.scrollbar()
                && (x - 4.0..x + 7.0).contains(&pos.0)
                && (top..top + h).contains(&pos.1)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                self.dir_browser.drag_to(pos.1, grab);
                self.drag = Some(grab);
                return out;
            }
            match self.dir_browser.key(key, self.mouse, &mut out) {
                dir_browser::DirOutcome::None => {}
                dir_browser::DirOutcome::Cancel => {
                    self.screen = Screen::OptionsPage(Page::Gameplay);
                    self.opt_cursor = GAMEPLAY.iter().position(|i| matches!(i, Item::UserDataDir)).unwrap_or(0);
                }
                dir_browser::DirOutcome::Apply { parent, copy } => {
                    self.pending_user_data_dir = Some((parent, copy));
                    self.screen = Screen::OptionsPage(Page::Gameplay);
                    self.opt_cursor = GAMEPLAY.iter().position(|i| matches!(i, Item::UserDataDir)).unwrap_or(0);
                    out.push(Action::Command("_apply_user_data_dir\n".into()));
                }
            }
            return out;
        }
        if self.screen == Screen::Help {
            if self.help.key(key, self.mouse, &mut out) {
                self.screen = Screen::Main;
                out.push(Action::Sound("misc/menu3.wav"));
            }
            return out;
        }
        // the wheel scrolls the card list itself; the pointer then picks the card under it
        if let (K_MWHEELUP | K_MWHEELDOWN, Screen::Load | Screen::Save) = (key, self.screen) {
            self.scroll_cards(if key == K_MWHEELUP { -CARD_WHEEL } else { CARD_WHEEL });
            if let Some(i) = self.mouse.and_then(|pos| self.item_at(pos)) {
                self.load_cursor = i;
            }
            return out;
        }
        if let (K_MWHEELUP | K_MWHEELDOWN, Screen::Mods | Screen::ModMaps | Screen::ModOnline) = (key, self.screen) {
            self.mods.wheel(self.mods_page(), key == K_MWHEELUP);
            if let Some(i) = self.mouse.and_then(|pos| self.item_at(pos)) {
                *self.mods.cursor_mut(self.mods_page()) = i;
            }
            return out;
        }
        // left click acts on the item under the pointer, never on a stale cursor
        if let (K_MWHEELUP | K_MWHEELDOWN, Screen::OptionsPage(p)) = (key, self.screen) {
            let sc = self.page_scroll(p);
            self.set_page_scroll(p, if key == K_MWHEELUP { sc.saturating_sub(2) } else { sc + 2 });
            if let Some(i) = self.mouse.and_then(|pos| self.item_at(pos)) {
                self.opt_cursor = i;
            }
            return out;
        }
        // the wheel scrolls the demo list without moving the selection
        if let (K_MWHEELUP | K_MWHEELDOWN, Screen::Demos) = (key, self.screen) {
            self.demos.wheel(key == K_MWHEELUP);
            return out;
        }
        if key == K_MOUSE1 && self.screen == Screen::Demos {
            return self.demos_click(cv, server_active);
        }
        if key == K_MOUSE1 {
            let Some(pos) = self.mouse else { return out };
            if let Some((bx, by, bw, bh)) = self.back_button()
                && (bx..bx + bw).contains(&pos.0)
                && (by..by + bh).contains(&pos.1)
            {
                return self.key(K_ESCAPE, cv, server_active);
            }
            if let Screen::OptionsPage(p) = self.screen
                && let Some((x, top, h, ty, th)) = self.scrollbar(p)
                && (x - 3.0..x + SCROLL_W + 4.0).contains(&pos.0)
                && (top..top + h).contains(&pos.1)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                self.drag_to(p, pos.1, grab);
                self.drag = Some(grab);
                return out;
            }
            if let Screen::Mods | Screen::ModMaps | Screen::ModOnline = self.screen
                && self.mods.click(self.mods_page(), pos, &mut out)
            {
                return out;
            }
            if let Screen::Mods | Screen::ModMaps | Screen::ModOnline = self.screen
                && let Some((x, top, h, ty, th)) = self.mods.scrollbar(self.mods_page())
                && (x - 4.0..x + 7.0).contains(&pos.0)
                && (top..top + h).contains(&pos.1)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                self.mods.drag_to(self.mods_page(), pos.1, grab);
                self.drag = Some(grab);
                return out;
            }
            if let Screen::Load | Screen::Save = self.screen
                && let Some((x, top, h, ty, th)) = self.card_scrollbar()
                && (x - 4.0..x + 7.0).contains(&pos.0)
                && (top..top + h).contains(&pos.1)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                self.card_drag_to(pos.1, grab);
                self.drag = Some(grab);
                return out;
            }
            if let Screen::Load | Screen::Save = self.screen
                && self.on_delete_button(pos)
                && let Some(sv) = self.save_at(self.load_cursor)
            {
                self.ask_delete(sv.clone());
                return out;
            }
            let Some(i) = self.item_at(pos) else { return out };
            if let Some(c) = self.cursor_mut() {
                *c = i;
            }
            // the changed-value marker left of a label resets the row
            if let Screen::OptionsPage(p) = self.screen
                && p != Page::Touch
                && pos.0 < LABEL_X - 2.0
                && self.changed(self.page_item(p, i), cv)
            {
                if self.reset_row(self.page_item(p, i), cv, &mut out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
                return out;
            }
            if let Screen::OptionsPage(p) = self.screen && let Some(row) = self.slider_at(pos, cv) {
                self.slider_drag = Some(row);
                self.set_slider(self.page_item(p, row), pos.0, cv);
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
            if self.screen == Screen::OptionsPage(Page::Touch) && self.pick_touch_choice(i, pos, cv) {
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
            // a click edits a text row (caret, drag to select); never submits it
            if let Screen::OptionsPage(p) = self.screen
                && matches!(self.page_item(p, i), Item::Text { .. })
            {
                self.text_click(i, pos.0, cv);
                return out;
            }
        }
        let key = match key {
            K_MWHEELUP => K_UPARROW,
            K_MWHEELDOWN => K_DOWNARROW,
            k => k,
        };
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1);
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);
        let wrap = |c: &mut usize, n: usize, out: &mut Vec<Action>| {
            if up {
                *c = (*c + n - 1) % n;
                out.push(Action::Sound("misc/menu1.wav"));
            } else if down {
                *c = (*c + 1) % n;
                out.push(Action::Sound("misc/menu1.wav"));
            }
        };
        match self.screen {
            Screen::Death => {
                let rows = self.death_rows();
                self.death_cursor = self.death_cursor.min(rows - 1);
                wrap(&mut self.death_cursor, rows, &mut out);
                let shortcut = (b'1' as i32..b'1' as i32 + rows as i32).contains(&key).then(|| (key - b'1' as i32) as usize);
                if enter || shortcut.is_some() {
                    let choice = shortcut.unwrap_or(self.death_cursor);
                    out.push(Action::Sound("misc/menu2.wav"));
                    out.push(Action::Command(format!("_death_choice {}\n", choice + 1)));
                }
            }
            Screen::None | Screen::HudEditor | Screen::TouchEditor | Screen::DirBrowser | Screen::ServerBrowser => {}
            Screen::Help => {}
            Screen::Main => {
                if back {
                    self.screen = Screen::None;
                    out.push(Action::Close);
                    out.push(Action::Sound("misc/menu3.wav"));
                    return out;
                }
                let n = self.main_items();
                self.main_cursor = self.main_cursor.min(n - 1);
                wrap(&mut self.main_cursor, n, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.main_cursor {
                        0 => self.screen = Screen::SinglePlayer,
                        1 => self.screen = Screen::Multiplayer,
                        2 => self.open(Screen::Options),
                        3 => self.open(Screen::Help),
                        4 => self.open(Screen::Quit),
                        5 => {
                            out.push(Action::Command("_mods\n".into()));
                            self.screen = Screen::Mods;
                        }
                        6 => out.push(Action::Command("menu_demos\n".into())),
                        i if Some(i) == self.touch_shortcut_index() => self.open_touch_from_main(),
                        7 => self.open_customize(false),
                        _ => {}
                    }
                }
            }
            Screen::SinglePlayer => {
                if back {
                    self.new_game_setup = false;
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.sp_cursor, customize::SP_ITEMS, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.sp_cursor {
                        0 => self.open_launch(launch::NEW_GAME_CMD.into(), false, None),
                        1 => {
                            out.push(Action::Command("_scansaves\n".into()));
                            self.autosaves = false;
                            self.open(Screen::Load);
                        }
                        3 => out.push(Action::Command("menu_levels\n".into())),
                        _ if server_active && cv.value_of("custom_active") != 0.0 && cv.value_of("modifier_nosaves") != 0.0 => {
                            let lines = vec![qcore::tr!("This game only autosaves."), qcore::tr!("(No Manual Saves modifier)")];
                            self.confirm(lines, qcore::t!("OK"), String::new(), false);
                        }
                        _ => {
                            if server_active {
                                self.autosaves = false;
                                out.push(Action::Command("_scansaves\n".into()));
                                self.open(Screen::Save);
                            }
                        }
                    }
                }
            }
            Screen::Multiplayer => {
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.mp_cursor, 3, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    let p = [Page::Join, Page::Host, Page::Setup][self.mp_cursor];
                    if p == Page::Host {
                        out.push(Action::Command("_netmaps\n".into()));
                    }
                    self.opt_cursor = 0;
                    self.screen = Screen::OptionsPage(p);
                }
            }
            Screen::Load | Screen::Save => {
                if back {
                    if self.autosaves && self.screen == Screen::Load {
                        self.autosaves = false;
                        self.load_cursor = 1;
                        self.save_scroll = 0.0;
                        out.push(Action::Command("_scansaves\n".into()));
                    } else {
                        self.screen = Screen::SinglePlayer;
                    }
                    return out;
                }
                let count = self.save_rows();
                self.load_cursor = self.load_cursor.min(count - 1);
                if up { self.load_cursor = (self.load_cursor + count - 1) % count; }
                if down { self.load_cursor = (self.load_cursor + 1) % count; }
                // left/right page by the visible cards
                if left { self.load_cursor = self.load_cursor.saturating_sub(CARDS_FULL); }
                if right { self.load_cursor = (self.load_cursor + CARDS_FULL).min(count - 1); }
                self.fix_card_scroll();
                if up || down || left || right { out.push(Action::Sound("misc/menu1.wav")); }
                let delete = matches!(key, K_DEL | K_BACKSPACE | K_XBUTTON);
                if delete && let Some(sv) = self.save_at(self.load_cursor).filter(|s| s.source == SaveSource::Own) {
                    self.ask_delete(sv.clone());
                    return out;
                }
                if enter {
                    let i = self.load_cursor;
                    if self.is_browse_row(i) {
                        self.autosaves = true;
                        self.load_cursor = 0;
                        self.save_scroll = 0.0;
                        out.push(Action::Sound("misc/menu2.wav"));
                        out.push(Action::Command("_scansaves\n".into()));
                    } else if self.screen == Screen::Load {
                        if let Some(sv) = self.save_at(i) {
                            let name = sv.name.clone();
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command(format!("load \"{name}\"\n")));
                        }
                    } else if let Some(sv) = self.save_at(i) {
                        let title = qcore::tr!("Overwrite \"{}\"?", clean_title(&sv.info.level, 40));
                        let when = qcore::tr!("Saved {}", sv.info.when(qcore::time::SystemTime::now()));
                        self.confirm(vec![title, when], qcore::t!("Overwrite"), format!("save \"{}\"\n", sv.name), true);
                    } else {
                        self.screen = Screen::None;
                        out.push(Action::Close);
                        out.push(Action::Command(format!("save {}\n", self.new_save_name())));
                    }
                }
            }
            Screen::Options => {
                if self.search_key(key, cv, &mut out) {
                    return out;
                }
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.page_cursor, PAGES.len(), &mut out);
                if enter || right {
                    out.push(Action::Sound("misc/menu2.wav"));
                    let p = PAGES[self.page_cursor].1;
                    if p == Page::Graphics {
                        out.push(Action::Command("_vid_menu\n".into()));
                    }
                    self.opt_cursor = 0;
                    self.screen = Screen::OptionsPage(p);
                }
            }
            Screen::OptionsPage(p) => {
                if back && !self.autoaim_conflicts.is_empty() && cv.value_of("assist_autoaim") != 0.0 {
                    self.autoaim_prompt = true;
                    self.autoaim_yes = false;
                    return out;
                }
                if back {
                    if p == Page::Setup {
                        out.push(Action::Command(Self::setup_commands(cv)));
                    }
                    self.screen = if p == Page::RenameDemo {
                        Screen::Demos
                    } else if p == Page::Export {
                        if self.export_back == Screen::None {
                            out.push(Action::Close);
                            out.push(Action::Command("_export_cancel\n".into()));
                        }
                        self.export_back
                    } else if is_net_page(p) {
                        Screen::Multiplayer
                    } else if p == Page::NewGameCustomize {
                        self.back_to_launch();
                        return out;
                    } else if p == Page::Customize {
                        self.main_cursor = MAIN_ITEMS;
                        Screen::Main
                    } else if matches!(p, Page::NewGameAddons | Page::Addons) {
                        self.opt_cursor = 1;
                        Screen::OptionsPage(if p == Page::Addons { Page::Customize } else { Page::NewGameCustomize })
                    } else if p == Page::Touch && std::mem::take(&mut self.touch_from_main) {
                        Screen::Main
                    } else if let Some((parent, row)) =
                        self.sub_return.take().filter(|r| r.1 == p).map(|r| (r.0, r.2)).or_else(|| parent_page(p))
                    {
                        self.opt_cursor = row;
                        Screen::OptionsPage(parent)
                    } else {
                        Screen::Options
                    };
                    return out;
                }
                let n = self.page_len(p);
                self.opt_cursor = self.opt_cursor.min(n - 1);
                wrap(&mut self.opt_cursor, n, &mut out);
                self.skip_headings(p, if up { -1 } else { 1 });
                let item = self.page_item(p, self.opt_cursor);
                match item {
                    Item::Text { cvar, .. } => {
                        if key == K_ABUTTON {
                            self.osk = Some(Osk::default());
                            out.push(Action::Sound("misc/menu2.wav"));
                        } else if matches!(key, K_BACKSPACE | K_DEL) {
                            self.text_delete(key == K_DEL, cv);
                        } else if self.text_nav(key, cv) {
                        } else if matches!(key, K_ENTER | K_KP_ENTER) && cvar == "_mp_address" {
                            self.run(Run::Connect, cv, &mut out);
                        } else if matches!(key, K_ENTER | K_KP_ENTER) && cvar == "_demo_rename" {
                            self.run(Run::RenameDemo, cv, &mut out);
                        }
                        return out;
                    }
                    Item::Run { run, .. } if enter => {
                        self.run(run, cv, &mut out);
                        return out;
                    }
                    Item::Run { .. } => return out,
                    Item::Bind { cmd, .. } => {
                        if enter {
                            self.capture = Some(self.opt_cursor);
                            out.push(Action::Sound("misc/menu2.wav"));
                        } else if matches!(key, K_BACKSPACE | K_DEL | K_XBUTTON) {
                            // clear this device kind's keys for the action
                            let pad = key == K_XBUTTON;
                            for k in keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == pad) {
                                out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
                                self.bindings[k as usize] = None;
                            }
                            out.push(Action::Sound("misc/menu3.wav"));
                        }
                        return out;
                    }
                    Item::Sub { page, .. } => {
                        if (enter || right) && sub_disabled(page, cv) {
                            out.push(Action::Sound("misc/menu3.wav"));
                        } else if enter || right {
                            self.sub_return = Some((p, page, self.opt_cursor));
                            self.opt_cursor = 0;
                            if page == Page::WeaponPriority {
                                self.wp.reset_state();
                            }
                            self.screen = Screen::OptionsPage(page);
                            out.push(Action::Sound("misc/menu2.wav"));
                        }
                        return out;
                    }
                    Item::ResetShaderParams => {
                        if enter {
                            cv.set("r_shader_params", "");
                            out.push(Action::Sound("misc/menu2.wav"));
                        }
                        return out;
                    }
                    Item::ResetBinds => {
                        if enter {
                            out.push(Action::Command("_resetbinds\n".into()));
                            out.push(Action::Sound("misc/menu2.wav"));
                        }
                        return out;
                    }
                    Item::UserDataDir => {
                        if enter && !cfg!(target_arch = "wasm32") {
                            self.open_dir_browser();
                            out.push(Action::Sound("misc/menu2.wav"));
                        }
                        return out;
                    }
                    _ => {}
                }
                if matches!(key, K_BACKSPACE | K_DEL | K_XBUTTON) {
                    if self.reset_row(item, cv, &mut out) {
                        out.push(Action::Sound("misc/menu3.wav"));
                    }
                    return out;
                }
                if p != Page::Touch && enter && self.options(item).len() > 2 && !self.item_disabled(item, cv) {
                    let cur = self.option_index(item, cv);
                    let mut d = Drop { item: self.opt_cursor, cursor: cur, scroll: 0 };
                    clamp_scroll(&mut d);
                    self.drop = Some(d);
                    out.push(Action::Sound("misc/menu2.wav"));
                    return out;
                }
                let dir = if left { -1 } else if right || enter { 1 } else { 0 };
                if dir != 0 && self.adjust(item, dir, enter, cv, &mut out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Screen::ModMaps if back && self.prev == Screen::SinglePlayer && !self.mods.has_filter(modmenu::Page::Maps) => {
                // Level Select opened from New Game: back returns there
                self.screen = Screen::SinglePlayer;
            }
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => {
                if key == K_YBUTTON {
                    self.osk = Some(Osk::default());
                    out.push(Action::Sound("misc/menu2.wav"));
                    return out;
                }
                let page = self.mods_page();
                let start = out.len();
                let next = self.mods.key(page, key, &mut out);
                // starting a map asks for the skill; switching mods mid-game asks first
                let cmd = out[start..].iter().find_map(|a| match a { Action::Command(c) if c.starts_with("game ") => Some(c.clone()), _ => None });
                if let Some(cmd) = cmd && (page == modmenu::Page::Maps || self.game_running) {
                    out.retain(|a| !matches!(a, Action::Close | Action::Command(_)));
                    if page == modmenu::Page::Maps {
                        let title = launch::map_of(&cmd).and_then(|m| self.mods.map_title(m));
                        self.open_launch(cmd, true, title);
                    } else {
                        self.confirm(vec![qcore::tr!("End the current game?"), qcore::tr!("Unsaved progress will be lost.")], qcore::t!("End game"), cmd, true);
                    }
                    return out;
                }
                self.screen = match next {
                    Some(modmenu::Page::List) => Screen::Mods,
                    Some(modmenu::Page::Maps) => Screen::ModMaps,
                    Some(modmenu::Page::Online) => Screen::ModOnline,
                    None if out.iter().any(|a| matches!(a, Action::Close)) => Screen::None,
                    None => Screen::Main,
                };
            }
            Screen::Demos => self.demos_key(key, cv, &mut out),
            Screen::Quit => {
                if left || right {
                    self.quit_cursor ^= 1;
                    out.push(Action::Sound("misc/menu1.wav"));
                }
                let yes = matches!(key, 0x79 | 0x59) || (enter && self.quit_cursor == 0);
                let no = matches!(key, 0x6e | 0x4e) || back || (enter && self.quit_cursor == 1);
                if yes {
                    out.push(Action::Command("quit\n".into()));
                } else if no {
                    self.screen = if self.prev == Screen::Quit { Screen::Main } else { self.prev };
                    if self.screen == Screen::None {
                        out.push(Action::Close);
                    }
                }
            }
        }
        out
    }
}
