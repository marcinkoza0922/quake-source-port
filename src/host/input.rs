use super::*;

impl Host {
    pub(super) fn handle_event(&mut self, ev: HostEvent) {
        match ev {
            HostEvent::Quit => self.quit = true,
            HostEvent::Resize => {
                if let Some(win) = &self.window {
                    let (w, h) = win.drawable_size();
                    self.r.resize(w, h);
                    // remember user-resized window size
                    if self.cv("vid_fullscreen") == 0.0 {
                        let (ww, wh) = win.window_size();
                        let cv = &mut self.server.core.cvars;
                        cv.set("vid_width", &ww.to_string());
                        cv.set("vid_height", &wh.to_string());
                    }
                }
            }
            HostEvent::Focus => {}
            HostEvent::PadFamily(f) => self.pad_family = crate::glyphs::Family::from_platform(f),
            HostEvent::Pad(id, ie) => self.pad_event(id, ie),
            HostEvent::PadRemoved(id) => self.pad_removed(id),
            HostEvent::Finger { id, phase, x, y } => self.finger_event(id, phase, x, y),
            HostEvent::MousePos { x, y } => {
                let Some(win) = &self.window else { return };
                let (pw, ph) = win.drawable_size();
                let (ww, wh) = win.window_size();
                let (px, py) = (x * pw as f32 / ww.max(1) as f32, y * ph as f32 / wh.max(1) as f32);
                let mut c = Canvas::new(pw, ph);
                if self.menu.active() {
                    c.menu_layout();
                    let pos = (px / c.scale - c.ox, py / c.scale - c.oy);
                    if let Some(snd) = self.menu.mouse_move(pos, &mut self.server.core.cvars) {
                        self.local_sound(snd);
                    }
                    // hovering a text row turns text input on before the first key
                    self.sync_key_dest();
                } else if self.key_dest() == KeyDest::Game && (self.demo_cursor() || self.exporting()) {
                    self.demo_pointer_move((px / c.scale, py / c.scale), (c.vw(), c.vh()));
                } else if self.key_dest() == KeyDest::Console && self.con.modern {
                    let cell = self.con.hit_cell(px / c.scale, py / c.scale);
                    self.con.hover_cell = cell;
                    if self.con.selection.dragging
                        && let Some(cell) = cell
                    {
                        self.con.selection.focus = cell;
                    }
                }
            }
            HostEvent::Input(ie) => {
                // the export screen takes every key and button (Esc included)
                if self.exporting() {
                    return self.export_input(&ie);
                }
                // the demo cursor: buttons and the wheel work the bar, not binds
                if self.key_dest() == KeyDest::Game && self.demo_cursor() {
                    match ie {
                        InputEvent::MouseButton { button, down } => return self.demo_pointer_button(button, down),
                        InputEvent::MouseWheel { .. } | InputEvent::MouseMotion { .. } => return,
                        _ => {}
                    }
                }
                if let InputEvent::MouseButton { button: 1, down } = ie {
                    if down {
                        if self.key_dest() == KeyDest::Console
                            && self.con.modern
                            && let Some(cell) = self.con.hover_cell
                        {
                            self.con.selection = crate::console::edit::Selection {
                                anchor: cell,
                                focus: cell,
                                active: true,
                                dragging: true,
                            };
                        }
                    } else {
                        self.menu.mouse_up();
                        if self.con.selection.dragging {
                            self.con.selection.dragging = false;
                            if let (Some(win), Some(s)) = (&self.window, self.con.selected_text()) {
                                let _ = win.set_clipboard_text(&s);
                            }
                        }
                    }
                }
                if let InputEvent::Key { key, down: true } = ie
                    && key == b'`' as i32
                {
                    self.toggle_console();
                    self.sync_key_dest();
                    return;
                }
                if let InputEvent::Text(c) = ie
                    && (c == '`' || c == '~')
                {
                    return;
                }
                // Demo playback controls.
                let pad_key = match ie {
                    InputEvent::Key { key, down } => Some((key, down)),
                    InputEvent::GamepadButton { button, down } => Some((button.key(), down)),
                    _ => None,
                };
                if let Some((key, down)) = pad_key
                    && self.demo_key(key, down)
                {
                    return;
                }
                // triggers are axes: their presses mark a demo clip (the
                // event still reaches the binds, which a demo ignores)
                if let InputEvent::GamepadAxis { axis: a @ (qinput::GamepadAxis::LeftTrigger | qinput::GamepadAxis::RightTrigger), value } = ie {
                    let i = usize::from(a == qinput::GamepadAxis::RightTrigger);
                    let on = value > self.cv("joy_deadzone_trigger");
                    if std::mem::replace(&mut self.demo_ui.triggers[i], on) != on && on {
                        self.demo_key([qinput::keys::K_LTRIGGER, qinput::keys::K_RTRIGGER][i], true);
                    }
                }
                // Key_Event: in a demo, a key in game mode brings up the menu.
                if let InputEvent::Key { key, down: true } = ie
                    && self.demo.is_some()
                    && self.key_dest() == KeyDest::Game
                    && key != qinput::keys::K_TAB
                {
                    self.menu.open(Screen::Main);
                    self.sync_key_dest();
                    return;
                }
                // Level complete: any key or button moves on (QC exits the
                // intermission on attack/jump/use).
                if self.cl.intermission != 0 && self.key_dest() == KeyDest::Game {
                    let press = match ie {
                        InputEvent::Key { key, down: true } => key != K_ESCAPE,
                        InputEvent::MouseButton { down: true, .. } => true,
                        InputEvent::GamepadButton { button, down: true } => button.key() != K_ESCAPE,
                        _ => false,
                    };
                    if press {
                        self.pend_attack = true;
                    }
                }
                let routed = qinput::handle_event(self, ie);
                match routed {
                    Some(Routed::Escape) => self.menu.open(Screen::Main),
                    Some(Routed::Key { key, dest: KeyDest::Console }) => {
                        self.console_key(key);
                    }
                    Some(Routed::Text { ch, dest: KeyDest::Console }) => {
                        if !self.input.key_down(qinput::keys::K_CTRL) && !self.input.key_down(qinput::keys::K_ALT) {
                            self.con.text(ch);
                        }
                    }
                    // Ctrl/Alt chords are shortcuts (Ctrl+V pastes), not typing
                    Some(Routed::Text { ch, dest: KeyDest::Menu }) => {
                        if !self.input.key_down(qinput::keys::K_CTRL) && !self.input.key_down(qinput::keys::K_ALT) {
                            self.menu.text(ch, &mut self.server.core.cvars);
                        }
                    }
                    Some(Routed::Key { key, dest: KeyDest::Menu }) => self.menu_key(key),
                    _ => {}
                }
                self.sync_key_dest();
            }
        }
    }

    fn console_key(&mut self, key: i32) {
        use qinput::keys::*;
        self.con.modern = self.cv("con_modern") != 0.0;
        if self.con.modern {
            let mods = crate::console::edit::Modifiers {
                ctrl: self.input.key_down(K_CTRL),
                shift: self.input.key_down(K_SHIFT),
                alt: self.input.key_down(K_ALT),
            };
            if key == K_ESCAPE {
                if let Some(pty) = &mut self.con.pty
                    && (pty.vt.in_alt_screen() || mods.shift)
                {
                    pty.write_bytes(b"\x1b");
                    return;
                }
                if self.con.search.active {
                    self.con.search.active = false;
                } else if self.con.completion.visible {
                    self.con.completion.clear();
                } else if self.con.forced {
                    self.menu.open(Screen::Main);
                } else {
                    self.con.open = false;
                }
                return;
            }
            self.sync_console_cvars();
            let win = &self.window;
            let clip_get = || win.as_ref().and_then(|w| w.clipboard_text());
            let clip_set = |s: &str| {
                if let Some(w) = win.as_ref() {
                    let _ = w.set_clipboard_text(s);
                }
            };
            if let Some(line) = self.con.key_modern(key, mods, &clip_get, &clip_set) {
                self.cbuf.add_text(&line);
                self.cbuf.add_text("\n");
            }
            return;
        }
        if key == K_ESCAPE {
            if self.con.forced {
                self.menu.open(Screen::Main);
            } else {
                self.con.open = false;
            }
        } else {
            let cmds = &self.cmds;
            let cv = &self.server.core.cvars;
            let sv = &self.server;
            let complete = |p: &str| -> Vec<String> {
                let mut v: Vec<String> = cmds.complete(p).into_iter().map(str::to_string).collect();
                v.extend(cv.complete(p).into_iter().map(str::to_string));
                v.extend(sv.complete_command(p).into_iter().map(str::to_string));
                v
            };
            if let Some(line) = self.con.key(key, &complete) {
                self.cbuf.add_text(&line);
                self.cbuf.add_text("\n");
            }
        }
    }

    pub(super) fn menu_key(&mut self, key: i32) {
        use qinput::keys::{K_CTRL, K_INS, K_SHIFT};
        self.menu.shift = self.input.key_down(K_SHIFT);
        let ctrl = self.input.key_down(K_CTRL);
        if self.menu.wants_text() && (ctrl || (self.menu.shift && key == K_INS)) && self.menu_clipboard(key, ctrl) {
            return;
        }
        let active = self.server.active();
        let acts = self.menu.key(key, &mut self.server.core.cvars, active);
        for a in acts {
            match a {
                Action::Command(c) => self.cbuf.add_text(&c),
                Action::Sound(s) => self.local_sound(s),
                Action::Close => {}
            }
        }
    }

    /// Clipboard keys on a menu text row: Ctrl+A selects all, Ctrl+C/X
    /// copy/cut the selection, Ctrl+V and Shift+Insert paste. True when
    /// `key` was one of them.
    fn menu_clipboard(&mut self, key: i32, ctrl: bool) -> bool {
        let cv = &mut self.server.core.cvars;
        let paste = !ctrl || key == b'v' as i32;
        match key {
            _ if paste => {
                if let Some(s) = self.window.as_ref().and_then(|w| w.clipboard_text()) {
                    // one line: newlines and tabs become spaces
                    let s: String = s.trim_end_matches(['\r', '\n']).chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
                    self.menu.text_insert(&s, cv);
                }
            }
            k if k == b'a' as i32 => self.menu.text_select_all(cv),
            k if k == b'c' as i32 || k == b'x' as i32 => {
                if let Some(s) = self.menu.text_selection(cv) {
                    if let Some(w) = &self.window {
                        let _ = w.set_clipboard_text(&s);
                    }
                    if k == b'x' as i32 {
                        self.menu.text_insert("", cv);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    /// Whether prompts should show gamepad glyphs.
    pub(super) fn pad_ui(&self) -> bool {
        self.input.gamepad_connected && self.input.last_device == Device::Gamepad
    }

    /// Glyph drawer for the controller family in use.
    pub(super) fn glyphs(&mut self) -> crate::glyphs::Glyphs {
        let family = self.pad_family.with_override(self.cv("joy_glyphs"));
        crate::glyphs::Glyphs { family }
    }

    /// Menu state that follows the input devices and bindings.
    /// The Quake theme (CD track 2) plays in the main menu while no game,
    /// demo or connection is running, and stops when one starts without a
    /// track of its own. `menu_music 0` turns it off.
    /// The Quake theme (CD track 2) plays in the main menu while no game or
    /// connection runs (attract demos keep it); it stops when one starts
    /// (or with `menu_music 0`).
    pub(super) fn sync_menu_music(&mut self) {
        // startup demos behind the menu don't count as a game
        let no_game = !self.server.active() && (self.cl.signon == 0 || self.demo.is_some());
        let on = self.server.core.cvars.value_of("menu_music") != 0.0;
        let Some(snd) = &mut self.snd else { return };
        if !self.menu_music && no_game && on && self.menu.screen == Screen::Main {
            self.menu_music = true;
            snd.play_cd_track(2, true);
        } else if self.menu_music && !(no_game && on) {
            self.menu_music = false;
            snd.stop_music();
        }
    }

    pub(super) fn sync_menu_ui(&mut self) {
        self.menu.game_running = self.server.active();
        self.menu.custom_running = self.server.active() && self.server.core.custom;
        self.menu.pad_ui = self.pad_ui();
        self.menu.glyphs = self.glyphs();
        // headless runs are tools and tests: never block them on the pop-up
        if !self.headless {
            self.menu.show_pending_preset();
        }
        if self.menu.active() {
            self.menu.bindings.clone_from(&self.input.bindings);
        }
        self.input.capture = self.menu.capturing();
    }

    /// Gamepad auto-repeat for held menu directions (keyboards repeat themselves).
    pub(super) fn menu_nav_repeat(&mut self) {
        use qinput::keys::*;
        if !self.menu.active() || self.menu.capturing() || self.input.last_device != Device::Gamepad {
            self.nav_hold = None;
            return;
        }
        let held = [K_DPAD_UP, K_DPAD_DOWN, K_DPAD_LEFT, K_DPAD_RIGHT, K_UPARROW, K_DOWNARROW, K_LEFTARROW, K_RIGHTARROW]
            .into_iter()
            .find(|&k| self.input.key_down(k));
        match (held, self.nav_hold) {
            (None, _) => self.nav_hold = None,
            (Some(k), Some((h, t))) if h == k => {
                if self.realtime >= t {
                    self.nav_hold = Some((k, t.max(self.realtime - 0.1) + 0.09));
                    self.menu_key(k);
                }
            }
            (Some(k), _) => self.nav_hold = Some((k, self.realtime + 0.35)),
        }
    }

    /// Queue a rumble (strengths 0..1), scaled by `joy_rumble`; only while
    /// the gamepad is the device in use.
    pub(super) fn add_rumble(&mut self, low: f32, high: f32, ms: u32) {
        let k = self.cv("joy_rumble").clamp(0.0, 1.0);
        if k <= 0.0 || !self.pad_ui() {
            return;
        }
        let m = |v: f32| (v.clamp(0.0, 1.0) * k * 65535.0) as u16;
        let (lo, hi) = (m(low), m(high));
        self.rumble = Some(match self.rumble {
            Some((a, b, t)) => (a.max(lo), b.max(hi), t.max(ms)),
            None => (lo, hi, ms),
        });
    }

    pub fn take_rumble(&mut self) -> Option<(u16, u16, u32)> {
        self.rumble.take()
    }

    /// Rumble on firing (ammo spent, axe swing) and on view punches.
    pub(super) fn track_rumble(&mut self) {
        let st = &self.cl.stats;
        let (w, ammo, wf, punch) = (st[STAT_ACTIVEWEAPON], st[STAT_AMMO], st[STAT_WEAPONFRAME], self.cl.punchangle);
        let (pw, pa, pwf, pp) = std::mem::replace(&mut self.fire_track, (w, ammo, wf, punch));
        if self.cl.signon != 4 || self.cl.intermission != 0 || self.demo.is_some() {
            return;
        }
        if (w == pw && ammo < pa) || (w == 4096 && pwf == 0 && wf != 0) {
            let (lo, hi, ms) = match w {
                1 => (0.35, 0.2, 90),
                2 => (0.7, 0.4, 140),
                4 => (0.12, 0.2, 50),
                8 => (0.2, 0.3, 60),
                16 => (0.5, 0.2, 120),
                32 => (0.8, 0.35, 160),
                64 => (0.15, 0.4, 60),
                _ => (0.2, 0.15, 60),
            };
            self.add_rumble(lo, hi, ms);
        }
        if punch != pp && punch != [0.0; 3] {
            let m = (punch.iter().fold(0.0f32, |a, x| a.max(x.abs())) / 6.0).min(1.0);
            self.add_rumble(0.6 * m, 0.3 * m, 100);
        }
    }
}
