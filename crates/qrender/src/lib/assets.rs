use super::*;

impl Renderer {
    /// Id of world submodel `*n` (n >= 1).
    pub fn submodel(&self, n: usize) -> Option<ModelId> {
        self.world
            .as_ref()?
            .submodel_ids
            .get(n.checked_sub(1)?)
            .copied()
    }

    /// Two-layer sky is taken from the map; this sets/clears a skybox
    /// (faces rt, bk, lf, ft, up, dn).
    pub fn set_skybox(&mut self, faces: Option<&[qformats::image::Rgba; 6]>) -> Result<()> {
        self.idle();
        if let Some(old) = self.skybox.take() {
            for s in old {
                if let Some(img) = self.textures[s as usize].take().and_then(|t| t.image) {
                    self.gpu.free_image(img);
                }
            }
        }
        let Some(faces) = faces else { return Ok(()) };
        let mut up = self.gpu.begin_upload()?;
        let mut slots = [NO_TEX; 6];
        for (i, f) in faces.iter().enumerate() {
            slots[i] = self.add_texture(&mut up, f.width, f.height, &[&f.pixels], true, Sampling::Clamp)?;
        }
        self.gpu.end_upload(up)?;
        self.skybox = Some(slots);
        Ok(())
    }

    /// Loads an alias model.
    pub fn load_alias(&mut self, mdl: &Mdl) -> Result<ModelId> {
        self.load_alias_glow(mdl, &[])
    }

    /// Loads an alias model whose skin `k` gets the true-colour RGBA glow
    /// layer `glow[k]` (when present and non-empty) instead of palette
    /// fullbrights: texels with alpha > 0 are drawn unlit in that colour,
    /// so add-on models can glow in colours Quake's palette lacks (green).
    pub fn load_alias_glow(&mut self, mdl: &Mdl, glow: &[Vec<u8>]) -> Result<ModelId> {
        self.idle();
        let (verts, idx) = convert::alias_mesh(mdl);
        let (poses, frames) = convert::alias_poses(mdl);
        let holey = mdl.flags & qformats::mdl::MF_HOLEY != 0;
        let mut up = self.gpu.begin_upload()?;
        let mut skins = Vec::new();
        let pal = self.palette;
        for (k, s) in mdl.skins.iter().enumerate() {
            let mut imgs = Vec::new();
            for img in &s.images {
                let (mut b, mut fb, cm) = convert::alias_skin(&pal, img, holey);
                if let Some(g) = glow.get(k).filter(|g| g.len() == b.len()) {
                    for (t, gl) in b.as_chunks_mut::<4>().0.iter_mut().zip(g.as_chunks::<4>().0) {
                        if gl[3] > 0 {
                            *t = [0, 0, 0, 255];
                        }
                    }
                    fb = Some(g.clone());
                }
                let (w, h) = (mdl.skin_width, mdl.skin_height);
                let bs = self.add_texture(&mut up, w, h, &[&b], true, Sampling::World)?;
                let fs = match fb {
                    Some(fb) => self.add_texture(&mut up, w, h, &[&fb], true, Sampling::World)?,
                    None => NO_TEX,
                };
                let cs = match cm {
                    Some(cm) => self.add_texture(&mut up, w, h, &[&cm], true, Sampling::Nearest)?,
                    None => NO_TEX,
                };
                imgs.push([bs, fs, cs]);
            }
            skins.push((imgs, s.intervals.clone()));
        }
        let usage = vk::BufferUsageFlags::TRANSFER_DST;
        let vb = bytes_of(&verts);
        let vbuf = self.gpu.buffer(
            vb.len() as u64,
            usage | vk::BufferUsageFlags::VERTEX_BUFFER,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, vbuf.buf, 0, vb)?;
        let ib = bytes_of(&idx);
        let ibuf = self.gpu.buffer(
            ib.len() as u64,
            usage | vk::BufferUsageFlags::INDEX_BUFFER,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, ibuf.buf, 0, ib)?;
        let base = self.alias_cpu.len() as u32;
        self.alias_cpu.extend_from_slice(&poses);
        let need = (self.alias_cpu.len() * 4) as u64;
        let mut grow = false;
        if need > self.alias_buf.size {
            let nb = self.gpu.buffer(
                (need * 2).max(4 << 20),
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                false,
            )?;
            self.gpu
                .upload_buffer(&mut up, nb.buf, 0, bytes_of(&self.alias_cpu))?;
            let old = std::mem::replace(&mut self.alias_buf, nb);
            self.gpu.end_upload(up)?;
            self.gpu.free_buffer(old);
            grow = true;
        } else {
            let dst = self.alias_buf.buf;
            self.gpu
                .upload_buffer(&mut up, dst, base as u64 * 4, bytes_of(&poses))?;
            self.gpu.end_upload(up)?;
        }
        if grow {
            self.write_static_descriptors();
        }
        let mut maxs_z: f32 = 0.0;
        let mut radius: f32 = mdl.bounding_radius;
        for f in &mdl.frames {
            let a = mdl.decode(f.bboxmin);
            let b = mdl.decode(f.bboxmax);
            maxs_z = maxs_z.max(b[2]);
            radius = radius
                .max(world::dot(a, a).sqrt())
                .max(world::dot(b, b).sqrt());
        }
        self.models.push(Model::Alias(Box::new(AliasModel {
            vbuf,
            ibuf,
            num_indices: idx.len() as u32,
            verts_base: base,
            num_verts: mdl.num_verts as u32,
            frames,
            skins,
            scale: mdl.scale,
            scale_origin: mdl.scale_origin,
            radius,
            maxs_z,
            holey,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Loads a sprite (SPR v1 palette or SPR32).
    pub fn load_sprite(&mut self, spr: &Spr) -> Result<ModelId> {
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let pal = self.palette;
        let mut frames = Vec::new();
        for f in &spr.frames {
            let mut images = Vec::new();
            for im in &f.images {
                let rgba = if spr.is_rgba() {
                    im.pixels.clone()
                } else {
                    convert::to_rgba(&pal, &im.pixels, Some(255))
                };
                let slot = self.add_texture(&mut up, im.width, im.height, &[&rgba], true, Sampling::Clamp)?;
                let left = im.origin[0] as f32;
                let up_ = im.origin[1] as f32;
                images.push((
                    slot,
                    [left, left + im.width as f32, up_ - im.height as f32, up_],
                ));
            }
            frames.push(SpriteFrameGpu {
                images,
                intervals: f.intervals.clone(),
            });
        }
        self.gpu.end_upload(up)?;
        self.models.push(Model::Sprite(SpriteModel {
            kind: spr.kind,
            frames,
            radius: spr.bounding_radius,
        }));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Registers a 2D picture (index 255 transparent). Persists across maps.
    pub fn register_pic(&mut self, pic: &Pic) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, &pic.pixels, Some(255));
        self.register_pic_rgba(pic.width, pic.height, &rgba)
    }

    pub fn register_pic_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Nearest)
    }

    /// Like [`register_pic_rgba`](Self::register_pic_rgba) but bilinear
    /// filtered (smooth shapes drawn larger than their texels).
    pub fn register_pic_rgba_linear(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Linear)
    }

    pub(super) fn register_pic_sampled(&mut self, width: u32, height: u32, rgba: &[u8], sampling: Sampling) -> Result<PicId> {
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let slot = self.add_texture(&mut up, width, height, &[rgba], false, sampling)?;
        self.gpu.end_upload(up)?;
        self.pics.push(PicInfo {
            slot,
            width,
            height,
        });
        Ok(PicId(self.pics.len() as u32 - 1))
    }

    /// Sets conchars (128×128 indices, 0 transparent) used by [`Draw2D::Char`].
    pub fn set_conchars(&mut self, pixels: &[u8]) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, pixels, Some(0));
        let id = self.register_pic_rgba(128, 128, &rgba)?;
        self.conchars = Some(id);
        Ok(id)
    }

    /// Replaces a picture's texels (same size it was registered with), for
    /// CPU-built atlases such as the UI text cache. Waits for the GPU.
    pub fn update_pic(&mut self, pic: PicId, rgba: &[u8]) -> Result<()> {
        let Some(&PicInfo { slot, width, height }) = self.pics.get(pic.0 as usize) else {
            return Err(Error::new("no such picture"));
        };
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let img = self.gpu.image(
            width,
            height,
            1,
            1,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageAspectFlags::COLOR,
        )?;
        self.gpu.upload_image(&mut up, &img, &[rgba])?;
        self.gpu.end_upload(up)?;
        let sampling = self.textures[slot as usize].as_ref().map_or(Sampling::Nearest, |t| t.sampling);
        self.write_texture_descriptor(slot, img.view, self.sampler_for(sampling));
        let old = self.textures[slot as usize].replace(Tex { image: Some(img), level: false, sampling });
        if let Some(img) = old.and_then(|t| t.image) {
            self.gpu.free_image(img);
        }
        Ok(())
    }
}
