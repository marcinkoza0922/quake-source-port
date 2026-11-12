//! Tumbling rigid pieces: gibs, heads and barrel fragments. A sphere of
//! `radius` against the world for contacts; orientation is free spin.
use qcore::math::Vec3;
use qrender::ModelId;

use super::math::{Quat, Rng};
use super::{Collide, STEP};

pub struct Body {
    pub model: ModelId,
    pub frame: u32,
    pub skin: u32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Quat,
    /// Angular velocity, radians/s.
    pub spin: Vec3,
    pub radius: f32,
    pub asleep: bool,
    still: f32,
    /// Seconds left (fragments); `f32::INFINITY` while a server entity owns it.
    pub life: f32,
    /// Server entity (number, model index) this replaces (gibs); hidden
    /// while the body exists.
    pub ent: Option<(usize, usize)>,
    /// Blood trail (gibs).
    pub trail: bool,
    pub trail_org: Vec3,
}

impl Body {
    pub fn new(model: ModelId, pos: Vec3, vel: Vec3, rot: Quat, radius: f32) -> Body {
        Body {
            model,
            frame: 0,
            skin: 0,
            pos,
            vel,
            rot,
            spin: Vec3::ZERO,
            radius,
            asleep: false,
            still: 0.0,
            life: f32::INFINITY,
            ent: None,
            trail: false,
            trail_org: pos,
        }
    }

    pub fn random_spin(&mut self, rng: &mut Rng, rate: f32) {
        self.spin = Vec3::new(rng.signed(), rng.signed(), rng.signed()) * rate;
    }

    pub fn push(&mut self, dv: Vec3, rng: &mut Rng) {
        self.vel += dv;
        let s = dv.length() * 0.03;
        self.spin += Vec3::new(rng.signed(), rng.signed(), rng.signed()) * s;
        self.asleep = false;
        self.still = 0.0;
    }

    pub fn wake(&mut self) {
        self.asleep = false;
        self.still = 0.0;
    }

    pub fn step(&mut self, gravity: f32, cm: &dyn Collide) {
        self.life -= STEP;
        if self.asleep {
            return;
        }
        if cm.water(self.pos) {
            self.vel = self.vel * 0.92 - Vec3::new(0.0, 0.0, gravity * 0.1 * STEP);
            self.spin *= 0.95;
        } else {
            self.vel.z -= gravity * STEP;
        }
        let to = self.pos + self.vel * STEP;
        if cm.solid(self.pos) {
            self.pos = to; // embedded: fall through rather than stick
        } else {
            let tr = cm.trace(self.pos, to);
            self.pos = if tr.0 < 1.0 { tr.1 + tr.2 * 0.1 } else { to };
            let mut normal = (tr.0 < 1.0).then_some(tr.2);
            let down = cm.trace(self.pos, self.pos - Vec3::new(0.0, 0.0, self.radius));
            if down.0 < 1.0 && down.2.z > 0.5 {
                self.pos.z = down.1.z + self.radius * down.2.z;
                normal = normal.or(Some(down.2));
            }
            if let Some(n) = normal {
                let vn = n * self.vel.dot(n);
                if vn.dot(n) < 0.0 {
                    let vt = self.vel - vn;
                    self.vel = vt * 0.7 - vn * 0.35;
                    self.spin *= 0.75;
                }
                if n.z > 0.7 && self.vel.length() < 30.0 {
                    self.vel = Vec3::ZERO;
                    self.spin *= 0.5;
                }
            }
        }
        self.rot = self.rot.integrate(self.spin, STEP);
        if self.vel.length_sq() < 1.0 && self.spin.length_sq() < 0.01 {
            self.still += STEP;
            if self.still > 0.3 {
                self.asleep = true;
                self.vel = Vec3::ZERO;
                self.spin = Vec3::ZERO;
            }
        } else {
            self.still = 0.0;
        }
    }

    /// Entity angles reproducing `rot` through the renderer's alias rotation.
    pub fn angles(&self) -> [f32; 3] {
        let m = self.rot.to_mat();
        let p = (-m[2][0]).clamp(-1.0, 1.0).asin();
        let yaw = m[1][0].atan2(m[0][0]);
        let roll = m[2][1].atan2(m[2][2]);
        [-p.to_degrees(), yaw.to_degrees(), roll.to_degrees()]
    }
}

#[cfg(test)]
mod tests {
    use super::super::math::entity_rotation;
    use super::*;

    #[test]
    fn angles_round_trip() {
        let mut b = Body::new(ModelId(0), Vec3::ZERO, Vec3::ZERO, Quat::IDENTITY, 2.0);
        b.rot = Quat::axis_angle(Vec3::new(0.2, 0.9, -0.3).normalize(), 0.8);
        let m = entity_rotation(b.angles());
        let r = b.rot.to_mat();
        for i in 0..3 {
            for j in 0..3 {
                assert!((m[i][j] - r[i][j]).abs() < 1e-3);
            }
        }
    }
}
