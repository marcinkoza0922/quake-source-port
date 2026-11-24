//! Assault rifle cues: CC0 recordings processed by `tools/gun_sounds.py`
//! (see `assets/gun-sounds/README.md`), and the synthesised rustle of
//! raising and lowering the sight.
use crate::{Sfx, load_sfx};
use std::f32::consts::{PI, TAU};

/// The first of [`SHOTS`] takes of a shot.
pub const SHOT: usize = 0;
pub const SHOTS: usize = 3;
pub const DRY: usize = 3;
pub const MAG_OUT: usize = 4;
pub const MAG_IN: usize = 5;
pub const BOLT: usize = 6;
pub const AIM_IN: usize = 7;
pub const AIM_OUT: usize = 8;
pub const CUES: usize = 9;

macro_rules! wav {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/gun-sounds/", $name, ".wav"))
    };
}

const RECORDED: [&[u8]; AIM_IN] = [
    wav!("rifle_shot_a"),
    wav!("rifle_shot_b"),
    wav!("rifle_shot_c"),
    wav!("rifle_dry"),
    wav!("rifle_magout"),
    wav!("rifle_magin"),
    wav!("rifle_bolt"),
];

/// Cue `kind` at the device rate.
pub fn render(kind: usize, rate: u32) -> Sfx {
    match RECORDED.get(kind) {
        Some(bytes) => load_sfx(&format!("rifle/{kind}"), bytes, rate).expect("bundled rifle sound"),
        None => rustle(kind == AIM_IN, rate),
    }
}

/// A struck piece of metal: a few inharmonic partials.
fn clank(t: f32, f: f32, decay: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    [(1.0, 1.0), (2.76, 0.6), (5.4, 0.35), (8.9, 0.2)].iter().map(|&(k, a)| (TAU * f * k * t).sin() * a * (-t * decay * (0.7 + 0.3 * k)).exp()).sum::<f32>() * 0.5
}

/// Cloth and sling: soft filtered noise, rising in or falling out.
fn rustle(raise: bool, rate: u32) -> Sfx {
    let duration = if raise { 0.22 } else { 0.20 };
    let n = (duration * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let mut seed = 0x2f6e_2b15u32 ^ u32::from(raise).wrapping_mul(0x9e37_79b9);
    let (mut mid, mut band) = (0.0f32, 0.0f32);
    // one-pole coefficients that sound the same at any device rate
    let pole = |hz: f32| 1.0 - (-TAU * hz / rate as f32).exp();
    let (k_mid, k_band) = (pole(2600.0), pole(900.0));
    for i in 0..n {
        let t = i as f32 / rate as f32;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        mid += (noise - mid) * k_mid;
        band += (mid - band) * k_band;
        let env = (t / duration * PI).sin().max(0.0).powf(if raise { 0.8 } else { 1.4 });
        let rattle = clank(t - if raise { 0.15 } else { 0.03 }, 2600.0, 120.0) * 0.12;
        let value = (band * 1.1 + (mid - band) * 0.35) * env * 0.8 + rattle;
        let fade = (t / 0.0015).min(1.0) * ((duration - t) / 0.02).min(1.0);
        data.push(((value * 0.5 * fade).tanh() * 24000.0) as i16);
    }
    Sfx { name: format!("rifle/aim{}", u8::from(raise)), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_and_bounded() {
        let s: Vec<_> = (0..CUES).map(|k| render(k, 22050)).collect();
        for (i, c) in s.iter().enumerate() {
            assert!(c.data.iter().any(|x| x.unsigned_abs() > 1000), "{i}");
            assert!(c.data.last().unwrap().unsigned_abs() < 300, "{i}");
            assert!(s[..i].iter().all(|o| o.data != c.data));
        }
        // shots start with a bang and die away without a second one
        for shot in &s[SHOT..SHOT + SHOTS] {
            let rms = |a: f32, b: f32| {
                let d = &shot.data[(a * 22050.0) as usize..(b * 22050.0) as usize];
                (d.iter().map(|&x| f32::from(x).powi(2)).sum::<f32>() / d.len() as f32).sqrt()
            };
            assert!(rms(0.0, 0.06) > 12000.0 && rms(0.1, 0.16) < rms(0.0, 0.06) * 0.8 && rms(0.3, 0.36) < 4000.0);
        }
    }
}
