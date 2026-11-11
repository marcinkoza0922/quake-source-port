//! Stick shaping, acceleration ramp, flick stick, gyro (ironwail in_sdl.c).

pub fn magnitude(a: [f32; 2]) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

/// Radial inner deadzone + outer threshold, magnitude rescaled to [0,1].
pub fn apply_deadzone(a: [f32; 2], deadzone: f32, outer: f32) -> [f32; 2] {
    let m = magnitude(a);
    if m <= deadzone {
        return [0.0; 2];
    }
    let nm = ((m - deadzone) / (1.0 - deadzone - outer).max(1e-4)).min(1.0);
    let s = nm / m;
    [a[0] * s, a[1] * s]
}

/// Radial response curve: magnitude^exponent, direction kept.
pub fn apply_easing(a: [f32; 2], exponent: f32) -> [f32; 2] {
    let m = magnitude(a);
    if m == 0.0 {
        return [0.0; 2];
    }
    let s = m.powf(exponent) / m;
    [a[0] * s, a[1] * s]
}

/// Sustained full-deflection yaw acceleration.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccelRamp {
    held: f32,
}

impl AccelRamp {
    /// Returns the speed multiplier (1 = none, up to `mult`).
    pub fn update(&mut self, mag: f32, threshold: f32, delay: f32, ramp: f32, mult: f32, ft: f32) -> f32 {
        if mag < threshold || mult <= 1.0 {
            self.held = 0.0;
            return 1.0;
        }
        self.held += ft;
        let t = if ramp > 0.0 { ((self.held - delay) / ramp).clamp(0.0, 1.0) } else if self.held >= delay { 1.0 } else { 0.0 };
        1.0 + (mult - 1.0) * t
    }
    pub fn reset(&mut self) {
        self.held = 0.0;
    }
}

fn normalize_angle(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn angle_diff(a: f32, b: f32) -> f32 {
    normalize_angle(a - b)
}

fn flick_easing(f: f32) -> f32 {
    let f = 1.0 - f;
    1.0 - f * f
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FlickParams {
    pub time: f32,
    pub deadzone: f32,
    pub noise_thresh: f32,
    pub adjust_speed: f32,
}

/// Flick stick: the look stick's direction turns the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlickStick {
    prev_scale: f32,
    prev_angle: f32,
    prev_lerp: f32,
    yaw: f32,
    yaw_delta: f32,
}

impl FlickStick {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Returns the yaw delta (degrees, Quake convention: + = left) for this frame.
    pub fn update(&mut self, raw: [f32; 2], p: &FlickParams, ft: f32) -> f32 {
        let mut angle = normalize_angle(raw[1].atan2(raw[0]).to_degrees() + 90.0);
        let scale = magnitude(raw);
        let active = scale > p.deadzone;
        let was = self.prev_scale > p.deadzone;
        let mut out = 0.0;
        if active != was {
            if !was {
                self.prev_lerp = 0.0;
                self.yaw = angle;
            }
        } else if active {
            let mut d = angle_diff(angle, self.prev_angle);
            if p.noise_thresh > 0.0 {
                let fs = d.abs() / p.noise_thresh;
                if fs < 1.0 {
                    d *= 0.05 + 0.95 * fs * fs;
                    angle = normalize_angle(self.prev_angle + d);
                }
            }
            self.yaw_delta += d;
        }
        let d = if p.adjust_speed > 0.0 { self.yaw_delta * (ft * p.adjust_speed).min(1.0) } else { self.yaw_delta };
        if d.abs() > 0.01 {
            out -= d;
            self.yaw_delta -= d;
        }
        let lerp = if p.time > 0.0 { (self.prev_lerp + ft / p.time).clamp(0.0, 1.0) } else { 1.0 };
        out -= self.yaw * (flick_easing(lerp) - flick_easing(self.prev_lerp));
        self.prev_scale = scale;
        self.prev_angle = angle;
        self.prev_lerp = lerp;
        out
    }
}

/// Soft noise threshold for gyro rates (deg/s).
pub fn gyro_filter(rate: f32, thresh: f32) -> f32 {
    if thresh <= 0.0 {
        return rate;
    }
    let f = rate.abs() / thresh;
    if f < 1.0 { rate * f } else { rate }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadzone_radial() {
        assert_eq!(apply_deadzone([0.1, 0.1], 0.175, 0.02), [0.0, 0.0]);
        let o = apply_deadzone([1.0, 0.0], 0.175, 0.02);
        assert!((o[0] - 1.0).abs() < 1e-6);
        let o = apply_deadzone([0.7, 0.7], 0.175, 0.02);
        assert!((magnitude(o) - 1.0).abs() < 1e-5);
        // direction kept
        assert!((o[0] - o[1]).abs() < 1e-6);
        let o = apply_deadzone([0.5, 0.0], 0.2, 0.0);
        assert!((o[0] - 0.375).abs() < 1e-5);
    }
    #[test]
    fn easing_curve() {
        let o = apply_easing([0.5, 0.0], 2.0);
        assert!((o[0] - 0.25).abs() < 1e-6);
        let o = apply_easing([0.0, -0.5], 1.0);
        assert!((o[1] + 0.5).abs() < 1e-6);
    }
    #[test]
    fn accel_ramp() {
        let mut r = AccelRamp::default();
        assert_eq!(r.update(1.0, 0.95, 0.1, 0.2, 2.0, 0.05), 1.0);
        r.update(1.0, 0.95, 0.1, 0.2, 2.0, 0.05);
        let m = r.update(1.0, 0.95, 0.1, 0.2, 2.0, 0.1);
        assert!((m - 1.5).abs() < 1e-5);
        assert_eq!(r.update(1.0, 0.95, 0.1, 0.2, 2.0, 1.0), 2.0);
        assert_eq!(r.update(0.5, 0.95, 0.1, 0.2, 2.0, 0.1), 1.0);
    }
    #[test]
    fn flick_turns_to_stick_direction() {
        let p = FlickParams { time: 0.1, deadzone: 0.9, noise_thresh: 2.0, adjust_speed: 30.0 };
        let mut f = FlickStick::default();
        let mut total = 0.0;
        for _ in 0..20 {
            total += f.update([1.0, 0.0], &p, 0.01); // stick right
        }
        assert!((total + 90.0).abs() < 1e-3, "{total}");
        assert_eq!(f.update([0.0, 0.0], &p, 0.01), 0.0);
    }
}
