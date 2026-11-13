//! On-screen touch controls (twin-zone): a floating move stick on the left
//! half, drag-to-look on the right half, and round buttons that run
//! commands. Pure state; the host feeds finger events and draws
//! [`TouchOverlay::buttons`] / [`TouchOverlay::stick`].
//!
//! Positions are window-normalised (0..1 both axes). Distances are in
//! "height units" (fractions of the window height) so circles stay round
//! at any aspect ratio.

/// Finger event phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
}

/// An on-screen button. `cmd` starting with `+` is held (its `-` runs on
/// release); anything else runs once on press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchButton {
    pub label: &'static str,
    pub cmd: &'static str,
    /// Centre as (distance from the right edge or, if negative, minus the
    /// distance from the left edge; distance from the bottom edge or, if
    /// negative, minus the distance from the top), in height units.
    pub at: [f32; 2],
    pub radius: f32,
    /// Dragging the finger on this button also turns the view.
    pub looks: bool,
}

// Kept above the modern HUD's bottom rows (health, ammo list).
pub const BUTTONS: [TouchButton; 5] = [
    TouchButton { label: "FIRE", cmd: "+attack", at: [0.17, 0.42], radius: 0.11, looks: true },
    TouchButton { label: "JUMP", cmd: "+jump", at: [0.55, 0.30], radius: 0.08, looks: false },
    TouchButton { label: "NEXT", cmd: "impulse 10", at: [0.11, 0.68], radius: 0.065, looks: false },
    TouchButton { label: "PREV", cmd: "impulse 12", at: [0.31, 0.64], radius: 0.055, looks: false },
    TouchButton { label: "MENU", cmd: "togglemenu", at: [-0.07, -0.07], radius: 0.045, looks: false },
];

/// Key number passed with `+cmd` so the release matches the press
/// (outside the real key range, so it never clashes with a held key).
pub const TOUCH_KEY_BASE: i32 = 1000;

/// Move stick radius (height units) before `touch_size` scaling.
pub const STICK_RADIUS: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    /// Floating stick: where the finger first landed, and where it is now.
    Move { origin: [f32; 2], pos: [f32; 2] },
    Look { last: [f32; 2] },
    Button { index: usize, last: [f32; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Finger {
    id: u64,
    role: Role,
}

/// Window shape and control scale, from the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchLayout {
    /// Width / height.
    pub aspect: f32,
    /// `touch_size` (1 = default).
    pub size: f32,
}

impl TouchLayout {
    /// Button centre in window-normalised coordinates, and radius in
    /// height units.
    pub fn button(&self, b: &TouchButton) -> ([f32; 2], f32) {
        let s = self.size.clamp(0.5, 2.0);
        let x = if b.at[0] >= 0.0 { 1.0 - b.at[0] * s / self.aspect } else { -b.at[0] * s / self.aspect };
        let y = if b.at[1] >= 0.0 { 1.0 - b.at[1] * s } else { -b.at[1] * s };
        ([x, y], b.radius * s)
    }

    fn stick_radius(&self) -> f32 {
        STICK_RADIUS * self.size.clamp(0.5, 2.0)
    }

    /// Distance between two normalised points in height units.
    fn dist(&self, a: [f32; 2], b: [f32; 2]) -> f32 {
        ((a[0] - b[0]) * self.aspect).hypot(a[1] - b[1])
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchOverlay {
    fingers: Vec<Finger>,
    /// Accumulated look drag since the last frame, in height units (+x
    /// right, +y down).
    look: [f32; 2],
}

impl TouchOverlay {
    /// Feed one finger event; returns console commands to run.
    pub fn event(&mut self, id: u64, phase: TouchPhase, pos: [f32; 2], lay: &TouchLayout) -> Vec<String> {
        let mut out = Vec::new();
        match phase {
            TouchPhase::Down => {
                // a stray Down for a finger we already track: restart it
                out.extend(self.lift(id));
                let role = match BUTTONS.iter().position(|b| {
                    let (c, r) = lay.button(b);
                    lay.dist(pos, c) <= r * 1.15
                }) {
                    Some(index) => {
                        let b = &BUTTONS[index];
                        out.push(match b.cmd.strip_prefix('+') {
                            Some(_) => format!("{} {}\n", b.cmd, TOUCH_KEY_BASE + index as i32),
                            None => format!("{}\n", b.cmd),
                        });
                        Role::Button { index, last: pos }
                    }
                    None if pos[0] < 0.5 && !self.fingers.iter().any(|f| matches!(f.role, Role::Move { .. })) => {
                        Role::Move { origin: pos, pos }
                    }
                    None => Role::Look { last: pos },
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
                        self.look[0] += (pos[0] - last[0]) * aspect;
                        self.look[1] += pos[1] - last[1];
                        *last = pos;
                    }
                    Role::Button { index, last } => {
                        if BUTTONS[*index].looks {
                            self.look[0] += (pos[0] - last[0]) * aspect;
                            self.look[1] += pos[1] - last[1];
                        }
                        *last = pos;
                    }
                }
            }
            TouchPhase::Up => out.extend(self.lift(id)),
        }
        out
    }

    fn lift(&mut self, id: u64) -> Option<String> {
        let i = self.fingers.iter().position(|f| f.id == id)?;
        let f = self.fingers.remove(i);
        match f.role {
            Role::Button { index, .. } => {
                let rest = BUTTONS[index].cmd.strip_prefix('+')?;
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

    /// Take the look drag accumulated since the last call (height units).
    pub fn take_look(&mut self) -> [f32; 2] {
        std::mem::take(&mut self.look)
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
        self.fingers.iter().any(|f| matches!(f.role, Role::Button { index: i, .. } if i == index))
    }

    pub fn any_finger(&self) -> bool {
        !self.fingers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const LAY: TouchLayout = TouchLayout { aspect: 16.0 / 9.0, size: 1.0 };

    #[test]
    fn fire_button_holds_and_looks() {
        let mut t = TouchOverlay::default();
        let (c, _) = LAY.button(&BUTTONS[0]);
        assert_eq!(t.event(1, TouchPhase::Down, c, &LAY), vec!["+attack 1000\n"]);
        assert!(t.pressed(0));
        t.event(1, TouchPhase::Move, [c[0] + 0.01, c[1]], &LAY);
        assert!(t.take_look()[0] > 0.0);
        assert_eq!(t.event(1, TouchPhase::Up, c, &LAY), vec!["-attack 1000\n"]);
        assert!(!t.pressed(0));
    }

    #[test]
    fn left_half_is_a_floating_stick() {
        let mut t = TouchOverlay::default();
        t.event(7, TouchPhase::Down, [0.2, 0.6], &LAY);
        t.event(7, TouchPhase::Move, [0.2, 0.6 - 0.06], &LAY);
        let v = t.move_vec(&LAY);
        assert!(v[0].abs() < 1e-4 && (v[1] + 0.5).abs() < 1e-3, "{v:?}");
        // past the rim: full deflection, base dragged along
        t.event(7, TouchPhase::Move, [0.2, 0.1], &LAY);
        assert!((t.move_vec(&LAY)[1] + 1.0).abs() < 1e-4);
        assert!(t.stick().unwrap().0[1] < 0.6);
        t.event(7, TouchPhase::Up, [0.2, 0.1], &LAY);
        assert_eq!(t.move_vec(&LAY), [0.0; 2]);
    }

    #[test]
    fn right_half_looks_and_one_shot_buttons() {
        let mut t = TouchOverlay::default();
        t.event(2, TouchPhase::Down, [0.6, 0.3], &LAY);
        t.event(2, TouchPhase::Move, [0.6, 0.35], &LAY);
        let l = t.take_look();
        assert!(l[0] == 0.0 && (l[1] - 0.05).abs() < 1e-5);
        assert_eq!(t.take_look(), [0.0; 2]);
        let (c, _) = LAY.button(&BUTTONS[2]);
        assert_eq!(t.event(3, TouchPhase::Down, c, &LAY), vec!["impulse 10\n"]);
        assert!(t.event(3, TouchPhase::Up, c, &LAY).is_empty());
    }

    #[test]
    fn release_all_lets_go_of_held_buttons() {
        let mut t = TouchOverlay::default();
        let (c, _) = LAY.button(&BUTTONS[1]);
        t.event(4, TouchPhase::Down, c, &LAY);
        t.event(5, TouchPhase::Down, [0.1, 0.5], &LAY);
        assert_eq!(t.release_all(), vec!["-jump 1001\n"]);
        assert!(!t.any_finger());
    }

    #[test]
    fn buttons_dont_overlap() {
        for lay in [LAY, TouchLayout { aspect: 4.0 / 3.0, size: 1.5 }] {
            for (i, a) in BUTTONS.iter().enumerate() {
                for b in &BUTTONS[i + 1..] {
                    let ((ca, ra), (cb, rb)) = (lay.button(a), lay.button(b));
                    assert!(lay.dist(ca, cb) > (ra + rb) * 1.15, "{} / {}", a.label, b.label);
                }
            }
        }
    }
}
