//! Per-frame view settings and accessibility overrides.

use super::weapon::Alignment;

/// View cvars read once per frame.
#[derive(Clone, Copy, Debug)]
pub struct ViewCvars {
    pub bob: f32,
    pub bobcycle: f32,
    pub bobup: f32,
    pub rollangle: f32,
    pub rollspeed: f32,
    pub kicktime: f32,
    pub kickroll: f32,
    pub kickpitch: f32,
    pub fov: f32,
    pub gunkick: bool,
    /// High-speed peripheral blur (r_speedblur).
    pub speed_blur: bool,
    pub viewmodel: Alignment,
    /// r_lerpmodels: blend alias frames.
    pub lerp_models: bool,
    /// r_lerpmove: smooth MOVESTEP (monster) movement over 0.1s.
    pub lerp_move: bool,
    /// v_flashscale: strength of damage, pickup and powerup screen flashes.
    pub flash: f32,
}

impl ViewCvars {
    /// a11y_reducemotion: no bob, strafe roll, damage kick, gun kick or speed blur.
    pub fn reduce_motion(mut self, on: bool) -> Self {
        if on {
            self.bob = 0.0;
            self.rollangle = 0.0;
            self.kickroll = 0.0;
            self.kickpitch = 0.0;
            self.gunkick = false;
            self.speed_blur = false;
        }
        self
    }

    /// A third-person camera doesn't bob or strafe-roll (the body it
    /// follows doesn't), and there's no viewmodel to show.
    pub fn third_person(mut self, on: bool) -> Self {
        if on {
            self.bob = 0.0;
            self.rollangle = 0.0;
            self.viewmodel = super::weapon::Alignment::Hidden;
        }
        self
    }
}
