//! Original synthesized akimbo pistol cues (no recorded or extracted audio).
use crate::Sfx;
use std::f32::consts::TAU;

/// Shots of the left and the right pistol (slightly different pitch).
pub const SHOT: usize = 0;
pub const DRY: usize = 2;
pub const MAG_OUT: usize = 3;
pub const MAG_IN: usize = 4;
pub const SLIDE: usize = 5;
pub const COUNT: usize = 6;

/// A short struck-metal click at `at` seconds.
fn click(t: f32, at: f32, pitch: f32, decay: f32, noise: f32) -> f32 {
    let u = t - at;
    if u < 0.0 {
        return 0.0;
    }
    let ring = (TAU * pitch * u).sin() + 0.6 * (TAU * pitch * 1.73 * u).sin() + 0.4 * (TAU * pitch * 2.61 * u).sin();
    (ring * 0.35 + noise * 0.8) * (-u * decay).exp()
}

pub fn render(kind: usize, rate: u32) -> Sfx {
    let kind = kind.min(COUNT - 1);
    let duration = [0.42, 0.42, 0.08, 0.22, 0.26, 0.34][kind];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x2f6e_2b1du32 + kind as u32 * 7919;
    let (mut low, mut mid, mut phase) = (0.0f32, 0.0f32, 0.0f32);
    // one-pole coefficients independent of the device rate
    let pole = |hz: f32| 1.0 - (-TAU * hz / rate as f32).exp();
    let (k_low, k_mid) = (pole(500.0), pole(2600.0));
    for i in 0..n {
        let t = i as f32 / rate as f32;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        low += (noise - low) * k_low;
        mid += (noise - mid) * k_mid;
        let value = match kind {
            0 | 1 => {
                let p = if kind == 0 { 1.0 } else { 1.09 };
                // crack, punchy falling thump, rolling tail and the slide's clack
                let crack = (noise * 0.7 + (noise - mid) * 0.5) * (-t * 85.0 * p).exp() * 1.6;
                phase += TAU * p * (62.0 + 260.0 * (-t * 45.0).exp()) / rate as f32;
                let thump = phase.sin() * (-t * 17.0).exp() * 1.3;
                let body = mid * 2.2 * (-t * 26.0).exp();
                let tail = low * 2.4 * (-t * 9.0).exp() * (1.0 - (-t * 60.0).exp());
                crack + thump + body + tail + click(t, 0.055, 2900.0 * p, 95.0, noise) * 0.22
            }
            DRY => click(t, 0.0, 2300.0, 130.0, noise) * 0.8 + click(t, 0.014, 3400.0, 160.0, noise) * 0.4,
            MAG_OUT => {
                // the catch, then the magazine scraping out
                let scrape = (noise - mid) * 0.55 * ((t - 0.03) * 40.0).clamp(0.0, 1.0) * (-(t - 0.03).max(0.0) * 16.0).exp();
                click(t, 0.0, 1900.0, 90.0, noise) * 0.7 + scrape
            }
            MAG_IN => {
                // scrape in, then seated with a thud
                let scrape = (noise - mid) * 0.35 * (t * 30.0).min(1.0) * if t < 0.08 { 1.0 } else { (-(t - 0.08) * 60.0).exp() };
                let thud = if t > 0.085 { (TAU * 130.0 * (t - 0.085)).sin() * (-(t - 0.085) * 35.0).exp() * 0.9 } else { 0.0 };
                scrape + thud + click(t, 0.085, 1450.0, 60.0, noise) * 0.8
            }
            _ => {
                // the slide drawn back and let go
                let draw = (noise - mid) * 0.5 * (t * 50.0).min(1.0) * if t < 0.1 { 1.0 } else { (-(t - 0.1) * 50.0).exp() };
                let home = if t > 0.16 { (TAU * 210.0 * (t - 0.16)).sin() * (-(t - 0.16) * 40.0).exp() * 0.6 } else { 0.0 };
                draw + click(t, 0.1, 2100.0, 90.0, noise) * 0.5 + click(t, 0.16, 1700.0, 45.0, noise) + home
            }
        };
        let attack = (t / 0.0008).min(1.0);
        let tail = ((duration - t) / 0.02).min(1.0);
        data.push(((value * attack * tail).tanh() * 23000.0) as i16);
    }
    Sfx { name: format!("akimbo/{kind}"), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_and_bounded() {
        for rate in [11025, 48000] {
            let s: Vec<_> = (0..COUNT).map(|k| render(k, rate)).collect();
            for (i, c) in s.iter().enumerate() {
                assert!(c.data.iter().any(|x| x.abs() > 4000), "{i}");
                assert_eq!(c.data[0], 0);
                assert!(c.data.last().unwrap().abs() < 200);
                assert!(s[..i].iter().all(|o| o.data != c.data));
            }
        }
    }
}
