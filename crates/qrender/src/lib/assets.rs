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
            slots[i] = self.add_texture(&mut up, f.width, f.height, &[&f.pixels], TexUse::Sky, Sampling::Clamp)?;
        }
        self.gpu.end_upload(up)?;
        self.skybox = Some(slots);
        Ok(())
    }

    pub(super) fn upload_alias_buffers(
        &mut self,
        mut up: gpu::Upload,
        verts: &[convert::AliasVertex],
        idx: &[u32],
        poses: &[u32],
    ) -> Result<(gpu::Buffer, gpu::Buffer, u32)> {
        let usage = vk::BufferUsageFlags::TRANSFER_DST;
        let vb = bytes_of(verts);
        let vbuf = self.gpu.buffer(
            (vb.len() as u64).max(4),
            usage | vk::BufferUsageFlags::VERTEX_BUFFER,
            false,
        )?;
        if !vb.is_empty() {
            self.gpu.upload_buffer(&mut up, vbuf.buf, 0, vb)?;
        }
        let ib = bytes_of(idx);
        let ibuf = self.gpu.buffer(
            (ib.len() as u64).max(4),
            usage | vk::BufferUsageFlags::INDEX_BUFFER,
            false,
        )?;
        if !ib.is_empty() {
            self.gpu.upload_buffer(&mut up, ibuf.buf, 0, ib)?;
        }
        let base = self.alias_cpu.len() as u32;
        if !poses.is_empty() {
            self.alias_cpu.extend_from_slice(poses);
            let need = (self.alias_cpu.len() * 4) as u64;
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
                self.write_static_descriptors();
                return Ok((vbuf, ibuf, base));
            }
            let dst = self.alias_buf.buf;
            self.gpu
                .upload_buffer(&mut up, dst, base as u64 * 4, bytes_of(poses))?;
        }
        self.gpu.end_upload(up)?;
        Ok((vbuf, ibuf, base))
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
                let bs = self.add_texture(&mut up, w, h, &[&b], TexUse::Base, Sampling::World)?;
                let fs = match fb {
                    Some(fb) => self.add_texture(&mut up, w, h, &[&fb], TexUse::Glow, Sampling::World)?,
                    None => NO_TEX,
                };
                let cs = match cm {
                    Some(cm) => self.add_texture(&mut up, w, h, &[&cm], TexUse::Index, Sampling::Nearest)?,
                    None => NO_TEX,
                };
                imgs.push([bs, fs, cs]);
            }
            skins.push((imgs, s.intervals.clone()));
        }
        let (vbuf, ibuf, base) = self.upload_alias_buffers(up, &verts, &idx, &poses)?;
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
            surfaces: vec![AliasSurface {
                first_index: 0,
                num_indices: idx.len() as u32,
                verts_base: base,
                num_verts: mdl.num_verts as u32,
                skins,
                holey,
            }],
            frames,
            scale: mdl.scale,
            scale_origin: mdl.scale_origin,
            radius,
            maxs_z,
            md3: false,
            iqm: false,
            num_bones: 0,
            bone_base: 0,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Converts a palette-indexed MD3 skin image into RGBA base, fullbright and
    /// player colour-translation map buffers using the renderer palette.
    pub fn md3_indexed_skin(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
        holey: bool,
    ) -> Md3SkinImage {
        let (base, fb, cmap) = convert::alias_skin(&self.palette, pixels, holey);
        Md3SkinImage {
            width,
            height,
            base,
            fb: fb.map(|d| (width, height, d)),
            cmap: cmap.map(|d| (width, height, d)),
        }
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
                let slot = self.add_texture(&mut up, im.width, im.height, &[&rgba], TexUse::Sprite, Sampling::Clamp)?;
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
        let slot = self.add_texture(&mut up, width, height, &[rgba], TexUse::Pic, sampling)?;
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

    /// Updates the stored dimensions of an existing picture so the next
    /// [`Self::update_pic_rows`] recreates its backing image at the new size
    /// without allocating a new `PicId` or texture slot.
    pub fn resize_pic(&mut self, pic: PicId, width: u32, height: u32) -> Result<()> {
        let Some(info) = self.pics.get_mut(pic.0 as usize) else {
            return Err(Error::new("no such picture"));
        };
        info.width = width;
        info.height = height;
        Ok(())
    }

    /// Replaces a picture's texels (same size it was registered with), for
    /// CPU-built atlases such as the UI text cache.
    pub fn update_pic(&mut self, pic: PicId, rgba: &[u8]) -> Result<()> {
        self.update_pic_rows(pic, rgba, u32::MAX)
    }

    /// Like [`Self::update_pic`], but only the first `rows` rows of `rgba`
    /// are uploaded and the rest of the picture becomes transparent black.
    /// When the backing image already matches `pic`'s dimensions, the upload
    /// is staged and recorded onto the next rendered frame without idling.
    pub fn update_pic_rows(&mut self, pic: PicId, rgba: &[u8], rows: u32) -> Result<()> {
        let Some(&PicInfo { slot, width, height }) = self.pics.get(pic.0 as usize) else {
            return Err(Error::new("no such picture"));
        };
        if width.max(height) > self.gpu.max_texture {
            return Err(Error::new("picture larger than the GPU supports"));
        }
        if let Some(Tex { image: Some(img), .. }) = self.textures.get(slot as usize).and_then(|t| t.as_ref())
            && img.width == width && img.height == height && img.mips == 1 && img.layers == 1
        {
            let img_handle = img.image;
            let rows = rows.min(height);
            let len = (width * rows * 4) as usize;
            let staging = if len > 0 {
                let mut buf = self.gpu.buffer(len as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
                buf.bytes_mut()[..len].copy_from_slice(&rgba[..len]);
                Some(buf)
            } else {
                None
            };
            // Drop any earlier staged upload for the same image (unsubmitted).
            let mut i = 0;
            while i < self.pending_pic_uploads.len() {
                if self.pending_pic_uploads[i].image == img_handle {
                    if let Some(old_st) = self.pending_pic_uploads.swap_remove(i).staging {
                        self.gpu.free_buffer(old_st);
                    }
                } else {
                    i += 1;
                }
            }
            self.pending_pic_uploads.push(PendingPicUpload {
                image: img_handle,
                width,
                height,
                rows,
                staging,
            });
            return Ok(());
        }
        self.idle();
        let tex = self.textures[slot as usize].as_mut();
        let sampling = tex.as_ref().map_or(Sampling::Nearest, |t| t.sampling);
        let old = tex.and_then(|t| t.image.take());
        if let Some(ref old_img) = old {
            let mut i = 0;
            while i < self.pending_pic_uploads.len() {
                if self.pending_pic_uploads[i].image == old_img.image {
                    if let Some(old_st) = self.pending_pic_uploads.swap_remove(i).staging {
                        self.gpu.free_buffer(old_st);
                    }
                } else {
                    i += 1;
                }
            }
        }
        let usage = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        let img = match self.gpu.image(width, height, 1, 1, vk::Format::R8G8B8A8_UNORM, usage, vk::ImageAspectFlags::COLOR) {
            Ok(img) => img,
            Err(e) => {
                if let Some(t) = self.textures[slot as usize].as_mut() {
                    t.image = old;
                }
                return Err(e);
            }
        };
        self.write_texture_descriptor(slot, img.view, self.sampler_for(sampling));
        let mut up = self.gpu.begin_upload()?;
        let r = if rows >= height { self.gpu.upload_image(&mut up, &img, &[rgba]) } else { self.gpu.upload_image_rows(&mut up, &img, rgba, rows) };
        let r = r.and_then(|_| self.gpu.end_upload(up));
        self.textures[slot as usize] = Some(Tex { image: Some(img), level: false, sampling });
        if let Some(img) = old {
            self.gpu.free_image(img);
        }
        r
    }
}
