//! Aiming down the rifle's sight, per local player: the hold/toggle input
//! (the server only sees a held alt-fire), the eased transition, the zoom
//! and look scale that go with it, and the recoil kick of the view.

/// Seconds to raise or lower the sight (the server's `AIM_TIME`).
const TIME: f32 = qserver::addons::rifle::AIM_TIME;
/// Degrees of climb and of sideways wander per shot, and how much is left
/// of them while aiming.
const CLIMB: f32 = 0.46;
const WANDER: f32 = 0.20;
const AIMED: f32 = 0.55;
/// The kick reaches the view at this rate and the climb settles back at
/// `SETTLE` after `REST` seconds without a shot (`RETURN` of it).
const SNAP: f32 = 26.0;
const REST: f32 = 0.12;
const SETTLE: f32 = 5.0;
const RETURN: f32 = 0.7;

/// What decides the sight's state this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    /// The rifle is in hand.
    pub held: bool,
    /// Alive, in the game and not in a menu.
    pub active: bool,
    pub alt: bool,
    /// Alt-fire toggles.
    pub toggle: bool,
    pub reloading: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ads {
    latched: bool,
    prev: bool,
    want: bool,
    t: f32,
    /// Kick not yet applied to the view (pitch, yaw).
    pending: [f32; 2],
    /// Climb to settle back from.
    climb: f32,
    rest: f32,
    shots: u32,
}

impl Ads {
    pub fn update(&mut self, dt: f32, f: Frame) {
        let pressed = f.alt && !self.prev;
        self.prev = f.alt;
        if !f.held || !f.active {
            (self.latched, self.want) = (false, false);
            if !f.held {
                (self.t, self.pending, self.climb) = (0.0, [0.0; 2], 0.0);
            }
        } else {
            self.latched = f.toggle && !f.reloading && (self.latched ^ pressed);
            self.want = !f.reloading && if f.toggle { self.latched } else { f.alt };
        }
        let step = dt / TIME;
        self.t = (self.t + if self.want { step } else { -step }).clamp(0.0, 1.0);
    }

    /// The alt-fire bit to send: the sight's state with the rifle, else `alt`.
    pub fn alt(&self, held: Option<qserver::addons::Toy>, alt: bool) -> bool {
        if held == Some(qserver::addons::Toy::Rifle) { self.want } else { alt }
    }

    /// How far the sight is raised, eased, 0..1.
    pub fn amount(&self) -> f32 {
        self.t * self.t * (3.0 - 2.0 * self.t)
    }

    /// The field of view (degrees) at full magnification `zoom`.
    pub fn fov(&self, base: f32, zoom: f32) -> f32 {
        if self.t <= 0.0 {
            return base;
        }
        let z = 1.0 + (zoom.clamp(1.0, 4.0) - 1.0) * self.amount();
        ((base.to_radians() * 0.5).tan() / z).atan().to_degrees() * 2.0
    }

    /// Look input scale, so that a hand movement covers as much screen.
    pub fn look_scale(&self, zoom: f32) -> f32 {
        1.0 / (1.0 + (zoom.clamp(1.0, 4.0) - 1.0) * self.amount())
    }

    /// One shot's recoil.
    pub fn kick(&mut self, recoil: f32) {
        self.shots = self.shots.wrapping_add(1);
        let k = recoil.clamp(0.0, 3.0) * (1.0 + (AIMED - 1.0) * self.amount());
        // wanders left and right like a held-down carbine: two slow waves
        let s = self.shots as f32;
        let side = (s * 0.9).sin() * 0.6 + (s * 0.37 + 1.0).sin() * 0.4;
        self.pending[0] -= CLIMB * k;
        self.pending[1] += WANDER * k * side;
        self.rest = 0.0;
    }

    /// Applies the recoil to the view angles; `look_down` is the player's
    /// own pitch input, which counts as settling.
    pub fn apply(&mut self, dt: f32, angles: &mut [f32; 3], look_down: f32) {
        let k = 1.0 - (-dt * SNAP).exp();
        let d = self.pending.map(|p| p * k);
        self.pending = [self.pending[0] - d[0], self.pending[1] - d[1]];
        angles[0] += d[0];
        angles[1] += d[1];
        self.climb = (self.climb - d[0] * RETURN - look_down.max(0.0)).max(0.0);
        self.rest += dt;
        if self.rest > REST && self.climb > 0.0 {
            let back = self.climb * (1.0 - (-dt * SETTLE).exp());
            self.climb -= back;
            angles[0] += back;
        }
    }
}

/// `cl`'s rifle is being reloaded.
pub fn reloading(cl: &qclient::ClientState) -> bool {
    let id = qserver::addons::Toy::Rifle.id();
    cl.mags.iter().filter(|m| m.toy == id && m.owner as usize + 1 == cl.viewentity)
        .any(|m| m.parts.iter().any(|p| p.reload_end.is_some_and(|t| t > cl.time)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qserver::addons::Toy;

    const HOLD: Frame = Frame { held: true, active: true, alt: true, toggle: false, reloading: false };

    #[test]
    fn hold_raises_and_lowers_the_sight_with_the_button() {
        let mut a = Ads::default();
        a.update(0.1, HOLD);
        assert!((a.amount() - 0.5).abs() < 0.01 && a.alt(Some(Toy::Rifle), false) && !a.alt(Some(Toy::Railgun), false));
        a.update(0.2, HOLD);
        assert_eq!(a.amount(), 1.0);
        let fov = a.fov(90.0, 1.3);
        assert!((fov - 75.2).abs() < 0.2, "{fov}");
        assert!((a.look_scale(1.3) - 1.0 / 1.3).abs() < 1e-6);
        a.update(0.3, Frame { reloading: true, ..HOLD });
        assert_eq!((a.amount(), a.alt(Some(Toy::Rifle), true)), (0.0, false));
        a.update(0.3, HOLD);
        assert_eq!(a.amount(), 1.0, "held through the reload: up again");
        a.update(0.01, Frame { held: false, ..HOLD });
        assert_eq!((a.amount(), a.fov(90.0, 1.3)), (0.0, 90.0));
    }

    #[test]
    fn toggle_flips_on_presses_and_resets() {
        let t = Frame { toggle: true, ..HOLD };
        let mut a = Ads::default();
        a.update(0.2, t);
        a.update(0.2, Frame { alt: false, ..t });
        assert!(a.alt(Some(Toy::Rifle), false) && a.amount() == 1.0);
        a.update(0.2, t);
        assert!(!a.alt(Some(Toy::Rifle), false));
        for reset in [Frame { active: false, alt: false, ..t }, Frame { held: false, alt: false, ..t }, Frame { reloading: true, alt: false, ..t }] {
            a.update(0.2, Frame { alt: false, ..t });
            a.update(0.2, t);
            assert!(a.alt(Some(Toy::Rifle), false));
            a.update(0.2, reset);
            a.update(0.2, Frame { alt: false, ..t });
            assert!(!a.alt(Some(Toy::Rifle), false), "{reset:?}");
        }
    }

    #[test]
    fn recoil_climbs_less_aimed_and_settles_back() {
        let run = |aim: bool| {
            let mut a = Ads::default();
            a.update(1.0, Frame { alt: aim, ..HOLD });
            let mut v = [0.0; 3];
            for _ in 0..10 {
                a.kick(1.0);
                for _ in 0..8 {
                    a.apply(0.01, &mut v, 0.0);
                }
            }
            let top = v[0];
            for _ in 0..200 {
                a.apply(0.01, &mut v, 0.0);
            }
            (top, v[0])
        };
        let ((hip, after), (aimed, _)) = (run(false), run(true));
        assert!(hip < -2.5 && aimed > hip * 0.65 && aimed < -1.0, "{hip} {aimed}");
        assert!(after > hip * 0.5 && after < 0.0, "{after}");
        let mut none = Ads::default();
        none.kick(0.0);
        let mut v = [0.0; 3];
        none.apply(0.1, &mut v, 0.0);
        assert_eq!(v, [0.0; 3]);
    }
}
