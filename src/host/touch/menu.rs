use super::*;
use qinput::keys::{K_MOUSE1, K_MWHEELDOWN, K_MWHEELUP};

const TAP_SLOP: f32 = 8.0;
const MIN_FLING_SPEED: f32 = 35.0;
const MAX_FLING_SPEED: f32 = 2400.0;
const SCROLL_DECAY: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gesture {
    Tap,
    SliderCandidate,
    Pointer,
    Scroll,
}

#[derive(Clone, Copy)]
pub(in crate::host) struct MenuFling {
    screen: Screen,
    velocity: f32,
    row_remainder: f32,
}

#[derive(Clone, Copy)]
pub(in crate::host) struct MenuTouch {
    id: u64,
    start: (f32, f32),
    y: f32,
    time_ns: u64,
    gesture: Gesture,
    scroll: MenuFling,
}

impl Host {
    pub(in crate::host) fn cancel_menu_touch(&mut self) {
        self.menu_fling = None;
        if self.menu_touch.take().is_some() {
            self.menu.hud_edit.mouse_up(&mut self.server.core.cvars);
            self.menu.mouse_up();
        }
    }

    pub(super) fn menu_finger(&mut self, id: u64, phase: TouchPhase, pos: (f32, f32), time_ns: u64) {
        if self.menu_touch.is_some_and(|t| t.scroll.screen != self.menu.screen) {
            self.cancel_menu_touch();
        }
        if phase == TouchPhase::Down {
            if self.menu_touch.is_some() {
                return;
            }
            self.menu_fling = None;
            let gesture = if self.menu.screen == Screen::TouchEditor {
                Gesture::Pointer
            } else if self.menu.slider_at(pos, &self.server.core.cvars).is_some() {
                Gesture::SliderCandidate
            } else {
                Gesture::Tap
            };
            self.menu_touch = Some(MenuTouch {
                id, start: pos, y: pos.1, time_ns, gesture,
                scroll: MenuFling { screen: self.menu.screen, velocity: 0.0, row_remainder: 0.0 },
            });
            if let Some(snd) = self.menu.mouse_move(pos, &mut self.server.core.cvars) {
                self.local_sound(snd);
            }
            if gesture == Gesture::Pointer {
                self.menu_key(K_MOUSE1);
            }
            return;
        }

        let Some(mut t) = self.menu_touch.filter(|t| t.id == id) else { return };
        let dx = (pos.0 - t.start.0).abs();
        let dy = (pos.1 - t.start.1).abs();
        let moved = dx.hypot(dy) > TAP_SLOP;
        match phase {
            TouchPhase::Move => {
                if moved && matches!(t.gesture, Gesture::Tap | Gesture::SliderCandidate) {
                    t.gesture = if t.gesture == Gesture::SliderCandidate && dx > dy {
                        self.menu.mouse_move(t.start, &mut self.server.core.cvars);
                        self.menu_key(K_MOUSE1);
                        Gesture::Pointer
                    } else {
                        Gesture::Scroll
                    };
                }
                match t.gesture {
                    Gesture::Pointer => { self.menu.mouse_move(pos, &mut self.server.core.cvars); }
                    Gesture::Scroll => {
                        let dt = time_ns.saturating_sub(t.time_ns) as f32 * 1e-9;
                        let delta = t.y - pos.1;
                        if dt > 0.0 {
                            let speed = (delta / dt).clamp(-MAX_FLING_SPEED, MAX_FLING_SPEED);
                            let weight = 1.0 - (-dt / 0.025).exp();
                            t.scroll.velocity += (speed - t.scroll.velocity) * weight;
                        }
                        t.y = pos.1;
                        t.time_ns = time_ns;
                        if !self.move_menu_list(&mut t.scroll, delta) {
                            t.scroll.velocity = 0.0;
                        }
                    }
                    _ => {}
                }
                self.menu_touch = Some(t);
            }
            TouchPhase::Up => {
                let fling = t.gesture == Gesture::Scroll && time_ns.saturating_sub(t.time_ns) <= 85_000_000
                    && t.scroll.velocity.abs() >= MIN_FLING_SPEED;
                if t.gesture == Gesture::Pointer {
                    self.menu.mouse_move(pos, &mut self.server.core.cvars);
                } else if t.gesture != Gesture::Scroll && !moved {
                    self.menu.mouse_move(pos, &mut self.server.core.cvars);
                    self.menu_key(K_MOUSE1);
                }
                self.cancel_menu_touch();
                self.menu_fling = fling.then_some(t.scroll);
            }
            TouchPhase::Cancel => self.cancel_menu_touch(),
            TouchPhase::Down => unreachable!(),
        }
    }

    pub(in crate::host) fn update_menu_fling(&mut self, dt: f32) {
        let Some(mut fling) = self.menu_fling.take().filter(|f| f.screen == self.menu.screen) else { return };
        let decay = (-SCROLL_DECAY * dt.min(0.1)).exp();
        let delta = fling.velocity * (1.0 - decay) / SCROLL_DECAY;
        fling.velocity *= decay;
        if self.move_menu_list(&mut fling, delta) && fling.velocity.abs() >= MIN_FLING_SPEED {
            self.menu_fling = Some(fling);
        }
    }

    fn move_menu_list(&mut self, fling: &mut MenuFling, delta: f32) -> bool {
        if let Some(moved) = self.menu.scroll_touch(delta) {
            return moved || delta == 0.0;
        }
        // Row-based widgets continue to use their own wheel navigation.
        const ROW_PITCH: f32 = 18.0;
        fling.row_remainder += delta;
        while fling.row_remainder.abs() >= ROW_PITCH && self.menu.screen == fling.screen {
            let down = fling.row_remainder > 0.0;
            fling.row_remainder -= ROW_PITCH * fling.row_remainder.signum();
            self.menu_key(if down { K_MWHEELDOWN } else { K_MWHEELUP });
        }
        self.menu.screen == fling.screen
    }
}
