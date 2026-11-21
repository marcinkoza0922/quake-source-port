//! On-screen touch controls: a floating move stick, drag-to-look, and a
//! dedicated finger gesture for the weapon wheel.
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
    Cancel,
}

/// An on-screen button. `cmd` starting with `+` is held (its `-` runs on
/// release); anything else runs once on press.
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
pub const BUTTONS: [TouchButton; 4] = [
    TouchButton { label: "FIRE", cmd: Some("+attack"), at: [0.17, 0.42], radius: 0.10, looks: true },
    TouchButton { label: "JUMP", cmd: Some("+jump"), at: [0.35, 0.25], radius: 0.075, looks: false },
    TouchButton { label: "WEAPON", cmd: None, at: [0.37, 0.49], radius: 0.075, looks: false },
    TouchButton { label: "MENU", cmd: Some("togglemenu"), at: [-0.085, -0.085], radius: 0.06, looks: false },
];
const WHEEL: usize = 2;
pub const PAUSE_BUTTON: usize = 3;
const HIT_SCALE: f32 = 1.15;

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
    Wheel { origin: [f32; 2], pos: [f32; 2] },
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
    pub left_handed: bool,
    /// Left, top, right and bottom insets, normalised to the window.
    pub insets: [f32; 4],
}

impl TouchLayout {
    pub fn new(aspect: f32, size: f32, left_handed: bool, insets: [f32; 4]) -> Self {
        Self { aspect: aspect.max(0.1), size, left_handed, insets }
    }

    fn scale(&self) -> f32 {
        let [l, t, r, b] = self.insets;
        let half_width = (1.0 - l - r) * self.aspect * 0.5;
        let height = 1.0 - t - b;
        // Keep the full hit areas within the safe rectangle and on their own
        // side even at maximum button size on narrow displays.
        self.size.clamp(0.5, 2.0).min(half_width / 0.48).min(height / 0.60).max(0.01)
    }

    fn mid_x(&self) -> f32 {
        (self.insets[0] + 1.0 - self.insets[2]) * 0.5
    }

    fn move_side(&self, x: f32) -> bool {
        if self.left_handed { x >= self.mid_x() } else { x < self.mid_x() }
    }

    fn contains(&self, [x, y]: [f32; 2]) -> bool {
        let [l, t, r, b] = self.insets;
        (l..=1.0 - r).contains(&x) && (t..=1.0 - b).contains(&y)
    }

    pub fn hits(&self, button: &TouchButton, pos: [f32; 2]) -> bool {
        let (center, radius) = self.button(button);
        self.dist(pos, center) <= radius * HIT_SCALE
    }

    /// Button centre in window-normalised coordinates, and radius in
    /// height units.
    pub fn button(&self, b: &TouchButton) -> ([f32; 2], f32) {
        let s = self.scale();
        let [l, t, r, bot] = self.insets;
        let at_x = if self.left_handed { -b.at[0] } else { b.at[0] };
        let x = if at_x >= 0.0 { 1.0 - r - at_x * s / self.aspect } else { l - at_x * s / self.aspect };
        let y = if b.at[1] >= 0.0 { 1.0 - bot - b.at[1] * s } else { t - b.at[1] * s };
        ([x, y], b.radius * s)
    }

    pub fn stick_radius(&self) -> f32 {
        STICK_RADIUS * self.scale()
    }

    pub fn resting_stick(&self) -> [f32; 2] {
        let [l, t, r, b] = self.insets;
        let dx = (self.stick_radius() + 0.08 * self.scale()) / self.aspect;
        let x = if self.left_handed { 1.0 - r - dx } else { l + dx };
        [x, t + (1.0 - t - b) * 0.58]
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
    wheel_release: Option<([f32; 2], [f32; 2])>,
}

/// The wheel tracks one finger relative to where it touched down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelGesture {
    Held([f32; 2]),
    Released([f32; 2]),
}

impl TouchOverlay {
    /// Feed one finger event; returns console commands to run.
    pub fn event(&mut self, id: u64, phase: TouchPhase, pos: [f32; 2], lay: &TouchLayout) -> Vec<String> {
        let mut out = Vec::new();
        match phase {
            TouchPhase::Down => {
                // a stray Down for a finger we already track: restart it
                out.extend(self.lift(id));
                if !lay.contains(pos) {
                    return out;
                }
                let role = match BUTTONS.iter().position(|b| lay.hits(b, pos)) {
                    Some(index) if self.pressed(index) => return out,
                    Some(WHEEL) => {
                        self.wheel_release = None;
                        Role::Wheel { origin: pos, pos }
                    }
                    Some(index) => {
                        let cmd = BUTTONS[index].cmd.expect("command button");
                        out.push(if cmd.starts_with('+') {
                            format!("{cmd} {}\n", TOUCH_KEY_BASE + index as i32)
                        } else {
                            format!("{cmd}\n")
                        });
                        Role::Button { index, last: pos }
                    }
                    None if lay.move_side(pos[0]) && self.stick().is_none() => {
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
                    Role::Wheel { pos: p, .. } => *p = pos,
                }
            }
            TouchPhase::Up => {
                if let Some(Role::Wheel { origin, .. }) = self.fingers.iter().find(|f| f.id == id).map(|f| f.role) {
                    self.wheel_release = Some((origin, pos));
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

    pub fn has_look(&self) -> bool {
        self.look != [0.0; 2]
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
        if let Some((origin, pos)) = self.wheel_release.take() {
            return Some(WheelGesture::Released(direction(origin, pos)));
        }
        self.fingers.iter().find_map(|f| match f.role {
            Role::Wheel { origin, pos } => Some(WheelGesture::Held(direction(origin, pos))),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    const LAY: TouchLayout = TouchLayout { aspect: 16.0 / 9.0, size: 1.0, left_handed: false, insets: [0.0; 4] };

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
    fn right_half_looks_and_pause_is_one_shot() {
        let mut t = TouchOverlay::default();
        t.event(2, TouchPhase::Down, [0.6, 0.3], &LAY);
        t.event(2, TouchPhase::Move, [0.6, 0.35], &LAY);
        let l = t.take_look();
        assert!(l[0] == 0.0 && (l[1] - 0.05).abs() < 1e-5);
        assert_eq!(t.take_look(), [0.0; 2]);
        let (c, _) = LAY.button(&BUTTONS[PAUSE_BUTTON]);
        assert_eq!(t.event(3, TouchPhase::Down, c, &LAY), vec!["togglemenu\n"]);
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
        for lay in [LAY, TouchLayout::new(4.0 / 3.0, 2.0, true, [0.12, 0.06, 0.04, 0.07])] {
            for (i, a) in BUTTONS.iter().enumerate() {
                let ([x, y], r) = lay.button(a);
                let [l, t, right, bot] = lay.insets;
                let r = r * HIT_SCALE;
                assert!(x - r / lay.aspect >= l && x + r / lay.aspect <= 1.0 - right);
                assert!(y - r >= t && y + r <= 1.0 - bot);
                for b in &BUTTONS[i + 1..] {
                    let ((ca, ra), (cb, rb)) = (lay.button(a), lay.button(b));
                    assert!(lay.dist(ca, cb) > (ra + rb) * 1.15, "{} / {}", a.label, b.label);
                }
            }
        }
    }

    #[test]
    fn left_handed_layout_swaps_stick_and_look_zones() {
        let lay = TouchLayout { left_handed: true, ..LAY };
        let mut t = TouchOverlay::default();
        t.event(1, TouchPhase::Down, [0.15, 0.5], &lay);
        t.event(1, TouchPhase::Move, [0.2, 0.5], &lay);
        t.event(2, TouchPhase::Down, [0.8, 0.6], &lay);
        t.event(2, TouchPhase::Move, [0.8, 0.55], &lay);
        assert!(t.take_look()[0] > 0.0);
        assert!(t.move_vec(&lay)[1] < 0.0);
    }

    #[test]
    fn wheel_keeps_final_swipe_and_cancels_without_selection() {
        let mut t = TouchOverlay::default();
        let (p, _) = LAY.button(&BUTTONS[WHEEL]);
        t.event(1, TouchPhase::Down, p, &LAY);
        assert_eq!(t.take_wheel(&LAY), Some(WheelGesture::Held([0.0; 2])));
        let end = [p[0], p[1] - 0.1];
        t.event(1, TouchPhase::Move, end, &LAY);
        assert_eq!(t.take_look(), [0.0; 2]);
        t.event(1, TouchPhase::Up, end, &LAY);
        assert_eq!(t.take_wheel(&LAY), Some(WheelGesture::Released([0.0, -1.0])));
        assert_eq!(t.take_wheel(&LAY), None);
        t.event(2, TouchPhase::Down, p, &LAY);
        t.event(2, TouchPhase::Cancel, end, &LAY);
        assert_eq!(t.take_wheel(&LAY), None);
    }
}
