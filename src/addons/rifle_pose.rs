//! Where the assault rifle's view model sits: at the hip, sights up, kicking
//! and being reloaded. Everything is relative to the camera's axes, so the
//! pose is the same at any pitch.
use qcore::math::{Vec3, angle_vectors};
use qrender::View;

use super::rifle_model::{APERTURE, LENS, TUBE};

/// The sight's middle in camera space (forward, right, up) at the hip, with
/// the Viewmodels setting at Center and at Right (Left mirrors it).
const CENTER: Vec3 = Vec3::new(20.0, 0.0, -8.8);
const HIP: Vec3 = Vec3::new(18.5, 6.2, -6.2);
/// Hip rotation: nose up, towards the middle of the screen, and roll.
const CENTER_TILT: [f32; 3] = [6.0, 0.0, 0.0];
const HIP_TILT: [f32; 3] = [2.0, 6.5, -6.0];
/// Aimed: distance of the sight at a 90 degree view zoomed 1.3 times, and
/// the range it is kept in at others so it looks much the same size.
const AIM_DIST: f32 = 14.0;
const AIM_RANGE: (f32, f32) = (10.5, 17.0);
const AIM_TAN: f32 = 0.577;

/// What moves the rifle this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    /// Sight raised, 0..1 (eased).
    pub aim: f32,
    /// The Viewmodels setting's side offset at the hip.
    pub side: f32,
    /// Seconds since the last shot.
    pub shot: f32,
    /// Reload progress 0..1, and whether a round has to be chambered.
    pub reload: Option<(f32, bool)>,
    pub time: f32,
    /// Horizontal speed.
    pub speed: f32,
}

/// The placed rifle: model space to world.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub origin: Vec3,
    /// Model X (muzzle), Y (left), Z (up) in the world.
    pub axes: [Vec3; 3],
}

impl Pose {
    pub fn at(&self, p: Vec3) -> Vec3 {
        self.origin + self.dir(p)
    }
    pub fn dir(&self, p: Vec3) -> Vec3 {
        self.axes[0] * p.x + self.axes[1] * p.y + self.axes[2] * p.z
    }
    /// Entity angles (R_RotateForEntity).
    pub fn angles(&self) -> [f32; 3] {
        super::cable_fx::angles_basis(self.axes[0], self.axes[2])
    }
    /// How visible a collimated dot along `dir` is from `eye`: 1 well inside
    /// both ends of the tube, 0 behind its rim.
    pub fn through(&self, eye: Vec3, dir: Vec3) -> f32 {
        let mut a = 1.0f32;
        for end in [-TUBE / 2.0, TUBE / 2.0] {
            let c = self.at(LENS + Vec3::new(end, 0.0, 0.0)) - eye;
            let along = c.dot(dir);
            if along < 1.0 {
                return 0.0;
            }
            let off = (c - dir * along).length() / APERTURE;
            a = a.min(((1.0 - off) / 0.08).clamp(0.0, 1.0));
        }
        a
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The magazine's offset (model space) during a reload; None while it is out
/// of sight.
pub fn magazine(u: f32) -> Option<Vec3> {
    let (out, back) = (smooth(0.14, 0.34, u), smooth(0.44, 0.64, u));
    if u > 0.34 && u < 0.44 {
        return None;
    }
    let k = if u < 0.4 { out * out } else { 1.0 - back };
    Some(Vec3::new(1.5, 0.5, -15.0) * k)
}

/// When (reload progress) the magazine leaves, seats and the bolt closes.
pub const MAG_OUT: f32 = 0.12;
pub const MAG_IN: f32 = 0.56;
pub const BOLT: f32 = 0.78;

pub fn pose(view: &View, m: &Motion) -> Pose {
    let (f, r, u) = angle_vectors(view.angles.into());
    let (k, hip) = (m.aim, 1.0 - m.aim);
    // -1 left, 0 centre, 1 right
    let side = (m.side / 8.0).clamp(-1.0, 1.0);
    let off = side.abs();
    let rest = CENTER * (1.0 - off) + Vec3::new(HIP.x, HIP.y * side.signum(), HIP.z) * off;
    let tilt0 = [
        CENTER_TILT[0] + (HIP_TILT[0] - CENTER_TILT[0]) * off,
        HIP_TILT[1] * side,
        HIP_TILT[2] * side,
    ];
    // idle sway and walking bob, almost still when aimed
    let calm = 1.0 - 0.9 * k;
    let walk = (m.speed / 320.0).clamp(0.0, 1.0) * calm;
    let step = m.time * 9.0;
    let sway = Vec3::new(
        0.0,
        (m.time * 1.1).sin() * 0.14 * calm + step.sin() * 0.45 * walk,
        (m.time * 1.7).sin() * 0.12 * calm - (step.cos().abs() - 0.6) * 0.45 * walk,
    );
    // recoil: back and a little up, recovering between shots
    let kick = (-m.shot * 17.0).exp();
    let back = kick * (1.5 * hip + 0.55 * k);
    let mut pitch = tilt0[0] * hip + kick * (1.6 * hip + 0.35 * k) + sway.z * 0.8;
    let mut yaw = tilt0[1] * hip + sway.y * 0.6;
    let mut roll = tilt0[2] * hip + kick * 0.6 * hip;
    let half = (view.fov_x.clamp(1.0, 179.0).to_radians() * 0.5).tan() * 0.75;
    let dist = (AIM_DIST * AIM_TAN / half).clamp(AIM_RANGE.0, AIM_RANGE.1);
    let mut at = rest * hip + Vec3::new(dist, 0.0, 0.0) * k + sway - Vec3::new(back, 0.0, 0.0);
    // the sight swings up in a shallow arc
    at.z -= (k * hip) * 2.0;
    if let Some((x, chamber)) = m.reload {
        let tilt = smooth(0.0, 0.14, x) * (1.0 - smooth(0.86, 1.0, x));
        let seat = smooth(MAG_IN + 0.02, MAG_IN + 0.06, x) * (1.0 - smooth(MAG_IN + 0.06, MAG_IN + 0.16, x));
        let pull = if chamber { smooth(BOLT - 0.04, BOLT, x) * (1.0 - smooth(BOLT, BOLT + 0.07, x)) } else { 0.0 };
        pitch += tilt * 17.0 + seat * 3.0 - pull * 2.5;
        yaw += tilt * 15.0;
        roll += tilt * 40.0 + pull * 5.0;
        at = at + Vec3::new(5.5, 1.0, 4.6) * tilt + Vec3::new(0.0, 0.0, 0.8) * seat - Vec3::new(1.2, 0.0, 0.0) * pull;
    }
    let (x, y, z) = basis(pitch, yaw, roll);
    // camera space is forward, right, up; the model's Y points left
    let world = |v: Vec3| f * v.x - r * v.y + u * v.z;
    let axes = [world(x), world(y), world(z)];
    let pivot = Vec3::from(view.origin) + f * at.x + r * at.y + u * at.z;
    let mut p = Pose { origin: Vec3::ZERO, axes };
    p.origin = pivot - p.dir(LENS);
    p
}

/// Axes (forward, left, up) turned by pitch (nose up), yaw (nose left), then
/// roll about the barrel (top to the right).
fn basis(pitch: f32, yaw: f32, roll: f32) -> (Vec3, Vec3, Vec3) {
    let ((sp, cp), (sy, cy), (sr, cr)) = (pitch.to_radians().sin_cos(), yaw.to_radians().sin_cos(), roll.to_radians().sin_cos());
    let x = Vec3::new(cp * cy, cp * sy, sp);
    let left = Vec3::new(-sy, cy, 0.0);
    let up = x.cross(left);
    (x, left * cr + up * sr, up * cr - left * sr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_aimed_sight_is_centred_at_every_view_angle() {
        for pitch in [-85.0, -30.0, 0.0, 60.0, 85.0] {
            for (yaw, roll, fov) in [(0.0, 0.0, 75.0), (200.0, -3.0, 60.0), (90.0, 5.0, 120.0)] {
                let view = View { origin: [10.0, -20.0, 30.0], angles: [pitch, yaw, roll], fov_x: fov, ..Default::default() };
                let p = pose(&view, &Motion { aim: 1.0, shot: 9.0, side: 8.0, ..Default::default() });
                let (f, r, u) = angle_vectors(view.angles.into());
                let c = p.at(LENS) - Vec3::from(view.origin);
                assert!(c.dot(r).abs() < 0.02 && c.dot(u).abs() < 0.02 && c.dot(f) > 10.0, "{c:?}");
                assert!((p.axes[0] - f).length() < 0.01 && (p.axes[2] - u).length() < 0.01);
                assert_eq!(p.through(Vec3::from(view.origin), f), 1.0);
                // the entity angles give the same axes back
                let a = p.angles();
                let (sy, cy) = a[1].to_radians().sin_cos();
                let (sp, cp) = a[0].to_radians().sin_cos();
                assert!((Vec3::new(cp * cy, cp * sy, sp) - p.axes[0]).length() < 0.01);
            }
        }
    }

    #[test]
    fn no_dot_from_the_hip_and_a_firing_sight_keeps_it() {
        let view = View { fov_x: 90.0, ..Default::default() };
        let f = Vec3::new(1.0, 0.0, 0.0);
        assert_eq!(pose(&view, &Motion { shot: 9.0, ..Default::default() }).through(Vec3::ZERO, f), 0.0);
        for shot in [0.0, 0.03, 0.08] {
            let p = pose(&view, &Motion { aim: 1.0, shot, speed: 320.0, time: 3.3, ..Default::default() });
            assert!(p.through(Vec3::ZERO, f) > 0.9, "{shot}");
        }
    }

    #[test]
    fn the_magazine_leaves_and_comes_back() {
        assert_eq!(magazine(0.0), Some(Vec3::ZERO * 1.0));
        assert_eq!(magazine(0.4), None);
        assert!(magazine(1.0).unwrap().length() < 1e-3);
        assert!(magazine(0.3).unwrap().z < -5.0);
    }
}
