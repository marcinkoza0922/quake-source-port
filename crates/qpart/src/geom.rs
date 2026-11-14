//! Particle geometry: textures (P_LoadTexture) and vertex batches
//! (R_Emit*Particle, R_DrawParticleBeam, R_AddClippedDecal).
use crate::system::{BeamSeg, Decal, Particle, Sink};
use crate::types::*;
use crate::util::{pcos, psin};
use crate::{FileSource, Registry, TypeId};
use qcore::math::Vec3;
mod image;
pub use image::*;
mod shapes;

/// Generated fallback textures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinTex {
    /// 4×4 white (sparks, lines).
    White,
    /// Soft horizontal band (untextured beams).
    Beam,
    /// Corner gradient (untextured spark fans).
    PTri,
    /// Soft disc (`ball`, `glow`, textured sparks).
    Ball,
    /// Fuzzy blob (anything else).
    Fuzzy,
    /// Classic particle disc (`classicparticle`).
    Classic,
}

/// A texture a particle type draws with.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TexRef {
    /// Image file (path including extension), premultiplied / nearest.
    File { path: String, premul: bool, nearest: bool },
    Builtin(BuiltinTex),
}

const IMAGE_EXTS: &[&str] = &["png", "tga", "jpg", "pcx", "lmp"];

/// RGBA pixels for a texture.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Registry {
    /// P_LoadTexture for every loaded type without a texture yet. Types
    /// whose image is missing get a builtin and full-texture coords.
    pub fn resolve_textures(&mut self, files: &mut dyn FileSource) {
        let mut cache: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
        for i in 0..self.types.len() {
            let t = &self.types[i];
            if !t.is_loaded() || t.tex.is_some() {
                continue;
            }
            let premul = t.looks.premul != 0;
            let nearest = t.looks.nearest;
            let mut found = None;
            if !t.texname.is_empty() {
                let name = t.texname.clone();
                found = cache
                    .entry(name.clone())
                    .or_insert_with(|| {
                        find_image(files, &format!("textures/{name}")).or_else(|| find_image(files, &name))
                    })
                    .clone();
            }
            let r = match found {
                Some(path) => TexRef::File { path, premul, nearest },
                None => {
                    let t = &mut self.types[i];
                    t.s1 = 0.0;
                    t.t1 = 0.0;
                    t.s2 = 1.0;
                    t.t2 = 1.0;
                    t.randsmax = 1;
                    let b = match t.looks.kind {
                        RenderType::Spark => BuiltinTex::White,
                        RenderType::Beam => BuiltinTex::Beam,
                        RenderType::SparkFan => BuiltinTex::PTri,
                        _ if t.texname.contains("classicparticle") => {
                            t.s2 = 0.5;
                            t.t2 = 0.5;
                            BuiltinTex::Classic
                        }
                        RenderType::TexturedSpark => BuiltinTex::Ball,
                        _ if t.texname.contains("glow") || t.texname.contains("ball") => BuiltinTex::Ball,
                        _ => BuiltinTex::Fuzzy,
                    };
                    TexRef::Builtin(b)
                }
            };
            let idx = match self.textures.iter().position(|x| *x == r) {
                Some(i) => i,
                None => {
                    self.textures.push(r);
                    self.textures.len() - 1
                }
            };
            self.types[i].tex = Some(idx);
        }
        if !self.textures.contains(&TexRef::Builtin(BuiltinTex::White)) {
            self.textures.push(TexRef::Builtin(BuiltinTex::White));
        }
    }

    /// Index of the white texture (lines are drawn with it).
    pub fn white_texture(&self) -> Option<usize> {
        self.textures.iter().position(|t| *t == TexRef::Builtin(BuiltinTex::White))
    }
}

/// Vertex for the particle pipeline.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// A run of indices drawn with one texture and blend mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Batch {
    /// Index into [`Registry::textures`].
    pub tex: usize,
    pub blend: BlendMode,
    /// Line list (sparks) instead of triangles.
    pub lines: bool,
    pub first_index: u32,
    pub num_indices: u32,
    /// Soft particle: fades where it meets scene depth (BEF_SOFT, set for
    /// textured `normal` particles only).
    pub soft: bool,
}

/// Draw pass of a blend mode: 0 subtractive/modulating, 1 alpha, 2 additive
/// (PScript_DrawParticleBatches order).
pub fn blend_pass(b: BlendMode) -> u8 {
    match b {
        BlendMode::Blend | BlendMode::BlendColour => 1,
        BlendMode::AddA | BlendMode::AddC | BlendMode::Premul => 2,
        BlendMode::Subtract | BlendMode::InvModA | BlendMode::InvModC => 0,
    }
}

/// View for billboarding.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewInfo {
    pub origin: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

/// Collects geometry from [`crate::System::frame`].
pub struct Geometry<'a> {
    /// Reject normal billboards wholly behind this view. Disable for shared portal geometry.
    pub cull_behind: bool,
    /// Optional tangent of the vertical half-FOV; only used with cull_behind.
    pub vertical_tan: Option<f32>,
    reg: &'a Registry,
    view: ViewInfo,
    particletime: f32,
    /// Origins of brush entities for entity-relative decals; None hides them.
    entity_origin: &'a dyn Fn(i32) -> Option<Vec3>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
}

fn to_color(c: [f32; 4]) -> [u8; 4] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Colour with the type's premultiply mode applied.
fn shade(rgba: [f32; 4], premul: u8) -> [u8; 4] {
    if premul == 0 {
        return to_color(rgba);
    }
    let a = rgba[3].min(1.0);
    to_color([rgba[0] * a, rgba[1] * a, rgba[2] * a, if premul == 2 { 0.0 } else { a }])
}

impl<'a> Geometry<'a> {
    pub fn new(reg: &'a Registry, view: ViewInfo, particletime: f32, entity_origin: &'a dyn Fn(i32) -> Option<Vec3>) -> Self {
        Geometry {
            cull_behind: false,
            vertical_tan: None,
            reg,
            view,
            particletime,
            entity_origin,
            vertices: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
        }
    }

    /// Batches sorted into draw order (stable within each pass).
    pub fn ordered_batches(&self) -> Vec<Batch> {
        let mut b = self.batches.clone();
        b.sort_by_key(|x| blend_pass(x.blend));
        b
    }

    fn begin(&mut self, ty: TypeId, lines: bool, verts: &[Vertex], idx: &[u32]) {
        let t = &self.reg.types[ty];
        let tex = if lines { self.reg.white_texture() } else { t.tex };
        let Some(tex) = tex else { return };
        let base = self.vertices.len() as u32;
        let first = self.indices.len() as u32;
        self.vertices.extend_from_slice(verts);
        self.indices.extend(idx.iter().map(|i| i + base));
        let n = idx.len() as u32;
        let soft = !lines && t.looks.kind == RenderType::Normal;
        if let Some(last) = self.batches.last_mut()
            && last.tex == tex
            && last.blend == t.looks.blendmode
            && last.lines == lines
            && last.soft == soft
            && last.first_index + last.num_indices == first
        {
            last.num_indices += n;
            return;
        }
        self.batches.push(Batch { tex, blend: t.looks.blendmode, lines, first_index: first, num_indices: n, soft });
    }

    fn view_scale(&self, p: &Particle, l: &Looks, base: f32, k: f32) -> f32 {
        let d = (p.org - self.view.origin).dot(self.view.forward);
        let s = (d * p.scale) * l.invscalefactor + p.scale * (l.scalefactor * 250.0);
        if s < 20.0 { base } else { base + s * k }
    }

}

impl Sink for Geometry<'_> {
    fn particle(&mut self, ty: TypeId, p: &Particle) {
        match self.reg.types[ty].looks.kind {
            RenderType::Normal => self.textured(ty, p),
            RenderType::UDecal => self.udecal(ty, p),
            RenderType::Spark => self.line_spark(ty, p),
            RenderType::SparkFan => self.fan_spark(ty, p),
            RenderType::TexturedSpark => self.textured_spark(ty, p),
            _ => {}
        }
    }

    fn beam(&mut self, ty: TypeId, seg: &BeamSeg, next: &BeamSeg, p: &Particle, q: &Particle) {
        if self.reg.types[ty].looks.kind != RenderType::Beam {
            return;
        }
        let pt = self.particletime;
        let v = (self.view.origin - q.org).normalize();
        let cr = next.dir.cross(v).normalize();
        let ts = next.texture_s * q.angle + pt * q.rotationspeed;
        let cq = to_color(q.rgba);
        let v0 = Vertex { pos: q.org.mul_add(-q.scale, cr).to_array(), uv: [ts, p.t1], color: cq };
        let v1 = Vertex { pos: q.org.mul_add(q.scale, cr).to_array(), uv: [ts, p.t2], color: cq };
        let v = (self.view.origin - p.org).normalize();
        let cr = seg.dir.cross(v).normalize();
        let ts = seg.texture_s * p.angle + pt * p.rotationspeed;
        let cp = to_color(p.rgba);
        let v2 = Vertex { pos: p.org.mul_add(p.scale, cr).to_array(), uv: [ts, p.t2], color: cp };
        let v3 = Vertex { pos: p.org.mul_add(-p.scale, cr).to_array(), uv: [ts, p.t1], color: cp };
        self.begin(ty, false, &[v0, v1, v2, v3], &[0, 1, 2, 0, 2, 3]);
    }

    fn decal(&mut self, ty: TypeId, d: &Decal) {
        let l = &self.reg.types[ty].looks;
        let ofs = if d.entity > 0 {
            match (self.entity_origin)(d.entity) {
                Some(o) => o,
                None => return,
            }
        } else {
            Vec3::ZERO
        };
        let v: [Vertex; 3] = std::array::from_fn(|i| {
            let color = if l.premul != 0 {
                let a = d.rgba[3].min(1.0);
                let s = d.valpha[i];
                to_color([d.rgba[0] * a * s, d.rgba[1] * a * s, d.rgba[2] * a * s, if l.premul == 2 { 0.0 } else { a * s }])
            } else {
                to_color([d.rgba[0], d.rgba[1], d.rgba[2], d.rgba[3] * d.valpha[i]])
            };
            Vertex { pos: (d.vertex[i] + ofs).to_array(), uv: d.texcoords[i], color }
        });
        self.begin(ty, false, &v, &[0, 1, 2]);
    }
}

#[cfg(test)]
mod tests;
