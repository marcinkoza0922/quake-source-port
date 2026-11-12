//! Collision against moving brush entities (doors, lifts, plats, trains):
//! a [`Scene`] adds the visible inline submodels to world traces, and
//! [`Debris::carry`] moves or wakes whatever rests on or is shoved by a
//! mover between frames. Rotated brush entities are skipped.
use qworld::{CONTENTS_SOLID, CollisionModel};

use super::*;

#[derive(Clone, Copy)]
pub struct Mover {
    pub ent: usize,
    sub: usize,
    pub origin: Vec3,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// The world plus this frame's movers.
pub struct Scene<'a> {
    pub cm: &'a CollisionModel,
    pub movers: &'a [Mover],
}

fn overlaps(lo: Vec3, hi: Vec3, m: &Mover) -> bool {
    (0..3).all(|k| lo[k] <= m.hi[k] && hi[k] >= m.lo[k])
}

impl Collide for Scene<'_> {
    fn trace(&self, a: Vec3, b: Vec3) -> (f32, Vec3, Vec3) {
        let mut best = self.cm.trace(a, b);
        if best.0 == 0.0 {
            return best;
        }
        let (lo, hi) = (a.min(b), a.max(b));
        for m in self.movers.iter().filter(|m| overlaps(lo, hi, m)) {
            let t = self.cm.hull(m.sub, 0).trace(a - m.origin, b - m.origin);
            // starting inside a mover: let it pass rather than stick
            if !t.startsolid && !t.allsolid && t.fraction < best.0 {
                best = (t.fraction, t.endpos + m.origin, t.plane_normal);
            }
        }
        best
    }

    fn solid(&self, p: Vec3) -> bool {
        self.cm.solid(p)
            || self.movers.iter().any(|m| {
                overlaps(p, p, m) && {
                    let h = self.cm.hull(m.sub, 0);
                    h.point_contents(h.first, p - m.origin) == CONTENTS_SOLID
                }
            })
    }

    fn water(&self, p: Vec3) -> bool {
        self.cm.water(p)
    }
}

/// How a point near a mover that moved by `d` is affected.
enum Effect {
    Carry,
    Wake,
    None,
}

fn effect(m: &Mover, d: Vec3, p: Vec3, r: f32) -> Effect {
    let (olo, ohi) = (m.lo - d, m.hi - d);
    let in_xy = |lo: Vec3, hi: Vec3, pad: f32| (0..2).all(|k| p[k] >= lo[k] - pad && p[k] <= hi[k] + pad);
    let riding = in_xy(olo, ohi, 2.0) && p.z >= ohi.z - 4.0 && p.z <= ohi.z + r + 6.0;
    let shoved = in_xy(m.lo, m.hi, r) && p.z >= m.lo.z - r && p.z <= m.hi.z + r;
    if riding || shoved {
        Effect::Carry
    } else if in_xy(olo.min(m.lo), ohi.max(m.hi), 16.0) && p.z >= olo.z.min(m.lo.z) - 16.0 && p.z <= ohi.z.max(m.hi.z) + 24.0 {
        Effect::Wake
    } else {
        Effect::None
    }
}

impl Debris {
    /// Collects this frame's movers and carries debris along with them.
    pub(super) fn update_movers(&mut self, cl: &ClientState, enabled: bool) {
        let prev = std::mem::take(&mut self.movers);
        let Some(cm) = self.cm.clone() else { return };
        if !enabled {
            return;
        }
        for &i in &cl.visedicts {
            let e = &cl.entities[i];
            if cl.debris_owned.get(i).copied().unwrap_or(false) || e.angles.iter().any(|a| *a != 0.0) {
                continue;
            }
            let Some(sub) = cl.model_name(e.model).and_then(|n| n.strip_prefix('*')).and_then(|n| n.parse::<usize>().ok())
            else {
                continue;
            };
            let Some(s) = cm.submodels.get(sub).filter(|_| sub > 0) else { continue };
            let origin = Vec3::from(e.origin);
            self.movers.push(Mover { ent: i, sub, origin, lo: s.mins + origin, hi: s.maxs + origin });
        }
        for m in &self.movers {
            let Some(p) = prev.iter().find(|p| p.ent == m.ent && p.sub == m.sub) else { continue };
            let d = m.origin - p.origin;
            if d.length_sq() < 1e-6 || d.length_sq() > 64.0 * 64.0 {
                continue; // still, or teleported
            }
            for c in &mut self.corpses {
                let mut wake = false;
                for k in 0..c.doll.pos.len() {
                    match effect(m, d, c.doll.pos[k], c.doll.radius_of(k)) {
                        Effect::Carry => {
                            c.doll.translate(k, d);
                            wake = true;
                        }
                        Effect::Wake => wake = true,
                        Effect::None => {}
                    }
                }
                if wake {
                    c.doll.wake();
                }
            }
            for b in &mut self.bodies {
                match effect(m, d, b.pos, b.radius) {
                    Effect::Carry => {
                        b.pos += d;
                        b.wake();
                    }
                    Effect::Wake => b.wake(),
                    Effect::None => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lift(z: f32) -> Mover {
        let origin = Vec3::new(0.0, 0.0, z);
        Mover { ent: 1, sub: 1, origin, lo: Vec3::new(-32.0, -32.0, -8.0) + origin, hi: Vec3::new(32.0, 32.0, 0.0) + origin }
    }

    #[test]
    fn lift_carries_and_wakes() {
        let m = lift(4.0);
        let d = Vec3::new(0.0, 0.0, 4.0);
        assert!(matches!(effect(&m, d, Vec3::new(0.0, 0.0, 2.0), 2.0), Effect::Carry), "resting on top");
        assert!(matches!(effect(&m, d, Vec3::new(40.0, 0.0, 2.0), 2.0), Effect::Wake), "beside it");
        assert!(matches!(effect(&m, d, Vec3::new(200.0, 0.0, 2.0), 2.0), Effect::None), "far away");
    }
}
