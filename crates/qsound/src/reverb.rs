//! Environmental stereo reverb (Freeverb-style comb + allpass network) and
//! 2-pole Butterworth underwater low-pass filter. Zero allocations in `process`.

use std::f32::consts::PI;

/// Live acoustic environment sent from the host's room probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReverbParams {
    pub enabled: bool,
    /// Normalized room size (0.15 = tight corridor, 0.92 = huge cavern).
    pub room_size: f32,
    /// High-frequency absorption (0.1 = bright stone/metal, 0.85 = soft dirt/water).
    pub damping: f32,
    /// Wet reverb gain (0.0..0.8), scaled by user's `s_reverb_amount`.
    pub wet: f32,
}

impl Default for ReverbParams {
    fn default() -> Self {
        Self {
            enabled: false,
            room_size: 0.45,
            damping: 0.45,
            wet: 0.0,
        }
    }
}

impl ReverbParams {
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            room_size: self.room_size.clamp(0.1, 0.94),
            damping: self.damping.clamp(0.05, 0.95),
            wet: self.wet.clamp(0.0, 0.85),
        }
    }
}

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    filter: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            filter: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damp1: f32, damp2: f32) -> f32 {
        let out = self.buf[self.idx];
        self.filter = out * damp2 + self.filter * damp1;
        // Flush denormals.
        if self.filter.abs() < 1e-20 {
            self.filter = 0.0;
        }
        self.buf[self.idx] = input + self.filter * feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.filter = 0.0;
        self.idx = 0;
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.idx];
        let out = -input + bufout;
        let next = input + bufout * 0.5;
        self.buf[self.idx] = if next.abs() < 1e-20 { 0.0 } else { next };
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.idx = 0;
    }
}

/// Stereo Freeverb processor with smooth parameter slewing.
pub(crate) struct Reverb {
    comb_l: Vec<Comb>,
    comb_r: Vec<Comb>,
    ap_l: Vec<Allpass>,
    ap_r: Vec<Allpass>,
    room_size: f32,
    damping: f32,
    wet: f32,
    energy: f32,
    rate: f32,
}

impl Reverb {
    pub fn new(rate: u32) -> Self {
        let scale = rate.max(8000) as f32 / 44100.0;
        let s = |n: usize| ((n as f32 * scale).round() as usize).max(1);
        let spread = s(STEREO_SPREAD);
        Self {
            comb_l: COMB_TUNING.iter().map(|&n| Comb::new(s(n))).collect(),
            comb_r: COMB_TUNING.iter().map(|&n| Comb::new(s(n) + spread)).collect(),
            ap_l: ALLPASS_TUNING.iter().map(|&n| Allpass::new(s(n))).collect(),
            ap_r: ALLPASS_TUNING.iter().map(|&n| Allpass::new(s(n) + spread)).collect(),
            room_size: 0.45,
            damping: 0.45,
            wet: 0.0,
            energy: 0.0,
            rate: rate.max(8000) as f32,
        }
    }

    pub fn clear(&mut self) {
        for c in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            c.clear();
        }
        for a in self.ap_l.iter_mut().chain(self.ap_r.iter_mut()) {
            a.clear();
        }
        self.energy = 0.0;
        self.wet = 0.0;
    }

    /// True if reverb is currently active or still ringing out its tail.
    pub fn is_active(&self, params: ReverbParams) -> bool {
        (params.enabled && params.wet > 0.001) || self.energy > 0.5 || self.wet > 0.001
    }

    /// Process stereo reverb send (`send`, 16-bit scale) and add wet output to `out`.
    pub fn process_add(&mut self, send: &[[f32; 2]], out: &mut [[i32; 2]], params: ReverbParams) {
        let target_wet = if params.enabled { params.wet } else { 0.0 };
        let step = (send.len() as f32 / (self.rate * 0.18)).clamp(0.01, 1.0);
        self.room_size += (params.room_size - self.room_size) * step;
        self.damping += (params.damping - self.damping) * step;
        self.wet += (target_wet - self.wet) * step;
        if self.wet < 0.0005 && target_wet == 0.0 && self.energy < 0.5 {
            self.wet = 0.0;
            return;
        }
        // Map normalized room_size 0.1..0.94 → feedback 0.58..0.93.
        let feedback = 0.54 + self.room_size * 0.41;
        let damp1 = self.damping * 0.65;
        let damp2 = 1.0 - damp1;
        // Input gain keeps parallel comb sum well-headroomed.
        const INPUT_GAIN: f32 = 0.018;
        let wet_gain = self.wet * 1.45;
        let mut peak = 0.0f32;

        for (s, o) in send.iter().zip(out.iter_mut()) {
            let mono = (s[0] + s[1]) * 0.5 * INPUT_GAIN;
            // Mild stereo cross-feed preserves directional early energy.
            let in_l = (s[0] * 0.65 + s[1] * 0.35) * INPUT_GAIN;
            let in_r = (s[1] * 0.65 + s[0] * 0.35) * INPUT_GAIN;
            let mut acc_l = 0.0f32;
            let mut acc_r = 0.0f32;
            for (cl, cr) in self.comb_l.iter_mut().zip(self.comb_r.iter_mut()) {
                acc_l += cl.process(in_l * 0.5 + mono * 0.5, feedback, damp1, damp2);
                acc_r += cr.process(in_r * 0.5 + mono * 0.5, feedback, damp1, damp2);
            }
            for (al, ar) in self.ap_l.iter_mut().zip(self.ap_r.iter_mut()) {
                acc_l = al.process(acc_l);
                acc_r = ar.process(acc_r);
            }
            let wl = acc_l * wet_gain;
            let wr = acc_r * wet_gain;
            peak = peak.max(wl.abs()).max(wr.abs());
            o[0] += wl as i32;
            o[1] += wr as i32;
        }
        self.energy = self.energy * 0.85 + peak * 0.15;
    }
}

/// Stereo 2-pole Butterworth biquad low-pass filter for underwater muffling.
pub(crate) struct UnderwaterFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl UnderwaterFilter {
    pub fn new(rate: u32, cutoff_hz: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * (cutoff_hz / rate.max(8000) as f32).clamp(0.001, 0.45);
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.2));
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w0) * 0.5) / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: ((1.0 - cos_w0) * 0.5) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    #[inline]
    pub fn process_sample(&mut self, ch: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        if self.z1[ch].abs() < 1e-20 {
            self.z1[ch] = 0.0;
        }
        if self.z2[ch].abs() < 1e-20 {
            self.z2[ch] = 0.0;
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_produces_decaying_tail_when_enabled() {
        let mut rev = Reverb::new(48000);
        let p = ReverbParams { enabled: true, room_size: 0.75, damping: 0.3, wet: 0.4 };
        let mut send = vec![[0.0f32; 2]; 2048];
        for s in send.iter_mut().take(128) {
            *s = [12000.0, 12000.0];
        }
        let mut out = vec![[0i32; 2]; 2048];
        rev.process_add(&send, &mut out, p);
        // Feed silence and verify the tail rings out.
        let silent = vec![[0.0f32; 2]; 2048];
        let mut tail = vec![[0i32; 2]; 2048];
        rev.process_add(&silent, &mut tail, p);
        let energy: i64 = tail.iter().map(|b| i64::from(b[0].abs() + b[1].abs())).sum();
        assert!(energy > 1000, "expected audible reverb tail, got {energy}");
    }

    #[test]
    fn underwater_biquad_attenuates_high_frequencies_strongly() {
        let mut f = UnderwaterFilter::new(48000, 550.0, 0.85);
        // Low freq (150 Hz) vs high freq (6000 Hz)
        let mut low_e = 0.0f32;
        let mut high_e = 0.0f32;
        for i in 0..2000 {
            let t = i as f32 / 48000.0;
            let lo = (2.0 * PI * 150.0 * t).sin() * 10000.0;
            let hi = (2.0 * PI * 6000.0 * t).sin() * 10000.0;
            let yl = f.process_sample(0, lo);
            let yh = f.process_sample(1, hi);
            if i >= 500 {
                low_e += yl.abs();
                high_e += yh.abs();
            }
        }
        assert!(low_e > high_e * 20.0, "expected strong HF rolloff: lo={low_e}, hi={high_e}");
    }
}
