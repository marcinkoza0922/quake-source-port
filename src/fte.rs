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
        // currents count as water (cont_qtof)
        _ if c <= -9 => FTECONTENTS_WATER,
        _ => FTECONTENTS_EMPTY,
    }
}

impl World for BspWorld<'_> {
    fn trace_line(&self, start: Vec3, end: Vec3) -> Trace {
        let tr = self.cm.hull(0, 0).trace(start, end);
        Trace { frac: tr.fraction, endpos: tr.endpos, normal: tr.plane_normal, entity: 0 }
    }

    fn point_contents(&self, p: Vec3) -> u32 {
        fte_contents(self.cm.point_contents(p))
    }

    fn decal_polys(&self, entity: i32, center: Vec3, radius: f32, normal: Vec3, noperp: bool, out: &mut dyn FnMut(&[Vec3])) {
        if entity != 0 {
            return;
        }
        let Some(m) = self.bsp.models.first() else { return };
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
    cm: Option<(String, CollisionModel)>,
    /// r_particledesc and map the effects were loaded for.
    loaded_for: Option<(String, String)>,
    palette: Option<Vec<u8>>,
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
        }
    }

    /// Reloads effects when `r_particledesc` or the map changed, then
    /// republishes the scripted effect names to the client.
    pub fn sync(&mut self, desc: &str, map: &str, fs: &qcore::fs::Fs, cl: &mut ClientState, print: &mut dyn FnMut(&str)) {
        let key = (desc.to_string(), map.to_string());
        if self.loaded_for.as_ref() == Some(&key) {
            return;
        }
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
        self.loaded_for = Some(key);
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
        if self.reg.types.is_empty() && queue.is_empty() && self.sys.run_list().is_empty() {
            return Default::default();
        }
        let Some(bsp) = bsp else { return Default::default() };
        if self.cm.as_ref().is_none_or(|(n, _)| n != mapname) {
            self.cm = Some((mapname.to_string(), CollisionModel::from_bsp(bsp)));
            self.trails.clear();
        }
        let cm = &self.cm.as_ref().unwrap().1;
        let world = BspWorld { bsp, cm };
        let mut f = files(fs);
        let ft = (cl.time - cl.oldtime).max(0.0) as f32;
        for q in queue {
            match q {
                FteRequest::Effect { name, org, dir, count } => {
                    if let Some(t) = self.reg.find(&name, &mut f) {
                        let d = dir.map(Vec3::from);
                        self.sys.run_effect(&self.reg, &world, org.into(), d, count, t, None);
                    }
                }
                FteRequest::Trail { name, start, end, entity } => {
                    if let Some(t) = self.reg.find(&name, &mut f) {
                        let slot = self.trails.entry(entity).or_insert(None);
                        self.sys.trail(&self.reg, &world, start.into(), end.into(), t, ft, entity, Some(slot));
                    }
                }
            }
        }
        // Effects may have loaded new types (namespaces on demand).
        self.reg.resolve_textures(&mut f);
        upload_textures(&self.reg, &mut self.pics, r, &mut f);

        let (fwd, right, up) = qcore::math::angle_vectors(view.angles.into());
        let vi = ViewInfo { origin: view.origin.into(), forward: fwd, right, up };
        let eo = |_: i32| None;
        let mut g = Geometry::new(&self.reg, vi, self.sys.particletime, &eo);
        let input = FrameInput { cl_time: cl.time, vieworg: view.origin.into(), rain: Vec::new() };
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
