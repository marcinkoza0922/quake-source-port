//! Gamepad aim assist: slowdown (friction) cone + rotational assist. No snap by default.

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add_scaled(a: V3, s: f32, b: V3) -> V3 {
    [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// Quake forward vector for [pitch, yaw, _] (pitch + = down).
pub fn forward(angles: V3) -> V3 {
    let (sp, cp) = angles[0].to_radians().sin_cos();
    let (sy, cy) = angles[1].to_radians().sin_cos();
    [cp * cy, cp * sy, -sp]
}

/// [pitch, yaw] of a direction (Quake convention).
pub fn vec_angles(d: V3) -> [f32; 2] {
    let yaw = d[1].atan2(d[0]).to_degrees();
    let pitch = -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees();
    [pitch, yaw]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Monster,
    Player,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimTarget {
    pub origin: V3,
    pub mins: V3,
    pub maxs: V3,
    pub velocity: V3,
    pub kind: TargetKind,
}

impl AimTarget {
    pub fn center(&self) -> V3 {
        [0, 1, 2].map(|i| self.origin[i] + 0.5 * (self.mins[i] + self.maxs[i]))
    }
    /// Bounding sphere radius.
    pub fn radius(&self) -> f32 {
        0.5 * len(sub(self.maxs, self.mins))
    }
}

/// Line-of-sight test; the binary implements this with world traces.
pub trait Visibility {
    fn visible(&self, from: V3, to: V3) -> bool;
}

/// Always-visible (tests / no world).
pub struct NoWorld;
impl Visibility for NoWorld {
    fn visible(&self, _: V3, _: V3) -> bool {
        true
    }
}

/// Per-frame world inputs.
pub struct AimContext<'a> {
    pub eye: V3,
    pub player_velocity: V3,
    pub targets: &'a [AimTarget],
    pub vis: &'a dyn Visibility,
    /// Server allow flag; see [`aim_assist_allowed`].
    pub allowed: bool,
}

/// SP/coop always; DM only when the server sets sv_aimassist.
pub fn aim_assist_allowed(deathmatch: bool, sv_aimassist: bool) -> bool {
    !deathmatch || sv_aimassist
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimParams {
    /// Base cone half-angle in degrees (target angular radius is added).
    pub cone: f32,
    pub range: f32,
    /// 0..1 look-speed reduction at cone center.
    pub slowdown: f32,
    /// 0..1 portion of target relative angular velocity added.
    pub rotation: f32,
    /// Pull toward target, fraction of error per second. 0 = off.
    pub snap: f32,
    /// Distance weight in target selection (angle weight is 1).
    pub weight_dist: f32,
    pub target_players: bool,
}

impl Default for AimParams {
    fn default() -> Self {
        Self { cone: 6.0, range: 2048.0, slowdown: 0.4, rotation: 0.5, snap: 0.0, weight_dist: 0.3, target_players: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimResult {
    /// Multiplier for gamepad look input this frame.
    pub scale: f32,
    /// Added [yaw, pitch] degrees (Quake look_delta convention).
    pub delta: [f32; 2],
    pub target: Option<usize>,
}

impl AimResult {
    pub const NONE: Self = Self { scale: 1.0, delta: [0.0; 2], target: None };
}

/// 1 at center, linear to 0 at the cone edge.
pub fn cone_factor(err_deg: f32, cone_deg: f32) -> f32 {
    if cone_deg <= 0.0 { 0.0 } else { (1.0 - err_deg / cone_deg).clamp(0.0, 1.0) }
}

/// Angular error between view and target center, and effective cone, in degrees.
fn target_geom(p: &AimParams, eye: V3, fwd: V3, t: &AimTarget) -> Option<(f32, f32, f32)> {
    let d = sub(t.center(), eye);
    let dist = len(d);
    if dist < 1.0 || dist > p.range {
        return None;
    }
    let err = (dot(d, fwd) / dist).clamp(-1.0, 1.0).acos().to_degrees();
    let cone = p.cone + (t.radius() / dist).atan().to_degrees();
    (err < cone).then_some((err, cone, dist))
}

/// Best target index (lowest angle+distance score) that is in cone and visible.
pub fn select_target(p: &AimParams, ctx: &AimContext, viewangles: V3) -> Option<(usize, f32)> {
    let fwd = forward(viewangles);
    let mut best: Option<(usize, f32, f32)> = None;
    for (i, t) in ctx.targets.iter().enumerate() {
        if t.kind == TargetKind::Player && !p.target_players {
            continue;
        }
        let Some((err, cone, dist)) = target_geom(p, ctx.eye, fwd, t) else { continue };
        let score = err / cone + p.weight_dist * dist / p.range;
        if best.is_some_and(|b| b.1 <= score) {
            continue;
        }
        if !ctx.vis.visible(ctx.eye, t.center()) {
            continue;
        }
        best = Some((i, score, cone_factor(err, cone)));
    }
    best.map(|(i, _, f)| (i, f))
}

/// Compute assist. `player_input`: move or look stick deflected this frame.
/// Caller must only call this for gamepad look (never mouse).
pub fn aim_assist(p: &AimParams, ctx: &AimContext, viewangles: V3, ft: f32, player_input: bool) -> AimResult {
    if !ctx.allowed || ft <= 0.0 {
        return AimResult::NONE;
    }
    let Some((i, f)) = select_target(p, ctx, viewangles) else { return AimResult::NONE };
    let t = &ctx.targets[i];
    let mut r = AimResult { scale: 1.0 - p.slowdown.clamp(0.0, 1.0) * f, delta: [0.0; 2], target: Some(i) };
    let d = sub(t.center(), ctx.eye);
    if player_input {
        // relative angular velocity over this frame
        let rv = sub(t.velocity, ctx.player_velocity);
        let a0 = vec_angles(d);
        let a1 = vec_angles(add_scaled(d, ft, rv));
        let fr = (2.0 * f).min(1.0);
        let k = p.rotation.clamp(0.0, 1.0) * fr;
        r.delta[0] += k * crate::gamepad::angle_diff(a1[1], a0[1]);
        r.delta[1] += k * (a1[0] - a0[0]);
    }
    if p.snap > 0.0 {
        let a = vec_angles(d);
        let s = (p.snap * ft).min(1.0) * f;
        r.delta[0] += s * crate::gamepad::angle_diff(a[1], viewangles[1]);
        r.delta[1] += s * (a[0] - viewangles[0]);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monster(origin: V3, vel: V3) -> AimTarget {
        AimTarget { origin, mins: [-16.0, -16.0, -24.0], maxs: [16.0, 16.0, 40.0], velocity: vel, kind: TargetKind::Monster }
    }
    fn ctx<'a>(eye: V3, pv: V3, t: &'a [AimTarget], vis: &'a dyn Visibility) -> AimContext<'a> {
        AimContext { eye, player_velocity: pv, targets: t, vis, allowed: true }
    }
    fn aim_at(eye: V3, p: V3) -> V3 {
        let a = vec_angles(sub(p, eye));
        [a[0], a[1], 0.0]
    }

    #[test]
    fn angles_roundtrip() {
        let a = [20.0, 135.0, 0.0];
        let b = vec_angles(forward(a));
        assert!((b[0] - 20.0).abs() < 1e-3 && (b[1] - 135.0).abs() < 1e-3);
    }

    #[test]
    fn cone_falloff() {
        assert_eq!(cone_factor(0.0, 6.0), 1.0);
        assert_eq!(cone_factor(6.0, 6.0), 0.0);
        assert_eq!(cone_factor(10.0, 6.0), 0.0);
        assert!((cone_factor(3.0, 6.0) - 0.5).abs() < 1e-6);
        // slowdown monotonic with angular error
        let p = AimParams::default();
        let t = [monster([512.0, 0.0, -8.0], [0.0; 3])];
        let c = ctx([0.0; 3], [0.0; 3], &t, &NoWorld);
        let mut last = 0.0;
        for yaw in [0.0f32, 2.0, 4.0, 6.0, 8.0] {
            let r = aim_assist(&p, &c, [0.0, yaw, 0.0], 0.016, false);
            assert!(r.scale >= last - 1e-6, "yaw {yaw}");
            last = r.scale;
        }
        let center = aim_assist(&p, &c, [0.0, 0.0, 0.0], 0.016, false);
        assert!((center.scale - 0.6).abs() < 1e-3);
        let out = aim_assist(&p, &c, [0.0, 30.0, 0.0], 0.016, false);
        assert_eq!(out, AimResult::NONE);
        // no rotation without input, no snap by default
        assert_eq!(center.delta, [0.0, 0.0]);
    }

    #[test]
    fn closer_target_has_wider_cone() {
        let p = AimParams::default();
        let near = [monster([100.0, 0.0, -8.0], [0.0; 3])];
        let far = [monster([1500.0, 0.0, -8.0], [0.0; 3])];
        let va = [0.0, 12.0, 0.0];
        assert!(select_target(&p, &ctx([0.0; 3], [0.0; 3], &near, &NoWorld), va).is_some());
        assert!(select_target(&p, &ctx([0.0; 3], [0.0; 3], &far, &NoWorld), va).is_none());
    }

    #[test]
    fn selection_weights_angle_and_distance() {
        let p = AimParams::default();
        // same angle, different distance: nearer wins
        let t = [monster([1000.0, 0.0, -8.0], [0.0; 3]), monster([300.0, 0.0, -8.0], [0.0; 3])];
        let c = ctx([0.0; 3], [0.0; 3], &t, &NoWorld);
        assert_eq!(select_target(&p, &c, [0.0; 3]).unwrap().0, 1);
        // dead-center far beats off-axis near
        let t = [monster([800.0, 0.0, -8.0], [0.0; 3]), monster([400.0, 50.0, -8.0], [0.0; 3])];
        let c = ctx([0.0; 3], [0.0; 3], &t, &NoWorld);
        assert_eq!(select_target(&p, &c, [0.0; 3]).unwrap().0, 0);
        // players skipped when disabled
        let mut pl = monster([300.0, 0.0, -8.0], [0.0; 3]);
        pl.kind = TargetKind::Player;
        let t = [pl];
        let c = ctx([0.0; 3], [0.0; 3], &t, &NoWorld);
        assert!(select_target(&AimParams { target_players: false, ..p }, &c, [0.0; 3]).is_none());
        assert!(select_target(&p, &c, [0.0; 3]).is_some());
    }

    struct Wall;
    impl Visibility for Wall {
        fn visible(&self, from: V3, to: V3) -> bool {
            // wall plane x = 200 blocks everything crossing it
            !((from[0] < 200.0) && (to[0] > 200.0))
        }
    }

    #[test]
    fn no_assist_through_walls() {
        let p = AimParams::default();
        let t = [monster([512.0, 0.0, -8.0], [0.0, 300.0, 0.0])];
        let c = ctx([0.0; 3], [0.0; 3], &t, &Wall);
        assert_eq!(aim_assist(&p, &c, [0.0; 3], 0.016, true), AimResult::NONE);
        // visible one behind... occluded one is skipped, visible one chosen
        let t = [monster([512.0, 0.0, -8.0], [0.0; 3]), monster([150.0, 10.0, -8.0], [0.0; 3])];
        let c = ctx([0.0; 3], [0.0; 3], &t, &Wall);
        assert_eq!(select_target(&p, &c, [0.0; 3]).unwrap().0, 1);
    }

    #[test]
    fn gating() {
        assert!(aim_assist_allowed(false, false));
        assert!(!aim_assist_allowed(true, false));
        assert!(aim_assist_allowed(true, true));
        let p = AimParams::default();
        let t = [monster([512.0, 0.0, -8.0], [0.0; 3])];
        let mut c = ctx([0.0; 3], [0.0; 3], &t, &NoWorld);
        c.allowed = false;
        assert_eq!(aim_assist(&p, &c, [0.0; 3], 0.016, true), AimResult::NONE);
    }

    /// Simulate: view starts on target; each frame only the assist moves the view.
    fn track(target_vel: V3, player_vel: V3, rotation: f32) -> f32 {
        let p = AimParams { rotation, ..Default::default() };
        let ft = 1.0 / 60.0;
        let mut eye = [0.0, 0.0, 0.0];
        let mut t = monster([400.0, 0.0, -8.0], target_vel);
        let mut va = aim_at(eye, t.center());
        let mut max_err: f32 = 0.0;
        for _ in 0..120 {
            let ts = [t];
            let c = ctx(eye, player_vel, &ts, &NoWorld);
            let r = aim_assist(&p, &c, va, ft, true);
            va[1] += r.delta[0];
            va[0] += r.delta[1];
            t.origin = add_scaled(t.origin, ft, target_vel);
            eye = add_scaled(eye, ft, player_vel);
            let want = aim_at(eye, t.center());
            let err = crate::gamepad::angle_diff(want[1], va[1]).abs().max((want[0] - va[0]).abs());
            max_err = max_err.max(err);
        }
        max_err
    }

    #[test]
    fn rotational_follows_strafing_target() {
        // target strafes at 200 u/s, 400 away: full rotation keeps crosshair on
        assert!(track([0.0, 200.0, 0.0], [0.0; 3], 1.0) < 0.5);
        // player strafes past a static target
        assert!(track([0.0; 3], [0.0, -250.0, 0.0], 1.0) < 0.5);
        // partial strength lags but helps vs. none
        let none = track([0.0, 200.0, 0.0], [0.0; 3], 0.0);
        let half = track([0.0, 200.0, 0.0], [0.0; 3], 0.5);
        assert!(half < none, "{half} {none}");
        // co-moving: no relative motion, no delta
        assert!(track([0.0, 200.0, 0.0], [0.0, 200.0, 0.0], 1.0) < 1e-3);
    }
}
