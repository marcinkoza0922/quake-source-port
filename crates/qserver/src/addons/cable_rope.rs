//! The cable add-on's tether: a rope from the socket to the player that
//! wraps around world corners ("ninja rope"). Its slack is measured along
//! the rope, so going round a pillar uses some of it up.
//!
//! Wrapping: when the line from the last anchor (the socket or the newest
//! pivot) to the player is blocked, a binary search between last frame's
//! (clear) player point and this frame's finds the corner, and a pivot is
//! added just outside it. Each pivot remembers which way the rope bends
//! there; once the player swings back past the straight line (and the line
//! from the anchor before it is clear again) the pivot is removed.

use qcore::math::Vec3;

/// Most corners one rope wraps; further corners are ignored (the rope
/// passes through them).
pub const MAX_PIVOTS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pivot {
    pub p: Vec3,
    /// (pivot − anchor before it) × (player − pivot) when it was made.
    bend: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    pub socket: Vec3,
    pub pivots: Vec<Pivot>,
    /// Player point of the last update (a clear line from the anchor).
    prev: Vec3,
}

impl Rope {
    pub fn new(socket: Vec3, player: Vec3) -> Self {
        Rope { socket, pivots: Vec::new(), prev: player }
    }

    /// The point the free stretch of rope hangs from.
    pub fn anchor(&self) -> Vec3 {
        self.pivots.last().map_or(self.socket, |p| p.p)
    }

    fn before_last(&self) -> Vec3 {
        let n = self.pivots.len();
        if n >= 2 { self.pivots[n - 2].p } else { self.socket }
    }

    /// Rope length up to the anchor (the part that no longer moves).
    pub fn fixed_length(&self) -> f32 {
        let mut len = 0.0;
        let mut a = self.socket;
        for p in &self.pivots {
            len += (p.p - a).length();
            a = p.p;
        }
        len
    }

    /// Whole rope length with the player at `x`.
    pub fn length(&self, x: Vec3) -> f32 {
        self.fixed_length() + (x - self.anchor()).length()
    }

    /// The socket moved (a brush entity it is plugged into): shift the rope.
    pub fn move_socket(&mut self, socket: Vec3) {
        self.socket = socket;
    }

    /// Follow the player to `x`. `hit(a, b)` traces the world: the impact
    /// point and surface normal when the line is blocked.
    pub fn update(&mut self, x: Vec3, hit: impl Fn(Vec3, Vec3) -> Option<(Vec3, Vec3)>) {
        // unwrap: the rope straightened through the newest pivot
        while let Some(b) = self.pivots.last().copied() {
            let a = self.before_last();
            if (b.p - a).cross(x - b.p).dot(b.bend) > 0.0 || hit(a, x).is_some() {
                break;
            }
            self.pivots.pop();
        }
        // wrap: the free stretch is blocked
        for _ in 0..4 {
            if self.pivots.len() >= MAX_PIVOTS {
                break;
            }
            let a = self.anchor();
            let Some((h, n)) = hit(a, x) else { break };
            let p = if hit(a, self.prev).is_none() { corner(a, self.prev, x, &hit).unwrap_or(h + n * 2.0) } else { h + n * 2.0 };
            let bend = (p - a).cross(x - p);
            if (p - a).length() < 1.0 || bend.length_sq() < 1e-4 {
                break;
            }
            self.pivots.push(Pivot { p, bend });
        }
        self.prev = x;
    }
}

/// The corner the line from `a` caught on while the far end moved from
/// `clear` (unblocked) to `blocked`: a point on the last clear line, as near
/// the obstacle as the search resolves, nudged a little off it.
fn corner(a: Vec3, clear: Vec3, blocked: Vec3, hit: &impl Fn(Vec3, Vec3) -> Option<(Vec3, Vec3)>) -> Option<Vec3> {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..12 {
        let m = (lo + hi) * 0.5;
        if hit(a, clear.lerp(blocked, m)).is_some() {
            hi = m;
        } else {
            lo = m;
        }
    }
    let (h, n) = hit(a, clear.lerp(blocked, hi))?;
    let end = clear.lerp(blocked, lo);
    // nearest point to the impact on the clear line a..end
    let d = end - a;
    let t = ((h - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
    let p = a + d * t;
    // off the obstacle: sideways towards where the line was still clear
    // (outside both faces of a corner), and off the face that was hit
    let mv = clear - blocked;
    let dir = d.normalize();
    let side = mv - dir * mv.dot(dir);
    let side = if side.length_sq() > 1e-6 { side.normalize() } else { n };
    Some(p + side * 2.0 + n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid axis-aligned box: slab test returning the entry point and
    /// face normal.
    fn boxed(lo: Vec3, hi: Vec3) -> impl Fn(Vec3, Vec3) -> Option<(Vec3, Vec3)> {
        move |a: Vec3, b: Vec3| {
            let d = b - a;
            let (mut t0, mut t1, mut n) = (0.0f32, 1.0f32, Vec3::ZERO);
            for k in 0..3 {
                let (s, e, l, h) = (a.to_array()[k], d.to_array()[k], lo.to_array()[k], hi.to_array()[k]);
                if e.abs() < 1e-6 {
                    if s < l || s > h {
                        return None;
                    }
                    continue;
                }
                let (mut ta, mut tb) = ((l - s) / e, (h - s) / e);
                if ta > tb {
                    std::mem::swap(&mut ta, &mut tb);
                }
                if ta > t0 {
                    t0 = ta;
                    let mut v = [0.0; 3];
                    v[k] = -e.signum();
                    n = Vec3::from_array(v);
                }
                t1 = t1.min(tb);
                if t0 > t1 {
                    return None;
                }
            }
            (t0 > 0.0 && t0 < 1.0).then(|| (a + d * t0, n))
        }
    }

    #[test]
    fn wraps_round_a_pillar_and_unwraps_coming_back() {
        // pillar 0..100 x 0..100, socket to its west
        let hit = boxed(Vec3::new(0.0, 0.0, -50.0), Vec3::new(100.0, 100.0, 50.0));
        let socket = Vec3::new(-100.0, 50.0, 0.0);
        let mut x = Vec3::new(-50.0, 150.0, 0.0);
        let mut rope = Rope::new(socket, x);
        // walk east along the north side, then south down the east side
        let mut path = Vec::new();
        for i in 0..=40 {
            path.push(Vec3::new(-50.0 + i as f32 * 5.0, 150.0, 0.0));
        }
        for i in 0..=30 {
            path.push(Vec3::new(150.0, 150.0 - i as f32 * 5.0, 0.0));
        }
        for &p in &path {
            x = p;
            rope.update(x, &hit);
            assert!(hit(rope.anchor(), x).is_none(), "free stretch blocked at {x:?}: {:?}", rope.pivots);
        }
        // caught on the north-west corner, then the north-east one
        assert_eq!(rope.pivots.len(), 2, "{:?}", rope.pivots);
        assert!((rope.pivots[0].p - Vec3::new(0.0, 100.0, 0.0)).length() < 4.0, "{:?}", rope.pivots);
        assert!((rope.pivots[1].p - Vec3::new(100.0, 100.0, 0.0)).length() < 4.0, "{:?}", rope.pivots);
        let wrapped = rope.length(x);
        assert!(wrapped > (x - socket).length() + 50.0);
        // walk back the same way: the rope unwinds
        for &p in path.iter().rev() {
            rope.update(p, &hit);
            assert!(hit(rope.anchor(), p).is_none());
        }
        assert!(rope.pivots.is_empty(), "{:?}", rope.pivots);
    }

    #[test]
    fn open_space_never_wraps() {
        let hit = |_: Vec3, _: Vec3| None;
        let mut rope = Rope::new(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0));
        rope.update(Vec3::new(300.0, 40.0, 0.0), hit);
        assert!(rope.pivots.is_empty());
        assert!((rope.length(Vec3::new(300.0, 40.0, 0.0)) - Vec3::new(300.0, 40.0, 0.0).length()).abs() < 0.01);
    }
}
