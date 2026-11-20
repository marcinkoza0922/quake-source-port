//! Air acceleration guidance and jump timing for local NetQuake physics.

use crate::draw::{Canvas, WHITE};

const EARLY_WINDOW: u32 = 12;
const GAIN: [f32; 4] = [0.25, 0.77, 0.55, 1.0];
const TARGET: [f32; 4] = [1.0, 0.77, 0.35, 1.0];
const TRACK: [f32; 4] = [0.31, 0.34, 0.38, 0.85];

pub(super) const SIZE: (f32, f32) = (172.0, 31.0);

#[derive(Clone, Copy)]
struct Angles {
    gain_start: f32,
    target: f32,
    gain_end: f32,
    wish: Option<f32>,
}

fn wrap(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

impl Angles {
    fn new(velocity: [f32; 2], wish: [f32; 2], wishspeed: f32, step: f32) -> Option<Self> {
        let speed = velocity[0].hypot(velocity[1]);
        if speed < 1.0 || !speed.is_finite() || !(wishspeed > 0.0 && step > 0.0) {
            return None;
        }
        let cap = wishspeed.min(30.0);
        let angle = |projection: f32| (projection / speed).clamp(-1.0, 1.0).acos().to_degrees();
        let wish = (wish != [0.0; 2])
            .then(|| wrap((wish[1].atan2(wish[0]) - velocity[1].atan2(velocity[0])).to_degrees()));
        // Speed squared changes by 2 * projection * push + push².
        // The maximum is at the clamp boundary, or perpendicular when the
        // available push already reaches the cap from a zero projection.
        Some(Self {
            gain_start: angle(cap),
            target: angle((cap - step).max(0.0)),
            gain_end: angle(-(step * 0.5).min(cap)),
            wish,
        })
    }

    fn correction(self) -> Option<f32> {
        let wish = self.wish?;
        Some(wrap(self.target.copysign(wish) - wish))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Jump {
    Early(u32),
    Takeoff(u32),
}

#[derive(Default)]
pub struct Strafe {
    active: bool,
    on_ground: bool,
    ground_frames: u32,
    air_frames: u32,
    air_press: Option<u32>,
    jump_held: bool,
    last_jump: Option<Jump>,
    angles: Option<Angles>,
}

impl Strafe {
    /// Sample completed physics ticks. Landing starts the ground counter
    /// at zero. A jump on the first possible tick leaves it at zero.
    pub(crate) fn tick(&mut self, on_ground: bool, jump: bool, rise: bool) {
        let pressed = jump && !self.jump_held;
        if !self.active {
            self.active = true;
        } else {
            match (self.on_ground, on_ground) {
                (true, true) => self.ground_frames = self.ground_frames.saturating_add(1),
                (true, false) => {
                    if jump && rise {
                        self.last_jump = Some(Jump::Takeoff(self.ground_frames));
                    }
                    self.ground_frames = 0;
                }
                (false, true) => {
                    // The landing tick is one tick before a jump can take off.
                    let early = if pressed {
                        Some(1)
                    } else {
                        self.air_press
                            .map(|frame| (self.air_frames - frame).saturating_add(2))
                    };
                    if let Some(frames) = early.filter(|&n| n <= EARLY_WINDOW) {
                        self.last_jump = Some(Jump::Early(frames));
                    }
                    self.air_frames = 0;
                    self.air_press = None;
                }
                (false, false) => {
                    self.air_frames = self.air_frames.saturating_add(1);
                    if pressed {
                        self.air_press = Some(self.air_frames);
                    }
                }
            }
        }
        self.on_ground = on_ground;
        self.jump_held = jump;
    }

    pub(crate) fn guide(&mut self, velocity: [f32; 2], wish: [f32; 2], wishspeed: f32, step: f32) {
        self.angles = Angles::new(velocity, wish, wishspeed, step);
    }

    pub(super) fn draw(&self, c: &mut Canvas, (w, h): (f32, f32), bg: bool, preview: bool) {
        if !preview && !self.active {
            return;
        }
        if bg {
            c.fill(0.0, 0.0, w, h, super::draw::BACKING);
        }

        let (angles, on_ground, ground_frames, last_jump) = if preview {
            (
                Angles::new([390.0, 0.0], [80.0, -300.0], 320.0, 44.0),
                false,
                0,
                Some(Jump::Takeoff(0)),
            )
        } else {
            (
                self.angles,
                self.on_ground,
                self.ground_frames,
                self.last_jump,
            )
        };

        let x = |deg: f32| w * 0.5 + deg / 180.0 * (w * 0.5 - 5.0);
        let y = 13.0;
        c.fill(x(-180.0), y, x(180.0) - x(-180.0), 4.0, TRACK);
        for deg in [-180.0, -90.0, 0.0, 90.0, 180.0] {
            c.fill(x(deg) - 0.25, y - 1.0, 0.5, 6.0, WHITE);
        }

        let instruction: String = if let Some(a) = angles {
            for (start, end, target) in [
                (a.gain_start, a.gain_end, a.target),
                (-a.gain_end, -a.gain_start, -a.target),
            ] {
                c.fill(x(start), y, x(end) - x(start), 4.0, GAIN);
                c.fill(x(target) - 0.5, y - 2.0, 1.0, 8.0, TARGET);
            }
            if let Some(wish) = a.wish {
                let mx = x(wish);
                c.fill(mx - 0.5, y - 3.0, 1.0, 10.0, WHITE);
                c.fill(mx - 1.5, y + 5.0, 3.0, 1.0, WHITE);
            }
            match a.correction().map(|d| d.round() as i32) {
                Some(0) => "0°".into(),
                Some(d) if d > 0 => format!("← {d}°"),
                Some(d) => format!("{}° →", -d),
                None => "—".into(),
            }
        } else {
            "—".into()
        };
        c.text_center(w * 0.5, 2.0, &instruction, 7.5, WHITE);

        let ground = if on_ground {
            format!("{}  {}f", qcore::tr!("Ground"), ground_frames.min(9999))
        } else {
            qcore::tr!("Airborne")
        };
        let (timing, color) = match last_jump {
            Some(Jump::Takeoff(0)) => ("0f".into(), GAIN),
            Some(Jump::Takeoff(n)) => (format!("+{}f", n.min(9999)), TARGET),
            Some(Jump::Early(n)) => (format!("−{n}f"), TARGET),
            None => ("—".into(), WHITE),
        };
        let jump = format!("{}  {timing}", qcore::tr!("Jump Timing"));
        c.text_raw(5.0, 22.0, &ground, 7.5, WHITE);
        c.text_right(w - 5.0, 22.0, &jump, 7.5, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_target_maximizes_quake_speed_gain() {
        for (speed, wishspeed, step) in [
            (320.0, 320.0, 44.4),
            (480.0, 100.0, 13.9),
            (7.0, 15.0, 5.0),
            (2.0, 320.0, 44.4),
        ] {
            let a = Angles::new([speed, 0.0], [0.0, -1.0], wishspeed, step).unwrap();
            let gain = |deg: f32| {
                let dot = speed * deg.to_radians().cos();
                let push = (wishspeed.min(30.0) - dot).clamp(0.0, step);
                2.0 * dot * push + push * push
            };
            for deg in 0..=180 {
                assert!(gain(a.target) + 0.001 >= gain(deg as f32));
            }
            assert!((a.correction().unwrap() - (90.0 - a.target)).abs() < 0.001);
        }
    }

    #[test]
    fn reports_early_clean_and_late_hops_without_counting_walk_offs() {
        let mut s = Strafe::default();
        s.tick(false, false, false);
        s.tick(false, true, false);
        s.tick(true, true, false);
        assert_eq!(s.last_jump, Some(Jump::Early(2)));

        s.tick(true, false, false);
        s.tick(false, true, true);
        assert_eq!(s.last_jump, Some(Jump::Takeoff(1)));

        s.tick(true, false, false);
        s.tick(false, true, true);
        assert_eq!(s.last_jump, Some(Jump::Takeoff(0)));

        s.tick(true, false, false);
        s.tick(true, false, false);
        s.tick(false, false, true);
        assert_eq!(s.last_jump, Some(Jump::Takeoff(0)));
        assert_eq!(s.ground_frames, 0);
    }
}
