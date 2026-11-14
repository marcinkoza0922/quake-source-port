//! Synthesised gel sprayer and powerwasher cues (no sampled audio).
//!
//! Gel: [`SPRAY`] wet squirting loop, [`SPLAT`] blob hitting a wall,
//! [`STRETCH`] a footstep on blue gel (rubbery stretching squeak),
//! [`BOING`] bouncing off blue gel, [`ZIP`]..=`ZIP + 2` footsteps on
//! orange gel at increasing speed (a rising, brighter whoosh-zip).
//! Water: [`JET`] / [`FAN`] spray loops, [`SPLASH`] where the jet lands.

use crate::Sfx;
use std::f32::consts::TAU;

pub const SPRAY: usize = 0;
pub const SPLAT: usize = 1;
pub const STRETCH: usize = 2;
pub const BOING: usize = 3;
pub const ZIP: usize = 4;
pub const JET: usize = 7;
pub const FAN: usize = 8;
pub const SPLASH: usize = 9;
pub const COUNT: usize = 10;

/// One-pole filters.
#[derive(Default)]
struct Lp(f32);
impl Lp {
    fn run(&mut self, x: f32, k: f32) -> f32 {
        self.0 += (x - self.0) * k;
        self.0
    }
}

pub fn render(kind: usize, rate: u32) -> Sfx {
    let kind = kind.min(COUNT - 1);
    let duration = match kind {
        SPRAY => 0.6,
        SPLAT => 0.2,
        STRETCH => 0.38,
        BOING => 0.7,
        JET | FAN => 0.8,
        SPLASH => 0.3,
        _ => 0.22,
    };
    let sr = rate as f32;
    let n = (duration * sr) as usize;
    let mut seed = 0x6e1_5eedu32 ^ (kind as u32).wrapping_mul(0x9e37_79b9);
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    // bubbles for the spray: (start, base freq)
    let bubbles: Vec<(f32, f32)> = (0..24).map(|k| (k as f32 * duration / 24.0 + rnd().abs() * 0.02, 380.0 + rnd().abs() * 700.0)).collect();
    let (mut a, mut b, mut c) = (Lp::default(), Lp::default(), Lp::default());
    let (mut ph, mut ph2) = (0.0f32, 0.0f32);
    let mut data = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let u = t / duration;
        let noise = rnd();
        let mut v = match kind {
            SPRAY => {
                // band-passed hiss with wet bubble blips; flat envelope so it loops
                let band = a.run(noise, 0.35) - b.run(noise, 0.04);
                let mut blip = 0.0;
                for &(s, f) in &bubbles {
                    let d = t - s;
                    if (0.0..0.03).contains(&d) {
                        blip += (TAU * f * (1.0 + d * 30.0) * d).sin() * (-d * 120.0).exp();
                    }
                }
                band * 0.9 + blip * 0.35
            }
            SPLAT => {
                ph += TAU * (160.0 * (1.0 - u * 0.6)) / sr;
                let body = ph.sin() * (-t * 30.0).exp();
                let wet = a.run(noise, 0.25) * (-t * 25.0).exp();
                body * 0.7 + wet * 1.4
            }
            STRETCH => {
                // a rubber band pulled: pitch glides up with a wobble and a
                // nasal squeak formant, soft at both ends
                let f = 170.0 + 330.0 * u.powf(0.7) + (TAU * 23.0 * t).sin() * 18.0;
                ph += TAU * f / sr;
                let saw = (ph / TAU).fract() * 2.0 - 1.0;
                let formant = a.run(saw, 0.18) - b.run(saw, 0.02);
                let env = (u * 6.0).min(1.0) * (1.0 - u).powf(1.5);
                (formant * 1.3 + (ph * 2.0).sin() * 0.25 + c.run(noise, 0.1) * 0.15) * env
            }
            BOING => {
                // spring "boing": fast rise, long wobbling decay
                let f = 110.0 + 380.0 * (1.0 - (-t * 18.0).exp()) + (TAU * 11.0 * t).sin() * 60.0 * (-t * 3.0).exp();
                ph += TAU * f / sr;
                ph2 += TAU * f * 2.01 / sr;
                let env = (t / 0.004).min(1.0) * (-t * 4.5).exp();
                (ph.sin() + ph2.sin() * 0.3 + a.run(noise, 0.05) * 0.4 * (-t * 30.0).exp()) * env
            }
            ZIP..=6 => {
                // fast slick zip: a swept noise band and a rising whistle;
                // higher tiers are brighter and higher
                let tier = (kind - ZIP) as f32;
                let k = 0.06 + (0.1 + tier * 0.08) * u;
                let band = a.run(noise, k.min(0.9)) - b.run(noise, 0.02);
                ph += TAU * (600.0 + tier * 300.0 + 1400.0 * u) / sr;
                let env = (u * 10.0).min(1.0) * (1.0 - u).powi(2);
                (band * 1.2 + ph.sin() * (0.15 + tier * 0.07)) * env
            }
            JET | FAN => {
                // pressure jet: bright hiss over a low rumble
                let hi = noise - a.run(noise, if kind == JET { 0.15 } else { 0.3 });
                let low = b.run(c.run(noise, 0.02), 0.05) * 6.0;
                hi * if kind == JET { 0.8 } else { 0.55 } + low * if kind == JET { 0.5 } else { 0.3 }
            }
            _ => {
                // splash
                let wet = a.run(noise, 0.4) - b.run(noise, 0.03);
                ph += TAU * (300.0 - 200.0 * u) / sr;
                (wet * 1.2 + ph.sin() * 0.3 * (-t * 40.0).exp()) * (-t * 12.0).exp()
            }
        };
        if !matches!(kind, SPRAY | JET | FAN) {
            v *= (t / 0.002).min(1.0) * ((duration - t) / 0.02).min(1.0);
        }
        data.push((v.tanh() * 20000.0) as i16);
    }
    let looped = matches!(kind, SPRAY | JET | FAN);
    if looped {
        // crossfade the end into the start so the loop has no click
        let x = (0.05 * sr) as usize;
        for k in 0..x.min(n / 2) {
            let w = k as f32 / x as f32;
            let (s, e) = (data[k] as f32, data[n - x + k] as f32);
            data[k] = (s * w + e * (1.0 - w)) as i16;
        }
        data.truncate(n - x);
    }
    Sfx { name: format!("gel/{kind}"), data, loop_start: looped.then_some(0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_audible_and_loops_loop() {
        for k in 0..COUNT {
            let s = render(k, 22050);
            let peak = s.data.iter().map(|v| v.unsigned_abs()).max().unwrap();
            assert!(peak > 3000, "cue {k} too quiet: {peak}");
            assert_eq!(s.loop_start.is_some(), matches!(k, SPRAY | JET | FAN));
        }
    }
}
