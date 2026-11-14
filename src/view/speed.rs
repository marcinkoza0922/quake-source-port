//! Velocity-driven peripheral blur, independent of camera or portal displacement.

/// Ordinary movement stays clear; long falls/launches ramp smoothly to full blur.
pub(super) fn strength(velocity: [f32; 3]) -> f32 {
    let speed = velocity.into_iter().map(|v| v * v).sum::<f32>().sqrt();
    let t = ((speed - 800.0) / 1200.0).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_and_jumping_stay_clear_but_fast_falls_and_launches_blur() {
        for v in [[0.0; 3], [320.0, 320.0, 270.0], [800.0, 0.0, 0.0]] {
            assert_eq!(strength(v), 0.0);
        }
        assert_eq!(strength([0.0, 0.0, -1400.0]), 0.5);
        assert_eq!(strength([1400.0, 0.0, 0.0]), 0.5);
        assert!(strength([700.0, 700.0, 700.0]) > 0.0);
        assert_eq!(strength([0.0, 0.0, -2000.0]), 1.0);
        assert_eq!(strength([10000.0; 3]), 1.0);
    }

    #[test]
    fn view_gates_speed_blur_and_does_not_carry_it_to_another_player() {
        use crate::view::{ViewCvars, ViewState, weapon::Alignment};
        use qnet::protocol::STAT_HEALTH;

        let mut cl = qclient::ClientState::new();
        cl.stats[STAT_HEALTH] = 100;
        cl.velocity = [0.0, 0.0, -2000.0];
        let cv = ViewCvars {
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
            speed_blur: true,
            viewmodel: Alignment::Hidden,
            lerp_models: false,
            lerp_move: false,
            flash: 1.0,
        };
        let lv = crate::assets::Level::default();
        let mut state = ViewState::default();
        assert_eq!(state.calc_refdef(&cl, &cv, &lv, 0.0).0.speed_blur, 1.0);
        assert_eq!(
            state
                .calc_refdef(&cl, &cv.reduce_motion(true), &lv, 0.0)
                .0
                .speed_blur,
            0.0
        );
        let disabled = ViewCvars {
            speed_blur: false,
            ..cv
        };
        assert_eq!(
            state.calc_refdef(&cl, &disabled, &lv, 0.0).0.speed_blur,
            0.0
        );
        cl.stats[STAT_HEALTH] = 0;
        assert_eq!(state.calc_refdef(&cl, &cv, &lv, 0.0).0.speed_blur, 0.0);
        cl.stats[STAT_HEALTH] = 100;
        cl.intermission = 1;
        assert_eq!(state.calc_refdef(&cl, &cv, &lv, 0.0).0.speed_blur, 0.0);
        cl.intermission = 0;
        cl.velocity = [0.0; 3];
        assert_eq!(state.calc_refdef(&cl, &cv, &lv, 0.0).0.speed_blur, 0.0);
    }
}
