//! Procedural surface-aware footstep and landing cues.
//! Synthesized offline into mono `Sfx` clips so footsteps work across every
//! Quake mod and map pack with zero external audio files.

use std::f32::consts::TAU;

use crate::Sfx;

/// Floor surface kind, matching `hitfx`'s BSP texture material classifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FootstepSurface {
    Stone,
    Metal,
    Wood,
    Dirt,
    Water,
    Slime,
    Lava,
}

impl FootstepSurface {
    pub const ALL: [Self; 7] = [
        Self::Stone,
        Self::Metal,
        Self::Wood,
        Self::Dirt,
        Self::Water,
        Self::Slime,
        Self::Lava,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Stone => "stone",
            Self::Metal => "metal",
            Self::Wood => "wood",
            Self::Dirt => "dirt",
            Self::Water => "water",
            Self::Slime => "slime",
            Self::Lava => "lava",
        }
    }
}

/// Number of distinct alternating footstep variations per surface.
pub const VARIANTS: usize = 4;

struct Rng(u32);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.0 >> 8) as f32 / 8_388_608.0) - 1.0
    }
}

/// Render a single footstep variation for `surface`.
pub fn render_step(surface: FootstepSurface, variant: usize, rate: u32) -> Sfx {
    let v = variant % VARIANTS;
    let data = synth(surface, v, false, rate.max(8000));
    Sfx {
        name: format!("footsteps/{}_{}", surface.name(), v),
        data,
        loop_start: None,
    }
}

/// Render a heavier landing thud for `surface`.
pub fn render_land(surface: FootstepSurface, rate: u32) -> Sfx {
    let data = synth(surface, 0, true, rate.max(8000));
    Sfx {
        name: format!("footsteps/{}_land", surface.name()),
        data,
        loop_start: None,
    }
}

fn synth(surface: FootstepSurface, variant: usize, land: bool, rate: u32) -> Vec<i16> {
    let sr = rate as f32;
    let dur = match (surface, land) {
        (FootstepSurface::Water | FootstepSurface::Slime, true) => 0.24,
        (FootstepSurface::Water | FootstepSurface::Slime, false) => 0.16,
        (_, true) => 0.17,
        (_, false) => 0.11,
    };
    let n = (dur * sr) as usize;
    let seed = (surface.index() as u32 + 1)
        .wrapping_mul(0x9E37_79B1)
        ^ (variant as u32).wrapping_mul(0x85EB_CA77)
        ^ if land { 0xC2B2_AE3D } else { 0 };
    let mut rng = Rng(seed | 1);
    // Subtle pitch/timbre jitter per variant so left/right steps feel organic.
    let pitch = 0.92 + (variant as f32) * 0.055 - if land { 0.18 } else { 0.0 };
    let gain = if land { 0.88 } else { 0.62 };

    let mut out = vec![0.0f32; n];
    let mut lp1 = 0.0f32;
    let mut lp2 = 0.0f32;
    let mut hp = 0.0f32;
    let mut prev_x = 0.0f32;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let noise = rng.next_f32();
        let attack = (t / 0.004).min(1.0);

        let y = match surface {
            FootstepSurface::Stone => {
                // Crisp gritty boot heel tap + short low stone thump.
                let heel_env = attack * (-t * 42.0).exp();
                let grit_env = ((t - 0.006).max(0.0) / 0.008).min(1.0) * (-t * 28.0).exp();
                lp1 += (1800.0 * pitch / sr).min(0.6) * (noise - lp1);
                hp = 0.92 * (hp + noise - prev_x);
                prev_x = noise;
                let thump = (TAU * (105.0 * pitch * (-t * 10.0).exp().max(0.5)) * t).sin() * heel_env;
                let scuff = (lp1 * 0.7 + hp * 0.35) * grit_env;
                thump * 0.55 + scuff * 0.65
            }
            FootstepSurface::Metal => {
                // Tight metallic ring modes + hard click transient.
                let click = noise * attack * (-t * 95.0).exp();
                let ring_env = attack * (-t * if land { 18.0 } else { 26.0 }).exp();
                let f1 = 420.0 * pitch;
                let f2 = 915.0 * pitch;
                let f3 = 1680.0 * pitch;
                let ring = (TAU * f1 * t).sin() * 0.45
                    + (TAU * f2 * t).sin() * 0.35
                    + (TAU * f3 * t).sin() * 0.22;
                let thud = (TAU * 130.0 * pitch * t).sin() * attack * (-t * 36.0).exp();
                ring * ring_env * 0.42 + click * 0.55 + thud * 0.45
            }
            FootstepSurface::Wood => {
                // Hollow warm board knock + dry scuff.
                let knock_env = attack * (-t * 30.0).exp();
                let f1 = 175.0 * pitch;
                let f2 = 340.0 * pitch;
                let knock = (TAU * f1 * t).sin() * 0.65 + (TAU * f2 * t).sin() * 0.30;
                lp1 += (950.0 * pitch / sr).min(0.5) * (noise - lp1);
                let scuff = lp1 * attack * (-t * 38.0).exp();
                knock * knock_env * 0.65 + scuff * 0.45
            }
            FootstepSurface::Dirt => {
                // Soft granular crunch + damped earthy thud.
                let env = (t / 0.010).min(1.0) * (-t * 22.0).exp();
                lp1 += (1100.0 * pitch / sr).min(0.5) * (noise - lp1);
                lp2 += (320.0 * pitch / sr).min(0.3) * (lp1 - lp2);
                let grain = (lp1 - lp2) * (0.65 + 0.35 * (TAU * 55.0 * t).sin());
                let thud = (TAU * 82.0 * pitch * t).sin() * attack * (-t * 32.0).exp();
                grain * env * 0.75 + thud * 0.42
            }
            FootstepSurface::Water => {
                // Splashy droplet scatter + wet low slosh.
                let splash_env = (t / 0.012).min(1.0) * (-t * if land { 11.0 } else { 17.0 }).exp();
                lp1 += (3400.0 * pitch / sr).min(0.75) * (noise - lp1);
                hp = 0.85 * (hp + lp1 - prev_x);
                prev_x = lp1;
                let slosh = (TAU * (260.0 + 180.0 * (-t * 14.0).exp()) * pitch * t).sin()
                    * attack
                    * (-t * 20.0).exp();
                hp * splash_env * 0.75 + slosh * 0.30
            }
            FootstepSurface::Slime => {
                // Thick viscous squelch with downward pitch sweep.
                let env = (t / 0.014).min(1.0) * (-t * 15.0).exp();
                let f = (460.0 * (-t * 9.0).exp() + 110.0) * pitch;
                lp1 += (1200.0 / sr).min(0.5) * (noise - lp1);
                let squelch = (TAU * f * t).sin() * (0.6 + 0.4 * lp1);
                squelch * env * 0.65 + lp1 * env * 0.35
            }
            FootstepSurface::Lava => {
                // Hot sizzle crackle + heavy cinder thump.
                let env = attack * (-t * 22.0).exp();
                hp = 0.94 * (hp + noise - prev_x);
                prev_x = noise;
                let crackle = if noise.abs() > 0.72 { hp * 1.4 } else { hp * 0.35 };
                let thud = (TAU * 95.0 * pitch * t).sin() * attack * (-t * 35.0).exp();
                crackle * env * 0.65 + thud * 0.50
            }
        };

        // Smooth fade-out tail on last 10 ms to guarantee zero clicks.
        let tail = ((dur - t) / 0.010).clamp(0.0, 1.0);
        *sample = y * gain * tail;
    }

    // Normalize softly to a consistent comfortable footstep level.
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max).max(1e-4);
    let target = if land { 19000.0 } else { 13500.0 };
    let scale = (target / peak).min(24000.0);
    out.into_iter()
        .map(|s| (s * scale).clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_surfaces_and_variants_non_empty() {
        for &surf in &FootstepSurface::ALL {
            for v in 0..VARIANTS {
                let s = render_step(surf, v, 48000);
                assert!(!s.data.is_empty());
                assert!(s.data.iter().any(|&x| x.abs() > 1000), "{:?} v{}", surf, v);
                assert!(s.name.starts_with("footsteps/"));
            }
            let l = render_land(surf, 48000);
            assert!(!l.data.is_empty());
            assert!(l.data.iter().any(|&x| x.abs() > 2000), "{:?} land", surf);
        }
    }
}
