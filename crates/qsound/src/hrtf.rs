//! Analytical 3D binaural (HRTF) spatializer:
//! - Woodworth-Schlosberg Interaural Time Difference (ITD) with fractional delay
//! - Brown-Duda frequency-dependent head shadow (ILD + contralateral low-pass)
//! - Front/back pinna shadowing and elevation presence cues.

use std::f32::consts::PI;

use crate::Vec3;

const HEAD_RADIUS_M: f32 = 0.0875;
const SPEED_OF_SOUND_M_S: f32 = 343.0;
const DELAY_LEN: usize = 64;
const DELAY_MASK: usize = DELAY_LEN - 1;

/// Per-channel binaural parameters computed from 3D direction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HrtfParams {
    pub left_gain: f32,
    pub right_gain: f32,
    pub left_delay: f32,
    pub right_delay: f32,
    /// Head-shadow 1-pole lowpass coefficient per ear (1.0 = bypass, lower = darker).
    pub left_lp: f32,
    pub right_lp: f32,
    /// Front/back pinna lowpass coefficient (1.0 = front, lower = behind).
    pub back_lp: f32,
    /// Elevation cue: positive = high presence (above), negative = warm chest bounce (below).
    pub elev_cue: f32,
}

impl Default for HrtfParams {
    fn default() -> Self {
        Self {
            left_gain: 0.0,
            right_gain: 0.0,
            left_delay: 0.0,
            right_delay: 0.0,
            left_lp: 1.0,
            right_lp: 1.0,
            back_lp: 1.0,
            elev_cue: 0.0,
        }
    }
}

/// Per-channel binaural DSP state (ring buffer + 1-pole filters).
#[derive(Clone)]
pub(crate) struct HrtfState {
    buf: [f32; DELAY_LEN],
    pos: usize,
    lp_l: f32,
    lp_r: f32,
    lp_back: f32,
    lp_elev: f32,
    delay_l: f32,
    delay_r: f32,
    primed: bool,
}

impl Default for HrtfState {
    fn default() -> Self {
        Self {
            buf: [0.0; DELAY_LEN],
            pos: 0,
            lp_l: 0.0,
            lp_r: 0.0,
            lp_back: 0.0,
            lp_elev: 0.0,
            delay_l: 0.0,
            delay_r: 0.0,
            primed: false,
        }
    }
}

fn lp_alpha(cutoff_hz: f32, rate: f32) -> f32 {
    (1.0 - (-2.0 * PI * cutoff_hz / rate.max(8000.0)).exp()).clamp(0.02, 1.0)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Compute binaural spatialization parameters for a unit direction vector `dir`
/// (from listener to source) and scalar attenuation `atten` (0..255 scale).
pub(crate) fn compute(
    dir: Vec3,
    mut forward: Vec3,
    right: Vec3,
    mut up: Vec3,
    atten: f32,
    rate: u32,
) -> HrtfParams {
    // Derive fallback basis when caller left forward/up at Default (zero).
    if dot(up, up) < 0.01 {
        up = [0.0, 0.0, 1.0];
    }
    if dot(forward, forward) < 0.01 {
        forward = cross(up, right);
        let fl = dot(forward, forward).sqrt();
        if fl > 0.001 {
            forward = forward.map(|v| v / fl);
        } else {
            forward = [1.0, 0.0, 0.0];
        }
    }

    let x = dot(dir, right).clamp(-1.0, 1.0); // +right, -left
    let y = dot(dir, forward).clamp(-1.0, 1.0); // +front, -back
    let z = dot(dir, up).clamp(-1.0, 1.0); // +above, -below

    let rate_f = rate.max(8000) as f32;
    let horiz = (x * x + y * y).sqrt();
    // Lateral angle from median plane (0 = front/back, pi/2 = directly left/right).
    let lateral = x.abs().atan2(horiz.clamp(1e-4, 1.0).max(y.abs()));
    let cos_el = (1.0 - z * z).max(0.0).sqrt();
    // Woodworth-Schlosberg ITD scaled by cos(elevation).
    let itd_sec = (HEAD_RADIUS_M / SPEED_OF_SOUND_M_S) * (lateral + lateral.sin()) * cos_el;
    let itd_samples = (itd_sec * rate_f).min((DELAY_LEN - 2) as f32);

    let (left_delay, right_delay) = if x >= 0.0 {
        // Sound on the right arrives later at the left ear.
        (itd_samples, 0.0)
    } else {
        (0.0, itd_samples)
    };

    // Interaural Level Difference (ILD): keep energy natural on headphones
    // (avoid complete silence in the far ear like classic hard panning).
    let ear_l = (-x).clamp(-1.0, 1.0); // +1 when directly left, -1 when directly right
    let ear_r = x.clamp(-1.0, 1.0);
    // Range ~0.35 (far ear) to 1.25 (near ear).
    let gain_l = atten * (0.80 + 0.45 * ear_l).max(0.0);
    let gain_r = atten * (0.80 + 0.45 * ear_r).max(0.0);

    // Head-shadow lowpass: far ear loses highs (down to ~1.5 kHz contralateral).
    let shadow_l = ((-ear_l).max(0.0)).powf(1.2); // 0 on near side, 1 on far side
    let shadow_r = ((-ear_r).max(0.0)).powf(1.2);
    let left_lp = if shadow_l > 0.01 {
        lp_alpha(18000.0 * (1.5 / 18.0_f32).powf(shadow_l), rate_f)
    } else {
        1.0
    };
    let right_lp = if shadow_r > 0.01 {
        lp_alpha(18000.0 * (1.5 / 18.0_f32).powf(shadow_r), rate_f)
    } else {
        1.0
    };

    // Front/back pinna shadowing: sounds behind the listener are gentler above ~4.5 kHz.
    let behind = (-y).max(0.0);
    let back_lp = if behind > 0.05 {
        lp_alpha(16000.0 * (4.2 / 16.0_f32).powf(behind), rate_f)
    } else {
        1.0
    };

    HrtfParams {
        left_gain: gain_l / 256.0,
        right_gain: gain_r / 256.0,
        left_delay,
        right_delay,
        left_lp,
        right_lp,
        back_lp,
        elev_cue: z * 0.28,
    }
}

impl HrtfState {
    #[inline]
    fn read_delayed(&self, delay: f32) -> f32 {
        let d = delay.clamp(0.0, (DELAY_LEN - 2) as f32);
        let i0 = d.floor() as usize;
        let frac = d - i0 as f32;
        let p0 = self.pos.wrapping_sub(i0) & DELAY_MASK;
        let p1 = self.pos.wrapping_sub(i0 + 1) & DELAY_MASK;
        self.buf[p0] + (self.buf[p1] - self.buf[p0]) * frac
    }

    #[inline]
    pub fn process_sample(&mut self, sample: f32, p: &HrtfParams) -> [f32; 2] {
        if !self.primed {
            self.delay_l = p.left_delay;
            self.delay_r = p.right_delay;
            self.primed = true;
        } else {
            // Smooth delay line transitions to prevent clicks while turning.
            self.delay_l += (p.left_delay - self.delay_l) * 0.015;
            self.delay_r += (p.right_delay - self.delay_r) * 0.015;
        }

        // Front/back pinna cue.
        let shaped = if p.back_lp < 0.999 {
            self.lp_back += p.back_lp * (sample - self.lp_back);
            self.lp_back
        } else {
            self.lp_back = sample;
            sample
        };

        // Elevation cue: simple high-shelf split around ~3 kHz.
        self.lp_elev += 0.30 * (shaped - self.lp_elev);
        let hi = shaped - self.lp_elev;
        let elev = shaped + hi * p.elev_cue;

        self.pos = (self.pos + 1) & DELAY_MASK;
        self.buf[self.pos] = elev;

        let dl = self.read_delayed(self.delay_l);
        let dr = self.read_delayed(self.delay_r);

        let l = if p.left_lp < 0.999 {
            self.lp_l += p.left_lp * (dl - self.lp_l);
            self.lp_l
        } else {
            self.lp_l = dl;
            dl
        };
        let r = if p.right_lp < 0.999 {
            self.lp_r += p.right_lp * (dr - self.lp_r);
            self.lp_r
        } else {
            self.lp_r = dr;
            dr
        };

        [l * p.left_gain, r * p.right_gain]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_source_delays_and_shadows_left_ear() {
        let p = compute([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], 255.0, 48000);
        assert!(p.right_gain > p.left_gain);
        assert!(p.left_gain > 0.0, "HRTF keeps head-shadowed far ear audible");
        assert!(p.left_delay > 15.0 && p.right_delay == 0.0, "ITD: l={} r={}", p.left_delay, p.right_delay);
        assert!(p.left_lp < p.right_lp, "far ear gets head-shadow lowpass");
    }

    #[test]
    fn behind_and_above_produce_distinct_cues() {
        let front = compute([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], 255.0, 48000);
        let back = compute([-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], 255.0, 48000);
        let above = compute([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], 255.0, 48000);
        let below = compute([0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], 255.0, 48000);
        assert!(back.back_lp < front.back_lp);
        assert!(above.elev_cue > 0.2 && below.elev_cue < -0.2);
        assert!(above.left_delay < 0.01 && above.right_delay < 0.01);
    }
}
