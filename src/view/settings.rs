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
    /// v_idlescale and the v_i* cycles/levels (mods stuff them for drunk or
    /// underwater sway).
    pub idle: Idle,
    /// Per-shift blend scale (contents, damage, bonus, powerup) from
    /// gl_cshiftpercent(_*) and gl_polyblend; 1 = vanilla.
    pub cshift: [f32; 4],
}

/// V_AddIdle's sway: `scale * sin(time * cycle) * level`, per pitch/yaw/roll.
#[derive(Clone, Copy, Debug, Default)]
pub struct Idle {
    pub scale: f32,
    pub cycle: [f32; 3],
    pub level: [f32; 3],
}

impl Idle {
    /// The cvar defaults (scale 0, so no sway).
    #[cfg(test)]
    pub const VANILLA: Idle = Idle { scale: 0.0, cycle: [1.0, 2.0, 0.5], level: [0.3, 0.3, 0.1] };

    /// Angle offsets at client time `t` with `scale` instead of the cvar's.
    pub fn offsets(&self, t: f64, scale: f32) -> [f32; 3] {
        // sin(cl.time * cycle) in double, as C promotes it
        std::array::from_fn(|i| scale * (t * f64::from(self.cycle[i])).sin() as f32 * self.level[i])
    }
}

impl ViewCvars {
    /// a11y_reducemotion: no bob, strafe roll, damage kick, gun kick or speed blur.
    pub fn reduce_motion(mut self, on: bool) -> Self {
        if on {
            // levels too: intermissions always sway at scale 1
            self.idle.level = [0.0; 3];
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
