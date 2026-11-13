//! Original, deterministic boomerang cues, generated at device rate: throw
//! whoosh, stun (wooden bonk then a cartoon "tweety birds" warble), catch
//! clack, and the short swish repeated while it spins through the air.
use crate::Sfx;
use std::f32::consts::TAU;

pub const THROW: usize = 0;
pub const STUN: usize = 1;
pub const CATCH: usize = 2;
pub const SWISH: usize = 3;

pub fn render(kind: usize, rate: u32) -> Sfx {
    let duration = [0.35, 1.6, 0.22, 0.16][kind];
    let sr = rate as f32;
    let n = (duration * sr) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x51ed_2701u32;
    let (mut lp, mut bp_lo, mut bp_hi) = (0.0f32, 0.0f32, 0.0f32);
    // three "birds", each its own pitch, warble rate and chirp rhythm
    let birds = [
        (2600.0, 13.0, 5.5, 0.0),
        (3300.0, 17.0, 4.3, 0.4),
        (2100.0, 11.0, 6.1, 0.9),
    ];
    let mut bird_phase = [0.0f32; 3];
    let mut ph = 0.0f32;
    for i in 0..n {
        let t = i as f32 / sr;
        let u = t / duration;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        let value = match kind {
            THROW | SWISH => {
                // band-passed noise whose centre sweeps (doppler-ish)
                let centre = if kind == THROW {
                    0.05 + 0.25 * (1.0 - u)
                } else {
                    0.12 + 0.1 * (u * TAU).sin().abs()
                };
                bp_lo += (noise - bp_lo) * centre;
                bp_hi += (bp_lo - bp_hi) * centre;
                let env =
                    (u * std::f32::consts::PI)
                        .sin()
                        .powf(if kind == THROW { 0.8 } else { 1.5 });
                (bp_lo - bp_hi) * 3.0 * env
            }
            STUN => {
                // wooden bonk: falling sine knock plus a noise click
                ph += TAU * (420.0 * (-t * 25.0).exp() + 180.0) / sr;
                let bonk = ph.sin() * (-t * 30.0).exp() * 0.9 + noise * (-t * 300.0).exp() * 0.4;
                let mut tweet = 0.0;
                for (k, &(base, warble, rhythm, offset)) in birds.iter().enumerate() {
                    let tt = t - 0.08 - offset * 0.1;
                    if tt <= 0.0 {
                        continue;
                    }
                    // chirps: rapid upward slides, gated by the rhythm
                    let cycle = (tt * rhythm).fract();
                    let gate = if cycle < 0.55 {
                        (cycle / 0.55 * std::f32::consts::PI).sin()
                    } else {
                        0.0
                    };
                    let slide = 1.0 + 0.35 * cycle;
                    let f = base * slide * (1.0 + 0.06 * (TAU * warble * tt).sin());
                    bird_phase[k] += TAU * f / sr;
                    tweet += bird_phase[k].sin() * gate * 0.22;
                }
                let fade = (1.0 - u).powf(0.7);
                bonk + tweet * fade
            }
            _ => {
                // catch: two wooden clacks
                lp += (noise - lp) * 0.3;
                let c1 = (-t * 60.0).exp();
                let c2 = if t > 0.06 {
                    (-(t - 0.06) * 70.0).exp() * 0.7
                } else {
                    0.0
                };
                ph += TAU * 700.0 / sr;
                (lp * 1.2 + ph.sin() * 0.5) * (c1 + c2)
            }
        };
        let attack = (t / 0.003).min(1.0);
        let tail = ((duration - t) / 0.02).min(1.0);
        data.push(((value * attack * tail).tanh() * 22000.0) as i16);
    }
    Sfx {
        name: format!("boomerang/{kind}"),
        data,
        loop_start: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_bounded_and_fade() {
        let s: Vec<_> = (0..4).map(|k| render(k, 22050).data).collect();
        for (i, d) in s.iter().enumerate() {
            assert!(d.iter().any(|x| x.abs() > 1000), "cue {i} silent");
            assert_eq!(d[0], 0);
            assert!(d.last().unwrap().abs() < 200);
            for o in &s[..i] {
                assert_ne!(d, o);
            }
        }
    }
}
