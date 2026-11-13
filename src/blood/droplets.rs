//! Airborne blood: fast droplets that fly ballistically and leave a surface
//! mark only where they land, instead of marks appearing instantly.
use super::*;

const GRAVITY: f32 = 800.0;
const MAX_AGE: f32 = 2.5;
pub(super) const MAX_DROPLETS: usize = 2048;

pub(super) struct Droplet {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Size of the mark left on landing.
    pub size: f32,
    pub age: f32,
}

impl Blood {
    /// Launches a droplet; `speed` sets how far it carries before gravity wins.
    pub(super) fn launch(&mut self, p: Vec3, dir: Vec3, speed: f32, size: f32) {
        if self.droplets.len() >= MAX_DROPLETS {
            self.droplets.swap_remove(0);
        }
        self.droplets.push(Droplet {
            pos: p,
            vel: dir * speed,
            size,
            age: 0.0,
        });
    }

    /// Advances droplets by `dt`, swept-tracing each step so fast ones
    /// cannot tunnel through thin walls. Landed droplets become marks.
    pub(super) fn step_droplets(&mut self, world: &dyn World, dt: f32, time: f64) {
        if dt <= 0.0 {
            return;
        }
        let mut landed = Vec::new();
        self.droplets.retain_mut(|d| {
            d.age += dt;
            let next = d.pos + d.vel * dt + Vec3::new(0.0, 0.0, -0.5 * GRAVITY * dt * dt);
            d.vel.z -= GRAVITY * dt;
            let tr = world.trace_line(d.pos, next);
            if tr.frac < 1.0 {
                if world.point_contents(tr.endpos) & qpart::FTECONTENTS_SKY == 0 {
                    let seg = next - d.pos;
                    let len = seg.length();
                    if len > 0.0 {
                        landed.push((d.pos, seg / len, len + 2.0, d.size));
                    }
                }
                return false;
            }
            d.pos = next;
            d.age < MAX_AGE
        });
        for (p, dir, range, size) in landed {
            self.splat_range(world, p, dir, time, size, range);
        }
    }

    /// Steps until every droplet has landed or expired (tests, debug).
    #[cfg(test)]
    pub(super) fn settle(&mut self, world: &dyn World, time: f64) {
        for _ in 0..400 {
            if self.droplets.is_empty() {
                break;
            }
            self.step_droplets(world, 1.0 / 120.0, time);
        }
    }

    /// Camera-facing streaks stretched along velocity.
    pub(super) fn draw_droplets(&self, view: Vec3, out: &mut Geometry) {
        for d in &self.droplets {
            let speed = d.vel.length();
            if speed < 1e-3 {
                continue;
            }
            let along = d.vel / speed * (speed * 0.008).clamp(0.5, 3.5);
            let to_eye = view - d.pos;
            let side = along.cross(to_eye);
            if side.length_sq() < 1e-6 {
                continue;
            }
            let width = (d.size * 0.04).clamp(0.25, 0.8);
            let side = side.normalize() * width;
            let base = out.0.len() as u32;
            for (pos, uv) in [
                (d.pos - along - side, [0.42, 0.42]),
                (d.pos - along + side, [0.58, 0.42]),
                (d.pos + along + side, [0.58, 0.58]),
                (d.pos + along - side, [0.42, 0.58]),
            ] {
                out.0.push(FteVertex {
                    pos: pos.to_array(),
                    uv: appearance::uv(uv, 0),
                    color: [70, 4, 2, 235],
                });
            }
            out.1
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}
