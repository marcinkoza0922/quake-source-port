//! Small 3x3 / quaternion helpers for the debris simulation.
use qcore::math::Vec3;

/// Row-major 3x3: `m[r][c]`, applied as `m * v`.
pub type Mat3 = [[f32; 3]; 3];

pub fn mul3(a: &Mat3, b: &Mat3) -> Mat3 {
    std::array::from_fn(|r| std::array::from_fn(|c| a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c]))
}

pub fn apply(m: &Mat3, v: Vec3) -> Vec3 {
    Vec3::new(
        m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
        m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
        m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
    )
}

/// The renderer's alias entity rotation (yaw, inverted pitch, roll).
pub fn entity_rotation(angles: [f32; 3]) -> Mat3 {
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    let rz = [[cy, -sy, 0.0], [sy, cy, 0.0], [0.0, 0.0, 1.0]];
    let ry = [[cp, 0.0, sp], [0.0, 1.0, 0.0], [-sp, 0.0, cp]];
    let rx = [[1.0, 0.0, 0.0], [0.0, cr, -sr], [0.0, sr, cr]];
    mul3(&mul3(&rz, &ry), &rx)
}

/// Unit quaternion (x, y, z, w).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat(pub [f32; 4]);

impl Quat {
    pub const IDENTITY: Quat = Quat([0.0, 0.0, 0.0, 1.0]);

    pub fn axis_angle(axis: Vec3, angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat([axis.x * s, axis.y * s, axis.z * s, c])
    }

    pub fn mul(self, o: Quat) -> Quat {
        let [ax, ay, az, aw] = self.0;
        let [bx, by, bz, bw] = o.0;
        Quat([
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        ])
    }

    pub fn normalize(self) -> Quat {
        let l = self.0.iter().map(|v| v * v).sum::<f32>().sqrt();
        if l < 1e-12 { Quat::IDENTITY } else { Quat(self.0.map(|v| v / l)) }
    }

    pub fn to_mat(self) -> Mat3 {
        let [x, y, z, w] = self.0;
        [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
            [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
            [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
        ]
    }

    /// Integrates angular velocity `w` (radians/s) over `dt`.
    pub fn integrate(self, w: Vec3, dt: f32) -> Quat {
        let (axis, a) = w.normalize_len();
        if a * dt < 1e-6 {
            return self;
        }
        Quat::axis_angle(axis, a * dt).mul(self).normalize()
    }
}

fn col(m: &Mat3, c: usize) -> Vec3 {
    Vec3::new(m[0][c], m[1][c], m[2][c])
}

/// Rotational part of `a` (Müller et al. 2016, "A Robust Method to Extract
/// the Rotational Part of Deformations"), warm-started from `q`.
pub fn extract_rotation(a: &Mat3, mut q: Quat, iters: usize) -> Quat {
    let (a0, a1, a2) = (col(a, 0), col(a, 1), col(a, 2));
    for _ in 0..iters {
        let r = q.to_mat();
        let (r0, r1, r2) = (col(&r, 0), col(&r, 1), col(&r, 2));
        let num = r0.cross(a0) + r1.cross(a1) + r2.cross(a2);
        let den = (r0.dot(a0) + r1.dot(a1) + r2.dot(a2)).abs() + 1e-9;
        let w = num / den;
        let (axis, ang) = w.normalize_len();
        if ang < 1e-7 {
            break;
        }
        q = Quat::axis_angle(axis, ang).mul(q).normalize();
    }
    q
}

/// Outer-product accumulate: `m += a * b^T * w`.
pub fn add_outer(m: &mut Mat3, a: Vec3, b: Vec3, w: f32) {
    for (r, ar) in [a.x, a.y, a.z].into_iter().enumerate() {
        m[r][0] += ar * b.x * w;
        m[r][1] += ar * b.y * w;
        m[r][2] += ar * b.z * w;
    }
}

/// Nearest `AVERTEXNORMALS` index through a 32^3 cube lookup (built once).
pub fn normal_index(n: Vec3) -> u8 {
    use std::sync::OnceLock;
    const N: usize = 32;
    static LUT: OnceLock<Vec<u8>> = OnceLock::new();
    let lut = LUT.get_or_init(|| {
        let mut t = vec![0u8; N * N * N];
        for (i, slot) in t.iter_mut().enumerate() {
            let c = |k: usize| (k as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let v = Vec3::new(c(i % N), c(i / N % N), c(i / (N * N))).normalize();
            *slot = nearest_normal(v);
        }
        t
    });
    let q = |v: f32| (((v.clamp(-1.0, 1.0) + 1.0) * 0.5 * N as f32) as usize).min(N - 1);
    lut[q(n.x) + q(n.y) * N + q(n.z) * N * N]
}

fn nearest_normal(v: Vec3) -> u8 {
    let mut best = (f32::MIN, 0u8);
    for (i, a) in qcore::anorms::AVERTEXNORMALS.iter().enumerate() {
        let d = v.dot(Vec3::from(*a));
        if d > best.0 {
            best = (d, i as u8);
        }
    }
    best.1
}

pub fn anorm(i: u8) -> Vec3 {
    Vec3::from(qcore::anorms::AVERTEXNORMALS[(i as usize).min(qcore::anorms::NUMVERTEXNORMALS - 1)])
}

/// Small deterministic PRNG (xorshift32).
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [-1, 1).
    pub fn signed(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_rotation_recovers_rotation() {
        let q = Quat::axis_angle(Vec3::new(0.3, 0.5, 0.8).normalize(), 1.1);
        let m = q.to_mat();
        let r = extract_rotation(&m, Quat::IDENTITY, 40).to_mat();
        for i in 0..3 {
            for j in 0..3 {
                assert!((m[i][j] - r[i][j]).abs() < 1e-3, "{m:?} {r:?}");
            }
        }
    }

    #[test]
    fn normal_lut_close_to_exact() {
        let v = Vec3::new(0.2, -0.7, 0.4).normalize();
        assert!(anorm(normal_index(v)).dot(v) > 0.95);
    }
}
