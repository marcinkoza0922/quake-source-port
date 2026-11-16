//! Level loading: world geometry, BSP textures, lightmap atlas.

use super::lightmap::Lightmaps;
use super::models::Model;
use super::textures::Sampling;
use super::{NO_TEX, Renderer};
use crate::{Bsp, ModelId, Result, TextureKind, convert, world};

pub(super) struct WorldRes {
    pub(super) cpu: world::World,
    pub(super) vbuf: wgpu::Buffer,
    /// Per BSP texture: base slot, fullbright slot, material flags.
    pub(super) materials: Vec<[u32; 3]>,
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
    }

    fn upload_bsp_textures(&mut self, bsp: &Bsp, materials: &mut Vec<[u32; 3]>, kinds: &mut Vec<TextureKind>) -> Result<()> {
        let pal = self.palette;
        for t in &bsp.textures {
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
                let s = self.add_texture(w, h, &[&solid], true, Sampling::World)?;
                let a = self.add_texture(w, h, &[&alpha], true, Sampling::World)?;
                self.sky_layers = [s, a];
            }
            let (base, fb) = convert::world_texture(&pal, &t);
            let bm: Vec<&[u8]> = base.iter().map(|m| m.as_slice()).collect();
            let b = self.add_texture(t.width, t.height, &bm, true, Sampling::World)?;
            let f = match &fb {
                Some(fb) => {
                    let fm: Vec<&[u8]> = fb.iter().map(|m| m.as_slice()).collect();
                    self.add_texture(t.width, t.height, &fm, true, Sampling::World)?
                }
                None => NO_TEX,
            };
            materials.push([b, f, flags]);
            kinds.push(t.kind);
        }
        if bsp.textures.is_empty() {
            materials.push([NO_TEX, NO_TEX, 0]);
            kinds.push(TextureKind::Normal);
        }
        Ok(())
    }

    /// Loads a new map (frees all previous level models/textures). `lit` is a
    /// parsed `.lit` (RGB per mono sample). Returns ids for submodels `*1..`.
    pub fn load_world(&mut self, bsp: &Bsp, lit: Option<&[u8]>) -> Result<Vec<ModelId>> {
        Ok(self.load_level(bsp, lit, &[])?.0)
    }

    /// `load_world` plus external brush models (`maps/b_*.bsp`). Returns
    /// (world submodel ids `*1..`, one id per entry of `brushes`).
    pub fn load_level(
        &mut self,
        bsp: &Bsp,
        lit: Option<&[u8]>,
        brushes: &[(&Bsp, Option<&[u8]>)],
    ) -> Result<(Vec<ModelId>, Vec<ModelId>)> {
        self.free_level();
        let mut cpu = world::World::build(bsp, lit);
        let world_subs = cpu.submodels.len();
        let mut materials = Vec::new();
        let mut kinds = Vec::new();
        self.upload_bsp_textures(bsp, &mut materials, &mut kinds)?;
        let mut brush_ids = Vec::new();
        for (b, blit) in brushes {
            let first = cpu.add_brush(b, *blit, materials.len() as u32);
            self.upload_bsp_textures(b, &mut materials, &mut kinds)?;
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
        self.lightmap = Lightmaps::for_world(&self.dev, &cpu);
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
