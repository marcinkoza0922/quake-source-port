//! Verlet ragdoll over a [`Rig`]: one particle per bone cluster, distance
//! links between them, per-bone rotations recovered by shape matching, and
//! linear-blend skinning of the original mesh into a packed pose the
//! renderer draws like any alias frame.
use std::rc::Rc;

use qcore::math::Vec3;

use super::Collide;
use super::math::{Mat3, Quat, add_outer, apply, extract_rotation, normal_index};
use super::rig::{MAX_BONES, Rig};

/// Packed skinned vertices for one ragdoll, ready for `RenderEntity::dyn_pose`.
#[derive(Default)]
pub struct Packed {
    pub verts: Vec<u32>,
    pub origin: Vec3,
    pub scale: Vec3,
    pub center: Vec3,
    pub radius: f32,
}

pub struct Ragdoll {
    pub rig: Rc<Rig>,
    pub pos: Vec<Vec3>,
    prev: Vec<Vec3>,
    rot: Vec<Quat>,
    /// Per-particle collision radius (the rig's, clamped to initial clearance).
    radii: Vec<f32>,
    /// Step size (`1/60`..`1/30` s) encoded in `pos - prev`.
    step: f32,
    /// Seconds spent nearly still; asleep past [`SLEEP_AFTER`].
    still: f32,
    pub asleep: bool,
    packed: Packed,
    dirty: bool,
    skin_scratch: Vec<Vec3>,
}

const SLEEP_AFTER: f32 = 1.0;

impl Ragdoll {
    /// Places the rig's rest pose at `origin` with entity `angles`, moving
    /// at `vel` (units/s).
    pub fn new(rig: Rc<Rig>, origin: Vec3, angles: [f32; 3], vel: Vec3, step: f32, cm: &dyn Collide) -> Ragdoll {
        let m = super::math::entity_rotation(angles);
        let q = mat_to_quat(&m);
        let pos: Vec<Vec3> = rig.centers.iter().map(|c| origin + apply(&m, *c)).collect();
        let prev = pos.iter().map(|p| *p - vel * step).collect();
        let n = pos.len();
        // Clamp collision radii to the clearance the corpse already has so it
        // does not pop up off the floor when taken over.
        let radii = pos
            .iter()
            .zip(&rig.radius)
            .map(|(p, &r)| {
                let tr = cm.trace(*p, *p - Vec3::new(0.0, 0.0, r));
                if tr.0 < 1.0 { (r * tr.0).max(0.5) } else { r }
            })
            .collect();
        Ragdoll {
            rig,
            pos,
            prev,
            rot: vec![q; n],
            radii,
            step,
            still: 0.0,
            asleep: false,
            packed: Packed::default(),
            dirty: true,
            skin_scratch: Vec::new(),
        }
    }

    pub fn set_step(&mut self, step: f32) {
        if (self.step - step).abs() > 1e-6 {
            let scale = step / self.step;
            for (p, prev) in self.pos.iter().zip(&mut self.prev) {
                *prev = *p - (*p - *prev) * scale;
            }
            self.step = step;
        }
    }

    fn radius(&self, i: usize) -> f32 {
        self.radii[i]
    }

    /// Adds `dv` (units/s) to particles within `range` of `p`, scaled by
    /// proximity; returns whether anything was hit.
    pub fn push(&mut self, p: Vec3, range: f32, dv: Vec3) -> bool {
        let mut hit = false;
        for i in 0..self.pos.len() {
            let d = (self.pos[i] - p).length();
            if d < range {
                let f = 1.0 - d / range;
                self.prev[i] -= dv * (f * self.step);
                hit = true;
            }
        }
        if hit {
            self.wake();
        }
        hit
    }

    /// A bullet or nail along `from..to` knocks the bone nearest the line.
    pub fn shot(&mut self, from: Vec3, to: Vec3, force: f32) {
        let Some((_, i)) = self.ray_gap(from, to) else { return };
        let dir = (to - from).normalize();
        let at = self.pos[i];
        self.push(at, 20.0, (dir * 260.0 + Vec3::new(0.0, 0.0, 40.0)) * force);
    }

    /// A killing blow knocks the whole body back along the shot, in
    /// proportion to the hit, so a good shot throws it instead of only
    /// twitching the bone it struck; `lift` also raises it.
    pub fn fling(&mut self, from: Vec3, to: Vec3, force: f32, lift: bool) {
        let flat = Vec3::new(to.x - from.x, to.y - from.y, 0.0).normalize();
        let up = if lift { KILL_LIFT } else { 0.0 };
        let dv = (flat * KILL_BACK + Vec3::new(0.0, 0.0, up)) * force;
        for p in &mut self.prev {
            *p -= dv * self.step;
        }
        self.wake();
    }

    /// Radial push away from `center` (explosions).
    pub fn blast(&mut self, center: Vec3, range: f32, speed: f32) -> bool {
        let mut hit = false;
        for i in 0..self.pos.len() {
            let (dir, d) = (self.pos[i] - center).normalize_len();
            if d < range {
                let f = 1.0 - d / range;
                let dv = (dir + Vec3::new(0.0, 0.0, 0.6)) * (speed * f);
                self.prev[i] -= dv * self.step;
                hit = true;
            }
        }
        if hit {
            self.wake();
        }
        hit
    }

    /// Closest particle distance to the segment `a..b`, minus its radius.
    pub fn ray_gap(&self, a: Vec3, b: Vec3) -> Option<(f32, usize)> {
        let (dir, len) = (b - a).normalize_len();
        (0..self.pos.len())
            .map(|i| {
                let t = (self.pos[i] - a).dot(dir).clamp(0.0, len.max(0.0));
                ((a + dir * t - self.pos[i]).length() - self.radius(i) - 6.0, i)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
    }

    /// Distance from `p` to the nearest bone's surface.
    pub fn gap(&self, p: Vec3) -> f32 {
        (0..self.pos.len()).map(|i| (self.pos[i] - p).length() - self.radius(i)).fold(f32::MAX, f32::min)
    }

    pub fn radius_of(&self, i: usize) -> f32 {
        self.radii[i]
    }

    /// Moves one particle without giving it velocity (carried by a mover).
    pub fn translate(&mut self, i: usize, d: Vec3) {
        self.pos[i] += d;
        self.prev[i] += d;
        self.dirty = true;
    }

    pub fn wake(&mut self) {
        self.asleep = false;
        self.still = 0.0;
    }

    /// One fixed simulation step (`self.step` seconds).
    pub fn step(&mut self, gravity: f32, cm: &dyn Collide, iters: usize, skip_down: bool) {
        if self.asleep {
            return;
        }
        let step = self.step;
        let scale60 = step * 60.0;
        let g = Vec3::new(0.0, 0.0, -gravity * step * step);
        let mut moved = 0.0f32;
        let water_damp = 0.9f32.powf(scale60);
        let air_damp = 0.995f32.powf(scale60);
        for i in 0..self.pos.len() {
            let water = cm.water(self.pos[i]);
            let damp = if water { water_damp } else { air_damp };
            let gi = if water { g * 0.15 } else { g };
            let v = (self.pos[i] - self.prev[i]) * damp;
            self.prev[i] = self.pos[i];
            self.pos[i] += v + gi;
        }
        for _ in 0..iters.max(1) {
            for l in &self.rig.links {
                let (a, b) = (l.a as usize, l.b as usize);
                let (dir, d) = (self.pos[b] - self.pos[a]).normalize_len();
                if d < 1e-4 {
                    continue;
                }
                let err = if l.hard {
                    d - l.rest
                } else if d < l.rest * 0.5 {
                    d - l.rest * 0.5
                } else {
                    continue;
                };
                let corr = dir * (err * 0.5);
                self.pos[a] += corr;
                self.pos[b] -= corr;
            }
        }
        for i in 0..self.pos.len() {
            self.collide(i, cm, skip_down);
            moved = moved.max((self.pos[i] - self.prev[i]).length());
        }
        self.match_rotations(iters);
        if moved < 2.4 * step {
            self.still += step;
            if self.still > SLEEP_AFTER {
                self.asleep = true;
                // snap still so the cached pose is exact
                self.prev.clone_from(&self.pos);
            }
        } else {
            self.still = 0.0;
        }
        self.dirty = true;
    }

    fn collide(&mut self, i: usize, cm: &dyn Collide, skip_down: bool) {
        let r = self.radius(i);
        let (from, to) = (self.prev[i], self.pos[i]);
        if cm.solid(from) {
            return; // started inside geometry; let the links pull it out
        }
        let mut hit = None;
        if (to - from).length_sq() >= 1e-10 {
            let tr = cm.trace(from, to);
            if tr.0 < 1.0 {
                self.pos[i] = tr.1 + tr.2 * 0.1;
                hit = Some(tr.2);
            }
        }
        if !skip_down {
            // keep the particle its radius above the floor
            let p = self.pos[i];
            let down = cm.trace(p, p - Vec3::new(0.0, 0.0, r));
            if down.0 < 1.0 && down.2.z > 0.5 {
                self.pos[i].z = down.1.z + r * down.2.z;
                hit = Some(down.2);
            }
        }
        if let Some(n) = hit {
            // friction and a little bounce, through the implied velocity
            let v = self.pos[i] - self.prev[i];
            let vn = n * v.dot(n);
            let vt = v - vn;
            let nv = vt * 0.55 - vn * 0.15;
            self.prev[i] = self.pos[i] - nv;
        }
    }

    /// Per-bone rotation from its linked neighbours (shape matching),
    /// regularised by the previous rotation so leaf bones keep their twist.
    fn match_rotations(&mut self, iters: usize) {
        let rig = &self.rig;
        for c in 0..self.pos.len() {
            let prev = self.rot[c];
            let mut a: Mat3 = [[0.0; 3]; 3];
            let mut wsum = 0.0;
            for &j in &rig.neighbors[c] {
                let j = j as usize;
                let r = rig.centers[j] - rig.centers[c];
                let d = self.pos[j] - self.pos[c];
                add_outer(&mut a, d, r, 1.0);
                wsum += r.length_sq();
            }
            let pm = prev.to_mat();
            let k = 0.3 * wsum.max(1.0);
            for (row, pr) in a.iter_mut().zip(&pm) {
                for (x, p) in row.iter_mut().zip(pr) {
                    *x += p * k;
                }
            }
            self.rot[c] = extract_rotation(&a, prev, iters.max(1));
        }
    }

    /// Conservative world-space bounding sphere `(center, radius)` enclosing all skinned vertices.
    pub fn bounds(&self) -> (Vec3, f32) {
        if !self.dirty {
            return (self.packed.center, self.packed.radius);
        }
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for &p in &self.pos {
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let mid = (lo + hi) * 0.5;
        let r_bones = self.pos.iter().map(|p| (*p - mid).length()).fold(0.0f32, f32::max);
        (mid, r_bones + self.rig.max_offset + 1.0)
    }

    /// Skins the mesh and packs it (cached while asleep and offscreen).
    pub fn pack(&mut self) -> &Packed {
        if self.dirty {
            self.skin();
            self.dirty = false;
        }
        &self.packed
    }

    fn skin(&mut self) {
        let rig = &self.rig;
        let mut mats = [[[0.0f32; 3]; 3]; MAX_BONES];
        for (slot, q) in mats.iter_mut().zip(&self.rot) {
            *slot = q.to_mat();
        }
        let n = rig.rest.len();
        self.skin_scratch.clear();
        self.skin_scratch.reserve(n.saturating_sub(self.skin_scratch.capacity()));
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for v in 0..n {
            let [b0, b1] = rig.bones[v].map(usize::from);
            let w = rig.weight[v];
            let p0 = self.pos[b0] + apply(&mats[b0], rig.rest[v] - rig.centers[b0]);
            let p = if w < 1.0 {
                let p1 = self.pos[b1] + apply(&mats[b1], rig.rest[v] - rig.centers[b1]);
                p0 * w + p1 * (1.0 - w)
            } else {
                p0
            };
            lo = lo.min(p);
            hi = hi.max(p);
            self.skin_scratch.push(p);
        }
        let ext = (hi - lo).max(Vec3::splat(1e-3));
        let scale = ext / 255.0;
        let inv = Vec3::new(1.0 / scale.x, 1.0 / scale.y, 1.0 / scale.z);
        self.packed.verts.clear();
        self.packed.verts.reserve(n.saturating_sub(self.packed.verts.capacity()));
        for (v, p) in self.skin_scratch.iter().enumerate() {
            let q = (*p - lo).mul_elem(inv);
            let b = |x: f32| (x + 0.5).clamp(0.0, 255.0) as u32;
            let nrm = apply(&mats[rig.bones[v][0] as usize], rig.normals[v]);
            self.packed.verts.push(b(q.x) | b(q.y) << 8 | b(q.z) << 16 | (normal_index(nrm) as u32) << 24);
        }
        self.packed.origin = lo;
        self.packed.scale = scale;
        self.packed.center = (lo + hi) * 0.5;
        self.packed.radius = (hi - lo).length() * 0.5;
    }
}

/// Rotation matrix to quaternion (Shepperd).
/// Per-pellet knock-back and optional lift (units/s) a killing volley adds
/// to the whole body; a shotgun's pellets add up.
const KILL_BACK: f32 = 25.0;
const KILL_LIFT: f32 = 30.0;

pub fn mat_to_quat(m: &Mat3) -> Quat {
    let t = m[0][0] + m[1][1] + m[2][2];
    let q = if t > 0.0 {
        let s = (t + 1.0).sqrt() * 2.0;
        [(m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s, 0.25 * s]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s, (m[2][1] - m[1][2]) / s]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [(m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s, (m[0][2] - m[2][0]) / s]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [(m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s, (m[1][0] - m[0][1]) / s]
    };
    Quat(q).normalize()
}
