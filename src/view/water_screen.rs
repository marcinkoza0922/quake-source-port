//! Enhanced Water screen effects of one view: which liquid the eye is in
//! (refraction) and, for a few seconds after it leaves water or slime,
//! droplets on the screen.

use super::{CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WATER};

/// Seconds the droplets last after leaving a liquid (the shaders fade them by 1.8).
pub const DROPS_LIFE: f64 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Liquid {
    #[default]
    None,
    Water,
    Slime,
    Lava,
}

impl Liquid {
    pub fn from_contents(c: i32) -> Self {
        match c {
            CONTENTS_WATER => Liquid::Water,
            CONTENTS_SLIME => Liquid::Slime,
            CONTENTS_LAVA => Liquid::Lava,
            _ => Liquid::None,
        }
    }

    fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Default)]
pub struct WaterScreen {
    eye: Liquid,
    /// Time the eye left water or slime, that liquid, and the pattern seed.
    exit: Option<(f64, Liquid, u32)>,
    exits: u32,
}

impl WaterScreen {
    /// Track the liquid at the eye at client time `time`; `fx` turns on
    /// refraction and droplets.
    pub fn update(&mut self, eye: Liquid, time: f64, fx: [bool; 2]) -> qrender::WaterScreen {
        if eye != Liquid::None {
            self.exit = None;
        } else if matches!(self.eye, Liquid::Water | Liquid::Slime) {
            self.exits = self.exits.wrapping_add(1);
            let seed = self.exits.wrapping_mul(0x9e37_79b9) ^ (time.to_bits() as u32);
            self.exit = Some((time, self.eye, seed));
        }
        self.eye = eye;
        let mut out = qrender::WaterScreen { refract: if fx[0] { eye.code() } else { 0 }, ..Default::default() };
        if let Some((t0, liquid, seed)) = self.exit {
            let age = time - t0;
            // a new level restarts the client clock
            if !(0.0..DROPS_LIFE).contains(&age) {
                self.exit = None;
            } else if fx[1] {
                out.drops = liquid.code();
                out.drops_age = age as f32;
                out.drops_seed = seed;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: [bool; 2] = [true, true];

    #[test]
    fn refraction_follows_the_eye_and_droplets_follow_surfacing() {
        let mut w = WaterScreen::default();
        let under = w.update(Liquid::Water, 1.0, ON);
        assert_eq!((under.refract, under.drops), (1, 0));
        let out = w.update(Liquid::None, 2.0, ON);
        assert_eq!((out.refract, out.drops, out.drops_age), (0, 1, 0.0));
        let later = w.update(Liquid::None, 3.5, ON);
        assert_eq!((later.drops, later.drops_age, later.drops_seed), (1, 1.5, out.drops_seed));
        assert_eq!(w.update(Liquid::None, 2.0 + DROPS_LIFE, ON).drops, 0);
    }

    #[test]
    fn diving_back_in_clears_the_droplets_and_each_exit_gets_a_new_pattern() {
        let mut w = WaterScreen::default();
        w.update(Liquid::Slime, 0.0, ON);
        let first = w.update(Liquid::None, 1.0, ON);
        assert_eq!(first.drops, 2);
        assert_eq!(w.update(Liquid::Slime, 1.2, ON).drops, 0);
        let second = w.update(Liquid::None, 1.4, ON);
        assert_eq!(second.drops, 2);
        assert_ne!(first.drops_seed, second.drops_seed);
    }

    #[test]
    fn lava_toggles_and_level_changes_leave_no_droplets() {
        let mut w = WaterScreen::default();
        w.update(Liquid::Lava, 0.0, ON);
        assert_eq!(w.update(Liquid::None, 1.0, ON).drops, 0);
        w.update(Liquid::Water, 2.0, [false, false]);
        assert_eq!(w.update(Liquid::None, 3.0, [false, false]), qrender::WaterScreen::default());
        // still tracked while off, shown once turned on
        assert_eq!(w.update(Liquid::None, 3.5, ON).drops, 1);
        // the client clock restarted on a new level
        assert_eq!(w.update(Liquid::None, 0.1, ON).drops, 0);
        assert_eq!(w.update(Liquid::Water, 1.0, [false, true]).refract, 0);
    }
}
