//! Touchscreen: fingers drive menus (tap to pick, drag to scroll), bring up
//! the OS keyboard in the console, and otherwise feed the on-screen game
//! controls in `qinput::touch`, which this file also draws.

use super::*;
use qinput::touch::{PAUSE_BUTTON, TouchButtons, TouchPhase};
use qplatform::FingerPhase;

mod menu;
pub(super) use menu::{MenuFling, MenuTouch};

impl Host {
    #[cfg(target_os = "android")]
    pub(crate) fn wants_phone_gyro(&self) -> bool {
        !self.backgrounded && self.key_dest() == KeyDest::Game && self.cl.signon == 4
            && self.cl.intermission == 0 && self.demo.is_none() && !self.automap.fullscreen
            && self.cv("gyro_phone_mode") != 0.0
    }

    pub(super) fn sync_touch_menu(&mut self) {
        let mode = self.cv("touch_enable");
        let touch = self.input.last_device == Device::Touch || mode == 2.0;
        self.menu.set_touch_ui(touch);
        if !touch { self.cancel_menu_touch(); }
        self.menu.set_touch_shortcut(touch && self.cl.signon == 4 && self.demo.is_none());
        let (w, h) = self.window.as_ref().map_or_else(|| self.r.size(), |win| win.drawable_size());
        self.menu.touch_aspect = (w as f32 / h.max(1) as f32).max(0.1);
        self.menu.display_scale = self.display_scale() * self.cv("touch_menu_scale").clamp(0.75, 1.5);
        if (touch || matches!(self.menu.screen, Screen::OptionsPage(crate::menu::Page::Touch) | Screen::TouchEditor))
            && let Some(win) = &self.window
        {
            self.input.touch_insets = win.safe_insets();
        }
        self.menu.touch_insets = self.input.touch_insets;
        if mode == 0.0 && self.input.touch.any_finger() {
            qinput::release_touch(self);
        }
        if self.menu.screen == Screen::Death && touch {
            if self.realtime >= self.touch_quickload_probe {
                self.menu.death_quickload = self.touch_quickload_exists();
            }
        } else {
            self.menu.death_quickload = false;
        }
        self.sync_touch_buttons();
    }

    /// Pixels per platform UI unit for the touch menus: `-displayscale`,
    /// Android's density from the display DPI, or the window's display scale.
    fn display_scale(&self) -> f32 {
        let arg = self.cmdline.iter().position(|a| a == "-displayscale").and_then(|i| self.cmdline.get(i + 1));
        if let Some(s) = arg.and_then(|s| s.parse::<f32>().ok()).filter(|s| s.is_finite() && *s > 0.0) {
            return s;
        }
        let Some(win) = &self.window else { return 1.0 };
        #[cfg(target_os = "android")]
        if let Some([cm, _]) = win.physical_size_cm() {
            // Android's dp: 160 per inch
            return win.drawable_size().0 as f32 / (cm / 2.54) / 160.0;
        }
        win.display_scale()
    }

    fn touch_quickload_exists(&mut self) -> bool {
        self.touch_quickload_probe = self.realtime + 0.5;
        crate::autosave::find(&self.user_dir(), "quick").is_some()
    }

    fn sync_touch_buttons(&mut self) {
        let active = self.input.touch_visible(&self.server.core.cvars) && self.key_dest() == KeyDest::Game
            && self.cl.signon == 4 && self.cl.intermission == 0 && self.demo.is_none()
            && !self.automap.fullscreen && self.freecam.cam.is_none() && self.cl.stats[STAT_HEALTH] > 0;
        let mut buttons = TouchButtons::default();
        if active {
            buttons.submerged = self.cl.entities.get(self.cl.viewentity).is_some_and(|ent| {
                let [x, y, z] = ent.origin;
                (-5..=-3).contains(&self.contents([x, y, z + self.cl.viewheight]))
            });
            if self.local_single_player() {
                buttons.quicksave = qserver::modifiers::value(&self.server.core, "modifier_nosaves") == 0.0;
                buttons.quickload = self.input.touch_buttons.quickload;
                if self.realtime >= self.touch_quickload_probe {
                    buttons.quickload = self.touch_quickload_exists();
                }
            } else {
                self.touch_quickload_probe = 0.0;
            }
        } else if self.menu.screen != Screen::Death {
            self.touch_quickload_probe = 0.0;
        }
        self.input.touch_buttons = buttons;
        if self.input.touch.any_finger() {
            let layout = self.input.touch_layout(&self.server.core.cvars);
            for cmd in self.input.touch.release_hidden(&layout) {
                self.cbuf.add_text(&cmd);
            }
        }
    }

    pub(super) fn add_touch_haptic(&mut self, strength: f32, duration: u32) {
        if cfg!(target_os = "android") && self.input.last_device == Device::Touch && self.cv("touch_haptics") != 0.0
            && !self.backgrounded && self.demo.is_none()
            && self.touch_haptic.is_none_or(|(_, ms)| ms <= duration)
        {
            self.touch_haptic = Some((strength, duration));
        }
    }

    #[cfg(target_os = "android")]
    pub fn take_touch_haptic(&mut self) -> Option<(f32, u32)> {
        self.touch_haptic.take().filter(|_| self.input.last_device == Device::Touch
            && !self.backgrounded && self.cv("touch_haptics") != 0.0)
    }

    pub(super) fn finger_event(&mut self, id: u64, phase: FingerPhase, x: f32, y: f32, timestamp_ns: u64) {
        // headless (`touch_tap`): the renderer's size
        let (pw, ph) = self.window.as_ref().map_or_else(|| self.r.size(), |w| w.drawable_size());
        let phase = match phase {
            FingerPhase::Down => TouchPhase::Down,
            FingerPhase::Move => TouchPhase::Move,
            FingerPhase::Up => TouchPhase::Up,
            FingerPhase::Cancel => TouchPhase::Cancel,
        };
        let down = phase == TouchPhase::Down;
        if down {
            self.input.last_device = Device::Touch;
            self.sync_touch_menu();
        }
        match self.key_dest() {
            KeyDest::Menu => {
                let mut canvas = Canvas::new(pw, ph);
                self.menu.canvas_layout(&mut canvas);
                let pos = (x * pw as f32 / canvas.scale - canvas.ox, y * ph as f32 / canvas.scale - canvas.oy);
                self.menu_finger(id, phase, pos, timestamp_ns);
                // a tap on a text row brings up the keyboard, again after it was dismissed
                if down && self.menu.wants_text() {
                    self.text_input = None;
                }
                self.sync_key_dest();
            }
            dest @ (KeyDest::Console | KeyDest::Message) => {
                // a tap brings the on-screen keyboard back after it was dismissed
                if down && let Some(w) = &self.window {
                    self.touch_console_keyboard = dest == KeyDest::Console;
                    if w.set_text_input(true).is_ok() {
                        self.text_input = Some(true);
                    }
                }
            }
            KeyDest::Game if down && self.demo.is_some() => {
                self.menu.open(Screen::Main);
                self.sync_key_dest();
            }
            KeyDest::Game if self.cl.intermission != 0 => self.pend_attack |= down,
            KeyDest::Game if self.cv("touch_enable") != 0.0 => {
                let aspect = pw as f32 / ph.max(1) as f32;
                self.handle_event(HostEvent::Input(InputEvent::Touch { id, phase, x, y, aspect, timestamp_ns }));
                if down && self.input.touch.button_held_by(id) {
                    self.add_touch_haptic(0.25, 20);
                }
            }
            KeyDest::Game if down && self.cl.signon == 4 => {
                let mut layout = self.input.touch_layout(&self.server.core.cvars);
                layout.aspect = pw as f32 / ph.max(1) as f32;
                if layout.hits(PAUSE_BUTTON, [x, y]) {
                    self.add_touch_haptic(0.25, 20);
                    self.menu.open(Screen::Main);
                    self.sync_key_dest();
                }
            }
            KeyDest::Game => {}
        }
    }
}

/// On-screen stick and buttons, over the HUD (game key dest only).
pub(super) fn draw_touch(c: &mut Canvas, input: &qinput::Input, cv: &qcore::cvar::Cvars) {
    let menu_only = cv.value(input.cv.touch_enable) == 0.0 && input.last_device == Device::Touch;
    if input.dest != KeyDest::Game || (!input.touch_visible(cv) && !menu_only) {
        return;
    }
    let mut layout = input.touch_layout(cv);
    c.reset_origin();
    let (vw, vh) = (c.vw(), c.vh());
    layout.aspect = vw / vh.max(1.0);
    crate::touch_controls::draw(c, &layout, Some(&input.touch), [0.0, 0.0, vw, vh], cv.value(input.cv.touch_opacity), menu_only);
}
