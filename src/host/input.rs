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
            HostEvent::MousePos { x, y } => {
                if !self.menu.active() {
                    return;
                }
                let Some(win) = &self.window else { return };
                let (pw, ph) = win.drawable_size();
                let (ww, wh) = win.window_size();
                let (px, py) = (x * pw as f32 / ww.max(1) as f32, y * ph as f32 / wh.max(1) as f32);
                let mut c = Canvas::new(pw, ph);
                c.menu_layout();
                let pos = (px / c.scale - c.ox, py / c.scale - c.oy);
                if let Some(snd) = self.menu.mouse_move(pos, &mut self.server.core.cvars) {
                    self.local_sound(snd);
                }
            }
            HostEvent::Input(ie) => {
                if let InputEvent::MouseButton { button: 1, down: false } = ie {
                    self.menu.mouse_up();
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
                    Some(Routed::Text { ch, dest: KeyDest::Console }) => self.con.text(ch),
                    Some(Routed::Text { ch, dest: KeyDest::Menu }) => self.menu.text(ch, &mut self.server.core.cvars),
                    Some(Routed::Key { key, dest: KeyDest::Menu }) => self.menu_key(key),
                    _ => {}
                }
                self.sync_key_dest();
            }
        }
    }

    pub(super) fn menu_key(&mut self, key: i32) {
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
    pub(super) fn sync_menu_ui(&mut self) {
        self.menu.pad_ui = self.pad_ui();
        self.menu.glyphs = self.glyphs();
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
