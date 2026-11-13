//! FTE scripted particles in the client: effect loading (`r_particledesc`),
//! the BSP-backed particle world, running queued client effects, and
//! building renderer geometry. The simulation itself is `qpart`.
use std::collections::HashMap;

use qclient::{ClientState, FteNames, FteRequest};
use qcore::math::Vec3;
use qformats::bsp::{Bsp, TextureKind};
use qpart::geom::{self, Geometry, TexRef, ViewInfo};
use qpart::{FrameInput, Registry, System, Trace, TrailHandle, World};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, Renderer};
use qworld::hull::CollisionModel;
pub(crate) mod world;
use world::*;
mod model_fx;
mod sky;
use sky::*;

impl World for BspWorld<'_> {
    /// CL_TraceLine: the world first, then brush entities overlapping the
    /// remaining segment.
    fn trace_line(&self, start: Vec3, end: Vec3) -> Trace {
        let tr = self.cm.hull(0, 0).trace(start, end);
        let mut best = Trace { frac: 1.0, endpos: end, normal: Vec3::new(0.0, 0.0, 1.0), entity: 0 };
        if tr.fraction < 1.0 {
            best = Trace { frac: tr.fraction, endpos: tr.endpos, normal: tr.plane_normal, entity: 0 };
            if best.frac <= 0.0 {
                return best;
            }
        }
        for be in self.ents {
            let (lo, hi) = (start.min(best.endpos) - Vec3::splat(1.0), start.max(best.endpos) + Vec3::splat(1.0));
            if (0..3).any(|k| be.mins[k] > hi[k] || be.maxs[k] < lo[k]) {
                continue;
            }
            let tr = self.cm.hull(be.submodel, 0).trace(be.local(start), be.local(end));
            if tr.fraction < best.frac {
                let endpos = be.dir_to_world(tr.endpos) + be.origin;
                best = Trace { frac: tr.fraction, endpos, normal: be.dir_to_world(tr.plane_normal), entity: be.entity };
                if best.frac <= 0.0 {
                    break;
                }
            }
        }
        best
    }

    fn entity_origin(&self, entity: i32) -> Option<Vec3> {
        // Rotated brush decals aren't supported (vkQuake drops them too).
        self.ents.iter().find(|b| b.entity == entity && b.axis.is_none()).map(|b| b.origin)
    }

    fn point_contents(&self, p: Vec3) -> u32 {
        fte_contents(self.cm.point_contents(p))
    }

    fn decal_polys(&self, entity: i32, center: Vec3, radius: f32, normal: Vec3, noperp: bool, out: &mut dyn FnMut(&[Vec3])) {
        let sub = if entity == 0 {
            0
        } else {
            match self.ents.iter().find(|b| b.entity == entity && b.axis.is_none()) {
                Some(b) => b.submodel,
                None => return,
            }
        };
        let Some(m) = self.bsp.models.get(sub) else { return };
        let mut stack = vec![m.headnode[0]];
        let mut poly = Vec::with_capacity(32);
        while let Some(n) = stack.pop() {
            if n < 0 {
                continue;
            }
            let Some(node) = self.bsp.nodes.get(n as usize) else { continue };
            let Some(pl) = self.bsp.planes.get(node.plane as usize) else { continue };
            let pn = Vec3::from(pl.normal);
            let dist = center.dot(pn) - pl.dist;
            if dist > radius {
                stack.push(node.children[0]);
                continue;
            }
            if dist < -radius {
                stack.push(node.children[1]);
                continue;
            }
            for fi in node.first_face..node.first_face + node.num_faces {
                let Some(f) = self.bsp.faces.get(fi as usize) else { continue };
                let tex = self
                    .bsp
                    .texinfo
                    .get(f.texinfo as usize)
                    .and_then(|ti| self.bsp.textures.get(ti.miptex.max(0) as usize))
                    .and_then(|t| t.as_ref());
                if tex.is_some_and(|t| matches!(t.kind, TextureKind::Sky | TextureKind::Liquid)) {
                    continue;
                }
                if noperp {
                    let fp = self.bsp.planes.get(f.plane as usize).map_or(Vec3::ZERO, |p| Vec3::from(p.normal));
                    let fnorm = if f.side != 0 { fp * -1.0 } else { fp };
                    if fnorm.dot(normal) > -0.5 {
                        continue;
                    }
                }
                poly.clear();
                for e in f.first_edge..f.first_edge + f.num_edges {
                    let Some(&se) = self.bsp.surfedges.get(e as usize) else { break };
                    let v = if se >= 0 {
                        self.bsp.edges.get(se as usize).map(|e| e[0])
                    } else {
                        self.bsp.edges.get((-se) as usize).map(|e| e[1])
                    };
                    if let Some(p) = v.and_then(|v| self.bsp.vertexes.get(v as usize)) {
                        poly.push(Vec3::from(*p));
                    }
                }
                if poly.len() >= 3 {
                    out(&poly);
                }
            }
            stack.push(node.children[0]);
            stack.push(node.children[1]);
        }
    }
}

/// Files from the game filesystem as a `qpart::FileSource`.
fn files(fs: &qcore::fs::Fs) -> impl FnMut(&str) -> Option<Vec<u8>> + '_ {
    |p: &str| fs.open(p)
}

fn to_blend(b: qpart::BlendMode) -> FteBlend {
    use qpart::BlendMode as B;
    match b {
        B::Blend => FteBlend::Blend,
        B::BlendColour => FteBlend::BlendColour,
        B::AddA => FteBlend::AddA,
        B::AddC => FteBlend::AddC,
        B::Subtract => FteBlend::Subtract,
        B::InvModA => FteBlend::InvModA,
        B::InvModC => FteBlend::InvModC,
        B::Premul => FteBlend::Premul,
    }
}

/// Client-side FTE particle state.
pub struct Fte {
    pub reg: Registry,
    pub sys: System,
    /// Uploaded textures, by `Registry::textures` index.
    pics: Vec<Option<PicId>>,
    /// Trail state per entity number.
    trails: HashMap<i32, Option<TrailHandle>>,
    /// Collision model of the current map.
    cm: Option<(String, std::rc::Rc<CollisionModel>)>,
    /// r_particledesc and map the effects were loaded for.
    loaded_for: Option<(String, String, String)>,
    palette: Option<Vec<u8>>,
    /// Client requests processed (r_partinfo).
    requests: u64,
    /// `r_effect` / `r_trail` effects by model name (PScript_UpdateModelEffects).
    model_fx: HashMap<String, qpart::ModelEffects>,
    /// Per entity: previous origin (model trails) and emitter state.
    ent_fx: HashMap<i32, ([f32; 3], Option<TrailHandle>)>,
    /// Sky triangles need rebuilding (effects or map changed).
    sky_dirty: bool,
    /// Submodels (0 = world) that have rain sky triangles.
    sky_models: std::collections::HashSet<usize>,
}

impl Fte {
    pub fn new() -> Self {
        Fte {
            reg: Registry::new(),
            sys: System::default(),
            pics: Vec::new(),
            trails: HashMap::new(),
            cm: None,
            loaded_for: None,
            palette: None,
            requests: 0,
            model_fx: HashMap::new(),
            ent_fx: HashMap::new(),
            sky_dirty: true,
            sky_models: Default::default(),
        }
    }

    /// Reloads effects when `r_particledesc` or the map changed, then
    /// republishes the scripted effect names to the client.
    #[allow(clippy::too_many_arguments)]
    pub fn sync(
        &mut self,
        desc: &str,
        map: &str,
        opts: qpart::RenderOptions,
        settings: qpart::Settings,
        fs: &qcore::fs::Fs,
        cl: &mut ClientState,
        print: &mut dyn FnMut(&str),
    ) {
        // Pool sizes take effect on the next reload (PScript_Startup).
        let resize = settings.max_particles != self.sys.settings.max_particles
            || settings.max_decals != self.sys.settings.max_decals;
        self.sys.settings = settings;
        // FinishParticleType applies the spark/beam cvars, so they reload.
        let key = (desc.to_string(), map.to_string(), format!("{opts:?}"));
        if self.loaded_for.as_ref() == Some(&key) && !resize {
            return;
        }
        self.reg.opts = opts;
        if self.palette.is_none() {
            self.palette = fs.open("gfx/palette.lmp");
        }
        if let Some(p) = &self.palette {
            self.reg.set_palette(p);
        }
        let mut f = files(fs);
        self.reg.set_desc(desc, (!map.is_empty()).then_some(map), &mut f);
        self.sys.clear(cl.time);
        self.trails.clear();
        self.sys.refresh_pe(&mut self.reg, &mut f);
        for m in self.reg.messages.drain(..) {
            print(&format!("{m}\n"));
        }
        self.publish(cl);
        self.model_fx.clear();
        self.ent_fx.clear();
        self.sky_dirty = true;
        self.loaded_for = Some(key);
    }

    /// `r_partinfo`: loaded configs, types and live particles per type.
    pub fn info(&self, cl: &ClientState) -> String {
        let mut o = format!(
            "configs: {:?}\ntypes: {} ({} loaded), textures: {}\nprecached: {}\nparticles: {}, beams: {}, decals: {}\n",
            self.reg.loaded_configs,
            self.reg.types.len(),
            self.reg.types.iter().filter(|t| t.is_loaded()).count(),
            self.reg.textures.len(),
            cl.particle_precache.iter().filter(|n| !n.is_empty()).count(),
            self.sys.num_particles(),
            self.sys.num_beams(),
            self.sys.num_decals(),
        );
        let pre: Vec<_> = cl.particle_precache.iter().filter(|n| !n.is_empty()).take(8).collect();
        o.push_str(&format!("first precaches: {pre:?}\nrequests: {}\n", self.requests));
        if !self.sky_models.is_empty() {
            let mut models: Vec<_> = self.sky_models.iter().copied().collect();
            models.sort_unstable();
            let emitting: Vec<_> = self.rain_emitters(cl).iter().map(|e| e.model).collect();
            o.push_str(&format!("rain submodels: {models:?}, emitting: {emitting:?}\n"));
        }
        for &t in self.sys.run_list() {
            let n = self.sys.particles_of(t).count();
            let ty = &self.reg.types[t];
            o.push_str(&format!("  {}.{}: {n}\n", ty.config, ty.name));
        }
        o
    }

    /// Tells the client which effect names are scripted.
    fn publish(&self, cl: &mut ClientState) {
        let names = self.reg.types.iter().filter(|t| t.is_loaded()).flat_map(|t| [t.name.clone(), format!("{}.{}", t.config, t.name)]);
        cl.fte_names = FteNames::new(names);
    }

    /// Runs the client's queued effects, steps the simulation and returns
    /// renderer geometry.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cl: &mut ClientState,
        bsp: Option<&Bsp>,
        fs: &qcore::fs::Fs,
        r: &mut Renderer,
        snd: Option<&mut qsound::SoundSystem>,
        view: &qrender::View,
        mapname: &str,
    ) -> (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>) {
        let queue = std::mem::take(&mut cl.fte_queue);
        self.requests += queue.len() as u64;
        if self.reg.types.is_empty() && queue.is_empty() && self.sys.run_list().is_empty() {
            return Default::default();
        }
        let Some(bsp) = bsp else { return Default::default() };
        if self.cm.as_ref().is_none_or(|(n, _)| n != mapname) {
            self.cm = Some((mapname.to_string(), std::rc::Rc::new(CollisionModel::from_bsp(bsp))));
            self.trails.clear();
            self.ent_fx.clear();
            self.sky_dirty = true;
        }
        let Some(cm) = self.cm.as_ref().map(|c| c.1.clone()) else { return Default::default() };
        let ents = brush_ents(cl, &cm);
        let world = BspWorld { bsp, cm: &cm, ents: &ents };
        let mut f = files(fs);
        // CL_ForceProtocolParticles: a server using particleeffectnum gets
        // effectinfo.txt loaded implicitly (weakly), for un-namespaced names.
        if !cl.particle_precache.is_empty() && !self.reg.loaded_configs.iter().any(|c| c == "effectinfo") {
            self.reg.load_set("effectinfo", true, false, &mut f);
        }
        let ft = (cl.time - cl.oldtime).max(0.0) as f32;
        for q in queue {
            match q {
                FteRequest::Effect { name, org, dir, count } => {
                    if let Some(t) = self.reg.find(&name, &mut f) {
                        let d = dir.map(Vec3::from);
                        self.sys.run_effect(&self.reg, &world, org.into(), d, count, t, None);
                    }
                }
                FteRequest::Trail { name, start, end, entity, interval } => {
                    if let Some(t) = self.reg.find(&name, &mut f) {
                        let slot = self.trails.entry(entity).or_insert(None);
                        let dt = interval.unwrap_or(ft);
                        self.sys.trail(&self.reg, &world, start.into(), end.into(), t, dt, entity, Some(slot));
                    }
                }
                FteRequest::Weather { min, max, dir, count, colour, snow } => {
                    let name = if snow { "snow" } else { "rain" };
                    let (lo, hi, d) = (min.into(), max.into(), dir.into());
                    self.sys.run_weather(&mut self.reg, &mut f, &world, lo, hi, d, count, colour, name);
                }
            }
        }
        // Effects may have loaded new configs (namespaces on demand, e.g.
        // `effectinfo.foo`); scripted names then replace classic effects.
        self.reg.resolve_textures(&mut f);
        if self.reg.dirty {
            self.reg.dirty = false;
            self.sys.refresh_pe(&mut self.reg, &mut f);
            self.publish(cl);
            self.model_fx.clear();
            self.sky_dirty = true;
        }
        if self.sky_dirty {
            self.sky_dirty = false;
            self.sys.clear_sky_tris();
            self.sky_models.clear();
            for (sub, tris) in sky_tris(bsp, &mut self.reg, &mut f) {
                self.sky_models.insert(sub);
                self.sys.set_sky_tris(sub, tris);
            }
        }
        self.model_effects(cl, &world, &mut f, ft);
        upload_textures(&self.reg, &mut self.pics, r, &mut f);

        let (fwd, right, up) = qcore::math::angle_vectors(view.angles.into());
        let vi = ViewInfo { origin: view.origin.into(), forward: fwd, right, up };
        let eo = |e: i32| world.entity_origin(e);
        let mut g = Geometry::new(&self.reg, vi, self.sys.particletime, &eo);
        let rain = self.rain_emitters(cl);
        let input = FrameInput { cl_time: cl.time, vieworg: view.origin.into(), rain };
        self.sys.frame(&self.reg, &world, &input, &mut g);

        for d in self.sys.dlights.drain(..) {
            let dl = cl.alloc_dlight(d.key);
            dl.origin = d.org.to_array();
            dl.radius = d.radius;
            dl.die = d.die;
            dl.decay = d.decay;
            dl.color = d.rgb;
        }
        let sounds = std::mem::take(&mut self.sys.sounds);
        if let Some(snd) = snd {
            for s in sounds {
                if let Some(sfx) = snd.precache(&s.name) {
                    snd.event(qsound::SoundEvent::Start {
                        entity: -1,
                        channel: 0,
                        sfx,
                        origin: s.org.to_array(),
                        volume: s.vol,
                        attenuation: s.atten,
                    });
                }
            }
        }

        let verts = g.vertices.iter().map(|v| FteVertex { pos: v.pos, uv: v.uv, color: v.color }).collect();
        let batches = g
            .ordered_batches()
            .into_iter()
            .filter_map(|b| {
                Some(FteBatch {
                    pic: self.pics.get(b.tex).copied().flatten()?,
                    blend: to_blend(b.blend),
                    lines: b.lines,
                    first_index: b.first_index,
                    num_indices: b.num_indices,
                    soft: b.soft,
                })
            })
            .collect();
        (verts, g.indices, batches)
    }
}

impl Fte {
}

#[cfg(test)]
mod tests;
