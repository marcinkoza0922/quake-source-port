//! Camera perspective (cl_camera): first person, third person centered
//! behind the player, or over the right/left shoulder. The third-person
//! camera is pulled in when the map is in the way.
use qcore::math::{Vec3, angle_vectors};
use qrender::View;
use qworld::hull::CollisionModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Camera {
    #[default]
    First,
    Center,
    ShoulderRight,
    ShoulderLeft,
}

/// Distance kept between the camera and a wall it was pulled in by.
const WALL_GAP: f32 = 4.0;

impl Camera {
    pub fn from_cvar(v: f32) -> Self {
        match v as i32 {
            1 => Camera::Center,
            2 => Camera::ShoulderRight,
            3 => Camera::ShoulderLeft,
            _ => Camera::First,
        }
    }

    pub fn third_person(self) -> bool {
        self != Camera::First
    }

    /// (back, right, up) from the eye, in map units.
    fn offset(self) -> (f32, f32, f32) {
        match self {
            Camera::First => (0.0, 0.0, 0.0),
            Camera::Center => (80.0, 0.0, 12.0),
            Camera::ShoulderRight => (56.0, 26.0, 8.0),
            Camera::ShoulderLeft => (56.0, -26.0, 8.0),
        }
    }

    /// Moves the first-person view to this camera. The side step is traced
    /// first and the pull-back from there, so a nearby wall can't put the
    /// camera on the other side of it.
    pub fn place(self, view: &mut View, cm: Option<&CollisionModel>, aim: &mut Aim, ft: f32) {
        if !self.third_person() {
            return;
        }
        view.angles[2] = 0.0;
        let (back, right, up) = self.offset();
        let (fwd, rt, _) = angle_vectors(Vec3::from_array(view.angles));
        let eye = Vec3::from_array(view.origin);
        let side = clip(cm, eye, eye + rt * right + Vec3::new(0.0, 0.0, up));
        let cam = clip(cm, side, side - fwd * back);
        view.origin = cam.to_array();
        // Shots leave the eye along `fwd`; turn the camera toward where that
        // ray lands so the crosshair (screen centre) marks the real impact.
        let land = clip(cm, eye, eye + fwd * AIM_RANGE);
        let target = eye + fwd * aim.update((land - eye).length().max(AIM_MIN), ft);
        let d = target - cam;
        let flat = (d.x * d.x + d.y * d.y).sqrt();
        if d.length() > 1.0 {
            view.angles[0] = -d.z.atan2(flat).to_degrees();
            view.angles[1] = d.y.atan2(d.x).to_degrees();
        }
    }
}

/// Convergence distance eased over time: while strafing, the aim ray jumps
/// between near and far surfaces, and following it instantly shakes the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct Aim {
    dist: f32,
}

impl Aim {
    /// Eases toward `target` in log space (~0.12 s time constant); `ft` 0
    /// or a first use snaps.
    fn update(&mut self, target: f32, ft: f32) -> f32 {
        if self.dist <= 0.0 || ft <= 0.0 {
            self.dist = target;
        } else {
            let k = 1.0 - (-ft / 0.12).exp();
            self.dist = (self.dist.ln() + (target.ln() - self.dist.ln()) * k).exp();
        }
        self.dist
    }
}

/// How far the aim ray is traced for crosshair convergence.
const AIM_RANGE: f32 = 8192.0;
/// Closest convergence point: nearer walls would swing the camera hard.
const AIM_MIN: f32 = 128.0;

/// The furthest point from `a` toward `b` that stays WALL_GAP clear of the map.
fn clip(cm: Option<&CollisionModel>, a: Vec3, b: Vec3) -> Vec3 {
    let Some(cm) = cm else { return b };
    let d = b - a;
    let len = d.length();
    if len <= 0.0 {
        return a;
    }
    let tr = cm.hull(0, 0).trace(a, b);
    if tr.startsolid || tr.allsolid {
        return a;
    }
    if tr.fraction >= 1.0 {
        return b;
    }
    a + d * ((tr.fraction * len - WALL_GAP).max(0.0) / len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qworld::hull::{CNode, HPlane, SubModel};
    use qworld::{CONTENTS_EMPTY, CONTENTS_SOLID};

    fn view() -> View {
        View { origin: [0.0; 3], angles: [0.0, 0.0, 30.0], fov_x: 90.0, ..Default::default() }
    }

    #[test]
    fn modes_offset_the_eye() {
        assert!(!Camera::from_cvar(0.0).third_person());
        let mut v = view();
        Camera::First.place(&mut v, None, &mut Aim::default(), 0.0);
        assert_eq!(v.origin, [0.0; 3]);
        let mut v = view();
        Camera::Center.place(&mut v, None, &mut Aim::default(), 0.0);
        let o = v.origin;
        assert!(o[0] < -70.0 && o[1].abs() < 0.01 && o[2] > 0.0);
        assert_eq!(v.angles[2], 0.0);
        let (mut r, mut l) = (view(), view());
        Camera::ShoulderRight.place(&mut r, None, &mut Aim::default(), 0.0);
        Camera::ShoulderLeft.place(&mut l, None, &mut Aim::default(), 0.0);
        // facing +x, right is -y
        assert!(r.origin[1] < -10.0);
        assert!(l.origin[1] > 10.0);
    }

    #[test]
    fn wall_behind_pulls_camera_in() {
        // solid for x < -32
        let sub = SubModel { mins: Vec3::splat(-1000.0), maxs: Vec3::splat(1000.0), origin: Vec3::ZERO, headnode: [0; 4] };
        let cm = CollisionModel {
            planes: vec![HPlane { normal: Vec3::new(1.0, 0.0, 0.0), dist: -32.0, kind: 0 }],
            hull0: vec![CNode { plane: 0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
            submodels: vec![sub],
            ..Default::default()
        };
        let mut v = view();
        Camera::Center.place(&mut v, Some(&cm), &mut Aim::default(), 0.0);
        let x = v.origin[0];
        assert!(x > -32.0 && x < -20.0, "{x}");
    }
}

#[cfg(test)]
mod aim_tests {
    use super::*;
    use qworld::hull::{CNode, HPlane, SubModel};
    use qworld::{CONTENTS_EMPTY, CONTENTS_SOLID};

    #[test]
    fn shoulder_camera_centre_meets_the_eye_aim_point() {
        // solid for x > 300
        let sub = SubModel { mins: Vec3::splat(-1000.0), maxs: Vec3::splat(1000.0), origin: Vec3::ZERO, headnode: [0; 4] };
        let cm = CollisionModel {
            planes: vec![HPlane { normal: Vec3::new(1.0, 0.0, 0.0), dist: 300.0, kind: 0 }],
            hull0: vec![CNode { plane: 0, children: [CONTENTS_SOLID, CONTENTS_EMPTY] }],
            submodels: vec![sub],
            ..Default::default()
        };
        let mut v = View { origin: [0.0; 3], angles: [10.0, 20.0, 0.0], fov_x: 90.0, ..Default::default() };
        let (fwd, _, _) = angle_vectors(Vec3::new(10.0, 20.0, 0.0));
        let land = clip(Some(&cm), Vec3::ZERO, fwd * AIM_RANGE);
        Camera::ShoulderRight.place(&mut v, Some(&cm), &mut Aim::default(), 0.0);
        let cam = Vec3::from_array(v.origin);
        let (cfwd, _, _) = angle_vectors(Vec3::from_array(v.angles));
        let hit = cam + cfwd * (land - cam).length();
        assert!((hit - land).length() < 0.5, "{hit:?} vs {land:?}");
    }
}

#[test]
fn aim_distance_eases_instead_of_jumping() {
    let mut a = Aim::default();
    assert_eq!(a.update(2000.0, 1.0 / 60.0), 2000.0);
    let d = a.update(150.0, 1.0 / 60.0);
    assert!(d > 1000.0 && d < 2000.0, "{d}");
    for _ in 0..120 {
        a.update(150.0, 1.0 / 60.0);
    }
    assert!((a.update(150.0, 1.0 / 60.0) - 150.0).abs() < 1.0);
}
