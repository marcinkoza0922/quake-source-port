// Adapted from the author-owned Padwight synthesis; reused with the owner’s permission.
// SPDX-License-Identifier: GPL-2.0-or-later
//! What happens to a cue after it's made: its bass and treble turned up or down, then driven
//! into soft clipping, then echoes of it, then the room it rings in. Echo and reverb add a tail after the cue.

use super::{RATE, SoundSpec, synth::Svf};

/// Where the bass and treble shelves turn, in Hz.
const BASS_CORNER: f32 = 200.0;
const TREBLE_CORNER: f32 = 4000.0;
/// How far apart echoes are, in seconds. Long enough to hear each on its own after a short cue.
const ECHO_DELAY: f32 = 0.13;
/// Tails run until they've fallen this far below the cue.
const SILENT: f32 = 0.001;
/// The reverb's combs, in samples at [`RATE`]: Freeverb's tunings, which don't share factors, so
/// their echoes never pile up into a ringing note.
const COMBS: [usize; 4] = [1116, 1188, 1277, 1356];
const ALLPASSES: [usize; 2] = [556, 441];

/// Bass and treble, in dB, as set and clamped.
fn tone(spec: &SoundSpec) -> (f32, f32) {
    (spec.bass.clamp(-12.0, 12.0), spec.treble.clamp(-12.0, 12.0))
}

/// How hard a full drive pushes the cue into the clipper: hard enough to all but square a tone.
const MOST_DRIVE: f32 = 10.0;

/// Echo and reverb, 0..=1, as set and clamped.
fn space(spec: &SoundSpec) -> (f32, f32) {
    (spec.echo.clamp(0.0, 1.0), spec.reverb.clamp(0.0, 1.0))
}

/// Applies `spec`'s effects to a cue, adding a bounded tail.
pub(super) fn apply(out: &mut Vec<f32>, spec: &SoundSpec) {
    let (bass, treble) = tone(spec);
    let (echo, reverb) = space(spec);
    let drive = spec.drive.clamp(0.0, 1.0);
    if bass == 0.0 && treble == 0.0 && drive == 0.0 && echo == 0.0 && reverb == 0.0 {
        return;
    }
    shelves(out, bass, treble);
    overdrive(out, drive);
    add_echo(out, echo);
    add_reverb(out, reverb);
    // A tail cut off at a sample boundary would click, same as a cue.
    let fade = (RATE as f32 * 0.004) as usize;
    let n = out.len();
    for (i, s) in out.iter_mut().enumerate().skip(n.saturating_sub(fade)) {
        *s *= (n - 1 - i) as f32 / fade as f32;
    }
}

/// Turns the lows and highs up or down: each shelf adds (or takes away) its filtered part.
fn shelves(out: &mut [f32], bass: f32, treble: f32) {
    let gain = |db: f32| 10_f32.powf(db / 20.0) - 1.0;
    let (low_gain, high_gain) = (gain(bass), gain(treble));
    let (mut low, mut high) = (Svf::default(), Svf::default());
    for s in out {
        let x = *s;
        *s = x + low_gain * low.run(x, BASS_CORNER, 0.7).low + high_gain * high.run(x, TREBLE_CORNER, 0.7).high;
    }
}

/// Soft-clips the cue: quiet parts are lifted toward the loud, and peaks round off, adding
/// harmonics. Divided by what its peak becomes, so the peak stays where it was.
fn overdrive(out: &mut [f32], drive: f32) {
    if drive == 0.0 {
        return;
    }
    let push = 1.0 + (MOST_DRIVE - 1.0) * drive;
    let peak = out.iter().fold(0.0_f32, |p, s| p.max(s.abs())).max(f32::EPSILON);
    let level = peak / push.tanh();
    for s in out {
        *s = level * (*s / peak * push).tanh();
    }
}

/// How loud the first echo is, and how much of each the next keeps.
fn echo_levels(echo: f32) -> (f32, f32) {
    (0.7 * echo, 0.2 + 0.4 * echo)
}

fn echo_tail(echo: f32) -> usize {
    if echo == 0.0 {
        return 0;
    }
    let (first, keep) = echo_levels(echo);
    let repeats = 1.0 + (SILENT / first).ln() / keep.ln();
    (repeats.ceil().max(1.0) * ECHO_DELAY * RATE as f32) as usize
}

/// Echoes that fade and darken, the way a far wall throws back less of the highs each time.
fn add_echo(out: &mut Vec<f32>, echo: f32) {
    let tail = echo_tail(echo);
    if tail == 0 {
        return;
    }
    let (first, keep) = echo_levels(echo);
    let delay = (ECHO_DELAY * RATE as f32) as usize;
    let dry = std::mem::take(out);
    let mut echoes = vec![0.0_f32; dry.len() + tail];
    let mut damp = Svf::default();
    for n in delay..echoes.len() {
        let back = dry.get(n - delay).copied().unwrap_or(0.0) + keep * echoes[n - delay];
        echoes[n] = damp.run(back, 3500.0, 0.7).low;
    }
    *out = echoes.iter().enumerate().map(|(n, e)| dry.get(n).copied().unwrap_or(0.0) + first * e).collect();
}

/// How much each comb keeps per pass: a bigger room rings longer.
fn room(reverb: f32) -> f32 {
    0.7 + 0.2 * reverb
}

fn reverb_tail(reverb: f32) -> usize {
    if reverb == 0.0 {
        return 0;
    }
    let longest = COMBS.iter().max().copied().unwrap_or(0) as f32;
    (longest * SILENT.ln() / room(reverb).ln()) as usize
}

/// A small room, after Freeverb: combs in parallel for the reflections, each losing a little of
/// the highs per pass, then allpasses in a row to smear them into a wash.
fn add_reverb(out: &mut Vec<f32>, reverb: f32) {
    let tail = reverb_tail(reverb);
    if tail == 0 {
        return;
    }
    out.resize(out.len() + tail, 0.0);
    let keep = room(reverb);
    let mut wet = vec![0.0_f32; out.len()];
    for delay in COMBS {
        let mut line = vec![0.0_f32; delay];
        let mut damped = 0.0_f32;
        for (n, (w, x)) in wet.iter_mut().zip(out.iter()).enumerate() {
            let back = line[n % delay];
            damped = back * 0.7 + damped * 0.3;
            line[n % delay] = x + keep * damped;
            *w += back;
        }
    }
    for delay in ALLPASSES {
        let mut line = vec![0.0_f32; delay];
        for (n, w) in wet.iter_mut().enumerate() {
            let back = line[n % delay];
            line[n % delay] = *w + 0.5 * back;
            *w = back - *w;
        }
    }
    // The wash is scaled to the cue's own peak, so how much is heard follows the setting alone.
    let peak = |v: &[f32]| v.iter().fold(0.0_f32, |p, s| p.max(s.abs())).max(f32::EPSILON);
    let mix = 0.6 * reverb * peak(out) / peak(&wet);
    for (s, w) in out.iter_mut().zip(wet) {
        *s += mix * w;
    }
}
