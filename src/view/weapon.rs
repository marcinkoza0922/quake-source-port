//! Shared camera-space placement for stock, mod and add-on view weapons.
use qcore::math::{Vec3, angle_vectors};
use qrender::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Hidden,
    Center,
    Right,
    Left,
}

impl Alignment {
    /// Preserve the classic 0/1 visibility values; 2/3 select a side.
    pub fn from_cvar(value: f32) -> Self {
        match value as i32 {
            0 => Self::Hidden,
            2 => Self::Right,
            3 => Self::Left,
            _ => Self::Center,
        }
    }

    pub fn visible(self) -> bool {
        self != Self::Hidden
    }

    pub fn offset(self, angles: [f32; 3]) -> Vec3 {
        let (_, right, _) = angle_vectors(angles.into());
        right
            * match self {
                Self::Right => 8.0,
                Self::Left => -8.0,
                _ => 0.0,
            }
    }
}

/// Alias entities use inverted pitch (R_RotateForEntity). Both the origin
/// and orientation follow the camera, so looking up/down cannot tip the gun.
pub fn portal_pose(view: View, bob: f32, recoil: f32) -> ([f32; 3], [f32; 3]) {
    let (forward, _, up) = angle_vectors(view.angles.into());
    let origin = Vec3::from(view.origin) + forward * (32.0 - recoil * 2.5) - up * (15.0 + bob);
    (
        origin.to_array(),
        [-view.angles[0], view.angles[1], view.angles[2]],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_and_mod_weapons_share_placement_and_visibility_rules() {
        use crate::assets::{Kind, Level, Model};
        use crate::view::{ViewCvars, ViewState};
        use qnet::protocol::{STAT_HEALTH, STAT_WEAPON};
        let mut cl = qclient::ClientState::new();
        cl.entities.resize(2, Default::default());
        cl.viewentity = 1;
        cl.stats[STAT_HEALTH] = 100;
        let lv = Level {
            models: vec![
                None,
                Some(Model {
                    id: qrender::ModelId(1),
                    kind: Kind::Alias,
                    flags: 0,
                }),
                Some(Model {
                    id: qrender::ModelId(42),
                    kind: Kind::Alias,
                    flags: 0,
                }),
            ],
            ..Default::default()
        };
        let mut cv = ViewCvars {
            bob: 0.0,
            bobcycle: 0.6,
            bobup: 0.5,
            rollangle: 0.0,
            rollspeed: 200.0,
            kicktime: 0.5,
            kickroll: 0.0,
            kickpitch: 0.0,
            fov: 90.0,
            gunkick: false,
            viewmodel: Alignment::Center,
            lerp_models: false,
            lerp_move: false,
            flash: 1.0,
        };
        for model in [1, 2] {
            cl.stats[STAT_WEAPON] = model;
            for pitch in [-80.0, 0.0, 80.0] {
                cl.viewangles = [pitch, 123.0, 0.0];
                let mut state = ViewState::default();
                cv.viewmodel = Alignment::Center;
                let (view, center) = state.calc_refdef(&cl, &cv, &lv, 0.0);
                let center = center.unwrap();
                for setting in [2.0, 3.0] {
                    cv.viewmodel = Alignment::from_cvar(setting);
                    let (_, side) = state.calc_refdef(&cl, &cv, &lv, 0.0);
                    let side = side.unwrap();
                    assert_eq!(side.model, center.model);
                    assert_eq!(side.angles, center.angles);
                    assert!(
                        (Vec3::from(side.origin)
                            - Vec3::from(center.origin)
                            - cv.viewmodel.offset(view.angles))
                        .length()
                            < 0.0001
                    );
                }
                cv.viewmodel = Alignment::Hidden;
                assert!(state.calc_refdef(&cl, &cv, &lv, 0.0).1.is_none());
                cv.viewmodel = Alignment::Center;
                cl.items = crate::sbar::IT_INVISIBILITY;
                assert!(state.calc_refdef(&cl, &cv, &lv, 0.0).1.is_none());
                cl.items = 0;
                cl.stats[STAT_HEALTH] = 0;
                assert!(state.calc_refdef(&cl, &cv, &lv, 0.0).1.is_none());
                cl.stats[STAT_HEALTH] = 100;
            }
        }
    }

    #[test]
    fn portal_stays_fixed_in_camera_space_at_extreme_look_angles() {
        for pitch in [-89.0, -60.0, 0.0, 60.0, 89.0] {
            for yaw in [0.0, 90.0, 245.0] {
                for roll in [-12.0, 0.0, 12.0] {
                    let view = View {
                        origin: [123.0, -45.0, 76.0],
                        angles: [pitch, yaw, roll],
                        ..Default::default()
                    };
                    let (origin, angles) = portal_pose(view, 0.0, 0.0);
                    let (f, r, u) = angle_vectors(view.angles.into());
                    let delta = Vec3::from(origin) - Vec3::from(view.origin);
                    assert!((delta.dot(f) - 32.0).abs() < 0.0001);
                    assert!(delta.dot(r).abs() < 0.0001);
                    assert!((delta.dot(u) + 15.0).abs() < 0.0001);
                    // Renderer local +X after Rz(yaw) * Ry(-pitch).
                    let (sy, cy) = angles[1].to_radians().sin_cos();
                    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
                    let axis = Vec3::new(cp * cy, cp * sy, -sp);
                    assert!((axis - f).length() < 0.0001);
                    for setting in [2.0, 3.0] {
                        let side = Alignment::from_cvar(setting).offset(view.angles);
                        assert!(side.dot(f).abs() < 0.0001);
                        assert!(side.dot(u).abs() < 0.0001);
                        assert!(
                            (side.dot(r) - if setting == 2.0 { 8.0 } else { -8.0 }).abs() < 0.0001
                        );
                    }
                }
            }
        }
        assert!(!Alignment::from_cvar(0.0).visible());
        assert_eq!(Alignment::from_cvar(1.0).offset([0.0; 3]), Vec3::ZERO);
    }
}
