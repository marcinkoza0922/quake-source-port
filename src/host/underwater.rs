//! Underwater view effects: the classic r_waterwarp, or with Enhanced Water
//! its refraction under liquids and droplets after leaving them.

use super::*;
use crate::view::water_screen::Liquid;

impl Host {
    /// r_waterwarp (0 off, 1 software ripple, 2 GLQuake FOV pulse) when the eye
    /// is in water, slime or lava.
    pub(super) fn waterwarp(&self, eye: [f32; 3], time: f64) -> qrender::WaterWarp {
        let t = (time % 1000.0) as f32;
        let mode = self.cv("r_waterwarp").round() as i32;
        // Enhanced Water draws its own underwater effects instead.
        if mode == 0 || self.cv("r_enhancedwater") != 0.0 || !(-5..=-3).contains(&self.contents(eye)) {
            return qrender::WaterWarp::None;
        }
        if mode == 2 { qrender::WaterWarp::Fov(t) } else { qrender::WaterWarp::Screen(t) }
    }

    /// The liquid at the eye and which Enhanced Water screen effects are on
    /// (r_water_refraction, r_water_droplets).
    pub(super) fn water_screen_input(&self, eye: [f32; 3]) -> (Liquid, [bool; 2]) {
        let on = self.cv("r_enhancedwater") != 0.0;
        let fx = [on && self.cv("r_water_refraction") != 0.0, on && self.cv("r_water_droplets") != 0.0];
        (Liquid::from_contents(self.contents(eye)), fx)
    }
}
