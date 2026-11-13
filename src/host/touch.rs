//! Touchscreen: fingers drive menus (tap to pick, drag to scroll), bring up
//! the OS keyboard in the console, and otherwise feed the on-screen game
//! controls in `qinput::touch`, which this file also draws.

use super::*;
use qinput::touch::{BUTTONS, TouchPhase};
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
}

impl Host {
    pub(super) fn finger_event(&mut self, id: u64, phase: FingerPhase, x: f32, y: f32) {
        let Some(win) = &self.window else { return };
        let (pw, ph) = win.drawable_size();
        let phase = match phase {
            FingerPhase::Down => TouchPhase::Down,
            FingerPhase::Move => TouchPhase::Move,
            FingerPhase::Up => TouchPhase::Up,
        };
        if self.cv("touch_enable") == 0.0 {
            return;
        }
        let down = phase == TouchPhase::Down;
        match self.key_dest() {
            KeyDest::Menu => self.menu_finger(id, phase, (x * pw as f32, y * ph as f32), (pw, ph)),
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
            KeyDest::Game => {
                if down && self.cl.intermission != 0 {
                    self.pend_attack = true;
                    return;
                }
                let aspect = pw as f32 / ph.max(1) as f32;
                self.handle_event(HostEvent::Input(InputEvent::Touch { id, phase, x, y, aspect }));
            }
        }
    }

    /// Tap = hover + click on release; vertical drag = mouse-wheel scroll.
    fn menu_finger(&mut self, id: u64, phase: TouchPhase, px: (f32, f32), (pw, ph): (u32, u32)) {
        let mut c = Canvas::new(pw, ph);
        c.menu_layout();
        let pos = (px.0 / c.scale - c.ox, px.1 / c.scale - c.oy);
        match phase {
            TouchPhase::Down => {
                self.menu_touch = Some(MenuTouch {
                    id,
                    start: pos,
                    scroll_y: pos.1,
                    dragged: false,
                });
                if let Some(snd) = self.menu.mouse_move(pos, &mut self.server.core.cvars) {
                    self.local_sound(snd);
                }
            }
            TouchPhase::Move => {
                let Some(mut t) = self.menu_touch.filter(|t| t.id == id) else {
                    return;
                };
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
                let Some(t) = self.menu_touch.take().filter(|t| t.id == id) else {
                    return;
                };
                if !t.dragged {
                    let _ = self.menu.mouse_move(pos, &mut self.server.core.cvars);
                    self.menu_key(qinput::keys::K_MOUSE1);
                }
                self.menu.mouse_up();
                self.sync_key_dest();
            }
        }
    }
}

/// On-screen stick and buttons, over the HUD (game key dest only).
pub(super) fn draw_touch(c: &mut Canvas, input: &qinput::Input, cv: &qcore::cvar::Cvars) {
    if !input.touch_visible(cv) || input.dest != KeyDest::Game {
        return;
    }
    let a = cv.value_of("touch_opacity").clamp(0.05, 1.0);
    let lay = qinput::touch::TouchLayout {
        aspect: c.vw() / c.vh(),
        size: cv.value_of("touch_size"),
    };
    c.reset_origin();
    let (vw, vh) = (c.vw(), c.vh());
    let disc = |c: &mut Canvas, p: [f32; 2], r: f32, fill: [f32; 4], edge: [f32; 4]| {
        let (x, y, r) = (p[0] * vw, p[1] * vh, r * vh);
        c.round_box(x - r, y - r, r * 2.0, r * 2.0, r, fill, Some(edge));
    };
    let edge = [1.0, 1.0, 1.0, 0.6 * a];
    for (i, b) in BUTTONS.iter().enumerate() {
        let (p, r) = lay.button(b);
        let held = input.touch.pressed(i);
        let fill = if held {
            [1.0, 0.85, 0.5, 0.45 * a]
        } else {
            [0.0, 0.0, 0.0, 0.35 * a]
        };
        disc(c, p, r, fill, edge);
        let size = (r * vh * 0.42).clamp(6.0, 14.0);
        c.text_center(p[0] * vw, p[1] * vh - size * 0.5, b.label, size, [1.0, 1.0, 1.0, 0.9 * a.max(0.5)]);
    }
    let sr = qinput::touch::STICK_RADIUS * lay.size.clamp(0.5, 2.0);
    match input.touch.stick() {
        Some((o, p)) => {
            disc(c, o, sr, [0.0, 0.0, 0.0, 0.25 * a], edge);
            // knob clamped to the rim
            let (dx, dy) = ((p[0] - o[0]) * lay.aspect, p[1] - o[1]);
            let d = dx.hypot(dy).max(1e-6);
            let k = (sr / d).min(1.0);
            let kp = [o[0] + dx * k / lay.aspect, o[1] + dy * k];
            disc(c, kp, sr * 0.45, [1.0, 1.0, 1.0, 0.35 * a], edge);
        }
        // resting hint where the thumb usually lands
        None => disc(
            c,
            [(sr + 0.1) / lay.aspect, 0.62],
            sr,
            [0.0, 0.0, 0.0, 0.12 * a],
            [1.0, 1.0, 1.0, 0.25 * a],
        ),
    }
}
