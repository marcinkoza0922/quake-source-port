//! On-screen touch controls: a floating move stick, drag-to-look, and a
//! dedicated finger gesture for the weapon wheel.
//!
//! Positions are window-normalised (0..1 both axes). Distances are in
//! "height units" (fractions of the window height) so circles stay round
//! at any aspect ratio.

mod layout;
pub use layout::{ButtonPlacement, LAYOUT_CVAR, TouchLayout};

/// A full-height swipe on platforms without physical metrics uses the
/// original 180-degree turn at the default sensitivity of 28 degrees/cm.
const FALLBACK_HEIGHT_CM: f32 = 180.0 / 28.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct TouchLook {
    pub size_cm: Option<[f32; 2]>,
    /// Maximum added gain, from zero (linear) to 0.5 (1.5x).
    pub acceleration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LookSample {
    pos: [f32; 2],
    time_ns: u64,
}

impl LookSample {
    fn advance(&mut self, next: Self, aspect: f32, settings: TouchLook) -> [f32; 2] {
        let [w, h] = settings.size_cm.unwrap_or([aspect * FALLBACK_HEIGHT_CM, FALLBACK_HEIGHT_CM]);
        let [x, y] = [(next.pos[0] - self.pos[0]) * w, (next.pos[1] - self.pos[1]) * h];
        let dt = next.time_ns.saturating_sub(self.time_ns) as f32 * 1e-9;
        *self = next;
        if settings.acceleration <= 0.0 || dt == 0.0 {
            return [x, y];
        }
        let speed = x.hypot(y) / dt.max(0.001);
        let t = ((speed - 12.0) / (65.0 - 12.0)).clamp(0.0, 1.0);
        let gain = 1.0 + settings.acceleration * t * t * (3.0 - 2.0 * t);
        [x * gain, y * gain]
    }
}

/// Finger event phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// An on-screen button. Commands starting with `+` are held until release.
/// Save and Load activate on release inside the button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchButton {
    pub label: &'static str,
    pub cmd: Option<&'static str>,
    /// Centre from the right and bottom safe edges in height units.
    /// Negative distances anchor to the opposite edge.
    pub at: [f32; 2],
    pub radius: f32,
    /// Dragging the finger on this button also turns the view.
    pub looks: bool,
}

// Kept above the modern HUD's bottom rows (health, ammo list).
pub const BUTTONS: [TouchButton; 8] = [
    TouchButton { label: "Fire", cmd: Some("+attack"), at: [0.17, 0.42], radius: 0.10, looks: true },
    TouchButton { label: "JUMP", cmd: Some("+jump"), at: [0.35, 0.25], radius: 0.075, looks: false },
    TouchButton { label: "WEAPON", cmd: None, at: [0.37, 0.49], radius: 0.075, looks: false },
    TouchButton { label: "MENU", cmd: Some("togglemenu"), at: [-0.085, -0.085], radius: 0.06, looks: false },
    TouchButton { label: "Swim Down", cmd: None, at: [0.53, 0.32], radius: 0.065, looks: false },
    TouchButton { label: "Save", cmd: Some("save quick"), at: [-0.26, -0.075], radius: 0.065, looks: false },
    TouchButton { label: "Load", cmd: Some("load quick"), at: [-0.42, -0.075], radius: 0.065, looks: false },
    TouchButton { label: "Fire", cmd: Some("+attack"), at: [-0.25, -0.28], radius: 0.085, looks: false },
];
const WHEEL: usize = 2;
pub const WEAPON_HOLD_NS: u64 = 200_000_000;
pub const PAUSE_BUTTON: usize = 3;
pub const SWIM_DOWN: usize = 4;
const QUICK_SAVE: usize = 5;
const QUICK_LOAD: usize = 6;
const MOVE_FIRE: usize = 7;
const HIT_SCALE: f32 = 1.15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TouchButtons {
    pub submerged: bool,
    pub quicksave: bool,
    pub quickload: bool,
}

impl TouchButtons {
    pub const PREVIEW: Self = Self { submerged: true, quicksave: true, quickload: true };
}

/// Key number passed with `+cmd` so the release matches the press
/// (outside the real key range, so it never clashes with a held key).
pub const TOUCH_KEY_BASE: i32 = 1000;

/// Move stick radius (height units) before `touch_size` scaling.
pub const STICK_RADIUS: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    /// Floating stick: where the finger first landed, and where it is now.
    Move { origin: [f32; 2], pos: [f32; 2] },
    Look { last: LookSample },
    Button { index: usize, last: LookSample },
    Wheel { start: LookSample, pos: [f32; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Finger {
    id: u64,
    role: Role,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchOverlay {
    fingers: Vec<Finger>,
    /// Accumulated look drag since the last frame, in centimeters (+x
    /// right, +y down).
    look: [f32; 2],
    wheel_release: Option<(LookSample, LookSample)>,
}

/// The wheel tracks one finger relative to where it touched down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelGesture {
    Held([f32; 2]),
    Released { aim: [f32; 2], cycle: u8 },
}

impl TouchOverlay {
    /// Feed one finger event; returns console commands to run.
    pub fn event(&mut self, id: u64, phase: TouchPhase, pos: [f32; 2], time_ns: u64, lay: &TouchLayout, look: TouchLook) -> Vec<String> {
        let sample = LookSample { pos, time_ns };
        let mut out = Vec::new();
        match phase {
            TouchPhase::Down => {
                // a stray Down for a finger we already track: restart it
                out.extend(self.lift(id));
                if !lay.contains(pos) {
                    return out;
                }
                let role = match lay.button_at(pos) {
                    Some(index) if self.pressed(index) => return out,
                    Some(WHEEL) => {
                        self.wheel_release = None;
                        Role::Wheel { start: sample, pos }
                    }
                    Some(index) => {
                        if let Some(cmd) = BUTTONS[index].cmd.filter(|_| !matches!(index, QUICK_SAVE | QUICK_LOAD)) {
                            out.push(if cmd.starts_with('+') {
                                format!("{cmd} {}\n", TOUCH_KEY_BASE + index as i32)
                            } else {
                                format!("{cmd}\n")
                            });
                        }
                        Role::Button { index, last: sample }
                    }
                    None if lay.move_side(pos[0]) && self.stick().is_none() => {
                        Role::Move { origin: pos, pos }
                    }
                    None => Role::Look { last: sample },
                };
                self.fingers.push(Finger { id, role });
            }
            TouchPhase::Move => {
                let r = lay.stick_radius();
                let aspect = lay.aspect;
                let Some(f) = self.fingers.iter_mut().find(|f| f.id == id) else { return out };
                match &mut f.role {
                    Role::Move { origin, pos: p } => {
                        *p = pos;
                        // the base follows a finger that runs past the rim
                        let d = lay.dist(pos, *origin);
                        if d > r {
                            let k = (d - r) / d;
                            origin[0] += (pos[0] - origin[0]) * k;
                            origin[1] += (pos[1] - origin[1]) * k;
                        }
                    }
                    Role::Look { last } => {
                        let delta = last.advance(sample, aspect, look);
                        self.look[0] += delta[0];
                        self.look[1] += delta[1];
                    }
                    Role::Button { index, last } if BUTTONS[*index].looks => {
                        let delta = last.advance(sample, aspect, look);
                        self.look[0] += delta[0];
                        self.look[1] += delta[1];
                    }
                    Role::Button { .. } => {}
                    Role::Wheel { pos: p, .. } => *p = pos,
                }
            }
            TouchPhase::Up => {
                match self.fingers.iter().find(|f| f.id == id).map(|f| f.role) {
                    Some(Role::Wheel { start, .. }) => self.wheel_release = Some((start, sample)),
                    Some(Role::Button { index, .. }) if matches!(index, QUICK_SAVE | QUICK_LOAD)
                        && lay.button_enabled(index) && lay.hits(index, pos) =>
                    {
                        if let Some(cmd) = BUTTONS[index].cmd {
                            out.push(format!("{cmd}\n"));
                        }
                    }
                    _ => {}
                }
                out.extend(self.lift(id));
            }
            TouchPhase::Cancel => out.extend(self.lift(id)),
        }
        out
    }

    fn lift(&mut self, id: u64) -> Option<String> {
        let i = self.fingers.iter().position(|f| f.id == id)?;
        let f = self.fingers.remove(i);
        match f.role {
            Role::Button { index, .. } => {
                let rest = BUTTONS[index].cmd?.strip_prefix('+')?;
                Some(format!("-{rest} {}\n", TOUCH_KEY_BASE + index as i32))
            }
            _ => None,
        }
    }

    /// Lift every finger (menu opened, controls hidden); returns the
    /// release commands for held buttons.
    pub fn release_all(&mut self) -> Vec<String> {
        let ids: Vec<u64> = self.fingers.iter().map(|f| f.id).collect();
        self.look = [0.0; 2];
        self.wheel_release = None;
        ids.into_iter().filter_map(|id| self.lift(id)).collect()
    }

    /// Release controls that disappeared while a finger was still on them.
    pub fn release_hidden(&mut self, layout: &TouchLayout) -> Vec<String> {
        let ids: Vec<_> = self.fingers.iter().filter_map(|f| match f.role {
            Role::Button { index, .. } if !layout.button_enabled(index) => Some(f.id),
            _ => None,
        }).collect();
        ids.into_iter().filter_map(|id| self.lift(id)).collect()
    }

    /// Move stick deflection, -1..1 per axis (+y = back).
    pub fn move_vec(&self, lay: &TouchLayout) -> [f32; 2] {
        let Some((o, p)) = self.stick() else { return [0.0; 2] };
        let r = lay.stick_radius();
        let v = [(p[0] - o[0]) * lay.aspect / r, (p[1] - o[1]) / r];
        let m = v[0].hypot(v[1]);
        // small deadzone so a resting thumb doesn't creep
        if m < 0.1 {
            [0.0; 2]
        } else if m > 1.0 {
            [v[0] / m, v[1] / m]
        } else {
            v
        }
    }

    /// Take the look drag accumulated since the last call (centimeters).
    pub fn take_look(&mut self) -> [f32; 2] {
        std::mem::take(&mut self.look)
    }

    pub fn has_look(&self) -> bool {
        self.look != [0.0; 2]
    }

    /// A finger on the aim side that can drag to turn the view.
    pub fn looking(&self, layout: &TouchLayout) -> bool {
        self.fingers.iter().filter_map(|f| match f.role {
            Role::Look { last } => Some(last.pos),
            Role::Button { index, last } if BUTTONS[index].looks => Some(last.pos),
            _ => None,
        }).any(|pos| layout.contains(pos) && !layout.move_side(pos[0]))
    }

    pub fn wheel_held(&self) -> bool {
        self.pressed(WHEEL)
    }

    pub fn take_wheel(&mut self, layout: &TouchLayout) -> Option<WheelGesture> {
        let direction = |origin: [f32; 2], pos: [f32; 2]| {
            let v = [(pos[0] - origin[0]) * layout.aspect, pos[1] - origin[1]];
            let d = v[0].hypot(v[1]);
            if d < 0.035 { [0.0; 2] } else { [v[0] / d, v[1] / d] }
        };
        if let Some((start, end)) = self.wheel_release.take() {
            let dx = (end.pos[0] - start.pos[0]) * layout.aspect;
            let dy = end.pos[1] - start.pos[1];
            let quick = end.time_ns.saturating_sub(start.time_ns) < WEAPON_HOLD_NS;
            let cycle = if quick && dx.abs() >= 0.055 && dx.abs() >= 1.5 * dy.abs() {
                if dx > 0.0 { 10 } else { 12 }
            } else {
                0
            };
            return Some(WheelGesture::Released { aim: direction(start.pos, end.pos), cycle });
        }
        self.fingers.iter().find_map(|f| match f.role {
            Role::Wheel { start, pos } => Some(WheelGesture::Held(direction(start.pos, pos))),
            _ => None,
        })
    }

    /// Active stick: base origin and finger position (normalised).
    pub fn stick(&self) -> Option<([f32; 2], [f32; 2])> {
        self.fingers.iter().find_map(|f| match f.role {
            Role::Move { origin, pos } => Some((origin, pos)),
            _ => None,
        })
    }

    /// Whether a finger is holding button `index` of [`BUTTONS`].
    pub fn pressed(&self, index: usize) -> bool {
        self.fingers.iter().any(|f| match f.role {
            Role::Button { index: i, .. } => i == index,
            Role::Wheel { .. } => index == WHEEL,
            _ => false,
        })
    }

    pub fn any_finger(&self) -> bool {
        !self.fingers.is_empty()
    }

    pub fn button_held_by(&self, id: u64) -> bool {
        self.fingers.iter().any(|f| f.id == id && matches!(f.role, Role::Button { .. } | Role::Wheel { .. }))
    }
}

#[cfg(test)]
mod tests;
