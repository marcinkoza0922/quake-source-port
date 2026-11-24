//! Reposing a player model's arms: each arm reaches for a point with a
//! two-bone solve, whatever it did in the model's own animation. Works on
//! bare vertices, so both model formats share it.

use qcore::math::Vec3;

use crate::debris::math::{Mat3, Quat, apply, mul3};

/// What a vertex belongs to; sides are 0 left, 1 right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Body,
    Upper(usize),
    Fore(usize),
    Hand(usize),
}

#[derive(Clone, Copy, Debug)]
struct Weight {
    side: usize,
    upper: f32,
    fore: f32,
}

/// The arms of one mesh.
pub struct Arms {
    weights: Vec<Weight>,
    shoulder: [Vec<usize>; 2],
    elbow: [Vec<usize>; 2],
    hand: [Vec<usize>; 2],
}

/// A reposed frame.
pub struct Posed {
    pub pos: Vec<Vec3>,
    /// How each vertex's normal turned.
    pub rot: Vec<Mat3>,
    /// Where the hands ended up.
    pub hands: [Vec3; 2],
}

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mean(pos: &[Vec3], idx: &[usize]) -> Vec3 {
    idx.iter().fold(Vec3::ZERO, |s, &i| s + pos[i]) / idx.len().max(1) as f32
}

/// The shortest turn from direction `a` to `b`.
fn between(a: Vec3, b: Vec3) -> Mat3 {
    let (a, b) = (a.normalize(), b.normalize());
    let (axis, s) = a.cross(b).normalize_len();
    if s < 1e-5 {
        return IDENTITY;
    }
    Quat::axis_angle(axis, s.atan2(a.dot(b))).to_mat()
}

impl Arms {
    /// `pos` is any frame (vertices at the same spot are the same point);
    /// None when a limb is missing.
    pub fn new(parts: &[Part], pos: &[Vec3], tris: impl IntoIterator<Item = [usize; 3]>) -> Option<Arms> {
        let mut ids = std::collections::HashMap::new();
        let point: Vec<usize> = pos
            .iter()
            .zip(parts)
            .map(|(p, part)| {
                let n = ids.len();
                // duplicated seam vertices belong to one limb
                *ids.entry((p.to_array().map(|c| (c * 32.0).round() as i32), std::mem::discriminant(part))).or_insert(n)
            })
            .collect();
        // which limbs each point touches: body, upper, fore or hand
        let mut near = vec![[false; 3]; ids.len()];
        for t in tris {
            for &v in &t {
                let k = match parts[v] {
                    Part::Body => 0,
                    Part::Upper(_) => 1,
                    Part::Fore(_) | Part::Hand(_) => 2,
                };
                for &w in &t {
                    near[point[w]][k] = true;
                }
            }
        }
        let mut arms = Arms {
            weights: Vec::with_capacity(parts.len()),
            shoulder: Default::default(),
            elbow: Default::default(),
            hand: Default::default(),
        };
        for (v, &part) in parts.iter().enumerate() {
            let [body, _, fore] = near[point[v]];
            let (side, upper, low) = match part {
                Part::Body => (0, 0.0, 0.0),
                // rings between limbs go half way
                Part::Upper(s) if fore => (s, 0.5, 0.5),
                Part::Upper(s) if body => (s, 0.6, 0.0),
                Part::Upper(s) => (s, 1.0, 0.0),
                Part::Fore(s) | Part::Hand(s) => (s, 0.0, 1.0),
            };
            match part {
                Part::Upper(s) if fore => arms.elbow[s].push(v),
                Part::Upper(s) if body => arms.shoulder[s].push(v),
                Part::Hand(s) => arms.hand[s].push(v),
                _ => {}
            }
            arms.weights.push(Weight { side, upper, fore: low });
        }
        let whole = [&arms.shoulder, &arms.elbow, &arms.hand].iter().all(|j| j.iter().all(|v| !v.is_empty()));
        whole.then_some(arms)
    }

    /// The vertices of forearm and hand, which move rigidly.
    pub fn forearm(&self, side: usize) -> impl Iterator<Item = usize> + '_ {
        self.weights.iter().enumerate().filter(move |(_, w)| w.side == side && w.fore == 1.0).map(|w| w.0)
    }

    /// Where the hands are in a frame as it is.
    #[cfg(test)]
    pub fn hands(&self, pos: &[Vec3]) -> [Vec3; 2] {
        [0, 1].map(|s| mean(pos, &self.hand[s]))
    }

    /// The frame with its hands reaching for `target(side, middle of the
    /// shoulders)`.
    pub fn reach(&self, pos: &[Vec3], target: impl Fn(usize, Vec3) -> Vec3) -> Posed {
        let shoulders = [0, 1].map(|s| mean(pos, &self.shoulder[s]));
        let mid = (shoulders[0] + shoulders[1]) * 0.5;
        let mut hands = [Vec3::ZERO; 2];
        // per side: shoulder, elbow before and after, turns of both bones
        let bones: [_; 2] = std::array::from_fn(|s| {
            let (a, e, h) = (shoulders[s], mean(pos, &self.elbow[s]), mean(pos, &self.hand[s]));
            let (l1, l2) = ((e - a).length(), (h - e).length());
            let to = target(s, mid) - a;
            let (dir, far) = to.normalize_len();
            let d = far.clamp((l1 - l2).abs() + 0.1, (l1 + l2) * 0.985);
            // elbows out and down
            let pole = Vec3::new(-0.2, if s == 0 { 0.8 } else { -0.8 }, -1.0);
            let bend = (pole - dir * pole.dot(dir)).normalize();
            let along = (d * d + l1 * l1 - l2 * l2) / (2.0 * d);
            let out = (l1 * l1 - along * along).max(0.0).sqrt();
            let (e2, h2) = (a + dir * along + bend * out, a + dir * d);
            let upper = between(e - a, e2 - a);
            let fore = mul3(&between(apply(&upper, h - e), h2 - e2), &upper);
            hands[s] = h2;
            (a, e, e2, upper, fore)
        });
        let mut out = Posed { pos: Vec::with_capacity(pos.len()), rot: Vec::with_capacity(pos.len()), hands };
        for (&p, w) in pos.iter().zip(&self.weights) {
            let (a, e, e2, upper, fore) = &bones[w.side];
            let moved = p * (1.0 - w.upper - w.fore) + (*a + apply(upper, p - *a)) * w.upper + (*e2 + apply(fore, p - *e)) * w.fore;
            out.pos.push(moved);
            out.rot.push(if w.fore >= 0.5 { *fore } else if w.upper >= 0.5 { *upper } else { IDENTITY });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two straight arms hanging from a small body.
    fn doll() -> (Vec<Part>, Vec<Vec3>, Vec<[usize; 3]>) {
        let (mut parts, mut pos, mut tris) = (Vec::new(), Vec::new(), Vec::new());
        for (s, y) in [(0, 8.0), (1, -8.0)] {
            let first = pos.len();
            for (part, z) in [(Part::Body, 22.0), (Part::Upper(s), 20.0), (Part::Upper(s), 12.0), (Part::Fore(s), 10.0), (Part::Hand(s), 4.0)] {
                for x in [-1.0, 1.0] {
                    parts.push(part);
                    pos.push(Vec3::new(x, y, z));
                }
            }
            for k in 0..4 {
                let a = first + k * 2;
                tris.extend([[a, a + 1, a + 2], [a + 1, a + 3, a + 2]]);
            }
        }
        (parts, pos, tris)
    }

    #[test]
    fn hands_reach_their_targets_and_limbs_keep_their_length() {
        let (parts, pos, tris) = doll();
        let arms = Arms::new(&parts, &pos, tris).unwrap();
        let goal = |s: usize, m: Vec3| m + Vec3::new(10.0, if s == 0 { 5.0 } else { -5.0 }, -3.0);
        let p = arms.reach(&pos, goal);
        let mid = Vec3::new(0.0, 0.0, 20.0);
        for s in 0..2 {
            assert!((p.hands[s] - goal(s, mid)).length() < 0.01, "{:?}", p.hands[s]);
            assert!((mean(&p.pos, &arms.hand[s]) - p.hands[s]).length() < 0.01);
            let fore: Vec<_> = arms.forearm(s).collect();
            for (&a, &b) in fore.iter().zip(&fore[1..]) {
                assert!(((pos[a] - pos[b]).length() - (p.pos[a] - p.pos[b]).length()).abs() < 0.01);
            }
        }
        // the body stays
        for (i, part) in parts.iter().enumerate().filter(|p| *p.1 == Part::Body) {
            assert_eq!(p.pos[i], pos[i], "{part:?}");
        }
        // out of reach: stretched towards it, not past the arm's length
        let far = arms.reach(&pos, |_, m| m + Vec3::new(100.0, 0.0, 0.0));
        assert!(far.hands.iter().zip([8.0, -8.0]).all(|(h, y)| (*h - Vec3::new(0.0, y, 20.0)).length() < 16.01));
        assert_eq!(arms.hands(&pos)[0], Vec3::new(0.0, 8.0, 4.0));
    }

    #[test]
    fn a_missing_limb_is_refused() {
        let (mut parts, pos, tris) = doll();
        parts.iter_mut().for_each(|p| if *p == Part::Hand(1) { *p = Part::Fore(1) });
        assert!(Arms::new(&parts, &pos, tris).is_none());
    }
}
