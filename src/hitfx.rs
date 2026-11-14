//! Surface hit effects: sparks, smoke, chips, splinters, dust and splashes
//! thrown off whatever a bullet, nail or explosion hits, chosen by the
//! material of the texture that was struck. Client-only and cosmetic;
//! `r_impact_fx` picks the intensity from subtle to spectacle.
use crate::blood::Geometry;
use crate::fte::world::{BspWorld, brush_ents};
use qcore::math::Vec3;
use qpart::World;
use qrender::{PicId, View};
use std::collections::HashMap;
mod draw;
mod spawn;

pub(crate) use spawn::Material;

/// Particle budget; the oldest are dropped first.
const MAX_PARTS: usize = 6000;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Additive streak stretched along its velocity.
    Spark,
    /// Short additive flash at the point of impact.
    Flash,
    /// Soft alpha puff that grows and fades.
    Smoke,
    /// Opaque tumbling fragment (stone chip, splinter, clod, droplet).
    Chip,
}

#[derive(Clone, Copy, Debug)]
struct Part {
    kind: Kind,
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size: f32,
    /// Size added per second (smoke spreads).
    grow: f32,
    /// Length / width for chips: splinters are long, chips square.
    stretch: f32,
    color: [u8; 3],
    alpha: f32,
    gravity: f32,
    drag: f32,
    spin: f32,
    angle: f32,
    /// Bounces left before a fragment comes to rest; 0 = no collision.
    bounces: u8,
    resting: bool,
}

impl Default for Part {
    fn default() -> Self {
        Part {
            kind: Kind::Chip,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            age: 0.0,
            life: 1.0,
            size: 1.0,
            grow: 0.0,
            stretch: 1.0,
            color: [255; 3],
            alpha: 1.0,
            gravity: 0.0,
            drag: 0.0,
            spin: 0.0,
            angle: 0.0,
            bounces: 0,
            resting: false,
        }
    }
}

#[derive(Default)]
pub struct HitFx {
    parts: Vec<Part>,
    cm: Option<qworld::CollisionModel>,
    pic: Option<PicId>,
    palette: Option<Vec<u8>>,
    tints: HashMap<String, [u8; 3]>,
    time: f64,
    seed: u32,
}

impl HitFx {
    pub fn clear(&mut self) {
        let (pic, palette) = (self.pic, self.palette.take());
        *self = Self::default();
        self.pic = pic;
        self.palette = palette;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cv: &qcore::cvar::Cvars,
        cl: &qclient::ClientState,
        lv: &crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut qrender::Renderer,
        view: &View,
        debris: &crate::debris::Debris,
        out: &mut Geometry,
    ) {
        let level = cv.value_of("r_impact_fx").clamp(0.0, 4.0) as u8;
        if level == 0 {
            if !self.parts.is_empty() {
                self.parts.clear();
            }
            return;
        }
        if cl.time < self.time {
            self.parts.clear();
        }
        let dt = (cl.time - self.time).clamp(0.0, 0.1) as f32;
        self.time = cl.time;
        let Some(bsp) = lv.bsp.as_ref() else { return };
        if self.palette.is_none() {
            self.palette = Some(fs.open("gfx/palette.lmp").unwrap_or_default());
        }
        let cm = self.cm.take().unwrap_or_else(|| qworld::CollisionModel::from_bsp(bsp));
        let brushes = brush_ents(cl, &cm);
        let world = BspWorld { bsp, cm: &cm, ents: &brushes };
        let eye = Vec3::from(view.origin);
        for &(kind, p) in &cl.weapon_impacts {
            let p = Vec3::from(p);
            let from = debris.shot_origin(eye, p);
            self.impact(&world, kind, p, from, level);
        }
        self.step(&world, dt, cv.value_of("sv_gravity").max(1.0));
        if self.parts.len() > MAX_PARTS {
            let extra = self.parts.len() - MAX_PARTS;
            self.parts.drain(..extra);
        }
        self.cm = Some(cm);
        if self.parts.is_empty() {
            return;
        }
        if self.pic.is_none() {
            self.pic = r.register_pic_rgba_linear(draw::W, draw::H, &draw::texture()).ok();
        }
        if let Some(pic) = self.pic {
            draw::draw(&self.parts, view, pic, out);
        }
    }

    /// Find the surface at an impact and throw off its material's effect.
    fn impact(&mut self, world: &BspWorld<'_>, kind: u8, p: Vec3, from: Vec3, level: u8) {
        if kind == 2 {
            // Explosions: the nearest surface around the blast sheds debris.
            let dirs = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
            let best = dirs
                .iter()
                .map(|d| world.trace_line(p, p + *d * 96.0))
                .filter(|t| t.frac > 0.0 && t.frac < 1.0)
                .min_by(|a, b| a.frac.total_cmp(&b.frac));
            if let Some(tr) = best {
                let mat = self.material(world, &tr);
                spawn::blast(self, mat, tr.endpos, tr.normal, level);
            }
            return;
        }
        let dir = (p - from).normalize();
        let dir = if dir.length_sq() > 0.5 { dir } else { Vec3::new(0.0, 0.0, -1.0) };
        let tr = world.trace_line(p - dir * 8.0, p + dir * 16.0);
        let (pos, normal, mat) = if tr.frac > 0.0 && tr.frac < 1.0 {
            (tr.endpos, tr.normal, self.material(world, &tr))
        } else {
            (p, -dir, (Material::Stone, [120, 115, 105]))
        };
        spawn::hit(self, mat, pos + normal * 0.5, normal, dir, kind, level);
    }

    fn material(&mut self, world: &BspWorld<'_>, tr: &qpart::Trace) -> (Material, [u8; 3]) {
        let Some(tex) = crate::impacts::surfaces::texture_at(world, tr) else {
            return (Material::Stone, [120, 115, 105]);
        };
        let mat = spawn::classify(&tex.name);
        let pal = self.palette.as_deref().unwrap_or(&[]);
        let tint = *self.tints.entry(tex.name.clone()).or_insert_with(|| spawn::average(tex, pal));
        (mat, tint)
    }

    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// -1..1
    fn crand(&mut self) -> f32 {
        self.rand() * 2.0 - 1.0
    }

    fn step(&mut self, world: &dyn World, dt: f32, gravity: f32) {
        if dt <= 0.0 {
            return;
        }
        for p in &mut self.parts {
            p.age += dt;
            if p.resting {
                continue;
            }
            p.vel.z -= gravity * p.gravity * dt;
            p.vel *= (1.0 - p.drag * dt).max(0.0);
            p.size += p.grow * dt;
            p.angle += p.spin * dt;
            let next = p.pos + p.vel * dt;
            if p.bounces > 0 {
                let tr = world.trace_line(p.pos, next);
                if tr.frac < 1.0 {
                    let n = tr.normal;
                    let vn = p.vel.dot(n);
                    p.vel = (p.vel - n * (vn * 1.45)) * 0.55;
                    p.pos = tr.endpos + n * 0.3;
                    p.spin *= 0.5;
                    p.bounces -= 1;
                    if p.bounces == 0 || (n.z > 0.7 && p.vel.length_sq() < 40.0 * 40.0) {
                        p.resting = n.z > 0.7;
                        p.vel = Vec3::ZERO;
                        if p.kind == Kind::Spark {
                            p.life = p.life.min(p.age + 0.05);
                        }
                    }
                    continue;
                }
            }
            p.pos = next;
        }
        self.parts.retain(|p| p.age < p.life);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_fall_and_expire() {
        let mut fx = HitFx::default();
        fx.parts.push(Part { gravity: 1.0, life: 0.5, ..Default::default() });
        let w = qpart::EmptyWorld;
        fx.step(&w, 0.1, 800.0);
        assert!(fx.parts[0].pos.z < 0.0);
        for _ in 0..5 {
            fx.step(&w, 0.1, 800.0);
        }
        assert!(fx.parts.is_empty());
    }
}
