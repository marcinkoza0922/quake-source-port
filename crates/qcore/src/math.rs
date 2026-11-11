//! Vector math following Quake conventions (angles in degrees: pitch, yaw, roll).
use std::ops::{Add, AddAssign, Div, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign};

pub const PITCH: usize = 0;
pub const YAW: usize = 1;
pub const ROLL: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }
    pub fn from_array(a: [f32; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }
    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }
    /// Normalizes; returns zero vector for zero length (like VectorNormalize).
    pub fn normalize(self) -> Self {
        self.normalize_len().0
    }
    /// Returns (normalized, original length).
    pub fn normalize_len(self) -> (Self, f32) {
        let l = self.length();
        if l == 0.0 { (Self::ZERO, 0.0) } else { (self * (1.0 / l), l) }
    }
    pub fn mul_add(self, scale: f32, b: Self) -> Self {
        self + b * scale
    }
    pub fn mul_elem(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    pub fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    pub fn lerp(self, o: Self, t: f32) -> Self {
        self + (o - self) * t
    }
}

macro_rules! binop {
    ($tr:ident, $f:ident, $atr:ident, $af:ident, $op:tt) => {
        impl $tr for Vec3 {
            type Output = Vec3;
            fn $f(self, o: Vec3) -> Vec3 { Vec3::new(self.x $op o.x, self.y $op o.y, self.z $op o.z) }
        }
        impl $atr for Vec3 {
            fn $af(&mut self, o: Vec3) { *self = *self $op o; }
        }
    };
}
binop!(Add, add, AddAssign, add_assign, +);
binop!(Sub, sub, SubAssign, sub_assign, -);

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Mul<Vec3> for f32 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}
impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl Index<usize> for Vec3 {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        match i {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            _ => panic!("Vec3 index {i}"),
        }
    }
}
impl IndexMut<usize> for Vec3 {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        match i {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => panic!("Vec3 index {i}"),
        }
    }
}
impl From<[f32; 3]> for Vec3 {
    fn from(a: [f32; 3]) -> Self {
        Self::from_array(a)
    }
}

/// AngleVectors: returns (forward, right, up).
pub fn angle_vectors(angles: Vec3) -> (Vec3, Vec3, Vec3) {
    let d2r = std::f32::consts::PI / 180.0;
    let (sy, cy) = (angles[YAW] * d2r).sin_cos();
    let (sp, cp) = (angles[PITCH] * d2r).sin_cos();
    let (sr, cr) = (angles[ROLL] * d2r).sin_cos();
    let forward = Vec3::new(cp * cy, cp * sy, -sp);
    let right = Vec3::new(
        -sr * sp * cy + cr * sy,
        -sr * sp * sy - cr * cy,
        -sr * cp,
    );
    let up = Vec3::new(cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp);
    (forward, right, up)
}

/// vectoangles (QuakeC builtin semantics; pitch positive = up).
pub fn vec_to_angles(v: Vec3) -> Vec3 {
    if v.y == 0.0 && v.x == 0.0 {
        return Vec3::new(if v.z > 0.0 { 90.0 } else { 270.0 }, 0.0, 0.0);
    }
    let mut yaw = v.y.atan2(v.x).to_degrees().trunc();
    if yaw < 0.0 {
        yaw += 360.0;
    }
    let fwd = (v.x * v.x + v.y * v.y).sqrt();
    let mut pitch = v.z.atan2(fwd).to_degrees().trunc();
    if pitch < 0.0 {
        pitch += 360.0;
    }
    Vec3::new(pitch, yaw, 0.0)
}

pub fn angle_mod(a: f32) -> f32 {
    (360.0 / 65536.0) * (((a * (65536.0 / 360.0)) as i32) & 65535) as f32
}

pub const PLANE_X: u8 = 0;
pub const PLANE_Y: u8 = 1;
pub const PLANE_Z: u8 = 2;
pub const PLANE_ANYX: u8 = 3;
pub const PLANE_ANYY: u8 = 4;
pub const PLANE_ANYZ: u8 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f32,
    /// PLANE_X.. for axial fast paths.
    pub ty: u8,
    /// Sign bits of normal (bit i set when normal[i] < 0).
    pub signbits: u8,
}

impl Plane {
    pub fn new(normal: Vec3, dist: f32) -> Self {
        let ty = if normal.x == 1.0 {
            PLANE_X
        } else if normal.y == 1.0 {
            PLANE_Y
        } else if normal.z == 1.0 {
            PLANE_Z
        } else {
            let a = Vec3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());
            if a.x >= a.y && a.x >= a.z {
                PLANE_ANYX
            } else if a.y >= a.z {
                PLANE_ANYY
            } else {
                PLANE_ANYZ
            }
        };
        Self::with_type(normal, dist, ty)
    }
    pub fn with_type(normal: Vec3, dist: f32, ty: u8) -> Self {
        let mut signbits = 0;
        for i in 0..3 {
            if normal[i] < 0.0 {
                signbits |= 1 << i;
            }
        }
        Self { normal, dist, ty, signbits }
    }
    /// Signed distance from point to plane (axial fast path).
    pub fn distance(&self, p: Vec3) -> f32 {
        if self.ty < 3 {
            p[self.ty as usize] - self.dist
        } else {
            self.normal.dot(p) - self.dist
        }
    }
    /// BoxOnPlaneSide: 1 = front, 2 = back, 3 = both.
    pub fn box_on_side(&self, mins: Vec3, maxs: Vec3) -> i32 {
        if self.ty < 3 {
            let t = self.ty as usize;
            if self.dist <= mins[t] {
                return 1;
            }
            if self.dist >= maxs[t] {
                return 2;
            }
            return 3;
        }
        let (mut near, mut far) = (Vec3::ZERO, Vec3::ZERO);
        for i in 0..3 {
            if self.normal[i] < 0.0 {
                near[i] = maxs[i];
                far[i] = mins[i];
            } else {
                near[i] = mins[i];
                far[i] = maxs[i];
            }
        }
        let mut s = 0;
        if self.normal.dot(far) >= self.dist {
            s = 1;
        }
        if self.normal.dot(near) < self.dist {
            s |= 2;
        }
        s
    }
}

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub mins: Vec3,
    pub maxs: Vec3,
}

impl Default for Bounds {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Bounds {
    /// ClearBounds equivalent.
    pub const EMPTY: Bounds = Bounds { mins: Vec3::splat(99999.0), maxs: Vec3::splat(-99999.0) };
    pub fn new(mins: Vec3, maxs: Vec3) -> Self {
        Self { mins, maxs }
    }
    pub fn add_point(&mut self, p: Vec3) {
        self.mins = self.mins.min(p);
        self.maxs = self.maxs.max(p);
    }
    pub fn union(self, o: Bounds) -> Bounds {
        Bounds::new(self.mins.min(o.mins), self.maxs.max(o.maxs))
    }
    pub fn is_empty(&self) -> bool {
        self.mins.x > self.maxs.x || self.mins.y > self.maxs.y || self.mins.z > self.maxs.z
    }
    pub fn intersects(&self, o: &Bounds) -> bool {
        self.mins.x <= o.maxs.x
            && self.maxs.x >= o.mins.x
            && self.mins.y <= o.maxs.y
            && self.maxs.y >= o.mins.y
            && self.mins.z <= o.maxs.z
            && self.maxs.z >= o.mins.z
    }
    pub fn contains(&self, p: Vec3) -> bool {
        (0..3).all(|i| p[i] >= self.mins[i] && p[i] <= self.maxs[i])
    }
    pub fn center(&self) -> Vec3 {
        (self.mins + self.maxs) * 0.5
    }
    /// RadiusFromBounds.
    pub fn radius(&self) -> f32 {
        let mut c = Vec3::ZERO;
        for i in 0..3 {
            c[i] = self.mins[i].abs().max(self.maxs[i].abs());
        }
        c.length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }
    #[test]
    fn vec_ops() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(a.dot(a), 14.0);
        assert_eq!(Vec3::new(1.0, 0.0, 0.0).cross(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(0.0, 0.0, 1.0));
        assert!((Vec3::new(3.0, 4.0, 0.0).normalize().length() - 1.0).abs() < 1e-6);
        assert_eq!(Vec3::ZERO.normalize(), Vec3::ZERO);
        assert_eq!(-a + a * 2.0, a);
    }
    #[test]
    fn angles() {
        let (f, r, u) = angle_vectors(Vec3::ZERO);
        assert!(close(f, Vec3::new(1.0, 0.0, 0.0)));
        assert!(close(r, Vec3::new(0.0, -1.0, 0.0)));
        assert!(close(u, Vec3::new(0.0, 0.0, 1.0)));
        let (f, ..) = angle_vectors(Vec3::new(0.0, 90.0, 0.0));
        assert!(close(f, Vec3::new(0.0, 1.0, 0.0)));
        let (f, ..) = angle_vectors(Vec3::new(90.0, 0.0, 0.0));
        assert!(close(f, Vec3::new(0.0, 0.0, -1.0))); // positive pitch looks down
        assert_eq!(vec_to_angles(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(0.0, 90.0, 0.0));
    }
    #[test]
    fn planes_bounds() {
        let p = Plane::new(Vec3::new(0.0, 0.0, 1.0), 10.0);
        assert_eq!(p.ty, PLANE_Z);
        let b = Bounds::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        assert_eq!(p.box_on_side(b.mins, b.maxs), 2);
        let p2 = Plane::new(Vec3::new(1.0, 1.0, 0.0).normalize(), 0.0);
        assert_eq!(p2.box_on_side(b.mins, b.maxs), 3);
        assert_eq!(p2.signbits, 0);
        let mut e = Bounds::EMPTY;
        assert!(e.is_empty());
        e.add_point(Vec3::new(1.0, 2.0, 3.0));
        assert!(e.contains(Vec3::new(1.0, 2.0, 3.0)));
        assert!(!b.intersects(&e));
        assert!(b.union(e).intersects(&e));
    }
}
