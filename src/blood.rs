//! Cosmetic blood, independent of scripted particle packs and server physics.
use crate::fte::world::{BspWorld, brush_ents};
use qcore::math::Vec3;
use qformats::Mdl;
use qpart::World;
use qrender::{FteBatch, FteBlend, FteVertex, ModelId, PicId, RenderEntity};
use std::collections::{HashMap, VecDeque};
mod appearance;
mod droplets;
#[cfg(test)]
mod effects_tests;
mod events;
pub(crate) mod mesh;
#[cfg(test)]
mod tests;

pub type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

#[derive(Clone, Copy)]
pub struct Settings {
    pub enabled: bool,
    pub amount: usize,
    pub size: f32,
    pub lifetime: f64,
    pub limit: usize,
    pub wounds: bool,
    pub gibs: bool,
}
impl Settings {
    pub fn read(cv: &qcore::cvar::Cvars) -> Self {
        Self {
            enabled: cv.value_of("r_blood") != 0.0,
            amount: cv.value_of("r_blood_amount").clamp(1.0, 12.0) as usize,
            size: cv.value_of("r_blood_size").clamp(0.25, 3.0),
            lifetime: cv.value_of("r_blood_lifetime").clamp(5.0, 600.0) as f64,
            limit: cv.value_of("r_blood_maxmarks").clamp(32.0, 2048.0) as usize,
            wounds: cv.value_of("r_blood_wounds") != 0.0,
            gibs: cv.value_of("r_blood_gibs") != 0.0,
        }
    }
}

struct Mark {
    born: f64,
    /// A brush entity or world (0).
    surface: i32,
    normal: Vec3,
    variant: u8,
    /// Wounds follow triangle topology, including a ragdoll takeover.
    owner: Option<(u32, ModelId)>,
    patches: Vec<mesh::Patch>,
}

#[derive(Default)]
pub struct Blood {
    marks: VecDeque<Mark>,
    models: HashMap<ModelId, Mdl>,
    model_ids: HashMap<usize, ModelId>,
    heads: std::collections::HashSet<ModelId>,
    cm: Option<qworld::CollisionModel>,
    pic: Option<PicId>,
    trails: HashMap<u32, Vec3>,
    time: f64,
    seed: u32,
    actors: HashMap<u32, (ModelId, Vec3)>,
    gibs: HashMap<u32, usize>,
    explosions: Vec<(Vec3, f64)>,
    droplets: Vec<droplets::Droplet>,
    pub debug_hits: Vec<(Vec3, bool)>,
}

impl Blood {
    pub fn clear(&mut self) {
        let pic = self.pic;
        *self = Self::default();
        self.pic = pic;
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / 16777216.0
    }

    fn splat(&mut self, world: &dyn World, p: Vec3, dir: Vec3, time: f64, size: f32) {
        self.splat_range(world, p, dir, time, size, 112.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn splat_range(
        &mut self,
        world: &dyn World,
        p: Vec3,
        dir: Vec3,
        time: f64,
        size: f32,
        range: f32,
    ) {
        let tr = world.trace_line(p, p + dir * range);
        if tr.frac >= 1.0
            || tr.frac <= 0.0
            || world.point_contents(tr.endpos) & qpart::FTECONTENTS_SKY != 0
        {
            return;
        }
        let origin = if tr.entity == 0 {
            Vec3::ZERO
        } else {
            let Some(o) = world.entity_origin(tr.entity) else {
                return;
            };
            o
        };
        let center = tr.endpos - origin;
        let variant = (self.random() * 16.0) as u8;
        let shape = [
            self.random() * std::f32::consts::TAU,
            0.7 + self.random() * 0.6,
        ];
        let mut patches = Vec::new();
        world.decal_polys(
            tr.entity,
            center,
            size * 1.5,
            tr.normal,
            false,
            &mut |poly| {
                // BSP traversal also visits adjoining walls and bevels. The
                // entire face must lie on the struck plane: accepting a face
                // with just one nearby vertex extrudes the 2D decal through
                // its unrestricted depth and leaves long, z-fighting bands.
                if poly
                    .iter()
                    .any(|p| (*p - center).dot(tr.normal).abs() > 0.5)
                {
                    return;
                }
                let points = mesh::project_shaped(poly, center, tr.normal, size, shape);
                if points.len() >= 3 {
                    patches.push(mesh::Patch {
                        tri: [0; 3],
                        side: 1.0,
                        points,
                    });
                }
            },
        );
        if !patches.is_empty() {
            self.marks.push_back(Mark {
                born: time,
                surface: tr.entity,
                normal: tr.normal,
                variant,
                owner: None,
                patches,
            });
        }
    }

    fn wound(
        &mut self,
        e: &RenderEntity,
        positions: &[Vec3],
        p: Vec3,
        n: Vec3,
        time: f64,
        size: f32,
    ) {
        let variant = (self.random() * 16.0) as u8;
        let shape = [
            self.random() * std::f32::consts::TAU,
            0.7 + self.random() * 0.6,
        ];
        let Some(m) = self.models.get(&e.model) else {
            return;
        };
        let mut patches = Vec::new();
        for t in &m.triangles {
            let tri = t.verts.map(|i| i as usize);
            let v = tri.map(|i| positions[i]);
            let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize();
            if normal.dot(n).abs() < 0.3 || v.iter().all(|v| (*v - p).dot(n).abs() > size * 0.5) {
                continue;
            }
            let points = mesh::project_shaped(&v, p, n, size, shape);
            if points.len() >= 3 {
                patches.push(mesh::Patch {
                    tri,
                    side: normal.dot(n).signum(),
                    points,
                });
            }
        }
        if !patches.is_empty() {
            self.marks.push_back(Mark {
                born: time,
                surface: 0,
                normal: n,
                variant,
                owner: Some((e.key, e.model)),
                patches,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        set: Settings,
        cl: &qclient::ClientState,
        lv: &crate::assets::Level,
        fs: &qcore::fs::Fs,
        r: &mut qrender::Renderer,
        ents: &[RenderEntity],
        dynverts: &[u32],
        view: Vec3,
        debris: &crate::debris::Debris,
        out: &mut Geometry,
    ) {
        if !set.enabled {
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
        // Cache only visible keyed MDLs; brush models and effects are excluded.
        for e in ents.iter().filter(|e| e.key != 0) {
            if let Some(server) = cl.entities.get(e.key as usize) {
                self.model_ids.insert(server.model, e.model);
            }
            if self.models.contains_key(&e.model) {
                continue;
            }
            if let Some(name) = cl
                .entities
                .get(e.key as usize)
                .and_then(|e| cl.model_name(e.model))
                && let Some(data) = fs.open(name)
                && let Ok(m) = Mdl::parse(&data)
            {
                if name
                    .rsplit('/')
                    .next()
                    .is_some_and(|n| n.starts_with("h_") || n.contains("head"))
                {
                    self.heads.insert(e.model);
                }
                self.models.insert(e.model, m);
            }
        }
        let impacts = !cl.blood_hits.is_empty() || !cl.shot_impacts.is_empty();
        let poses: HashMap<u32, (&RenderEntity, Vec<Vec3>)> = ents
            .iter()
            .filter_map(|e| {
                if e.key == 0 || e.viewmodel {
                    return None;
                }
                let m = self.models.get(&e.model)?;
                if !impacts
                    && !(set.gibs && (m.flags & (4 | 32) != 0 || self.heads.contains(&e.model)))
                    && !self.marks.iter().any(|m| m.owner == Some((e.key, e.model)))
                {
                    return None;
                }
                Some((e.key, (e, mesh::positions(m, e, dynverts, cl.time))))
            })
            .collect();
        self.impacts(set, cl, &world, &poses, view, debris);
        self.step_droplets(&world, dt, cl.time);
        if set.gibs {
            let mut sources: Vec<_> = debris.blood_sources().collect();
            sources.extend(
                poses
                    .iter()
                    .filter(|(_, (e, _))| {
                        self.models[&e.model].flags & (4 | 32) != 0 || self.heads.contains(&e.model)
                    })
                    .map(|(&key, (e, _))| (key, Vec3::from(e.origin))),
            );
            let mut next = HashMap::new();
            for (key, p) in sources {
                let prev = self.trails.get(&key).copied().unwrap_or(p);
                if (p - prev).length_sq() >= 9.0 && (p - prev).length_sq() < 256.0 * 256.0 {
                    let n = ((p - prev).length() / 4.0).ceil().min(16.0) as usize;
                    for i in 1..=n {
                        self.splat(
                            &world,
                            prev.lerp(p, i as f32 / n as f32),
                            Vec3::new(0.0, 0.0, -0.05),
                            cl.time,
                            4.0 * set.size,
                        );
                    }
                    next.insert(key, p);
                } else {
                    next.insert(key, prev);
                }
            }
            self.trails = next;
        } else {
            self.trails.clear();
        }
        self.marks.retain(|m| {
            cl.time - m.born < set.lifetime
                && m.owner.is_none_or(|(key, model)| {
                    cl.entities.get(key as usize).is_some_and(|e| {
                        cl.model_name(e.model).is_some()
                            && lv
                                .models
                                .get(e.model)
                                .copied()
                                .flatten()
                                .is_some_and(|m| m.id == model)
                            && e.msgtime == cl.mtime[0]
                    })
                })
        });
        while self.marks.len() > set.limit {
            self.marks.pop_front();
        }
        if self.pic.is_none() {
            self.pic = r
                .register_pic_rgba_linear(
                    appearance::WIDTH as u32,
                    appearance::WIDTH as u32,
                    &appearance::texture(),
                )
                .ok();
        }
        if let Some(pic) = self.pic {
            self.draw(set, cl.time, &world, &poses, view, pic, out);
        }
        self.cm = Some(cm);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        set: Settings,
        time: f64,
        world: &dyn World,
        poses: &HashMap<u32, (&RenderEntity, Vec<Vec3>)>,
        view: Vec3,
        pic: PicId,
        out: &mut Geometry,
    ) {
        let first = out.1.len() as u32;
        for m in &self.marks {
            if m.owner.is_some() && !set.wounds {
                continue;
            }
            let fade =
                ((set.lifetime - (time - m.born)) / (set.lifetime * 0.15).min(5.0)).clamp(0.0, 1.0);
            let pose = m
                .owner
                .and_then(|(k, id)| poses.get(&k).filter(|(e, _)| e.model == id));
            if m.owner.is_some() && pose.is_none() {
                continue;
            }
            let origin = if m.surface == 0 {
                Vec3::ZERO
            } else {
                let Some(o) = world.entity_origin(m.surface) else {
                    continue;
                };
                o
            };
            for patch in &m.patches {
                let tri = pose.map(|(_, v)| patch.tri.map(|i| v[i]));
                let normal = if let Some(v) = tri {
                    (v[1] - v[0]).cross(v[2] - v[0]).normalize() * patch.side
                } else {
                    m.normal
                };
                let base = out.0.len() as u32;
                for p in &patch.points {
                    let pos = tri.map_or(p.pos + origin, |v| {
                        v[0] * p.bary.x + v[1] * p.bary.y + v[2] * p.bary.z
                    });
                    out.0.push(FteVertex {
                        pos: (pos + normal * 0.12).to_array(),
                        uv: appearance::uv(p.uv, m.variant),
                        color: [
                            38,
                            2,
                            1,
                            (fade * 220.0 * pose.map_or(1.0, |(e, _)| e.alpha) as f64) as u8,
                        ],
                    });
                }
                for i in 1..patch.points.len() - 1 {
                    out.1
                        .extend_from_slice(&[base, base + i as u32, base + i as u32 + 1]);
                }
            }
        }
        self.draw_droplets(view, out);
        let count = out.1.len() as u32 - first;
        if count > 0 {
            out.2.push(FteBatch {
                pic,
                blend: FteBlend::Blend,
                lines: false,
                soft: false,
                first_index: first,
                num_indices: count,
            });
        }
    }
}
