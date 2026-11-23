//! Bounded, client-only weapon marks and physical nails.
use crate::blood::{Geometry, mesh};
use crate::fte::world::{BspWorld, BrushEnt, cached_world};
use qcore::math::Vec3;
use qpart::World;
use qrender::{FteBatch, FteBlend, FteVertex, ModelId, PicId, RenderEntity};
use std::collections::{HashMap, VecDeque};
mod nails;
pub(crate) mod surfaces;
mod teleporters;
#[cfg(test)]
mod tests;

/// `r_particle_quality`: surface hit-particle budgets. Mark lifetime and
/// count come only from `r_impact_lifetime` / `r_impact_limit`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
}

impl Quality {
    pub fn from_cvar(v: f32) -> Self {
        match v.round() as i32 {
            ..=0 => Self::Low,
            1 => Self::Medium,
            _ => Self::High,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Tier {
    pub lifetime: f64,
    pub limit: usize,
    pub fx_level: u8,
    pub fx_mult: f32,
    pub max_parts: usize,
}

/// Impact effects need the Enhanced FX master and Enhanced Particles.
pub(crate) fn impacts_on(cv: &qcore::cvar::Cvars) -> bool {
    cv.value_of("r_enhancedfx") != 0.0 && cv.value_of("r_enhancedparticles") != 0.0
}

fn mark_settings(cv: &qcore::cvar::Cvars) -> [bool; 3] {
    ["r_impact_bullets", "r_impact_nails", "r_impact_scorches"]
        .map(|n| cv.value_of(n) != 0.0 && impacts_on(cv))
}

pub(crate) fn tier(cv: &qcore::cvar::Cvars) -> Tier {
    let quality = Quality::from_cvar(cv.value_of("r_particle_quality"));
    let lifetime = cv.value_of("r_impact_lifetime").clamp(5.0, 600.0) as f64;
    let limit = cv.value_of("r_impact_limit").clamp(32.0, 2048.0) as usize;
    let level = if impacts_on(cv) { cv.value_of("r_impact_fx").clamp(0.0, 4.0) as u8 } else { 0 };
    match quality {
        Quality::Low => Tier {
            lifetime,
            limit,
            fx_level: level.min(2),
            fx_mult: 0.5,
            max_parts: 1024,
        },
        Quality::Medium => Tier {
            lifetime,
            limit,
            fx_level: level.min(3),
            fx_mult: 1.0,
            max_parts: 2500,
        },
        Quality::High => Tier {
            lifetime,
            limit,
            fx_level: level,
            fx_mult: 1.0,
            max_parts: 6000,
        },
    }
}

struct Mark {
    kind: u8,
    born: f64,
    entity: i32,
    normal: Vec3,
    center: Vec3,
    radius: f32,
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
    models: HashMap<ModelId, Option<mesh::CachedMdl>>,
    flights: HashMap<usize, Flight>,
    flights_scratch: HashMap<usize, Flight>,
    intercepted: std::collections::HashSet<usize>,
    teleporters: teleporters::TeleportImpacts,
    pic: Option<PicId>,
    time: f64,
}
impl Impacts {
    pub fn is_active(&self) -> bool {
        !self.marks.is_empty() || !self.nails.is_empty() || !self.flights.is_empty() || !self.intercepted.is_empty()
    }

    pub fn clear(&mut self) {
        let pic = self.pic;
        *self = Self::default();
        self.pic = pic;
    }

    pub fn teleport_ripples(&self) -> &[[f32; 4]] {
        self.teleporters.ripples()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cv: &qcore::cvar::Cvars,
        cl: &qclient::ClientState,
        lv: &crate::assets::Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        fs: &qcore::fs::Fs,
        r: &mut qrender::Renderer,
        ents: &mut Vec<RenderEntity>,
        dynverts: &[u32],
        view: Vec3,
        debris: &crate::debris::Debris,
        frustum: Option<&qrender::ViewFrustum>,
        out: &mut Geometry,
    ) {
        let enabled = mark_settings(cv);
        if !enabled.iter().any(|v| *v) {
            let teleporters = std::mem::take(&mut self.teleporters);
            self.clear();
            self.teleporters = teleporters;
            self.time = cl.time;
            return;
        }
        let dt = (cl.time - self.time).clamp(0.0, 0.1) as f32;
        self.time = cl.time;
        let Some(world) = cached_world(cl, lv, brushes) else { return };
        if enabled[1] {
            for e in ents.iter().filter(|e| e.key != 0 && !e.viewmodel) {
                if self.models.contains_key(&e.model) {
                    continue;
                }
                if let Some(name) = cl
                    .entities
                    .get(e.key as usize)
                    .and_then(|e| cl.model_name(e.model))
                {
                    let cached = if mesh::is_mdl_path(name) {
                        fs.open(name)
                            .and_then(|data| qformats::Mdl::parse(&data).ok())
                            .map(|m| mesh::CachedMdl::from_mdl(m, false))
                    } else {
                        None
                    };
                    self.models.insert(e.model, cached);
                }
            }
        }
        let mut poses: Poses<'_> = HashMap::new();
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
                        ents,
                        dynverts,
                        &mut poses,
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
            let mut flights = std::mem::take(&mut self.flights_scratch);
            flights.clear();
            let mut prev_flights = std::mem::take(&mut self.flights);
            for (key, e) in cl.active_entities() {
                if e.effects & qclient::EF_QSP_PELLET != 0
                    || !cl
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
                if let Some(prev) = prev_flights.get(&key) {
                    let a = prev.pos;
                    if (f.pos - a).length_sq() > 0.01
                        && self.nail(
                            &world,
                            ents,
                            dynverts,
                            &mut poses,
                            a,
                            f.pos,
                            cl.time,
                            false,
                        )
                    {
                        self.intercepted.insert(key);
                    }
                }
            }
            for (key, f) in prev_flights.drain() {
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
                    ents,
                    dynverts,
                    &mut poses,
                    f.pos - f.vel.normalize() * 8.0,
                    end,
                    cl.time,
                    false,
                );
            }
            self.intercepted.retain(|key| flights.contains_key(key));
            self.flights_scratch = prev_flights;
            self.flights = flights;
        } else {
            self.nails.clear();
            self.flights.clear();
            self.intercepted.clear();
        }
        let tier = tier(cv);
        let lifetime = tier.lifetime;
        let limit = tier.limit;
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
        if self
            .nails
            .iter()
            .any(|n| matches!(n.anchor, Some(Anchor::Mesh { .. })))
        {
            for n in &self.nails {
                if let Some(Anchor::Mesh { key, .. }) = n.anchor
                    && !poses.contains_key(&key)
                    && let Some(e) = ents.iter().rev().find(|e| e.key == key && !e.viewmodel)
                    && let Some(Some(cm)) = self.models.get(&e.model)
                    && cm.has_death_pose
                    && frustum.is_none_or(|f| f.contains_sphere(e.origin, cm.entity_radius(e)))
                {
                    poses.insert(key, (e, mesh::positions(&cm.mdl, e, dynverts, cl.time)));
                }
            }
        }
        self.step_nails(&world, &poses, dt, cv.value_of("sv_gravity").max(1.0));
        if self.pic.is_none() && (!self.marks.is_empty() || !self.nails.is_empty()) {
            self.pic = r
                .register_pic_rgba_linear(96, 32, &surfaces::texture())
                .ok();
        }
        if let Some(pic) = self.pic {
            self.draw(&world, &poses, frustum, pic, cl.time, lifetime, out);
        }
        drop(poses);
        ents.retain(|e| !self.intercepted.contains(&(e.key as usize)));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        world: &dyn World,
        poses: &Poses<'_>,
        frustum: Option<&qrender::ViewFrustum>,
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
            if frustum.is_some_and(|f| !f.contains_sphere((m.center + origin).to_array(), m.radius)) {
                continue;
            }
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
            if frustum.is_some_and(|f| !f.contains_sphere(tip.to_array(), 7.0)) {
                continue;
            }
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
