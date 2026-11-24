//! Original synthesized assault rifle cues (no recorded or extracted audio).
use crate::Sfx;
use std::f32::consts::TAU;

pub const SHOT: usize = 0;
pub const DRY: usize = 1;
pub const MAG_OUT: usize = 2;
pub const MAG_IN: usize = 3;
pub const BOLT: usize = 4;
pub const AIM_IN: usize = 5;
pub const AIM_OUT: usize = 6;
pub const CUES: usize = 7;

/// A struck piece of metal: a few inharmonic partials.
fn clank(t: f32, f: f32, decay: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    [(1.0, 1.0), (2.76, 0.6), (5.4, 0.35), (8.9, 0.2)].iter().map(|&(k, a)| (TAU * f * k * t).sin() * a * (-t * decay * (0.7 + 0.3 * k)).exp()).sum::<f32>() * 0.5
}

/// Noise burst at `t` since its start.
fn burst(noise: f32, t: f32, decay: f32) -> f32 {
    if t < 0.0 { 0.0 } else { noise * (-t * decay).exp() }
}

pub fn render(kind: usize, rate: u32) -> Sfx {
    let duration = [0.42, 0.09, 0.34, 0.36, 0.40, 0.22, 0.20][kind];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x2f6e_2b15u32 ^ (kind as u32).wrapping_mul(0x9e37_79b9);
    let (mut low, mut mid, mut band) = (0.0f32, 0.0f32, 0.0f32);
    let mut ph = 0.0f32;
    // one-pole coefficients that sound the same at any device rate
    let pole = |hz: f32| 1.0 - (-TAU * hz / rate as f32).exp();
    let (k_low, k_mid, k_band) = (pole(420.0), pole(2600.0), pole(900.0));
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let u = t / duration;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        low += (noise - low) * k_low;
        mid += (noise - mid) * k_mid;
        band += (mid - band) * k_band;
        let value = match kind {
            SHOT => {
                // muzzle crack, chest thump, the bolt's clatter, a short tail
                ph += TAU * (62.0 + 210.0 * (-t * 55.0).exp()) / rate as f32;
                let crack = mid * 2.6 * (-t * 60.0).exp() + noise * 0.5 * (-t * 160.0).exp();
                let thump = ph.sin() * 1.25 * (-t * 21.0).exp();
                let tail = low * 2.6 * (-t * 11.0).exp() + (mid - band) * 0.5 * (-t * 16.0).exp();
                let bolt = clank(t - 0.028, 1900.0, 70.0) * 0.16 + clank(t - 0.055, 1500.0, 80.0) * 0.12;
                crack + thump + tail + bolt
            }
            DRY => clank(t, 2300.0, 90.0) * 0.8 + burst(noise, t, 220.0) * 0.4,
            MAG_OUT => {
                let catch = clank(t, 1700.0, 85.0) * 0.7 + burst(noise, t, 260.0) * 0.4;
                let slide = (mid - band) * 1.3 * ((t - 0.04) / 0.05).clamp(0.0, 1.0) * ((0.24 - t) / 0.12).clamp(0.0, 1.0);
                catch + slide + clank(t - 0.20, 900.0, 60.0) * 0.3
            }
            MAG_IN => {
                let slide = (mid - band) * 1.1 * (t / 0.07).min(1.0) * ((0.13 - t) / 0.03).clamp(0.0, 1.0);
                let seat = clank(t - 0.13, 780.0, 45.0) * 0.9 + burst(low, t - 0.13, 40.0) * 3.0;
                slide + seat + clank(t - 0.165, 2100.0, 95.0) * 0.5 + burst(noise, t - 0.165, 240.0) * 0.3
            }
            BOLT => {
                let back = (mid - band) * 1.4 * (t / 0.05).min(1.0) * ((0.14 - t) / 0.05).clamp(0.0, 1.0);
                let stop = clank(t - 0.12, 1250.0, 60.0) * 0.6;
                let home = clank(t - 0.235, 950.0, 38.0) * 1.0 + burst(noise, t - 0.235, 120.0) * 0.6;
                back + stop + home
            }
            // cloth and sling: soft filtered noise, rising in or falling out
            _ => {
                let env = (u * std::f32::consts::PI).sin().max(0.0).powf(if kind == AIM_IN { 0.8 } else { 1.4 });
                let rattle = clank(t - if kind == AIM_IN { 0.15 } else { 0.03 }, 2600.0, 120.0) * 0.12;
                (band * 1.1 + (mid - band) * 0.35) * env * 0.8 + rattle
            }
        };
        let attack = (t / 0.0015).min(1.0);
        let tail = ((duration - t) / 0.02).min(1.0);
        let gain = [1.0, 0.55, 0.7, 0.8, 0.8, 0.5, 0.45][kind];
        data.push(((value * gain * attack * tail).tanh() * 24000.0) as i16);
    }
    Sfx { name: format!("rifle/{kind}"), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_and_bounded() {
        let s: Vec<_> = (0..CUES).map(|k| render(k, 22050)).collect();
        for (i, c) in s.iter().enumerate() {
            assert!(c.data.iter().any(|x| x.abs() > 1000), "{i}");
            assert_eq!(c.data[0], 0);
            assert!(c.data.last().unwrap().abs() < 250, "{i}");
            assert!(s[..i].iter().all(|o| o.data != c.data));
        }
        // the shot starts with a bang
        let peak = |r: std::ops::Range<usize>| s[SHOT].data[r].iter().map(|x| x.unsigned_abs()).max().unwrap();
        assert!(peak(0..1000) > 15000 && peak(8000..9000) < peak(0..1000) / 3);
    }
}
