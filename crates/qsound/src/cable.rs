//! Synthesised male-to-male cable cues (no sampled audio): 0 plug in (a
//! solid clunk, then mains hum swelling with crackle), 1 yank out (rubbery
//! pop and a spark), 2 miss (dull clunk with a small spark off a dead
//! surface). The bolt itself uses the stock lightning gun sounds.

use crate::Sfx;
use std::f32::consts::TAU;

pub fn render(kind: usize, rate: u32) -> Sfx {
    let kind = kind.min(2);
    let duration = [0.7, 0.35, 0.25][kind];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x9e37_79b9u32 ^ (kind as u32 * 7919);
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    let (mut low, mut high, mut phase, mut hum) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut crackle = 0.0f32;
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let noise = rnd() * 2.0 - 1.0;
        low += (noise - low) * 0.06;
        high = noise - high * 0.5;
        // sparse crackle: random pops that decay fast
        if rnd() < [0.004, 0.006, 0.003][kind] * (-t * 6.0).exp() {
            crackle = 1.0;
        }
        crackle *= 0.985;
        let v = match kind {
            0 => {
                // clunk of the plug seating, then 100/150 Hz hum swelling in
                phase += TAU * (140.0 + 220.0 * (-t * 45.0).exp()) / rate as f32;
                hum += TAU * 100.0 / rate as f32;
                let clunk = phase.sin() * (-t * 28.0).exp() * 0.9 + low * 3.0 * (-t * 40.0).exp();
                let swell = (t / 0.25).min(1.0) * (1.0 - t / duration).max(0.0);
                let buzz = (hum.sin() + 0.6 * (hum * 1.5).sin() + 0.3 * (hum * 3.0).sin().signum()) * 0.28 * swell;
                clunk + buzz + high * crackle * 0.5
            }
            1 => {
                // rubber pop: a fast downward chirp plus a burst of sparks
                phase += TAU * (900.0 * (-t * 30.0).exp() + 120.0) / rate as f32;
                phase.sin() * (-t * 22.0).exp() * 0.8 + high * (-t * 25.0).exp() * 0.45 + high * crackle * 0.6
            }
            _ => {
                phase += TAU * (210.0 + 80.0 * (-t * 50.0).exp()) / rate as f32;
                phase.sin() * (-t * 30.0).exp() * 0.8 + low * 2.0 * (-t * 50.0).exp() + high * (-((t - 0.03).max(0.0)) * 60.0).exp() * 0.25
            }
        };
        let attack = (t / 0.002).min(1.0);
        let tail = ((duration - t) / 0.04).min(1.0);
        data.push(((v * attack * tail).tanh() * 20000.0) as i16);
    }
    Sfx { name: format!("cable/{kind}"), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_audible_bounded_and_end_silent() {
        for k in 0..3 {
            let s = render(k, 22050).data;
            assert!(s.iter().any(|x| x.abs() > 2000), "{k}");
            assert_eq!(s[0], 0);
            assert!(s.last().unwrap().abs() < 1500, "{k}");
        }
    }
}
