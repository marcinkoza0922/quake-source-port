//! Explicit local-play auto-aim, independent of gamepad slowdown assistance.
use crate::aim::{self, AimContext, AimTarget, TargetKind};

type V3 = [f32; 3];

#[derive(Clone, Copy, Debug)]
pub struct LockOnParams {
    pub enabled: bool,
    /// The host must reject online games, including locally hosted ones.
    pub allowed: bool,
    pub toggle: bool,
    pub reacquire: bool,
    /// Acquisition cone half-angle in degrees, for continuous mode only.
    pub cone: f32,
    pub range: f32,
    /// Horizontal and vertical field of view in degrees.
    pub fov: [f32; 2],
    pub target_players: bool,
}

impl Default for LockOnParams {
    fn default() -> Self {
        Self { enabled: false, allowed: false, toggle: false, reacquire: false, cone: 8.0, range: 2048.0, fov: [90.0, 74.0], target_players: false }
    }
}

#[derive(Default, Debug)]
pub struct LockOn {
    pub target: Option<u32>,
    /// Whether continuous mode is switched on (or the hold button is held).
    pub active: bool,
    held: bool,
    cycle: bool,
    toggle: bool,
    spent: bool,
}

fn dot(a: V3, b: V3) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}

fn angle_delta(a: f32, b: f32) -> f32 {
    (a - b + 180.0).rem_euclid(360.0) - 180.0
}

/// Distance and crosshair angle of a visible, eligible target.
fn geometry(p: &LockOnParams, ctx: &AimContext, angles: V3, t: &AimTarget) -> Option<(f32, f32)> {
    if t.kind == TargetKind::Player && !p.target_players {
        return None;
    }
    let center = t.center();
    let d = [0, 1, 2].map(|i| center[i] - ctx.eye[i]);
    let distance = dot(d, d).sqrt();
    if distance <= 0.001 || distance > p.range.max(0.0) {
        return None;
    }
    let forward = aim::forward(angles);
    let yaw = angles[1].to_radians();
    let right = [-yaw.sin(), yaw.cos(), 0.0];
    let up = [forward[1] * right[2] - forward[2] * right[1], forward[2] * right[0] - forward[0] * right[2], forward[0] * right[1] - forward[1] * right[0]];
    let depth = dot(d, forward);
    if depth <= 0.0
        || dot(d, right).abs() > depth * (p.fov[0].clamp(1.0, 179.0) * 0.5).to_radians().tan()
        || dot(d, up).abs() > depth * (p.fov[1].clamp(1.0, 179.0) * 0.5).to_radians().tan()
        || !ctx.vis.visible(ctx.eye, center)
    {
        return None;
    }
    Some((distance, (depth / distance).clamp(-1.0, 1.0).acos().to_degrees()))
}

impl LockOn {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Returns exact `[yaw, pitch]` correction for the final input view angles.
    /// `held` and `cycle` are button levels; edges are detected here.
    pub fn update(&mut self, p: &LockOnParams, ctx: Option<&AimContext>, angles: V3, held: bool, cycle: bool) -> Option<[f32; 2]> {
        let pressed = held && !self.held;
        let cycle_pressed = cycle && !self.cycle;
        self.held = held;
        self.cycle = cycle;
        if !p.enabled || !p.allowed || ctx.is_none() || p.toggle != self.toggle {
            self.target = None;
            self.active = false;
        }
        self.toggle = p.toggle;
        if !p.enabled || !p.allowed {
            return None;
        }
        let ctx = ctx?;
        if p.toggle {
            if pressed {
                self.active = !self.active;
            }
        } else {
            self.active = held;
        }
        if !held || pressed { self.spent = false; }
        if !self.active {
            self.target = None;
            return None;
        }
        // IDs survive compaction of the host's live-target list.
        let previous = self.target;
        let current = previous.and_then(|id| ctx.targets.iter().find(|t| t.id == id && {
            let d = [0, 1, 2].map(|i| t.center()[i] - ctx.eye[i]);
            dot(d, d).sqrt() <= p.range.max(0.0)
                && (t.kind != TargetKind::Player || p.target_players)
        }));
        if previous.is_some() && current.is_none() && !p.toggle && !p.reacquire {
            self.spent = true;
        }
        self.target = current.map(|t| t.id);
        if cycle_pressed && let Some(current) = self.target {
            // Stable ID order means repeated presses visit every visible enemy.
            let eligible = || ctx.targets.iter().filter(|t| t.id != current && geometry(p, ctx, angles, t).is_some());
            self.target = eligible().filter(|t| t.id > current).min_by_key(|t| t.id)
                .or_else(|| eligible().min_by_key(|t| t.id)).map(|t| t.id).or(Some(current));
        }
        if self.spent && !p.toggle && !p.reacquire { return None; }
        if self.target.is_none() {
            self.target = ctx.targets.iter().filter_map(|t| {
                let (distance, angle) = geometry(p, ctx, angles, t)?;
                if p.toggle && angle > p.cone.clamp(0.0, 90.0) {
                    return None;
                }
                Some((t.id, distance))
            }).min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))).map(|t| t.0);
        }
        let t = ctx.targets.iter().find(|t| Some(t.id) == self.target)?;
        let center = t.center();
        let aim = aim::vec_angles([0, 1, 2].map(|i| center[i] - ctx.eye[i]));
        Some([angle_delta(aim[1], angles[1]), angle_delta(aim[0], angles[0])])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim::{NoWorld, Visibility};

    fn target(id: u32, origin: V3) -> AimTarget {
        AimTarget { id, origin, mins: [0.0; 3], maxs: [0.0; 3], velocity: [0.0; 3], kind: TargetKind::Monster }
    }
    fn params() -> LockOnParams {
        LockOnParams { enabled: true, allowed: true, ..Default::default() }
    }
    fn ctx(targets: &[AimTarget]) -> AimContext<'_> {
        AimContext { eye: [0.0; 3], player_velocity: [0.0; 3], targets, vis: &NoWorld, allowed: false }
    }
    #[test]
    fn hold_selects_nearest_and_release_unlocks() {
        let mut s = LockOn::default();
        let targets = [target(1, [500.0, 0.0, 0.0]), target(2, [100.0, 50.0, 0.0])];
        let c = ctx(&targets);
        let delta = s.update(&params(), Some(&c), [0.0; 3], true, false).unwrap();
        assert_eq!(s.target, Some(2));
        assert!((delta[0] - 26.565).abs() < 0.01);
        assert!(s.update(&params(), Some(&c), [0.0; 3], false, false).is_none());
        assert_eq!(s.target, None);
    }
    #[test]
    fn continuous_latches_reacquires_and_switches_off() {
        let p = LockOnParams { toggle: true, ..params() };
        let mut s = LockOn::default();
        let far = [target(1, [100.0, 30.0, 0.0])];
        assert!(s.update(&p, Some(&ctx(&far)), [0.0; 3], true, false).is_none());
        assert!(s.active);
        let near = [target(2, [100.0, 2.0, 0.0])];
        assert!(s.update(&p, Some(&ctx(&near)), [0.0; 3], false, false).is_some());
        assert_eq!(s.target, Some(2));
        let replacement = [target(3, [100.0, -2.0, 0.0])];
        s.update(&p, Some(&ctx(&replacement)), [0.0; 3], false, false);
        assert_eq!(s.target, Some(3));
        assert!(s.update(&p, Some(&ctx(&replacement)), [0.0; 3], true, false).is_none());
        assert!(!s.active);
    }
    #[test]
    fn tracks_by_id_and_cycles_once_per_press() {
        let p = params();
        let mut s = LockOn::default();
        let targets = [target(1, [100.0, 0.0, 0.0]), target(2, [200.0, 20.0, 0.0]), target(3, [300.0, 0.0, 0.0])];
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false);
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, true);
        assert_eq!(s.target, Some(2));
        let reordered = [targets[2], targets[1], targets[0]];
        s.update(&p, Some(&ctx(&reordered)), [0.0; 3], true, true);
        assert_eq!(s.target, Some(2));
        s.update(&p, Some(&ctx(&reordered)), [0.0; 3], true, false);
        s.update(&p, Some(&ctx(&reordered)), [0.0; 3], true, true);
        assert_eq!(s.target, Some(3));
        s.update(&p, Some(&ctx(&reordered)), [0.0; 3], true, false);
        s.update(&p, Some(&ctx(&reordered)), [0.0; 3], true, true);
        assert_eq!(s.target, Some(1));
    }
    #[test]
    fn rejects_online_friends_out_of_view_and_out_of_range() {
        let mut p = params();
        let mut s = LockOn::default();
        let mut player = target(1, [100.0, 0.0, 0.0]);
        player.kind = TargetKind::Player;
        let targets = [player, target(2, [-100.0, 0.0, 0.0]), target(3, [100.0, 1000.0, 0.0]), target(4, [3000.0, 0.0, 0.0])];
        assert!(s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false).is_none());
        p.target_players = true;
        assert!(s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false).is_some());
        p.allowed = false;
        assert!(s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false).is_none());
        assert_eq!(s.target, None);
        assert!(!s.active);
    }
    #[test]
    fn occlusion_keeps_lock_and_death_requires_fresh_press() {
        struct Wall;
        impl Visibility for Wall {
            fn visible(&self, _: V3, _: V3) -> bool { false }
        }
        let targets = [target(1, [100.0, 0.0, 0.0])];
        let mut c = ctx(&targets);
        let mut s = LockOn::default();
        s.update(&params(), Some(&c), [0.0; 3], true, false);
        c.vis = &Wall;
        assert!(s.update(&params(), Some(&c), [0.0; 3], true, false).is_some());
        assert_eq!(s.target, Some(1));
        c.vis = &NoWorld;
        s.update(&params(), Some(&c), [0.0; 3], true, false);
        assert!(s.update(&params(), Some(&ctx(&[])), [0.0; 3], true, false).is_none());
        assert_eq!(s.target, None);
        assert!(s.update(&params(), Some(&c), [0.0; 3], true, false).is_none());
        s.update(&params(), Some(&c), [0.0; 3], false, false);
        assert!(s.update(&params(), Some(&c), [0.0; 3], true, false).is_some());
    }
    #[test]
    fn hold_can_opt_into_reacquisition() {
        let p = LockOnParams { reacquire: true, ..params() };
        let mut s = LockOn::default();
        s.update(&p, Some(&ctx(&[target(1, [100.0, 0.0, 0.0])])), [0.0; 3], true, false);
        s.update(&p, Some(&ctx(&[target(2, [150.0, 0.0, 0.0])])), [0.0; 3], true, false);
        assert_eq!(s.target, Some(2));
    }
    #[test]
    fn follows_motion_without_switching_to_closer_enemy() {
        let mut s = LockOn::default();
        let targets = [target(1, [100.0, 0.0, 0.0])];
        s.update(&params(), Some(&ctx(&targets)), [0.0; 3], true, false);
        let moved = [target(1, [100.0, 30.0, 20.0]), target(2, [50.0, 0.0, 0.0])];
        let delta = s.update(&params(), Some(&ctx(&moved)), [0.0; 3], true, false).unwrap();
        assert_eq!(s.target, Some(1));
        assert!(delta[0] > 10.0 && delta[1] < -5.0);
    }
    #[test]
    fn vertical_fov_and_yaw_wrap() {
        let mut p = params();
        p.fov = [90.0, 40.0];
        let mut s = LockOn::default();
        let high = [target(1, [100.0, 0.0, 100.0])];
        assert!(s.update(&p, Some(&ctx(&high)), [0.0; 3], true, false).is_none());
        let behind = [target(2, [-100.0, -1.0, 0.0])];
        let correction = s.update(&p, Some(&ctx(&behind)), [0.0, 179.0, 0.0], true, false).unwrap();
        assert!(correction[0] > 0.0 && correction[0] < 2.0);
    }
    #[test]
    fn missing_context_and_mode_change_reset_continuous_state() {
        let mut p = LockOnParams { toggle: true, ..params() };
        let targets = [target(1, [100.0, 0.0, 0.0])];
        let mut s = LockOn::default();
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false);
        assert!(s.active);
        assert!(s.update(&p, None, [0.0; 3], false, false).is_none());
        assert!(!s.active);
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], false, false);
        assert!(!s.active);
        p.toggle = false;
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false);
        assert_eq!(s.target, Some(1));
        p.enabled = false;
        s.update(&p, Some(&ctx(&targets)), [0.0; 3], true, false);
        assert_eq!(s.target, None);
    }

    #[test]
    fn returning_from_online_requires_fresh_toggle_press() {
        let mut p = LockOnParams { toggle: true, ..params() };
        let targets = [target(1, [100.0, 0.0, 0.0])];
        let c = ctx(&targets);
        let mut s = LockOn::default();
        s.update(&p, Some(&c), [0.0; 3], true, false);
        assert!(s.active);
        p.allowed = false;
        s.update(&p, Some(&c), [0.0; 3], true, false);
        p.allowed = true;
        assert!(s.update(&p, Some(&c), [0.0; 3], true, false).is_none());
        assert!(!s.active);
        s.update(&p, Some(&c), [0.0; 3], false, false);
        assert!(s.update(&p, Some(&c), [0.0; 3], true, false).is_some());
    }
    #[test]
    fn mode_change_does_not_turn_held_button_into_toggle_press() {
        let mut p = params();
        let targets = [target(1, [100.0, 0.0, 0.0])];
        let c = ctx(&targets);
        let mut s = LockOn::default();
        s.update(&p, Some(&c), [0.0; 3], true, false);
        assert!(s.active);
        p.toggle = true;
        assert!(s.update(&p, Some(&c), [0.0; 3], true, false).is_none());
        assert!(!s.active);
        s.update(&p, Some(&c), [0.0; 3], false, false);
        assert!(s.update(&p, Some(&c), [0.0; 3], true, false).is_some());
    }

}
