// Adapted from the author-owned Padwight synthesis; reused with the owner’s permission.
// SPDX-License-Identifier: GPL-2.0-or-later
//! How each cue is made. Small physical sounds are a strike and the ringing of the thing struck,
//! so most cues are a few sine partials struck at a time and dying at their own rates, over bursts
//! of noise shaped by a filter. The electronic ones are tones that glide, or bend with FM.

use std::f32::consts::{PI, TAU};

use super::{RATE, SoundKind};

/// Synthesizes a cue at `pitch` and `length` (already clamped), normalized to a peak of 1
/// before volume.
pub(super) fn synthesize(kind: SoundKind, pitch: f32, length: f32) -> Vec<f32> {
    let mut cue = Cue::new(kind.seconds() * length, pitch, length);
    match kind {
        SoundKind::Click => click(&mut cue),
        SoundKind::Tick => tick(&mut cue),
        SoundKind::Pop => pop(&mut cue),
        SoundKind::Blip => blip(&mut cue),
        SoundKind::Chime => chime(&mut cue),
        SoundKind::Clink => clink(&mut cue),
        SoundKind::Thud => thud(&mut cue),
        SoundKind::Knock => knock(&mut cue),
        SoundKind::Coin => coin(&mut cue),
    }
    cue.finish()
}

fn click(cue: &mut Cue) {
    cue.burst(Burst { decay: 350.0, ..Burst::new(Filter::Band(3800.0, 1.2)) });
    cue.voice(Voice { gain: 0.35, decay: 260.0, ..Voice::ring(0.0, 2000.0) });
}

/// A wood block's modes, high and short, over the snap of the stick.
fn tick(cue: &mut Cue) {
    cue.partials(0.0, 2400.0, &[(1.0, 1.0, 180.0), (2.32, 0.35, 300.0), (4.1, 0.15, 450.0)]);
    cue.burst(Burst { gain: 0.5, decay: 900.0, ..Burst::new(Filter::Band(6000.0, 0.8)) });
}

/// A bubble's tone falling fast, and the smack of it letting go.
fn pop(cue: &mut Cue) {
    cue.voice(Voice { to: 380.0, glide: 0.035, attack: 0.001, decay: 45.0, ..Voice::ring(0.0, 1100.0) });
    cue.burst(Burst { gain: 0.25, decay: 600.0, ..Burst::new(Filter::Low(2000.0)) });
}

fn blip(cue: &mut Cue) {
    let square = Voice { wave: Wave::Square, to: 1200.0, glide: 0.05, attack: 0.002, decay: 25.0, ..Voice::ring(0.0, 600.0) };
    cue.voice(square);
}

/// Two bell strokes a fourth apart, each with a bell's out-of-tune partials.
fn chime(cue: &mut Cue) {
    for (at, f, gain) in [(0.0, 1318.5, 0.7), (0.09, 1760.0, 1.0)] {
        for (ratio, level, decay) in [(1.0, 1.0, 6.0), (2.0, 0.25, 9.0), (2.76, 0.35, 12.0), (5.4, 0.12, 20.0)] {
            cue.voice(Voice { gain: gain * level, attack: 0.002, decay, ..Voice::ring(at, f * ratio) });
        }
    }
}

/// A small metal bar's modes, a hair out of tune with themselves, and the rattle of the strike.
fn clink(cue: &mut Cue) {
    let modes = [(1.0, 1.0, 40.0), (1.003, 0.5, 40.0), (2.76, 0.6, 60.0), (5.40, 0.35, 90.0)];
    cue.partials(0.0, 2200.0, &modes);
    cue.burst(Burst { gain: 0.5, decay: 700.0, ..Burst::new(Filter::High(5000.0)) });
}

/// A falling low tone and a soft push of air, driven a little so small speakers still hear it.
fn thud(cue: &mut Cue) {
    cue.voice(Voice { to: 55.0, glide: 0.08, attack: 0.001, decay: 18.0, ..Voice::ring(0.0, 150.0) });
    cue.burst(Burst { gain: 0.6, decay: 120.0, ..Burst::new(Filter::Low(400.0)) });
    cue.drive(2.5);
}

/// A door's low modes, and the rap of the knuckle on its surface.
fn knock(cue: &mut Cue) {
    cue.partials(0.0, 210.0, &[(1.0, 0.8, 35.0), (2.29, 0.45, 50.0), (5.48, 0.2, 100.0)]);
    cue.burst(Burst { gain: 0.6, decay: 70.0, ..Burst::new(Filter::Band(250.0, 3.0)) });
    cue.burst(Burst { gain: 0.5, decay: 400.0, ..Burst::new(Filter::Band(1800.0, 0.9)) });
}

fn coin(cue: &mut Cue) {
    cue.voice(Voice { wave: Wave::Square, gain: 0.8, attack: 0.001, decay: 60.0, ..Voice::ring(0.0, 987.77) });
    cue.voice(Voice { wave: Wave::Square, attack: 0.001, decay: 9.0, ..Voice::ring(0.07, 1318.5) });
}

/// A cue being built: voices and bursts mix into `out`.
struct Cue {
    out: Vec<f32>,
    /// Every frequency is multiplied by this.
    pitch: f32,
    /// Every time is multiplied by this, so a longer cue is the same sound slowed, not a longer tail.
    stretch: f32,
    noise: Noise,
}

impl Cue {
    fn new(seconds: f32, pitch: f32, stretch: f32) -> Self {
        Cue { out: vec![0.0; (seconds * RATE as f32) as usize], pitch, stretch, noise: Noise(0x2545_f491) }
    }

    /// The first sample at `at` seconds (in the kind's own time), and how long a sample is in it.
    fn start(&self, at: f32) -> (usize, f32) {
        ((at * self.stretch * RATE as f32) as usize, 1.0 / (RATE as f32 * self.stretch))
    }

    fn voice(&mut self, v: Voice) {
        let (start, dt) = self.start(v.at);
        let rate_dt = 1.0 / RATE as f32;
        let mut phase = 0.0_f32;
        for (n, s) in self.out.iter_mut().skip(start).enumerate() {
            let x = n as f32 * dt;
            let fade = (-x * v.decay).exp();
            if fade < 1e-4 {
                break;
            }
            let glide = if v.glide > 0.0 { (x / v.glide).min(1.0) } else { 1.0 };
            let f = v.from * (v.to / v.from).powf(glide) * self.pitch;
            // A partial past what the rate can carry would fold back down as a wrong note.
            if f >= 0.45 * RATE as f32 {
                continue;
            }
            phase = (phase + TAU * f * rate_dt) % TAU;
            let attack = if v.attack > 0.0 { (x / v.attack).min(1.0) } else { 1.0 };
            let wave = match v.wave {
                Wave::Sine => phase.sin(),
                // A sine pushed into a rounded square: hollow and retro without the harsh edge.
                Wave::Square => (3.0 * phase.sin()).tanh(),

            };
            *s += v.gain * attack * attack * fade * wave;
        }
    }

    /// Struck partials of `f`, each as (ratio, gain, decay).
    fn partials(&mut self, at: f32, f: f32, modes: &[(f32, f32, f32)]) {
        for &(ratio, gain, decay) in modes {
            self.voice(Voice { gain, decay, ..Voice::ring(at, f * ratio) });
        }
    }

    fn burst(&mut self, b: Burst) {
        let (start, dt) = self.start(b.at);
        // Bands go through two filters in a row: one alone lets so much noise past its edges that
        // every burst hisses alike.
        let (mut filter, mut again) = (Svf::default(), Svf::default());
        for (n, s) in self.out.iter_mut().skip(start).enumerate() {
            let x = n as f32 * dt;
            let attack = if b.attack > 0.0 { (x / b.attack).min(1.0) } else { 1.0 };
            let raw = self.noise.sample() * attack * (-x * b.decay).exp();
            let pitch = self.pitch;
            *s += b.gain
                * match b.filter {
                    Filter::Low(f) => filter.run(raw, f * pitch, 0.7).low,
                    Filter::High(f) => filter.run(raw, f * pitch, 0.7).high,
                    Filter::Band(f, q) => again.run(filter.run(raw, f * pitch, q).band, f * pitch, q).band,

                };
        }
    }

    /// Soft-clips the mix, adding harmonics a low sound needs to be heard on small speakers.
    fn drive(&mut self, amount: f32) {
        let peak = self.out.iter().fold(0.0_f32, |p, s| p.max(s.abs())).max(f32::EPSILON);
        for s in &mut self.out {
            *s = (*s / peak * amount).tanh();
        }
    }

    /// Fades the end, so no cue stops with a click, and normalizes to a peak of 1.
    fn finish(mut self) -> Vec<f32> {
        let fade = (RATE as f32 * 0.004) as usize;
        let n = self.out.len();
        for (i, s) in self.out.iter_mut().enumerate().skip(n.saturating_sub(fade)) {
            *s *= (n - 1 - i) as f32 / fade as f32;
        }
        let peak = self.out.iter().fold(0.0_f32, |p, s| p.max(s.abs())).max(f32::EPSILON);
        self.out.iter_mut().for_each(|s| *s /= peak);
        self.out
    }
}

/// A tone: struck, or faded in, and dying away, perhaps gliding from one pitch to another.
#[derive(Debug, Clone, Copy)]
struct Voice {
    /// When it starts, in seconds.
    at: f32,
    /// Its frequency at the start and where it glides to, in Hz.
    from: f32,
    to: f32,
    /// How long the glide takes, in seconds. It's exponential, so each step sounds even.
    glide: f32,
    gain: f32,
    /// The fade in, in seconds. None is a strike.
    attack: f32,
    /// How fast it dies away, per second.
    decay: f32,
    wave: Wave,
}

impl Voice {
    /// A sine struck at `at`, ringing at `f` and dying at the default rate.
    fn ring(at: f32, f: f32) -> Self {
        Voice { at, from: f, to: f, glide: 0.0, gain: 1.0, attack: 0.0, decay: 20.0, wave: Wave::Sine }
    }
}

#[derive(Debug, Clone, Copy)]
enum Wave {
    Sine,
    Square,
}

/// A burst of noise through a filter, struck (or faded in) at `at` and dying at `decay`.
#[derive(Debug, Clone, Copy)]
struct Burst {
    at: f32,
    gain: f32,
    attack: f32,
    decay: f32,
    filter: Filter,
}

impl Burst {
    fn new(filter: Filter) -> Self {
        Burst { at: 0.0, gain: 1.0, attack: 0.0, decay: 200.0, filter }
    }
}

/// Which part of a burst's noise is heard, in Hz.
#[derive(Debug, Clone, Copy)]
enum Filter {
    Low(f32),
    High(f32),
    /// A band around a frequency, narrower as `q` grows.
    Band(f32, f32),
}

/// A state-variable filter (the zero-delay form), which stays stable as its frequency moves.
#[derive(Default)]
pub(super) struct Svf {
    ic1: f32,
    ic2: f32,
}

pub(super) struct SvfOut {
    pub(super) low: f32,
    pub(super) band: f32,
    pub(super) high: f32,
}

impl Svf {
    pub(super) fn run(&mut self, v0: f32, f: f32, q: f32) -> SvfOut {
        let g = (PI * f.min(0.45 * RATE as f32) / RATE as f32).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let (a2, v3) = (g * a1, v0 - self.ic2);
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + g * a2 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        // The band output peaks at q; scaled by 1/q, every band peaks at 1.
        SvfOut { low: v2, band: v1 * k, high: v0 - k * v1 - v2 }
    }
}

/// Noise that is the same every time, so cues don't change between plays.
struct Noise(u32);

impl Noise {
    fn sample(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}
