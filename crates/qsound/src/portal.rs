//! Original, deterministic induction-projector cues, generated at device rate.
use crate::Sfx;
use std::f32::consts::TAU;

/// Fire, placed, rejected/dissipating, and traversal respectively.
pub fn render(kind: usize, slot: usize, rate: u32) -> Sfx {
    let duration = [0.24, 0.52, 0.43, 0.65][kind];
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x8ac4_319bu32;
    let (mut phase, mut bass, mut low) = (0.0f32, 0.0f32, 0.0f32);
    let pitch = if slot == 0 { 1.15 } else { 0.85 };
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let u = t / duration;
        seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        low += (noise - low) * 0.12;
        let (freq, tone, air) = match kind {
            0 => (150.0 + 1300.0 * (-t * 30.0).exp(), (-t * 17.0).exp(), (-t * 28.0).exp()),
            1 => (360.0 + 320.0 * u.sqrt(), (1.0-u).powf(1.8), (1.0-u).powi(3) * 0.3),
            2 => (90.0 + 550.0 * (1.0-u).powi(2), (1.0-u).powi(2) * 0.4, (1.0-u).powf(1.4)),
            _ => (100.0 + 280.0 * (u * std::f32::consts::PI).sin(), (1.0-u) * 0.35, (u * std::f32::consts::PI).sin().powi(2)),
        };
        phase += TAU * freq * pitch / rate as f32;
        bass += TAU * (65.0 + 30.0 * (1.0-u)) / rate as f32;
        let fm = (phase + 1.8 * (phase * 0.501).sin()).sin();
        let chime = if kind == 1 { (phase * 1.498).sin() * 0.22 } else { 0.0 };
        let body = if kind == 0 { bass.sin() * (-t * 22.0).exp() * 0.5 } else { 0.0 };
        let attack = (t / 0.004).min(1.0);
        let tail = ((duration-t) / 0.025).min(1.0);
        let value = ((fm * 0.4 + chime) * tone + (noise * 0.18 + low * 1.4) * air + body) * attack * tail;
        data.push((value.tanh() * 23000.0) as i16);
    }
    Sfx { name: format!("portal/{kind}_{slot}"), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_bounded_and_fade_to_silence() {
        let sounds: Vec<_> = (0..4).flat_map(|k| (0..2).map(move |s| render(k,s,22050).data)).collect();
        for (i,s) in sounds.iter().enumerate() {
            assert!(s.iter().any(|x| x.abs() > 1000));
            assert_eq!(s[0],0);
            assert!(s.last().unwrap().abs() < 100);
            for other in &sounds[..i] { assert_ne!(s,other); }
        }
    }
}
