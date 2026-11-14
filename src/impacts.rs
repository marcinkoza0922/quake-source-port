//! Bounded, client-only weapon marks and physical nails.
use crate::blood::{Geometry, mesh};
use crate::fte::world::{BspWorld, brush_ents};
use qcore::math::Vec3;
use qpart::World;
use qrender::{FteBatch, FteBlend, FteVertex, ModelId, PicId, RenderEntity};
use std::collections::{HashMap, VecDeque};
mod nails;
mod surfaces;
#[cfg(test)]
mod tests;

struct Mark {
    kind: u8,
    born: f64,
    entity: i32,
    normal: Vec3,
    points: Vec<mesh::Point>,
}
struct Nail {
    born: f64,
    pos: Vec3,
    dir: Vec3,
    vel: Vec3,
    anchor: Option<Anchor>,
    sleeping: bool,
    /// Entity and local offset of the surface supporting a loose sleeping nail.
    support: Option<(i32, Vec3)>,
}
enum Anchor {
    Brush(i32),
    Mesh {
        key: u32,
        model: ModelId,
        tri: [usize; 3],
        bary: Vec3,
        side: f32,
    },
}
struct Flight {
    pos: Vec3,
    vel: Vec3,
}
type Poses<'a> = HashMap<u32, (&'a RenderEntity, Vec<Vec3>)>;

#[derive(Default)]
pub struct Impacts {
    marks: VecDeque<Mark>,
    nails: VecDeque<Nail>,
    models: HashMap<ModelId, qformats::Mdl>,
    flights: HashMap<usize, Flight>,
    intercepted: std::collections::HashSet<usize>,
    cm: Option<qworld::CollisionModel>,
    pic: Option<PicId>,
    time: f64,
}
impl Impacts {
    pub fn clear(&mut self) {
        let pic = self.pic;
        *self = Self::default();
        self.pic = pic;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cv: &qcore::cvar::Cvars,
        cl: &qclient::ClientState,
        lv: &crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut qrender::Renderer,
        ents: &mut Vec<RenderEntity>,
        dynverts: &[u32],
        view: Vec3,
        debris: &crate::debris::Debris,
        out: &mut Geometry,
    ) {
        let enabled = ["r_impact_bullets", "r_impact_nails", "r_impact_scorches"]
            .map(|n| cv.value_of(n) != 0.0);
        if !enabled.iter().any(|v| *v) {
            self.clear();
            return;
        }
        if cl.time < self.time {
            self.clear();
        }
        let dt = (cl.time - self.time).clamp(0.0, 0.1) as f32;
        self.time = cl.time;
        let Some(bsp) = lv.bsp.as_ref() else { return };
        let cm = self
            .cm
            .take()
            .unwrap_or_else(|| qworld::CollisionModel::from_bsp(bsp));
        let brushes = brush_ents(cl, &cm);
        let world = BspWorld {
            bsp,
            cm: &cm,
            ents: &brushes,
        };
        if enabled[1] {
            for e in ents.iter().filter(|e| e.key != 0 && !e.viewmodel) {
                if self.models.contains_key(&e.model) {
                    continue;
                }
                if let Some(name) = cl
                    .entities
                    .get(e.key as usize)
                    .and_then(|e| cl.model_name(e.model))
                    && let Some(data) = fs.open(name)
                    && let Ok(m) = qformats::Mdl::parse(&data)
                {
                    self.models.insert(e.model, m);
                }
            }
        }
        let poses: Poses<'_> = ents
            .iter()
            .filter_map(|e| {
                let m = self.models.get(&e.model)?;
                // Death sequences identify flesh across mods, excluding weapon/effect MDLs.
                if e.key == 0
                    || e.viewmodel
                    || !m.frames.iter().any(|f| {
                        f.poses.iter().any(|p| {
                            ["death", "deth", "dth", "die", "dead"]
                                .iter()
                                .any(|s| p.name.to_ascii_lowercase().contains(s))
                        })
                    })
                {
                    return None;
                }
                Some((e.key, (e, mesh::positions(m, e, dynverts, cl.time))))
            })
            .collect();
        for &(kind, p) in &cl.weapon_impacts {
            if !enabled.get(kind as usize).copied().unwrap_or(false) {
                continue;
            }
            let p = Vec3::from(p);
            let a = debris.shot_origin(view, p);
            let dir = (p - a).normalize();
            match kind {
                0 => self.surface(&world, p - dir * 8.0, p + dir * 16.0, kind, cl.time),
                1 => {
                    self.nail(
                        &world,
                        &poses,
                        p - dir * 24.0,
                        p + dir * 16.0,
                        cl.time,
                        true,
                    );
                }
                2 => {
                    for d in [
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(-1.0, 0.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                        Vec3::new(0.0, -1.0, 0.0),
                        Vec3::new(0.0, 0.0, 1.0),
                        Vec3::new(0.0, 0.0, -1.0),
                    ] {
                        self.surface(&world, p, p + d * 96.0, kind, cl.time);
                    }
                }
                _ => {}
            }
        }
        if enabled[1] {
            // Flesh hits produce blood rather than TE_SPIKE. A disappearing nail
            // supplies its own flight line, also allowing contact with ragdolls.
            let mut flights = HashMap::new();
            for (key, e) in cl.active_entities() {
                if !cl
                    .model_name(e.model)
                    .is_some_and(|n| n == "progs/spike.mdl" || n == "progs/s_spike.mdl")
                {
                    continue;
                }
                let pos = Vec3::from(e.msg_origins[0]);
                let delta = (cl.mtime[0] - cl.mtime[1]) as f32;
                let vel = if delta > 0.001 {
                    (pos - Vec3::from(e.msg_origins[1])) / delta
                } else {
                    Vec3::ZERO
                };
                if vel.length_sq() > 250.0 * 250.0 {
                    flights.insert(key, Flight { pos, vel });
                }
            }
            for (&key, f) in &flights {
                if self.intercepted.contains(&key) {
                    continue;
                }
                if let Some(prev) = self.flights.get(&key) {
                    let a = prev.pos;
                    if (f.pos - a).length_sq() > 0.01
                        && self.nail(&world, &poses, a, f.pos, cl.time, false)
                    {
                        self.intercepted.insert(key);
                    }
                }
            }
            for (key, f) in std::mem::take(&mut self.flights) {
                if self.intercepted.contains(&key) {
                    continue;
                }
                if flights.contains_key(&key) {
                    continue;
                }
                let end = f.pos + f.vel * 0.1;
                if cl
                    .weapon_impacts
                    .iter()
                    .any(|(k, p)| *k == 1 && (Vec3::from(*p) - f.pos).length() < 180.0)
                {
                    continue;
                }
                self.nail(
                    &world,
                    &poses,
                    f.pos - f.vel.normalize() * 8.0,
                    end,
                    cl.time,
                    false,
                );
            }
            self.intercepted.retain(|key| flights.contains_key(key));
            self.flights = flights;
        } else {
            self.nails.clear();
            self.flights.clear();
            self.intercepted.clear();
        }
        let lifetime = cv.value_of("r_impact_lifetime").clamp(5.0, 600.0) as f64;
        let limit = cv.value_of("r_impact_limit").clamp(32.0, 2048.0) as usize;
        self.marks
            .retain(|m| enabled[m.kind as usize] && cl.time - m.born < lifetime);
        self.nails.retain(|n| {
            cl.time - n.born < lifetime
                && match &n.anchor {
                    Some(Anchor::Mesh { key, model, .. }) => {
                        cl.entities.get(*key as usize).is_some_and(|e| {
                            e.msgtime == cl.mtime[0]
                                && lv
                                    .models
                                    .get(e.model)
                                    .copied()
                                    .flatten()
                                    .is_some_and(|m| m.id == *model)
                        })
                    }
                    _ => true,
                }
        });
        while self.marks.len() + self.nails.len() > limit {
            if self.marks.front().map(|m| m.born).unwrap_or(f64::INFINITY)
                < self.nails.front().map(|n| n.born).unwrap_or(f64::INFINITY)
            {
                self.marks.pop_front();
            } else {
                self.nails.pop_front();
            }
        }
        self.step_nails(&world, &poses, dt, cv.value_of("sv_gravity").max(1.0));
        if self.pic.is_none() {
            self.pic = r
                .register_pic_rgba_linear(96, 32, &surfaces::texture())
                .ok();
        }
        if let Some(pic) = self.pic {
            self.draw(&world, &poses, pic, cl.time, lifetime, out);
        }
        drop(poses);
        ents.retain(|e| !self.intercepted.contains(&(e.key as usize)));
        self.cm = Some(cm);
    }

    fn draw(
        &self,
        world: &dyn World,
        poses: &Poses<'_>,
        pic: PicId,
        time: f64,
        life: f64,
        out: &mut Geometry,
    ) {
        let first = out.1.len() as u32;
        for m in &self.marks {
            let origin = if m.entity == 0 {
                Vec3::ZERO
            } else {
                let Some(o) = world.entity_origin(m.entity) else {
                    continue;
                };
                o
            };
            let base = out.0.len() as u32;
            let alpha = fade(time - m.born, life);
            for p in &m.points {
                out.0.push(FteVertex {
                    pos: (p.pos + origin + m.normal * 0.12).to_array(),
                    uv: [
                        (p.uv[0] + if m.kind == 0 { 0.0 } else { 1.0 }) / 3.0,
                        p.uv[1],
                    ],
                    color: if m.kind == 0 {
                        [90, 82, 70, alpha]
                    } else {
                        [12, 10, 8, alpha]
                    },
                });
            }
            for i in 1..m.points.len() - 1 {
                out.1.extend([base, base + i as u32, base + i as u32 + 1]);
            }
        }
        batch(pic, first, out);
        let first = out.1.len() as u32;
        for n in &self.nails {
            if matches!(&n.anchor, Some(Anchor::Mesh {key,..}) if !poses.contains_key(key)) {
                continue;
            }
            let (u, v) = mesh::axes(n.dir);
            // Opaque square shaft and tapered tip, visible from every direction.
            let origin = match n.anchor {
                Some(Anchor::Brush(entity)) if entity != 0 => {
                    let Some(o) = world.entity_origin(entity) else {
                        continue;
                    };
                    o
                }
                _ => Vec3::ZERO,
            };
            let tip = n.pos + origin;
            let tail = tip - n.dir * 6.0;
            let ring = [u, v, -u, -v].map(|a| tail + a * 0.3);
            let tipring = [u, v, -u, -v].map(|a| tip - n.dir * 1.0 + a * 0.3);
            let alpha = fade(time - n.born, life);
            for k in 0..4 {
                let j = (k + 1) % 4;
                let shade = [150, 95, 65, 115][k];
                let base = out.0.len() as u32;
                for p in [ring[k], ring[j], tipring[j], tipring[k], tip] {
                    out.0.push(FteVertex {
                        pos: p.to_array(),
                        uv: [5.0 / 6.0, 0.5],
                        color: [shade, shade, shade, alpha],
                    });
                }
                out.1.extend([
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                    base + 3,
                    base + 2,
                    base + 4,
                ]);
            }
        }
        batch(pic, first, out);
    }
}
fn fade(age: f64, life: f64) -> u8 {
    (((life - age) / 5.0).clamp(0.0, 1.0) * 255.0) as u8
}
fn batch(pic: PicId, first: u32, out: &mut Geometry) {
    let count = out.1.len() as u32 - first;
    if count > 0 {
        out.2.push(FteBatch {
            pic,
            blend: FteBlend::Blend,
            lines: false,
            soft: false, gel: false,
            first_index: first,
            num_indices: count,
        });
    }
}
