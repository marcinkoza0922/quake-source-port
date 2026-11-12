//! Small math helpers and the RNG used by the particle system.
use qcore::math::Vec3;
use std::f32::consts::PI;

const SINTABLE_ENTRIES: usize = 128;

/// FTE's low-resolution sin table (r_part_fte.c redefines sin/cos with it,
/// which visibly quantizes circle/spiral spawn shapes).
pub fn psin(x: f32) -> f32 {
    let i = (x * ((SINTABLE_ENTRIES / 2) as f32 / PI)) as i32;
    let a = (i as usize % SINTABLE_ENTRIES) as f32 * PI / (SINTABLE_ENTRIES / 2) as f32;
    a.sin()
}

pub fn pcos(x: f32) -> f32 {
    let i = (x * ((SINTABLE_ENTRIES / 2) as f32 / PI)) as i32;
    let a = (i as usize % SINTABLE_ENTRIES) as f32 * PI / (SINTABLE_ENTRIES / 2) as f32;
    a.cos()
}

/// COM_Rand-like generator: 15-bit outputs (0..=0x7fff).
#[derive(Clone, Debug)]
pub struct Rng(u32);

pub const RAND_MAX: u32 = 0x7fff;

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// COM_Rand.
    pub fn rand(&mut self) -> u32 {
        self.next_u32() >> 17
    }
    /// frandom: 0..=1.
    pub fn frandom(&mut self) -> f32 {
        self.rand() as f32 / RAND_MAX as f32
    }
    /// crandom: -1..=1.
    pub fn crandom(&mut self) -> f32 {
        self.rand() as f32 * (2.0 / RAND_MAX as f32) - 1.0
    }
    /// hrandom: -0.5..=0.5.
    pub fn hrandom(&mut self) -> f32 {
        self.frandom() - 0.5
    }
    /// crand: (rand % 32767) / 16383.5 - 1.
    pub fn crand(&mut self) -> f32 {
        (self.rand() % 32767) as f32 / 16383.5 - 1.0
    }
    /// Full-range float for the update pass (ufrandom).
    pub fn ufrandom(&mut self) -> f32 {
        self.next_u32() as f32 * (1.0 / 4_294_967_296.0)
    }
    pub fn ucrandom(&mut self) -> f32 {
        self.next_u32() as f32 * (2.0 / 4_294_967_296.0) - 1.0
    }
}

/// PerpendicularVector (mathlib.c).
pub fn perpendicular(src: Vec3) -> Vec3 {
    let mut pos = 0;
    let mut minelem = 1.0f32;
    for i in 0..3 {
        if src[i].abs() < minelem {
            pos = i;
            minelem = src[i].abs();
        }
    }
    let mut temp = Vec3::ZERO;
    temp[pos] = 1.0;
    // ProjectPointOnPlane (Quake's version divides by |n|^2 twice).
    let inv = 1.0 / src.length_sq();
    (temp - src * (src.dot(temp) * inv * inv)).normalize()
}

/// RotatePointAroundVector: rotate `point` around unit `dir` by `degrees`.
pub fn rotate_around(dir: Vec3, point: Vec3, degrees: f32) -> Vec3 {
    let (s, c) = degrees.to_radians().sin_cos();
    point * c + dir.cross(point) * s + dir * (dir.dot(point) * (1.0 - c))
}

/// VectorVectors: right/up from a forward vector.
pub fn vector_vectors(forward: Vec3) -> (Vec3, Vec3) {
    let right = if forward.x == 0.0 && forward.y == 0.0 {
        Vec3::new(0.0, if forward.z != 0.0 { -1.0 } else { 0.0 }, 0.0)
    } else {
        Vec3::new(forward.y, -forward.x, 0.0).normalize()
    };
    (right, right.cross(forward))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers() {
        let p = perpendicular(Vec3::new(0.0, 0.0, 1.0));
        assert!(p.dot(Vec3::new(0.0, 0.0, 1.0)).abs() < 1e-6 && (p.length() - 1.0).abs() < 1e-6);
        let p = perpendicular(Vec3::new(1.0, 2.0, 3.0).normalize());
        assert!(p.dot(Vec3::new(1.0, 2.0, 3.0)).abs() < 1e-5);
        let r = rotate_around(Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), 90.0);
        assert!((r - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
        assert!((psin(PI / 2.0) - 1.0).abs() < 1e-6);
        assert!((pcos(-PI) + 1.0).abs() < 1e-6);
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            let f = rng.frandom();
            assert!((0.0..=1.0).contains(&f));
            assert!((-1.0..=1.0).contains(&rng.crand()));
        }
    }
}
