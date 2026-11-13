//! Original synthesized railgun cues (no recorded or extracted audio).
use crate::Sfx;
use std::f32::consts::{PI, TAU};

pub const FIRE: usize = 0;
pub const HUM: usize = 1;
pub const CLICK: usize = 2;
pub const IMPACT: usize = 3;

/// Fire (crack, ringing zap, then the rising recharge whine), the idle hum
/// (seamless one second loop), the dry click and the slug's impact hiss.
pub fn render(kind: usize, rate: u32) -> Sfx {
    let duration = [1.45, 1.0, 0.09, 0.35][kind];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x51ed_270bu32;
    let (mut low, mut ph, mut ph2) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let u = t / duration;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        low += (noise - low) * 0.25;
        let value = match kind {
            FIRE => {
                // electric crack + descending ring
                ph += TAU * (220.0 + 2400.0 * (-t * 14.0).exp()) / rate as f32;
                let ring = (ph + 2.5 * (ph * 2.01).sin()).sin() * (-t * 5.0).exp();
                let crack = noise * (-t * 45.0).exp() * 1.4;
                let thump = (TAU * 55.0 * t).sin() * (-t * 12.0).exp() * 0.8;
                // recharge whine from 0.35 s, rising, ending in a soft tick
                let w = ((t - 0.35) / 1.0).clamp(0.0, 1.0);
                ph2 += TAU * (300.0 + 1500.0 * w * w) / rate as f32;
                let whine = if t > 0.35 { ph2.sin() * 0.22 * (w * PI).sin().powf(0.5) } else { 0.0 };
                let tick = if t > 1.36 { (TAU * 3000.0 * t).sin() * (-(t - 1.36) * 90.0).exp() * 0.4 } else { 0.0 };
                ring * 0.7 + crack + thump + whine + tick
            }
            HUM => {
                // integer-Hz partials so one second loops without a seam
                let a = (TAU * 60.0 * t).sin() * 0.5 + (TAU * 120.0 * t).sin() * 0.3 + (TAU * 181.0 * t).sin() * 0.12;
                let wobble = 1.0 + 0.25 * (TAU * 3.0 * t).sin();
                a * wobble * 0.35
            }
            CLICK => (TAU * 1800.0 * t).sin() * (-t * 80.0).exp() + noise * (-t * 120.0).exp() * 0.5,
            _ => (noise * 0.5 + low * 1.5) * (1.0 - u).powi(2) + (TAU * 900.0 * t).sin() * (-t * 30.0).exp() * 0.3,
        };
        let attack = if kind == HUM { 1.0 } else { (t / 0.002).min(1.0) };
        let tail = if kind == HUM { 1.0 } else { ((duration - t) / 0.02).min(1.0) };
        data.push(((value * attack * tail).tanh() * 22000.0) as i16);
    }
    Sfx { name: format!("rail/{kind}"), data, loop_start: (kind == HUM).then_some(0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_and_bounded() {
        let s: Vec<_> = (0..4).map(|k| render(k, 22050)).collect();
        for (i, c) in s.iter().enumerate() {
            assert!(c.data.iter().any(|x| x.abs() > 1000), "{i}");
            if i != HUM {
                assert_eq!(c.data[0], 0);
                assert!(c.data.last().unwrap().abs() < 200);
            }
        }
        assert_eq!(s[HUM].loop_start, Some(0));
        // the hum loops seamlessly: last sample leads into the first
        let h = &s[HUM].data;
        assert!((h[h.len() - 1] as i32 - h[0] as i32).abs() < 1500);
    }
}
