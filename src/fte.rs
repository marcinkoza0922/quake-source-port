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

/// Particle world over the client's map: hull-0 traces, contents, and the
/// render faces for decal clipping.
struct BspWorld<'a> {
    bsp: &'a Bsp,
    cm: &'a CollisionModel,
    /// Inline brush entities (doors, lifts) for traces and decals.
    ents: &'a [BrushEnt],
}

/// A visible inline brush model entity (CL_PrepareTraceLineEntities).
#[derive(Clone, Copy, Debug)]
struct BrushEnt {
    entity: i32,
    submodel: usize,
    origin: Vec3,
    /// forward, right, up when rotated.
    axis: Option<[Vec3; 3]>,
    mins: Vec3,
    maxs: Vec3,
}

impl BrushEnt {
    /// Entity-space point (the renderer's brush rotation, right negated).
    fn local(&self, p: Vec3) -> Vec3 {
        let d = p - self.origin;
        match self.axis {
            Some(a) => Vec3::new(d.dot(a[0]), -d.dot(a[1]), d.dot(a[2])),
            None => d,
        }
    }
    fn dir_to_world(&self, v: Vec3) -> Vec3 {
        match self.axis {
            Some(a) => a[0] * v.x - a[1] * v.y + a[2] * v.z,
            None => v,
        }
    }
}

/// Brush entities of the client's frame.
fn brush_ents(cl: &ClientState, cm: &CollisionModel) -> Vec<BrushEnt> {
    let mut out = Vec::new();
    for (i, e) in cl.active_entities() {
        let Some(sub) = cl.model_name(e.model).and_then(|n| n.strip_prefix('*')).and_then(|n| n.parse::<usize>().ok()) else {
            continue;
        };
        let Some(m) = cm.submodels.get(sub) else { continue };
        let origin = Vec3::from(e.origin);
        let rotated = e.angles != [0.0; 3];
        let (mins, maxs) = if rotated {
            let r = m.mins.length().max(m.maxs.length());
            (origin - Vec3::splat(r), origin + Vec3::splat(r))
        } else {
            (origin + m.mins, origin + m.maxs)
        };
        let axis = rotated.then(|| {
            let (f, r, u) = qcore::math::angle_vectors(Vec3::from(e.angles));
            [f, r, u]
        });
        out.push(BrushEnt { entity: i as i32, submodel: sub, origin, axis, mins, maxs });
    }
    out
}

fn fte_contents(c: i32) -> u32 {
    use qpart::*;
    match c {
        -1 => FTECONTENTS_EMPTY,
        -2 => FTECONTENTS_SOLID,
        -3 => FTECONTENTS_WATER,
        -4 => FTECONTENTS_SLIME,
        -5 => FTECONTENTS_LAVA,
        -6 => FTECONTENTS_SKY,
        // anything else (currents, clip) is assumed water (cont_qtof)
        _ => FTECONTENTS_WATER,
    }
}

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
            let tris = sky_tris(bsp, &mut self.reg, &mut f);
            if !tris.is_empty() {
                self.sys.set_sky_tris(1, tris);
            }
        }
        self.model_effects(cl, &world, &mut f, ft);
        upload_textures(&self.reg, &mut self.pics, r, &mut f);

        let (fwd, right, up) = qcore::math::angle_vectors(view.angles.into());
        let vi = ViewInfo { origin: view.origin.into(), forward: fwd, right, up };
        let eo = |e: i32| world.entity_origin(e);
        let mut g = Geometry::new(&self.reg, vi, self.sys.particletime, &eo);
        // The world is model 1 at the origin (brush entities don't rain yet).
        let rain = vec![qpart::RainEmitter {
            model: 1,
            axis: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)],
            origin: Vec3::ZERO,
        }];
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
                })
            })
            .collect();
        (verts, g.indices, batches)
    }
}

impl Fte {
    fn model_fx_of(&mut self, name: &str, f: &mut dyn qpart::FileSource) -> qpart::ModelEffects {
        if let Some(m) = self.model_fx.get(name) {
            return *m;
        }
        let m = self.reg.model_effects(name, f);
        self.model_fx.insert(name.to_string(), m);
        m
    }

    /// Per model index: drawn by its `replace` emitter instead.
    pub fn replaced_models(&mut self, cl: &ClientState, fs: &qcore::fs::Fs) -> Vec<bool> {
        if self.reg.associated.iter().all(|a| a.flags & qpart::EMIT_REPLACE == 0) {
            return Vec::new();
        }
        let mut f = files(fs);
        (0..cl.model_precache.len())
            .map(|i| {
                cl.model_name(i).is_some_and(|n| {
                    let m = self.model_fx_of(n, &mut f);
                    m.emit.is_some() && m.flags & qpart::EMIT_REPLACE != 0
                })
            })
            .collect()
    }

    /// `r_effect` / `r_trail` effects of visible entities (CL_RelinkEntities).
    fn model_effects(&mut self, cl: &ClientState, world: &BspWorld<'_>, f: &mut dyn qpart::FileSource, ft: f32) {
        if self.reg.associated.is_empty() {
            return;
        }
        for &i in &cl.visedicts {
            let Some(e) = cl.entities.get(i) else { continue };
            let Some(name) = cl.model_name(e.model) else { continue };
            let me = self.model_fx_of(name, f);
            if me.trail.is_none() && me.emit.is_none() {
                continue;
            }
            let key = i as i32;
            let entry = self.ent_fx.entry(key).or_insert((e.origin, None));
            let oldorg = std::mem::replace(&mut entry.0, e.origin);
            if let Some(t) = me.trail {
                let slot = self.trails.entry(key).or_insert(None);
                self.sys.trail(&self.reg, world, oldorg.into(), e.origin.into(), t, ft, key, Some(slot));
            }
            if let Some(t) = me.emit {
                let (fwd, _, up) = qcore::math::angle_vectors(e.angles.into());
                let dir = if me.flags & qpart::EMIT_FORWARDS != 0 {
                    // the "stupid vanilla bug": alias models pitch the other way
                    if name.ends_with(".mdl") { Vec3::new(fwd.x, fwd.y, -fwd.z) } else { fwd }
                } else {
                    up * -1.0
                };
                let mut slot = self.ent_fx.get(&key).and_then(|x| x.1);
                self.sys.run_effect(&self.reg, world, e.origin.into(), Some(dir), ft, t, Some(&mut slot));
                if let Some(x) = self.ent_fx.get_mut(&key) {
                    x.1 = slot;
                }
            }
        }
    }
}

/// Sky-triangle rain emitters of the world (PScript_RecalculateSkyTris):
/// worldspawn `[_]texpart_<texture>` keys, else effects named `tex_<texture>`.
fn sky_tris(bsp: &Bsp, reg: &mut Registry, f: &mut dyn qpart::FileSource) -> Vec<qpart::SkyTri> {
    let mut remap: HashMap<usize, qpart::TypeId> = HashMap::new();
    let mut s = bsp.entities.as_str();
    if let Some(("{", rest)) = qcore::cmd::com_parse(s) {
        s = rest;
        while let Some((k, rest)) = qcore::cmd::com_parse(s) {
            if k == "}" {
                break;
            }
            let Some((v, rest)) = qcore::cmd::com_parse(rest) else { break };
            s = rest;
            let key = k.strip_prefix('_').unwrap_or(k).trim_end();
            if key.len() > 8 && key[..8].eq_ignore_ascii_case("texpart_") {
                for (t, tex) in bsp.textures.iter().enumerate() {
                    if tex.as_ref().is_some_and(|m| m.name.eq_ignore_ascii_case(&key[8..]))
                        && let Some(ty) = reg.find(v, f)
                    {
                        remap.insert(t, ty);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let Some(m) = bsp.models.first() else { return out };
    let mut by_tex: HashMap<usize, Option<qpart::TypeId>> = HashMap::new();
    for fi in m.first_face.max(0) as usize..(m.first_face + m.num_faces).max(0) as usize {
        let Some(face) = bsp.faces.get(fi) else { continue };
        let Some(ti) = bsp.texinfo.get(face.texinfo as usize) else { continue };
        let t = ti.miptex.max(0) as usize;
        let ty = *by_tex.entry(t).or_insert_with(|| {
            remap.get(&t).copied().or_else(|| {
                let name = bsp.textures.get(t).and_then(|x| x.as_ref())?.name.clone();
                reg.find(&format!("tex_{name}"), f)
            })
        });
        let Some(ty) = ty else { continue };
        let mut verts = Vec::new();
        for e in face.first_edge..face.first_edge + face.num_edges {
            let Some(&se) = bsp.surfedges.get(e as usize) else { break };
            let v = if se >= 0 { bsp.edges.get(se as usize).map(|e| e[0]) } else { bsp.edges.get((-se) as usize).map(|e| e[1]) };
            if let Some(p) = v.and_then(|v| bsp.vertexes.get(v as usize)) {
                verts.push(Vec3::from(*p));
            }
        }
        let pn = bsp.planes.get(face.plane as usize).map_or(Vec3::ZERO, |p| Vec3::from(p.normal));
        let normal = if face.side != 0 { pn * -1.0 } else { pn };
        out.extend(qpart::sky_tris_for_polygon(&verts, normal, ty));
    }
    out
}

/// Uploads textures added to the registry since the last call.
fn upload_textures(reg: &Registry, pics: &mut Vec<Option<PicId>>, r: &mut Renderer, f: &mut dyn qpart::FileSource) {
    while pics.len() < reg.textures.len() {
        let t = &reg.textures[pics.len()];
        let img = match t {
            TexRef::Builtin(b) => Some(geom::builtin_image(*b)),
            TexRef::File { path, premul, .. } => geom::load_image(f, path, reg.palette.as_deref(), *premul),
        };
        let nearest = matches!(t, TexRef::File { nearest: true, .. });
        let pic = img.and_then(|i| {
            if nearest {
                r.register_pic_rgba(i.width, i.height, &i.rgba).ok()
            } else {
                r.register_pic_rgba_linear(i.width, i.height, &i.rgba).ok()
            }
        });
        pics.push(pic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qworld::hull::{CNode, HPlane, SubModel};

    /// World: empty. Submodel 1: solid box x ±16, y ±4, z ±16.
    fn box_model() -> CollisionModel {
        let pl = |n: [f32; 3], d: f32| HPlane { normal: Vec3::from(n), dist: d, kind: 3 };
        let planes = vec![
            pl([1.0, 0.0, 0.0], 100_000.0),
            pl([1.0, 0.0, 0.0], 16.0),
            pl([-1.0, 0.0, 0.0], 16.0),
            pl([0.0, 1.0, 0.0], 4.0),
            pl([0.0, -1.0, 0.0], 4.0),
            pl([0.0, 0.0, 1.0], 16.0),
            pl([0.0, 0.0, -1.0], 16.0),
        ];
        // node 0: world (all empty); nodes 1..=6: front empty, back next, last back solid
        let mut hull0 = vec![CNode { plane: 0, children: [-1, -1] }];
        for i in 1..=6 {
            hull0.push(CNode { plane: i, children: [-1, if i == 6 { -2 } else { i as i32 + 1 }] });
        }
        let sub = |h: i32, mins: [f32; 3], maxs: [f32; 3]| SubModel {
            mins: Vec3::from(mins),
            maxs: Vec3::from(maxs),
            origin: Vec3::ZERO,
            headnode: [h, 0, 0, 0],
        };
        CollisionModel {
            planes,
            hull0,
            submodels: vec![sub(0, [-1e5; 3], [1e5; 3]), sub(1, [-16.0, -4.0, -16.0], [16.0, 4.0, 16.0])],
            ..Default::default()
        }
    }

    fn ent(yaw: f32) -> BrushEnt {
        let origin = Vec3::new(100.0, 0.0, 0.0);
        let axis = (yaw != 0.0).then(|| {
            let (f, r, u) = qcore::math::angle_vectors(Vec3::new(0.0, yaw, 0.0));
            [f, r, u]
        });
        BrushEnt { entity: 7, submodel: 1, origin, axis, mins: origin - Vec3::splat(17.0), maxs: origin + Vec3::splat(17.0) }
    }

    #[test]
    fn traces_hit_brush_entities() {
        let cm = box_model();
        let bsp = Bsp::default();
        for (yaw, hit_x) in [(0.0, 84.0), (90.0, 96.0)] {
            let ents = [ent(yaw)];
            let w = BspWorld { bsp: &bsp, cm: &cm, ents: &ents };
            let t = w.trace_line(Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0));
            assert_eq!(t.entity, 7, "yaw {yaw}");
            assert!((t.endpos.x - hit_x).abs() < 0.1, "yaw {yaw}: {:?}", t.endpos);
            assert!((t.normal - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-3, "yaw {yaw}: {:?}", t.normal);
            // Missing it entirely.
            let t = w.trace_line(Vec3::new(0.0, 50.0, 0.0), Vec3::new(200.0, 50.0, 0.0));
            assert_eq!((t.frac, t.entity), (1.0, 0));
        }
        let ents = [ent(0.0)];
        let w = BspWorld { bsp: &bsp, cm: &cm, ents: &ents };
        assert_eq!(w.entity_origin(7), Some(Vec3::new(100.0, 0.0, 0.0)));
        assert_eq!(fte_contents(-3), qpart::FTECONTENTS_WATER);
        assert_eq!(fte_contents(-2), qpart::FTECONTENTS_SOLID);
    }
}
