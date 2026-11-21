//! Touchscreen: fingers drive menus (tap to pick, drag to scroll), bring up
//! the OS keyboard in the console, and otherwise feed the on-screen game
//! controls in `qinput::touch`, which this file also draws.

use super::*;
use qinput::touch::{PAUSE_BUTTON, TouchPhase};
use qplatform::FingerPhase;

/// Menu-space drag (virtual pixels) that counts as a scroll, not a tap.
const TAP_SLOP: f32 = 8.0;
/// Menu-space drag per wheel step while scrolling.
const SCROLL_STEP: f32 = 16.0;

/// The finger working the menu: id, where it landed, the y of the last
/// scroll step, and whether it has been dragged past the tap slop.
#[derive(Clone, Copy, Debug)]
pub(super) struct MenuTouch {
    id: u64,
    start: (f32, f32),
    scroll_y: f32,
    dragged: bool,
    slider: bool,
    screen: Screen,
}

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
        self.menu.set_touch_shortcut(touch && self.cl.signon == 4 && self.demo.is_none());
        let (w, h) = self.window.as_ref().map_or_else(|| self.r.size(), |win| win.drawable_size());
        self.menu.touch_aspect = (w as f32 / h.max(1) as f32).max(0.1);
        if (touch || matches!(self.menu.screen, Screen::OptionsPage(crate::menu::Page::Touch) | Screen::TouchEditor))
            && let Some(win) = &self.window
        {
            self.input.touch_insets = win.safe_insets();
        }
        self.menu.touch_insets = self.input.touch_insets;
        if mode == 0.0 && self.input.touch.any_finger() {
            qinput::release_touch(self);
        }
    }

    pub(super) fn cancel_menu_touch(&mut self) {
        if self.menu_touch.take().is_some() {
            self.menu.hud_edit.mouse_up(&mut self.server.core.cvars);
            self.menu.mouse_up();
        }
    }

    pub(super) fn finger_event(&mut self, id: u64, phase: FingerPhase, x: f32, y: f32, timestamp_ns: u64) {
        let Some(win) = &self.window else { return };
        let (pw, ph) = win.drawable_size();
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
                self.menu_finger(id, phase, (x * pw as f32, y * ph as f32), (pw, ph));
                // a tap on a text row brings up the keyboard, again after it was dismissed
                if down && self.menu.wants_text() {
                    self.text_input = None;
                }
                self.sync_key_dest();
            }
            KeyDest::Console | KeyDest::Message => {
                // a tap brings the on-screen keyboard back after it was dismissed
                if down && let Some(w) = &self.window {
                    let _ = w.set_text_input(true);
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
            }
            KeyDest::Game if down && self.cl.signon == 4 => {
                let mut layout = self.input.touch_layout(&self.server.core.cvars);
                layout.aspect = pw as f32 / ph.max(1) as f32;
                if layout.hits(PAUSE_BUTTON, [x, y]) {
                    self.menu.open(Screen::Main);
                    self.sync_key_dest();
                }
            }
            KeyDest::Game => {}
        }
    }

    /// Tap = hover + click on release; vertical drag = mouse-wheel scroll.
    fn menu_finger(&mut self, id: u64, phase: TouchPhase, px: (f32, f32), (pw, ph): (u32, u32)) {
        let mut c = Canvas::new(pw, ph);
        self.menu.canvas_layout(&mut c);
        let pos = (px.0 / c.scale - c.ox, px.1 / c.scale - c.oy);
        if self.menu_touch.is_some_and(|t| t.screen != self.menu.screen) {
            self.cancel_menu_touch();
        }
        match phase {
            TouchPhase::Down => {
                if self.menu_touch.is_some() { return; }
                let slider = self.menu.screen == Screen::TouchEditor || self.menu.touch_slider_at(pos).is_some();
                self.menu_touch = Some(MenuTouch {
                    id,
                    start: pos,
                    scroll_y: pos.1,
                    dragged: false,
                    slider,
                    screen: self.menu.screen,
                });
                if let Some(snd) = self.menu.mouse_move(pos, &mut self.server.core.cvars) {
                    self.local_sound(snd);
                }
                if slider { self.menu_key(qinput::keys::K_MOUSE1); }
            }
            TouchPhase::Move => {
                let Some(mut t) = self.menu_touch.filter(|t| t.id == id) else {
                    return;
                };
                if t.slider {
                    let _ = self.menu.mouse_move(pos, &mut self.server.core.cvars);
                    return;
                }
                t.dragged |= (pos.0 - t.start.0).hypot(pos.1 - t.start.1) > TAP_SLOP;
                while t.dragged && (pos.1 - t.scroll_y).abs() >= SCROLL_STEP {
                    // finger down pulls the list down, i.e. scrolls up
                    let up = pos.1 > t.scroll_y;
                    t.scroll_y += if up { SCROLL_STEP } else { -SCROLL_STEP };
                    self.menu_key(if up { qinput::keys::K_MWHEELUP } else { qinput::keys::K_MWHEELDOWN });
                }
                self.menu_touch = Some(t);
            }
            TouchPhase::Up => {
                let Some(t) = self.menu_touch.filter(|t| t.id == id) else {
                    return;
                };
                if t.slider {
                    let _ = self.menu.mouse_move(pos, &mut self.server.core.cvars);
                } else if !t.dragged {
                    let _ = self.menu.mouse_move(pos, &mut self.server.core.cvars);
                    self.menu_key(qinput::keys::K_MOUSE1);
                }
                self.cancel_menu_touch();
                self.sync_key_dest();
            }
            TouchPhase::Cancel => {
                if self.menu_touch.is_some_and(|t| t.id == id) { self.cancel_menu_touch(); }
            }
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
