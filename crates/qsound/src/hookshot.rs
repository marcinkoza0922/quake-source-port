//! Synthesised hookshot cues (no sampled audio): 0 fire (clank, then the
//! chain rattling out), 1 latch (heavy metal thunk), 2 retract (rattle
//! winding back in), 3 miss (light clink off an unhookable surface).

use crate::Sfx;
use std::f32::consts::TAU;

pub fn render(kind: usize, rate: u32) -> Sfx {
    let duration = [0.55, 0.3, 0.5, 0.22][kind.min(3)];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x51ed_270bu32 ^ kind as u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32
    };
    // chain links: short ringing clicks at jittered intervals
    let mut clicks: Vec<(f32, f32, f32)> = Vec::new(); // (time, freq, gain)
    let (from, to, rate_hz) = match kind {
        0 => (0.04, duration, 38.0),
        2 => (0.0, duration, 30.0),
        _ => (1.0, 0.0, 1.0),
    };
    let mut t = from;
    while t < to {
        clicks.push((t, 2200.0 + rnd() * 2800.0, 0.35 + rnd() * 0.4));
        t += (0.6 + rnd() * 0.8) / rate_hz;
    }
    let (mut low, mut phase) = (0.0f32, 0.0f32);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let u = t / duration;
        let noise = rnd() * 2.0 - 1.0;
        low += (noise - low) * 0.08;
        let mut v = 0.0;
        for &(ct, f, g) in &clicks {
            let d = t - ct;
            if (0.0..0.03).contains(&d) {
                v += (TAU * f * d).sin() * (-d * 180.0).exp() * g + noise * (-d * 600.0).exp() * 0.3 * g;
            }
        }
        let env = match kind {
            0 => 1.0 - u * 0.6,
            2 => 0.5 + u * 0.4,
            _ => 1.0,
        };
        v *= env;
        match kind {
            0 => {
                // spring-loaded launch clank
                phase += TAU * (180.0 + 400.0 * (-t * 25.0).exp()) / rate as f32;
                v += (phase.sin() + 0.4 * (phase * 2.71).sin()) * (-t * 30.0).exp() * 0.8;
            }
            1 => {
                phase += TAU * (95.0 + 60.0 * (-t * 40.0).exp()) / rate as f32;
                v += phase.sin() * (-t * 14.0).exp() * 0.9 + low * 2.5 * (-t * 25.0).exp()
                    + (phase * 7.3).sin() * (-t * 22.0).exp() * 0.25;
            }
            3 => {
                phase += TAU * 3100.0 / rate as f32;
                v += (phase.sin() + 0.5 * (phase * 1.47).sin()) * (-t * 28.0).exp() * 0.5 + noise * (-t * 90.0).exp() * 0.4;
            }
            _ => {}
        }
        let attack = (t / 0.002).min(1.0);
        let tail = ((duration - t) / 0.03).min(1.0);
        data.push(((v * attack * tail).tanh() * 22000.0) as i16);
    }
    Sfx { name: format!("hookshot/{kind}"), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_audible_bounded_and_end_silent() {
        for k in 0..4 {
            let s = render(k, 22050).data;
            assert!(s.iter().any(|x| x.abs() > 2000), "{k}");
            assert_eq!(s[0], 0);
            assert!(s.last().unwrap().abs() < 1500);
        }
    }
}
