//! Particle geometry: textures (P_LoadTexture) and vertex batches
//! (R_Emit*Particle, R_DrawParticleBeam, R_AddClippedDecal).
use crate::system::{BeamSeg, Decal, Particle, Sink};
use crate::types::*;
use crate::util::{pcos, psin};
use crate::{FileSource, Registry, TypeId};
use qcore::math::Vec3;

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

const IMAGE_EXTS: &[&str] = &["tga", "pcx", "lmp"];

/// RGBA pixels for a texture.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Builds a builtin texture.
pub fn builtin_image(b: BuiltinTex) -> Image {
    const N: usize = 64;
    let img = |w: usize, h: usize, f: &dyn Fn(usize, usize) -> [u8; 4]| {
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Image { width: w as u32, height: h as u32, rgba }
    };
    let half = N as f32 * 0.5 - 1.0;
    match b {
        BuiltinTex::White => img(4, 4, &|_, _| [255; 4]),
        BuiltinTex::Beam => img(N, N, &|_, y| {
            let dy = (y as f32 - 0.5 * N as f32) / half;
            let d = (256.0 * (1.0 - dy * dy)).max(0.0);
            [255, 255, 255, d as i32 as u8]
        }),
        BuiltinTex::PTri => img(N, N, &|x, y| {
            let d = (256.0 * (1.0 - (x as f32 / half + y as f32 / half))).max(0.0);
            let v = d as i32 as u8;
            [v, v, v, v / 2]
        }),
        BuiltinTex::Ball => img(N, N, &|x, y| {
            let dx = (x as f32 - 0.5 * N as f32) / half;
            let dy = (y as f32 - 0.5 * N as f32) / half;
            let d = (255.0 * (1.0 - (dx * dx + dy * dy))).max(0.0);
            [255, 255, 255, d as u8]
        }),
        BuiltinTex::Fuzzy => {
            #[rustfmt::skip]
            const EXP: [[u8; 16]; 16] = [
                [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0], [0,0,0,0,1,0,0,0,1,0,0,1,0,0,0,0],
                [0,0,0,1,1,1,1,1,3,1,1,2,1,0,0,0], [0,0,0,1,1,1,1,4,4,4,5,4,2,1,1,0],
                [0,0,1,1,6,5,5,8,6,8,3,6,3,2,1,0], [0,0,1,5,6,7,5,6,8,8,8,3,3,1,0,0],
                [0,0,0,1,6,8,9,9,9,9,4,6,3,1,0,0], [0,0,2,1,7,7,9,9,9,9,5,3,1,0,0,0],
                [0,0,2,4,6,8,9,9,9,9,8,6,1,0,0,0], [0,0,2,2,3,5,6,8,9,8,8,4,4,1,0,0],
                [0,0,1,2,4,1,8,7,8,8,6,5,4,1,0,0], [0,1,1,1,7,8,1,6,7,5,4,7,1,0,0,0],
                [0,1,2,1,1,5,1,3,4,3,1,1,0,0,0,0], [0,0,0,0,0,1,1,1,1,1,0,0,0,0,0,0],
                [0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            ];
            img(16, 16, &|x, y| [255, 255, 255, (EXP[x][y] as u32 * 255 / 9) as u8])
        }
        BuiltinTex::Classic => img(N, N, &|x, y| {
            // R_ParticleTextureLookup(x, y, 8), stored column-major in C.
            let (dx, dy) = (y as i32 - 16, x as i32 - 16);
            let r = (dx * dx + dy * dy).min(255);
            [255, 255, 255, (8 * (255 - r)).min(255) as u8]
        }),
    }
}

/// Loads an image file as RGBA (Image_LoadImage subset: tga, pcx, lmp).
/// `path` must include the extension.
pub fn load_image(files: &mut dyn FileSource, path: &str, palette: Option<&[u8; 768]>, premul: bool) -> Option<Image> {
    let data = files.load(path)?;
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    let pal_rgba = |pixels: &[u8], pal: &[[u8; 3]]| -> Vec<u8> {
        pixels
            .iter()
            .flat_map(|&i| {
                let c = pal.get(i as usize).copied().unwrap_or([0; 3]);
                [c[0], c[1], c[2], if i == 255 { 0 } else { 255 }]
            })
            .collect()
    };
    let game_pal = || -> Vec<[u8; 3]> {
        palette.map_or_else(|| vec![[0; 3]; 256], |p| p.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
    };
    let mut img = match ext.as_str() {
        "tga" => {
            let t = qformats::image::parse_tga(&data).ok()?;
            Image { width: t.width, height: t.height, rgba: t.pixels }
        }
        "pcx" => {
            let (p, pal) = qformats::image::parse_pcx(&data).ok()?;
            Image { width: p.width, height: p.height, rgba: pal_rgba(&p.pixels, &pal) }
        }
        "lmp" => {
            let p = qformats::image::parse_pic(&data).ok()?;
            Image { width: p.width, height: p.height, rgba: pal_rgba(&p.pixels, &game_pal()) }
        }
        _ => return None,
    };
    if premul {
        for px in img.rgba.chunks_mut(4) {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = (*c as u32 * a / 255) as u8;
            }
        }
    }
    Some(img)
}

fn find_image(files: &mut dyn FileSource, base: &str) -> Option<String> {
    IMAGE_EXTS.iter().map(|e| format!("{base}.{e}")).find(|p| files.load(p).is_some())
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
        if let Some(last) = self.batches.last_mut()
            && last.tex == tex
            && last.blend == t.looks.blendmode
            && last.lines == lines
            && last.first_index + last.num_indices == first
        {
            last.num_indices += n;
            return;
        }
        self.batches.push(Batch { tex, blend: t.looks.blendmode, lines, first_index: first, num_indices: n });
    }

    fn view_scale(&self, p: &Particle, l: &Looks, base: f32, k: f32) -> f32 {
        let d = (p.org - self.view.origin).dot(self.view.forward);
        let s = (d * p.scale) * l.invscalefactor + p.scale * (l.scalefactor * 250.0);
        if s < 20.0 { base } else { base + s * k }
    }

    fn textured(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let scale = if l.scalefactor == 1.0 { p.scale * 0.25 } else { self.view_scale(p, l, 0.25, 0.001) };
        let c = shade(p.rgba, l.premul);
        let (pr, pu) = (self.view.right * 1.5, self.view.up * 1.5);
        let pos = if p.angle != 0.0 {
            let (x, y) = (psin(p.angle) * scale, pcos(p.angle) * scale);
            [
                p.org - pr * x - pu * y,
                p.org - pr * y + pu * x,
                p.org + pr * x + pu * y,
                p.org + pr * y - pu * x,
            ]
        } else {
            [p.org - pu * scale, p.org - pr * scale, p.org + pu * scale, p.org + pr * scale]
        };
        self.quad(ty, pos, p, c);
    }

    fn quad(&mut self, ty: TypeId, pos: [Vec3; 4], p: &Particle, c: [u8; 4]) {
        let uv = [[p.s1, p.t1], [p.s1, p.t2], [p.s2, p.t2], [p.s2, p.t1]];
        let v: [Vertex; 4] = std::array::from_fn(|i| Vertex { pos: pos[i].to_array(), uv: uv[i], color: c });
        self.begin(ty, false, &v, &[0, 1, 2, 0, 2, 3]);
    }

    fn udecal(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let c = shade(p.rgba, l.premul);
        let (sd, td) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let pos = if p.angle != 0.0 {
            let (x, y) = (psin(p.angle) * p.scale, pcos(p.angle) * p.scale);
            [p.org - sd * x - td * y, p.org - sd * y + td * x, p.org + sd * x + td * y, p.org + sd * y - td * x]
        } else {
            [p.org - td * p.scale, p.org - sd * p.scale, p.org + td * p.scale, p.org + sd * p.scale]
        };
        self.quad(ty, pos, p, c);
    }

    fn line_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let (c0, c1) = if l.premul != 0 {
            let a = p.rgba[3].min(1.0);
            let mut c0 = to_color([p.rgba[0] * a, p.rgba[1] * a, p.rgba[2] * a, if l.premul == 2 { 0.0 } else { a }]);
            if l.premul == 2 {
                c0[3] = 0;
            }
            (c0, [0; 4])
        } else {
            let c0 = to_color(p.rgba);
            (c0, [c0[0], c0[1], c0[2], 0])
        };
        let v = [
            Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c0 },
            Vertex { pos: p.org.mul_add(-0.1, p.vel).to_array(), uv: [p.s2, p.t2], color: c1 },
        ];
        self.begin(ty, true, &v, &[0, 1]);
    }

    fn fan_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let scale = self.view_scale(p, l, 0.05, 0.0001);
        // vkQuake scales the premultiplied colour by 255 twice here (always
        // saturating); use the same shading as the other kinds instead.
        let c = shade(p.rgba, l.premul);
        let o2 = p.org.mul_add(-scale, p.vel);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let v = [
            Vertex { pos: p.org.to_array(), uv: [p.s1, p.t1], color: c },
            Vertex { pos: o2.mul_add(-p.scale, cr).to_array(), uv: [p.s1, p.t2], color: c },
            Vertex { pos: o2.mul_add(p.scale, cr).to_array(), uv: [p.s2, p.t1], color: c },
        ];
        self.begin(ty, false, &v, &[0, 1, 2]);
    }

    fn textured_spark(&mut self, ty: TypeId, p: &Particle) {
        let l = &self.reg.types[ty].looks;
        let c = shade(p.rgba, l.premul);
        let half = p.scale * 0.5;
        let (movedir, len) = p.vel.normalize_len();
        let mut length = if l.stretch < 0.0 {
            -l.stretch
        } else if l.stretch != 0.0 {
            len * l.stretch
        } else {
            len * 0.05
        };
        if length < half * l.minstretch {
            length = half * l.minstretch;
        }
        let o2 = p.org.mul_add(-length, movedir);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let a = o2.mul_add(-half, cr);
        let b = o2.mul_add(half, cr);
        let o2 = p.org.mul_add(length, movedir);
        let cr = (self.view.origin - o2).cross(p.vel).normalize();
        let pos = [a, b, o2.mul_add(half, cr), o2.mul_add(-half, cr)];
        self.quad(ty, pos, p, c);
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
mod tests {
    use super::*;
    use crate::system::{EmptyWorld, FrameInput, System};

    fn files<'a>(m: &'a [(&'a str, Vec<u8>)]) -> impl FnMut(&str) -> Option<Vec<u8>> + 'a {
        move |p: &str| m.iter().find(|(k, _)| *k == p).map(|(_, v)| v.clone())
    }

    fn tga(w: u16, h: u16) -> Vec<u8> {
        let mut d = vec![0u8; 18];
        d[2] = 2;
        d[12..14].copy_from_slice(&w.to_le_bytes());
        d[14..16].copy_from_slice(&h.to_le_bytes());
        d[16] = 32;
        d[17] = 0x28;
        d.extend(std::iter::repeat_n([10u8, 20, 30, 128], w as usize * h as usize).flatten());
        d
    }

    #[test]
    fn textures_resolve_with_fallbacks() {
        let fx = "r_part a\n{\ntexture particles/fteparticlefont\ntcoords 0 0 64 64 256\nscale 4\n}\n\
                  r_part b\n{\ntexture ball\ntcoords 0 0 64 64 256\nscale 4\n}\n\
                  r_part c\n{\ntype spark\n}\n\
                  r_part d\n{\ntexture nothing\nscale 1\nblend premul_add\n}\n";
        let data = vec![("particles/x.cfg", fx.as_bytes().to_vec()), ("textures/particles/fteparticlefont.tga", tga(2, 2))];
        let mut f = files(&data);
        let mut r = Registry::new();
        r.load_set("x", false, true, &mut f);
        let tex = |r: &Registry, n: &str| r.textures[r.types.iter().find(|t| t.name == n).unwrap().tex.unwrap()].clone();
        assert_eq!(
            tex(&r, "a"),
            TexRef::File { path: "textures/particles/fteparticlefont.tga".into(), premul: false, nearest: false }
        );
        assert_eq!(r.types[0].s2, 0.25);
        assert_eq!(tex(&r, "b"), TexRef::Builtin(BuiltinTex::Ball));
        // Fallback resets the coords to the whole texture.
        assert_eq!(r.types[1].s2, 1.0);
        assert_eq!(tex(&r, "c"), TexRef::Builtin(BuiltinTex::White));
        assert_eq!(tex(&r, "d"), TexRef::Builtin(BuiltinTex::Fuzzy));
        let img = load_image(&mut f, "textures/particles/fteparticlefont.tga", None, true).unwrap();
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(&img.rgba[..4], &[15, 10, 5, 128]);
        for b in [BuiltinTex::White, BuiltinTex::Beam, BuiltinTex::PTri, BuiltinTex::Ball, BuiltinTex::Fuzzy, BuiltinTex::Classic] {
            let i = builtin_image(b);
            assert_eq!(i.rgba.len(), (i.width * i.height * 4) as usize);
        }
        // Ball: opaque centre, transparent corner.
        let ball = builtin_image(BuiltinTex::Ball);
        assert_eq!(ball.rgba[(32 * 64 + 32) * 4 + 3], 255);
        assert_eq!(ball.rgba[3], 0);
    }

    #[test]
    fn builds_batches_per_kind() {
        let fx = "r_part q\n{\ntexture ball\nscale 8\ncount 4\ndie 1\nalpha 1\nrgbf 1 0 0\n}\n\
                  r_part s\n{\ntype spark\ncount 3\ndie 1\nvelbias 0 0 100\n}\n\
                  r_part t\n{\ntexture ball\ntype texturedspark\nscale 2\ncount 2\ndie 1\nvelbias 100 0 0\nblend add\n}\n";
        let data = vec![("particles/x.cfg", fx.as_bytes().to_vec())];
        let mut f = files(&data);
        let mut r = Registry::new();
        r.load_set("x", false, true, &mut f);
        let mut s = System::default();
        let id = |n: &str| r.types.iter().position(|t| t.name == n).unwrap();
        for n in ["q", "s", "t"] {
            s.run_effect(&r, &EmptyWorld, Vec3::new(100.0, 0.0, 0.0), None, 1.0, id(n), None);
        }
        let view = ViewInfo {
            origin: Vec3::ZERO,
            forward: Vec3::new(1.0, 0.0, 0.0),
            right: Vec3::new(0.0, -1.0, 0.0),
            up: Vec3::new(0.0, 0.0, 1.0),
        };
        let eo = |_| None;
        let mut g = Geometry::new(&r, view, 0.0, &eo);
        s.frame(&r, &EmptyWorld, &FrameInput { cl_time: 0.01, ..Default::default() }, &mut g);
        assert_eq!(g.batches.len(), 3);
        let by = |lines: bool, blend: BlendMode| g.batches.iter().find(|b| b.lines == lines && b.blend == blend).unwrap();
        assert_eq!(by(false, BlendMode::Blend).num_indices, 4 * 6);
        assert_eq!(by(true, BlendMode::Blend).num_indices, 3 * 2);
        assert_eq!(by(false, BlendMode::AddA).num_indices, 2 * 6);
        assert_eq!(g.vertices.len(), 4 * 4 + 3 * 2 + 2 * 4);
        assert!(g.indices.iter().all(|&i| (i as usize) < g.vertices.len()));
        // Textured quad: scalefactor 0 -> distance scaling; red, opaque.
        let v = g.vertices[g.indices[by(false, BlendMode::Blend).first_index as usize] as usize];
        // (alpha has faded for 0.01 s)
        assert_eq!(&v.color[..3], &[255, 0, 0]);
        assert!(v.color[3] > 250);
        // Draw order: alpha before additive.
        let o = g.ordered_batches();
        assert!(blend_pass(o[0].blend) <= blend_pass(o[2].blend));
    }
}
