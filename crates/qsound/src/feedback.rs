//! Built-in combat cues, adapted from Padwight with its owner's permission.
//! Original synthesis and effects are used under GPL-2.0-or-later here.
//! No recordings, file loading or audio-thread DSP are involved.

use crate::Sfx;
mod synth;
mod effects;
const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundKind { Click, Tick, Pop, Blip, Chime, Clink, Thud, Knock, Coin }

impl SoundKind {
    pub const ALL: [Self; 9] = [Self::Click, Self::Tick, Self::Pop, Self::Blip, Self::Chime, Self::Clink, Self::Thud, Self::Knock, Self::Coin];
    fn seconds(self) -> f32 {
        match self {
            Self::Click | Self::Tick => 0.03,
            Self::Pop => 0.09,
            Self::Blip => 0.08,
            Self::Chime => 0.6,
            Self::Clink => 0.18,
            Self::Thud => 0.22,
            Self::Knock => 0.14,
            Self::Coin => 0.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundSpec {
    pub kind: SoundKind,
    pub volume: f32,
    /// Envelope and event timing, independent of pitch.
    pub length: f32,
    /// Playback speed: changes both timing and pitch.
    pub speed: f32,
    pub pitch: f32,
    pub bass: f32,
    pub treble: f32,
    pub drive: f32,
    pub echo: f32,
    pub reverb: f32,
}

impl Default for SoundSpec {
    fn default() -> Self {
        Self { kind: SoundKind::Tick, volume: 0.6, length: 1.0, speed: 1.0, pitch: 1.0, bass: 0.0, treble: 0.0, drive: 0.0, echo: 0.0, reverb: 0.0 }
    }
}

impl SoundSpec {
    pub fn sanitized(mut self) -> Self {
        fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
            if v.is_finite() { v.clamp(lo, hi) } else { fallback }
        }
        self.volume = bounded(self.volume, 0.0, 1.0, 0.6);
        self.length = bounded(self.length, 0.25, 3.0, 1.0);
        self.speed = bounded(self.speed, 0.5, 2.0, 1.0);
        self.pitch = bounded(self.pitch, 0.5, 2.0, 1.0);
        self.bass = bounded(self.bass, -12.0, 12.0, 0.0);
        self.treble = bounded(self.treble, -12.0, 12.0, 0.0);
        self.drive = bounded(self.drive, 0.0, 1.0, 0.0);
        self.echo = bounded(self.echo, 0.0, 1.0, 0.0);
        self.reverb = bounded(self.reverb, 0.0, 1.0, 0.0);
        self
    }

    /// Generate once when settings change; callers cache at the device rate.
    pub fn render(self, rate: u32) -> Sfx {
        let spec = self.sanitized();
        let mut out = synth::synthesize(spec.kind, spec.pitch * spec.speed, spec.length / spec.speed);
        effects::apply(&mut out, &spec);
        let peak = out.iter().fold(1.0_f32, |p, s| p.max(s.abs()));
        let gain = 0.9 * spec.volume / peak;
        let rate = rate.clamp(8_000, 192_000);
        let len = (out.len() as u64 * u64::from(rate) / u64::from(RATE)) as usize;
        let data = (0..len).map(|i| {
            let pos = i as f64 * f64::from(RATE) / f64::from(rate);
            let j = pos as usize;
            let a = out[j.min(out.len() - 1)];
            let b = out[(j + 1).min(out.len() - 1)];
            ((a + (b - a) * (pos - j as f64) as f32) * gain * i16::MAX as f32) as i16
        }).collect();
        Sfx { name: format!("combat/{:?}", spec.kind), data, loop_start: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_and_pitch_are_independent_and_speed_changes_duration() {
        let base = SoundSpec::default();
        let n = base.render(RATE).data.len();
        assert_eq!(SoundSpec { pitch: 2.0, ..base }.render(RATE).data.len(), n);
        assert_eq!(SoundSpec { length: 2.0, ..base }.render(RATE).data.len(), 2 * n);
        assert_eq!(SoundSpec { speed: 2.0, ..base }.render(RATE).data.len(), n / 2);
        assert_ne!(base.render(RATE).data, SoundSpec { pitch: 2.0, ..base }.render(RATE).data);
    }

    #[test]
    fn all_cues_and_extreme_effects_are_bounded_and_fade_out() {
        for kind in SoundKind::ALL {
            for (pitch, length, speed) in [(0.5, 3.0, 0.5), (2.0, 0.25, 2.0)] {
                let spec = SoundSpec { kind, pitch, length, speed, bass: 12.0, treble: 12.0, drive: 1.0, echo: 1.0, reverb: 1.0, ..Default::default() };
                let s = spec.render(48_000);
                assert!(s.data.len() < 48_000 * 10);
                assert!(s.data.iter().any(|s| s.abs() > 100));
                assert!(s.data.iter().all(|s| i32::from(*s).abs() <= 29_500));
                assert!(s.data.last().unwrap().abs() < 50);
                assert!(s.data.len() > SoundSpec { echo: 0.0, reverb: 0.0, ..spec }.render(48_000).data.len());
            }
        }
    }

    #[test]
    fn invalid_settings_and_muting_are_safe() {
        let spec = SoundSpec { pitch: f32::NAN, length: f32::INFINITY, echo: f32::NAN, ..Default::default() };
        assert_eq!(spec.sanitized(), SoundSpec::default());
        assert!(SoundSpec { volume: 0.0, ..spec }.render(RATE).data.iter().all(|&s| s == 0));
    }
}
