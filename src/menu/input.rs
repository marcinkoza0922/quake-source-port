use super::*;

impl Menu {
    /// Mouse hover: moves the cursor onto the item under the pointer.
    /// Returns a sound to play when the selection changed.
    pub fn mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) -> Option<&'static str> {
        self.sync_autoaim_mapping(cv);
        self.mouse = Some(pos);
        if self.autoaim_prompt { return None; }
        if let (Some(i), Screen::OptionsPage(p)) = (self.slider_drag, self.screen) {
            self.set_slider(self.page_item(p, i), pos.0, cv);
            return None;
        }
        if let (Some(g), Screen::OptionsPage(p)) = (self.drag, self.screen) {
            self.drag_to(p, pos.1, g);
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
        let i = self.item_at(pos)?;
        let c = self.cursor_mut()?;
        (*c != i).then(|| {
            *c = i;
            "misc/menu1.wav"
        })
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        self.sync_autoaim_mapping(cv);
        let mut out = Vec::new();
        if self.autoaim_prompt {
            self.autoaim_prompt_key(key, cv, &mut out);
            return out;
        }
        if let Some(row) = self.capture.take() {
            if self.screen != Screen::OptionsPage(Page::Controls) || key == K_ESCAPE {
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
            if let Item::Bind { cmd, .. } = self.page_item(Page::Controls, row) {
                // two keys per device kind: a third replaces both
                let same: Vec<i32> = keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == is_pad_key(key)).collect();
                if same.len() >= 2 && !same.contains(&key) {
                    for k in same {
                        out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
                        self.bindings[k as usize] = None;
                    }
                }
                out.push(Action::Command(bind_cmd(key, cmd)));
                if let Some(b) = self.bindings.get_mut(key as usize) {
                    *b = Some(cmd.to_string());
                }
                out.push(Action::Sound("misc/menu1.wav"));
            }
            return out;
        }
        if let Some(o) = self.osk {
            self.osk_key(key, o, cv, &mut out);
            return out;
        }
        if let (Some(d), Screen::OptionsPage(p)) = (self.drop, self.screen) {
            let item = self.page_item(p, d.item);
            self.drop_key(key, d, item, cv, &mut out);
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
        // left click acts on the item under the pointer, never on a stale cursor
        if let (K_MWHEELUP | K_MWHEELDOWN, Screen::OptionsPage(p)) = (key, self.screen) {
            let sc = self.page_scroll(p);
            self.set_page_scroll(p, if key == K_MWHEELUP { sc.saturating_sub(2) } else { sc + 2 });
            if let Some(i) = self.mouse.and_then(|pos| self.item_at(pos)) {
                self.opt_cursor = i;
            }
            return out;
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
                && (x - 4.0..x + 7.0).contains(&pos.0)
                && (top..top + h).contains(&pos.1)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                self.drag_to(p, pos.1, grab);
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
            let Some(i) = self.item_at(pos) else { return out };
            if let Some(c) = self.cursor_mut() {
                *c = i;
            }
            if let Screen::OptionsPage(p) = self.screen
                && let item @ (Item::Slider { .. } | Item::ShaderParam(_)) = self.page_item(p, i)
                && pos.0 >= SLIDER_X.0 - 8.0
                && pos.0 < SLIDER_X.1 + 8.0
            {
                self.set_slider(item, pos.0, cv);
                self.slider_drag = Some(i);
                out.push(Action::Sound("misc/menu3.wav"));
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
                wrap(&mut self.death_cursor, 4, &mut out);
                let shortcut = (b'1' as i32..=b'4' as i32).contains(&key).then(|| (key - b'1' as i32) as usize);
                if enter || shortcut.is_some() {
                    let choice = shortcut.unwrap_or(self.death_cursor);
                    out.push(Action::Sound("misc/menu2.wav"));
                    out.push(Action::Command(format!("_death_choice {}\n", choice + 1)));
                }
            }
            Screen::None => {}
            Screen::Main => {
                if back {
                    self.screen = Screen::None;
                    out.push(Action::Close);
                    out.push(Action::Sound("misc/menu3.wav"));
                    return out;
                }
                wrap(&mut self.main_cursor, MAIN_ITEMS, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.main_cursor {
                        0 => self.screen = Screen::SinglePlayer,
                        1 => self.screen = Screen::Multiplayer,
                        2 => self.screen = Screen::Options,
                        4 => self.open(Screen::Quit),
                        5 => {
                            out.push(Action::Command("_mods\n".into()));
                            self.screen = Screen::Mods;
                        }
                        _ => {}
                    }
                }
            }
            Screen::SinglePlayer => {
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.sp_cursor, 3, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.sp_cursor {
                        0 => {
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command("maxplayers 1\ndeathmatch 0\ncoop 0\nmap start\n".into()));
                        }
                        1 => {
                            out.push(Action::Command("_scansaves\n".into()));
                            self.autosaves = false;
                            self.open(Screen::Load);
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
                if enter {
                    let i = self.load_cursor;
                    if self.screen == Screen::Load {
                        if !self.autosaves && i == 1 {
                            self.autosaves = true;
                            self.load_cursor = 0;
                            self.save_scroll = 0.0;
                            out.push(Action::Command("_scansaves\n".into()));
                        } else if self.saves[i].is_some() {
                            let name = self.save_slot_name(i).unwrap();
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command(format!("load {name}\n")));
                        }
                    } else {
                        self.screen = Screen::None;
                        out.push(Action::Close);
                        out.push(Action::Command(format!("save s{i}\n")));
                    }
                }
            }
            Screen::Options => {
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
                    self.screen = if is_net_page(p) {
                        Screen::Multiplayer
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
                let item = self.page_item(p, self.opt_cursor);
                match item {
                    Item::Text { cvar, .. } => {
                        if key == K_ABUTTON {
                            self.osk = Some(Osk::default());
                            out.push(Action::Sound("misc/menu2.wav"));
                        } else if key == K_BACKSPACE {
                            let mut v = cv.by_name(cvar).map_or(String::new(), |c| c.string.clone());
                            v.pop();
                            cv.set(cvar, &v);
                        } else if enter && cvar == "_mp_address" {
                            self.run(Run::Connect, cv, &mut out);
                        }
                        return out;
                    }
                    Item::Run { run, .. } if enter => {
                        self.run(run, cv, &mut out);
                        return out;
                    }
                    Item::Server(k) if enter => {
                        if let Some(sv) = self.servers.get(k) {
                            cv.set("_mp_address", &sv.address.clone());
                            self.run(Run::Connect, cv, &mut out);
                        }
                        return out;
                    }
                    Item::Run { .. } | Item::Server(_) => return out,
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
                    _ => {}
                }
                if enter && self.options(item).len() > 2 {
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
            Screen::Mods | Screen::ModMaps => {
                self.mods.level_select = cv.value_of("ui_levelselect") != 0.0;
                let page = if self.screen == Screen::Mods { modmenu::Page::List } else { modmenu::Page::Maps };
                self.screen = match self.mods.key(page, key, &mut out) {
                    Some(modmenu::Page::List) => Screen::Mods,
                    Some(modmenu::Page::Maps) => Screen::ModMaps,
                    None if out.iter().any(|a| matches!(a, Action::Close)) => Screen::None,
                    None => Screen::Main,
                };
            }
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
