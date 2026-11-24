//! The cable add-on's visible rope: a verlet chain between fixed anchors
//! (the socket, the corners the server's tether wraps round, the player's
//! hand) that sags, swings and lies on the floor (point traces against the
//! world), drawn as a camera-facing ribbon of dark rubber.
use qcore::math::Vec3;
use crate::gfx::Renderer;
use qrender::{FteBatch, FteBlend, FteVertex, PicId};

pub type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

/// Target link length (units) and the most links one rope gets.
const LINK: f32 = 10.0;
const MAX_NODES: usize = 96;
/// Constraint passes per step, velocity kept per step, rope half width.
const PASSES: usize = 20;
const DAMP: f32 = 0.985;
const HALF_WIDTH: f32 = 0.9;
/// Seconds without a step after which a rope is laid afresh.
const STALE: f64 = 0.25;

/// World point trace: impact point and normal when blocked.
pub type Hit<'a> = &'a dyn Fn(Vec3, Vec3) -> Option<(Vec3, Vec3)>;

#[derive(Clone, Debug, Default)]
pub struct RopeSim {
    pts: Vec<Vec3>,
    old: Vec<Vec3>,
    /// Node index of each anchor.
    anchor_at: Vec<usize>,
    /// Rest length of the links of each span between anchors.
    rest: Vec<f32>,
    /// Simulation time of the last step.
    pub time: f64,
}

impl RopeSim {
    pub fn points(&self) -> &[Vec3] {
        &self.pts
    }

    /// (Re)lay the nodes along `anchors` for a cable `length` long, keeping
    /// the shape of the previous rope where there was one.
    fn rebuild(&mut self, anchors: &[Vec3], length: f32) {
        let spans: Vec<f32> = anchors.windows(2).map(|w| (w[1] - w[0]).length().max(0.1)).collect();
        let taut: f32 = spans.iter().sum();
        let total = length.max(taut);
        let budget = ((total / LINK) as usize).clamp(spans.len().max(1), MAX_NODES - 1);
        let old = std::mem::take(&mut self.pts);
        self.anchor_at = vec![0];
        let mut pts = vec![anchors[0]];
        for (j, &d) in spans.iter().enumerate() {
            let k = ((budget as f32 * d / taut).round() as usize).max(1);
            for s in 1..=k {
                pts.push(anchors[j].lerp(anchors[j + 1], s as f32 / k as f32));
            }
            self.anchor_at.push(pts.len() - 1);
        }
        // keep the old shape: resample the old polyline by arc length
        if old.len() >= 2 {
            let cum = arc(&old);
            let n = pts.len() - 1;
            for (i, p) in pts.iter_mut().enumerate() {
                if self.anchor_at.contains(&i) {
                    continue;
                }
                *p = sample(&old, &cum, i as f32 / n as f32);
            }
        }
        self.old = pts.clone();
        self.pts = pts;
        self.rest = vec![0.0; spans.len()];
    }

    /// Moves the free nodes of a two-anchor rope rigidly with its ends
    /// (their midpoint, and their heading about the vertical), so a loop
    /// held between the hands is carried along instead of dragged: walking
    /// and turning would otherwise whip it up past the eye. Call before
    /// [`Self::step`] with the same anchors.
    pub fn carry(&mut self, anchors: &[Vec3]) {
        let [a1, b1] = anchors else { return };
        if self.anchor_at.len() != 2 || self.pts.is_empty() {
            return;
        }
        let (a0, b0) = (self.pts[self.anchor_at[0]], self.pts[self.anchor_at[1]]);
        let (m0, m1) = ((a0 + b0) * 0.5, (*a1 + *b1) * 0.5);
        let heading = |d: Vec3| (d.x * d.x + d.y * d.y > 1.0).then(|| d.y.atan2(d.x));
        let turn = heading(*b1 - *a1).zip(heading(b0 - a0)).map_or(0.0, |(h1, h0)| h1 - h0);
        let (s, c) = turn.sin_cos();
        let move_ = |p: Vec3| {
            let d = p - m0;
            m1 + Vec3::new(d.x * c - d.y * s, d.x * s + d.y * c, d.z)
        };
        for i in 0..self.pts.len() {
            if !self.anchor_at.contains(&i) {
                self.pts[i] = move_(self.pts[i]);
                self.old[i] = move_(self.old[i]);
            }
        }
    }

    /// Advance to `time` with the anchors at `anchors` (socket first).
    pub fn step(&mut self, anchors: &[Vec3], length: f32, time: f64, gravity: f32, hit: Hit) {
        if anchors.len() < 2 {
            return;
        }
        // not stepped for a while (put away, or the hand loop while
        // plugged in) or an anchor jumped further than the rope reaches
        // (teleport): the old nodes would stay behind, stretched out or
        // caught behind walls, so lay a fresh rope
        let stale = time - self.time > STALE
            || (self.anchor_at.len() == anchors.len()
                && self.anchor_at.iter().zip(anchors).any(|(&i, &a)| self.pts.get(i).is_none_or(|&p| (p - a).length() > length)));
        if stale {
            self.pts.clear();
        }
        if self.anchor_at.len() != anchors.len() || self.pts.is_empty() {
            self.rebuild(anchors, length);
            self.time = time;
        }
        let dt = ((time - self.time) as f32).clamp(0.0, 0.05);
        self.time = time;
        // slack shared out in proportion to each span's straight length
        let spans: Vec<f32> = anchors.windows(2).map(|w| (w[1] - w[0]).length()).collect();
        let taut: f32 = spans.iter().sum::<f32>().max(0.1);
        // a cable at full stretch is pre-tensioned a little, or the
        // iterative constraints let it sag like a slack one
        let scale = length / taut;
        let scale = if scale < 1.02 { 0.97 } else { scale };
        for (j, &d) in spans.iter().enumerate() {
            let links = (self.anchor_at[j + 1] - self.anchor_at[j]) as f32;
            self.rest[j] = d * scale / links;
        }
        for (k, &i) in self.anchor_at.iter().enumerate() {
            self.pts[i] = anchors[k];
            self.old[i] = anchors[k];
        }
        if dt <= 0.0 {
            return;
        }
        let g = Vec3::new(0.0, 0.0, -gravity * dt * dt);
        let before: Vec<Vec3> = self.pts.clone();
        for i in 0..self.pts.len() {
            if self.anchor_at.contains(&i) {
                continue;
            }
            let p = self.pts[i];
            let v = (p - self.old[i]) * DAMP;
            self.old[i] = p;
            self.pts[i] = p + v + g;
        }
        for _ in 0..PASSES {
            let mut span = 0;
            for i in 0..self.pts.len() - 1 {
                if i == self.anchor_at[span + 1] {
                    span += 1;
                }
                let rest = self.rest[span.min(self.rest.len() - 1)];
                let (a, b) = (self.pts[i], self.pts[i + 1]);
                let d = b - a;
                let len = d.length();
                // a rope pulls but never pushes
                if len <= rest || len < 1e-4 {
                    continue;
                }
                let fa = !self.anchor_at.contains(&i);
                let fb = !self.anchor_at.contains(&(i + 1));
                let corr = d * ((len - rest) / len);
                match (fa, fb) {
                    (true, true) => {
                        self.pts[i] = a + corr * 0.5;
                        self.pts[i + 1] = b - corr * 0.5;
                    }
                    (true, false) => self.pts[i] = a + corr,
                    (false, true) => self.pts[i + 1] = b - corr,
                    _ => {}
                }
            }
        }
        // collide each free node's whole move this step with the world
        for (i, &from) in before.iter().enumerate() {
            if self.anchor_at.contains(&i) {
                continue;
            }
            if let Some((at, n)) = hit(from, self.pts[i]) {
                let p = at + n * 0.5;
                // friction: lose most of the speed along the surface
                self.old[i] = p.lerp(self.old[i], 0.3);
                self.pts[i] = p;
            }
        }
    }
}

fn arc(pts: &[Vec3]) -> Vec<f32> {
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum.last().unwrap() + (w[1] - w[0]).length());
    }
    cum
}

/// Point at fraction `t` of the polyline's length.
fn sample(pts: &[Vec3], cum: &[f32], t: f32) -> Vec3 {
    let total = *cum.last().unwrap();
    let s = t * total;
    let i = cum.partition_point(|&c| c < s).clamp(1, pts.len() - 1);
    let seg = (cum[i] - cum[i - 1]).max(1e-4);
    pts[i - 1].lerp(pts[i], ((s - cum[i - 1]) / seg).clamp(0.0, 1.0))
}

/// The rubber texture: dark, with a soft highlight along the middle.
pub fn rubber_pic(r: &mut Renderer) -> Option<PicId> {
    let mut px = Vec::with_capacity(16 * 16 * 4);
    for j in 0..16 {
        let y = (j as f32 + 0.5) / 8.0 - 1.0;
        let shade = 1.0 - y.abs();
        let v = (22.0 + 48.0 * shade * shade) as u8;
        let a = if y.abs() > 0.95 { 0 } else { 255 };
        for _ in 0..16 {
            px.extend([v, v, (f32::from(v) * 1.08) as u8, a]);
        }
    }
    r.register_pic_rgba_linear(16, 16, &px).ok()
}

/// Ribbon quads along `pts` (with `last` replacing the final point when
/// given: the viewer's own hand), facing `eye`.
pub fn draw(pts: &[Vec3], last: Option<Vec3>, eye: Vec3, pic: PicId, geom: &mut Geometry) {
    if pts.len() < 2 {
        return;
    }
    let first = geom.1.len() as u32;
    let n = pts.len();
    let at = |i: usize| if i == n - 1 { last.unwrap_or(pts[i]) } else { pts[i] };
    for i in 0..n - 1 {
        let (a, b) = (at(i), at(i + 1));
        let d = b - a;
        if d.length_sq() < 1e-4 {
            continue;
        }
        let c = (a + b) * 0.5;
        let side = d.cross(eye - c);
        if side.length_sq() < 1e-6 {
            continue;
        }
        let side = side.normalize() * HALF_WIDTH;
        // overlap a little so the joints have no gaps
        let along = d * 0.5 + d.normalize() * (HALF_WIDTH * 0.6);
        let base = geom.0.len() as u32;
        for (x, y, uv) in [(-1.0, -1.0, [0.0, 0.0]), (1.0, -1.0, [1.0, 0.0]), (1.0, 1.0, [1.0, 1.0]), (-1.0, 1.0, [0.0, 1.0])] {
            geom.0.push(FteVertex { pos: (c + along * x + side * y).to_array(), uv, color: [255; 4] });
        }
        geom.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let count = geom.1.len() as u32 - first;
    if count > 0 {
        geom.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: count, soft: false, gel: false });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slack_rope_sags_onto_the_floor_and_a_taut_one_stays_straight() {
        // floor at z = 0
        let floor = |a: Vec3, b: Vec3| -> Option<(Vec3, Vec3)> {
            (b.z < 0.0 && a.z >= 0.0).then(|| {
                let t = a.z / (a.z - b.z);
                (a.lerp(b, t), Vec3::new(0.0, 0.0, 1.0))
            })
        };
        let anchors = [Vec3::new(0.0, 0.0, 40.0), Vec3::new(100.0, 0.0, 40.0)];
        let mut rope = RopeSim::default();
        for k in 0..400 {
            rope.step(&anchors, 300.0, k as f64 / 60.0, 800.0, &floor);
        }
        let low = rope.points().iter().map(|p| p.z).fold(f32::MAX, f32::min);
        assert!((0.0..1.5).contains(&low), "lies on the floor: {low}");
        assert!(rope.points().iter().all(|p| p.z >= 0.0));
        let mut taut = RopeSim::default();
        for k in 0..400 {
            taut.step(&anchors, 100.0, k as f64 / 60.0, 800.0, &floor);
        }
        let sag = taut.points().iter().map(|p| 40.0 - p.z).fold(0.0, f32::max);
        assert!(sag < 6.0, "taut rope sags {sag}");
    }

    #[test]
    fn a_rope_picked_up_again_elsewhere_is_laid_afresh() {
        // a wall at x = 500 that no node may cross
        let wall = |a: Vec3, b: Vec3| -> Option<(Vec3, Vec3)> {
            ((a.x - 500.0) * (b.x - 500.0) < 0.0).then(|| (Vec3::new(500.0, b.y, b.z), Vec3::new((a.x - 500.0).signum(), 0.0, 0.0)))
        };
        let near = |r: &RopeSim, at: Vec3| r.points().iter().all(|p| (*p - at).length() < 80.0);
        let here = [Vec3::new(1000.0, 0.0, 40.0), Vec3::new(1010.0, 0.0, 40.0)];
        let there = [Vec3::new(0.0, 0.0, 40.0), Vec3::new(10.0, 0.0, 40.0)];
        // put away for a second (or plugged in), then held behind the wall
        let mut rope = RopeSim::default();
        for k in 0..60 {
            rope.step(&here, 70.0, k as f64 / 60.0, 800.0, &wall);
        }
        rope.step(&there, 70.0, 2.0, 800.0, &wall);
        assert!(near(&rope, there[0]), "{:?}", rope.points());
        // teleported while held: the very next frame
        let mut rope = RopeSim::default();
        for k in 0..60 {
            rope.step(&here, 70.0, k as f64 / 60.0, 800.0, &wall);
        }
        rope.step(&there, 70.0, 1.0, 800.0, &wall);
        assert!(near(&rope, there[0]), "{:?}", rope.points());
    }

    #[test]
    fn a_carried_hand_loop_never_whips_above_the_hands() {
        let floor = |a: Vec3, b: Vec3| -> Option<(Vec3, Vec3)> {
            (b.z < 0.0 && a.z >= 0.0).then(|| (a.lerp(b, a.z / (a.z - b.z)), Vec3::new(0.0, 0.0, 1.0)))
        };
        let hands = |k: usize| {
            // walking 320 u/s, stopping and turning back and forth
            let t = k as f32 / 120.0;
            let at = Vec3::new(320.0 * t.min(1.0) - 320.0 * (t - 2.0).clamp(0.0, 1.0), 0.0, 30.0);
            let yaw = (t * 3.0).sin() * 1.5;
            let side = Vec3::new(-yaw.sin(), yaw.cos(), 0.0) * 9.0;
            [at - side, at + side]
        };
        let mut rope = RopeSim::default();
        let mut top = f32::MIN;
        for k in 0..600 {
            let a = hands(k);
            rope.carry(&a);
            rope.step(&a, 70.0, k as f64 / 120.0, 800.0, &floor);
            if k > 30 {
                top = rope.points().iter().map(|p| p.z).fold(top, f32::max);
            }
        }
        assert!(top <= 30.5, "the loop rose to {top} with the hands at 30");
    }

    #[test]
    fn adding_an_anchor_keeps_the_shape() {
        let none = |_: Vec3, _: Vec3| None;
        let mut rope = RopeSim::default();
        let two = [Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0)];
        for k in 0..100 {
            rope.step(&two, 220.0, k as f64 / 60.0, 800.0, &none);
        }
        let low = rope.points().iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let three = [Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)];
        rope.step(&three, 220.0, 100.0 / 60.0, 800.0, &none);
        let low2 = rope.points().iter().map(|p| p.z).fold(f32::MAX, f32::min);
        assert!(low < -20.0 && (low2 - low).abs() < 15.0, "{low} {low2}");
    }
}
