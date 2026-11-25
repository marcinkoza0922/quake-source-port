//! Level loading: world geometry, BSP textures, lightmap atlas.

use super::models::Model;
use super::textures::Sampling;
use crate::convert::TexUse;
use super::{NO_TEX, Renderer};
use crate::{Bsp, LevelBsp, ModelId, Result, TextureKind, convert, world};

pub(super) struct WorldRes {
    pub(super) cpu: world::World,
    pub(super) vbuf: wgpu::Buffer,
    /// Per BSP texture: base slot, fullbright slot, material flags, packed
    /// miptex size (`convert::packed_size`).
    pub(super) materials: Vec<[u32; 4]>,
    pub(super) kinds: Vec<TextureKind>,
    pub(super) submodel_ids: Vec<ModelId>,
    /// Per-texture surface buckets reused while building draws.
    pub(super) buckets: Vec<Vec<u32>>,
}

impl Renderer {
    fn free_level(&mut self) {
        for i in 0..self.textures.len() {
            if self.textures[i].as_ref().is_some_and(|t| t.level) {
                self.free_texture(i as u32);
            }
        }
        for m in self.models.drain(..) {
            if let Model::Alias(a) = m {
                a.ibuf.destroy();
            }
        }
        if let Some(w) = self.world.take() {
            w.vbuf.destroy();
        }
        self.sky_layers = [NO_TEX; 2];
        self.skybox = None;
        self.sky_group = None;
        self.hidden_entities.clear();
    }

    fn upload_bsp_textures(&mut self, lb: LevelBsp, materials: &mut Vec<[u32; 4]>, kinds: &mut Vec<TextureKind>) -> Result<()> {
        let pal = self.palette;
        let bsp = lb.bsp;
        for (ti, t) in bsp.textures.iter().enumerate() {
            let checker: Vec<u8> = (0..16 * 16).map(|i| if ((i % 16) / 8 + (i / 128)) % 2 == 0 { 0 } else { 15 }).collect();
            let fallback = qformats::bsp::MipTex {
                name: "notexture".into(),
                width: 16,
                height: 16,
                kind: TextureKind::Normal,
                mips: vec![checker],
                shift: 0,
            };
            let mut t = t.clone().unwrap_or_else(|| fallback.clone());
            if t.mips.is_empty() {
                t = qformats::bsp::MipTex { name: t.name.clone(), kind: t.kind, ..fallback };
            }
            let flags = world::material_flags(&t.name, t.kind);
            if t.kind == TextureKind::Sky
                && self.sky_layers[0] == NO_TEX
                && let Some((w, h, solid, alpha)) =
                    convert::sky_layers(&pal, &t, bsp.version == Some(qformats::bsp::BspVersion::Quake64))
            {
                let s = self.add_texture(w, h, &[&solid], TexUse::Base, Sampling::World)?;
                let a = self.add_texture(w, h, &[&alpha], TexUse::Base, Sampling::World)?;
                self.sky_layers = [s, a];
            }
            let (base, fb) = convert::world_texture_images(&pal, &t, lb.external(ti));
            let b = self.add_texture(base.width, base.height, &base.mip_refs(), TexUse::Base, Sampling::World)?;
            let f = match &fb {
                Some(fb) => self.add_texture(fb.width, fb.height, &fb.mip_refs(), TexUse::Glow, Sampling::World)?,
                None => NO_TEX,
            };
            materials.push([b, f, flags, convert::packed_size(&t)]);
            kinds.push(t.kind);
        }
        if bsp.textures.is_empty() {
            materials.push([NO_TEX, NO_TEX, 0, 0]);
            kinds.push(TextureKind::Normal);
        }
        Ok(())
    }

    /// Loads a new map (frees all previous level models/textures). `lit` is a
    /// parsed `.lit` (RGB per mono sample). Returns ids for submodels `*1..`.
    pub fn load_world(&mut self, bsp: &Bsp, lit: Option<&[u8]>) -> Result<Vec<ModelId>> {
        Ok(self.load_level(LevelBsp::new(bsp, lit), &[])?.0)
    }

    pub fn has_world(&self) -> bool {
        self.world.is_some()
    }

    pub fn model_bounds(&self, id: ModelId) -> Option<([f32; 3], [f32; 3])> {
        match self.models.get(id.0 as usize)? {
            Model::Brush(sm) => {
                let sub = self.world.as_ref()?.cpu.submodels.get(*sm as usize)?;
                Some((sub.mins, sub.maxs))
            }
            Model::Alias(am) => {
                let r = am.radius.max(16.0);
                Some(([-r; 3], [r; 3]))
            }
            Model::Sprite(sm) => {
                let r = sm.radius.max(16.0);
                Some(([-r; 3], [r; 3]))
            }
        }
    }

    pub fn world_box_leaves(&self, mins: [f32; 3], maxs: [f32; 3], out: &mut Vec<u32>) {
        if let Some(w) = &self.world {
            w.cpu.box_leaves(mins, maxs, out);
        }
    }

    pub fn world_pvs_bits(&mut self, eye: [f32; 3]) -> Option<&[u32]> {
        self.world.as_mut()?.cpu.pvs_bits(eye)
    }

    /// `load_world` plus external brush models (`maps/b_*.bsp`) and external
    /// replacement textures. Returns (world submodel ids `*1..`, one id per
    /// entry of `brushes`).
    pub fn load_level(&mut self, level: LevelBsp, brushes: &[LevelBsp]) -> Result<(Vec<ModelId>, Vec<ModelId>)> {
        self.free_level();
        let mut cpu = world::World::build_ext(level.bsp, level.lit, level.hdr_lit, level.deluxe);
        let world_subs = cpu.submodels.len();
        let mut materials = Vec::new();
        let mut kinds = Vec::new();
        self.upload_bsp_textures(level, &mut materials, &mut kinds)?;
        let mut brush_ids = Vec::new();
        for b in brushes {
            let first = cpu.add_brush_ext(b.bsp, b.lit, b.hdr_lit, b.deluxe, materials.len() as u32);
            self.upload_bsp_textures(*b, &mut materials, &mut kinds)?;
            self.models.push(Model::Brush(first));
            brush_ids.push(ModelId(self.models.len() as u32 - 1));
        }
        let vbytes = crate::bytes_of(&cpu.vertices);
        let vbuf = self.dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("world vertices"),
            size: (vbytes.len() as u64).max(4).next_multiple_of(4),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !vbytes.is_empty() {
            self.queue.write_buffer(&vbuf, 0, vbytes);
        }
        self.lightmap = self.lightmap.for_world(&self.dev, &cpu);
        self.bufs.frame_group = None;
        let mut ids = Vec::new();
        for i in 1..world_subs {
            self.models.push(Model::Brush(i as u32));
            ids.push(ModelId(self.models.len() as u32 - 1));
        }
        self.world = Some(WorldRes {
            buckets: vec![Vec::new(); materials.len()],
            cpu,
            vbuf,
            materials,
            kinds,
            submodel_ids: ids.clone(),
        });
        Ok((ids, brush_ids))
    }
}
