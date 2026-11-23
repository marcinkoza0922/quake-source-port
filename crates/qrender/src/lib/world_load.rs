use super::*;

impl Renderer {
    pub(super) fn free_level(&mut self) {
        self.idle();
        self.free_rt_world();
        self.hidden_entities.clear();
        self.isolated_portal = None;
        self.isolated_distance = None;
        for i in 0..self.textures.len() {
            if self.textures[i].as_ref().is_some_and(|t| t.level)
                && let Some(t) = self.textures[i].take()
                && let Some(img) = t.image
            {
                self.gpu.free_image(img);
            }
        }
        for m in self.models.drain(..) {
            if let Model::Alias(a) = m {
                self.gpu.free_buffer(a.vbuf);
                self.gpu.free_buffer(a.ibuf);
            }
        }
        if let Some(w) = self.world.take() {
            self.gpu.free_buffer(w.vbuf);
            self.gpu.free_buffer(w.samples);
            self.gpu.free_buffer(w.deluxe_samples);
            self.gpu.free_buffer(w.surfs);
            self.gpu.free_buffer(w.materials);
            let c = w.cull;
            for b in [c.marks, c.surfs, c.src, c.surf_frame].into_iter().chain(c.out).chain(c.cmdbuf) {
                self.gpu.free_buffer(b);
            }
        }
        self.alias_cpu.clear();
        self.sky_layers = [NO_TEX; 2];
        self.skybox = None;
    }

    /// Uploads a BSP's textures (or their external replacements), appending
    /// to `materials`/`tex_info`.
    pub(super) fn upload_bsp_textures(
        &mut self,
        up: &mut gpu::Upload,
        lb: LevelBsp,
        materials: &mut Vec<[u32; 4]>,
        tex_info: &mut Vec<(TextureKind, u32)>,
    ) -> Result<()> {
        let pal = self.palette;
        let bsp = lb.bsp;
        for (ti, t) in bsp.textures.iter().enumerate() {
            let checker: Vec<u8> = (0..16 * 16)
                .map(|i| {
                    if ((i % 16) / 8 + (i / 128)) % 2 == 0 {
                        0
                    } else {
                        15
                    }
                })
                .collect();
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
                t = qformats::bsp::MipTex {
                    name: t.name.clone(),
                    kind: t.kind,
                    ..fallback
                };
            }
            let flags = world::material_flags(&t.name, t.kind);
            if t.kind == TextureKind::Sky
                && self.sky_layers[0] == NO_TEX
                && let Some((w, h, solid, alpha)) = convert::sky_layers(&pal, &t, bsp.version == Some(qformats::bsp::BspVersion::Quake64))
            {
                let s = self.add_texture(up, w, h, &[&solid], true, Sampling::World)?;
                let a = self.add_texture(up, w, h, &[&alpha], true, Sampling::World)?;
                self.sky_layers = [s, a];
            }
            let (base, fb) = convert::world_texture_images(&pal, &t, lb.external(ti));
            let b = self.add_texture(up, base.width, base.height, &base.mip_refs(), true, Sampling::World)?;
            let f = match &fb {
                Some(fb) => self.add_texture(up, fb.width, fb.height, &fb.mip_refs(), true, Sampling::World)?,
                None => NO_TEX,
            };
            materials.push([b, f, flags, convert::packed_size(&t)]);
            tex_info.push((t.kind, flags));
        }
        if bsp.textures.is_empty() {
            materials.push([NO_TEX, NO_TEX, 0, 0]);
            tex_info.push((TextureKind::Normal, 0));
        }
        Ok(())
    }

    /// Loads a new map (frees all previous level models/textures). `lit` is a
    /// parsed `.lit` (RGB per mono sample). Returns ids for submodels `*1..`.
    pub fn load_world(&mut self, bsp: &Bsp, lit: Option<&[u8]>) -> Result<Vec<ModelId>> {
        Ok(self.load_level(LevelBsp::new(bsp, lit), &[])?.0)
    }

    /// `load_world` plus external brush models (`maps/b_*.bsp`, each with
    /// its own textures and lightmaps, drawn like submodels) and external
    /// replacement textures. Returns (world submodel ids `*1..`, one id per
    /// entry of `brushes`).
    pub fn load_level(&mut self, level: LevelBsp, brushes: &[LevelBsp]) -> Result<(Vec<ModelId>, Vec<ModelId>)> {
        let bsp = level.bsp;
        self.free_level();
        self.water_fx.reset();
        let mut cpu = world::World::build_ext(bsp, level.lit, level.hdr_lit, level.deluxe);
        let world_subs = cpu.submodels.len();
        let mut up = self.gpu.begin_upload()?;
        let mut materials: Vec<[u32; 4]> = Vec::new();
        let mut tex_info = Vec::new();
        self.upload_bsp_textures(&mut up, level, &mut materials, &mut tex_info)?;
        let mut brush_ids = Vec::new();
        for b in brushes {
            let first = cpu.add_brush_ext(b.bsp, b.lit, b.hdr_lit, b.deluxe, materials.len() as u32);
            self.upload_bsp_textures(&mut up, *b, &mut materials, &mut tex_info)?;
            self.models.push(Model::Brush(first));
            brush_ids.push(ModelId(self.models.len() as u32 - 1));
        }
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let vbytes = bytes_of(&cpu.vertices);
        // ray-traced reflections build BLASes from it and read hit vertices
        let rt_usage = if self.gpu.rt.is_some() {
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR | vk::BufferUsageFlags::STORAGE_BUFFER
        } else {
            vk::BufferUsageFlags::empty()
        };
        let vbuf = self.gpu.buffer(
            vbytes.len() as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | rt_usage,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, vbuf.buf, 0, vbytes)?;
        let samples = self
            .gpu
            .buffer((cpu.samples.len() * 4) as u64, usage, false)?;
        self.gpu
            .upload_buffer(&mut up, samples.buf, 0, bytes_of(&cpu.samples))?;
        let dlx_slice: &[u32] = if cpu.deluxe_samples.is_empty() {
            &[0x0080_8080]
        } else {
            &cpu.deluxe_samples
        };
        let deluxe_samples = self
            .gpu
            .buffer((dlx_slice.len() * 4) as u64, usage, false)?;
        self.gpu
            .upload_buffer(&mut up, deluxe_samples.buf, 0, bytes_of(dlx_slice))?;
        let surfs = self.gpu.buffer(
            std::mem::size_of_val(cpu.lm_surfs.as_slice()) as u64,
            usage,
            false,
        )?;
        self.gpu
            .upload_buffer(&mut up, surfs.buf, 0, bytes_of(&cpu.lm_surfs))?;
        let mats = self
            .gpu
            .buffer((materials.len() * 16) as u64, usage, false)?;
        self.gpu
            .upload_buffer(&mut up, mats.buf, 0, bytes_of(&materials))?;
        let tex_has_fb: Vec<bool> = (0..materials.len())
            .map(|t| {
                if materials[t][1] != NO_TEX {
                    return true;
                }
                cpu.anims.get(t).is_some_and(|(prim, alts, _)| {
                    prim.iter()
                        .chain(alts)
                        .any(|&f| materials.get(f as usize).is_some_and(|m| m[1] != NO_TEX))
                })
            })
            .collect();
        let (cull, water_marks) = self.build_world_cull(&mut up, &cpu, &tex_info, &tex_has_fb)?;
        let brush_has_water: Vec<bool> = cpu
            .submodels
            .iter()
            .enumerate()
            .map(|(i, sm)| {
                i > 0
                    && (sm.first_surface..sm.first_surface + sm.num_surfaces).any(|si| {
                        cpu.surfaces.get(si as usize).is_some_and(|s| {
                            s.kind == TextureKind::Liquid
                                && tex_info.get(s.texture as usize).is_some_and(|t| matches!(t.1 & 0xf, 1 | 3))
                        })
                    })
            })
            .collect();
        let has_brush_water = brush_has_water.iter().any(|&b| b);
        self.gpu.end_upload(up)?;
        if let Some(old) = self.lightmap.take() {
            self.gpu.free_image(old);
        }
        let lm_layers = cpu.atlas_layers * if cpu.has_deluxe { 2 } else { 1 };
        self.lightmap = Some(Self::make_lightmap(
            &mut self.gpu,
            world::ATLAS,
            lm_layers,
        )?);
        let mut ids = Vec::new();
        for i in 1..world_subs {
            self.models.push(Model::Brush(i as u32));
            ids.push(ModelId(self.models.len() as u32 - 1));
        }
        self.buckets = vec![Vec::new(); tex_info.len()];
        self.world = Some(WorldGpu {
            portals: crate::portals::discover(bsp, &cpu),
            draw_surfaces: cpu.surfaces.iter().map(DrawSurface::from).collect(),
            cpu,
            vbuf,
            samples,
            deluxe_samples,
            surfs,
            materials: mats,
            tex_info,
            tex_has_fb,
            water_marks,
            brush_has_water,
            has_brush_water,
            submodel_ids: ids.clone(),
            cull,
            scratch_bits: Vec::new(),
            scratch_cmds: Vec::new(),
        });
        self.write_static_descriptors();
        Ok((ids, brush_ids))
    }

    /// Uploads cull.comp's static inputs. Draw commands are one per batch
    /// texture used by world leaf surfaces, ordered opaque, sky, liquids by
    /// type, each owning a region of the output index buffer sized for all
    /// its surfaces.
    pub(super) fn build_world_cull(
        &mut self,
        up: &mut gpu::Upload,
        cpu: &world::World,
        tex_info: &[(TextureKind, u32)],
        tex_has_fb: &[bool],
    ) -> Result<(WorldCull, Vec<(u32, u32)>)> {
        let (marks, mut surfs) = cpu.gpu_cull(|_| 0);
        let mut seen = vec![false; cpu.surfaces.len()];
        let mut cap: std::collections::BTreeMap<u32, u32> = Default::default();
        let mut water_marks: Vec<(u32, u32)> = Vec::new();
        for &[li, si] in &marks {
            if let Some(s) = cpu.surfaces.get(si as usize)
                && s.kind == TextureKind::Liquid
                && tex_info.get(s.texture as usize).is_some_and(|t| matches!(t.1 & 0xf, 1 | 3))
            {
                water_marks.push((li, si));
            }
            if !std::mem::replace(&mut seen[si as usize], true) {
                *cap.entry(cpu.slot_texture(si)).or_default() += cpu.surfaces[si as usize].num_indices;
            }
        }
        let cat = |t: u32| match tex_info.get(t as usize).map_or(TextureKind::Normal, |i| i.0) {
            TextureKind::Sky => 1,
            TextureKind::Liquid => 1 + (tex_info[t as usize].1 & 0xf).clamp(1, 4) as usize,
            _ => 0,
        };
        let has_fb = |t: u32| tex_has_fb.get(t as usize).copied().unwrap_or(false);
        let mut order: Vec<u32> = cap.keys().copied().collect();
        // Put fullbright-bearing opaque textures first so the bloom glow pass
        // can draw only `opaque_fb` without touching non-glowing world geometry.
        order.sort_by_key(|&t| {
            let c = cat(t);
            (c, c == 0 && !has_fb(t), t)
        });
        let cats: [std::ops::Range<u32>; 6] = std::array::from_fn(|c| {
            let a = order.iter().position(|&t| cat(t) >= c).unwrap_or(order.len()) as u32;
            let b = order.iter().position(|&t| cat(t) > c).unwrap_or(order.len()) as u32;
            a..b
        });
        let fb_end = cats[0].start
            + order[cats[0].start as usize..cats[0].end as usize]
                .iter()
                .take_while(|&&t| has_fb(t))
                .count() as u32;
        let opaque_fb = cats[0].start..fb_end;
        let mut cmd_of = std::collections::HashMap::new();
        let mut cmd_first = Vec::with_capacity(order.len());
        let mut total = 0u32;
        for (i, &t) in order.iter().enumerate() {
            cmd_of.insert(t, i as u32);
            cmd_first.push(total);
            total += cap[&t];
        }
        for (si, s) in surfs.iter_mut().enumerate() {
            if seen[si] {
                s.center[3] = f32::from_bits(cmd_of[&cpu.slot_texture(si as u32)]);
            }
        }
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let mut make = |data: &[u8]| -> Result<Buffer> {
            let b = self.gpu.buffer(data.len() as u64, usage, false)?;
            self.gpu.upload_buffer(up, b.buf, 0, data)?;
            Ok(b)
        };
        let marks_b = make(bytes_of(&marks))?;
        let surfs_b = make(bytes_of(&surfs))?;
        let src_b = make(bytes_of(&cpu.indices))?;
        let frame_b = make(&vec![0u8; cpu.surfaces.len().max(1) * 4])?;
        let mut out = Vec::new();
        let mut cmdbuf = Vec::new();
        for _ in 0..FRAMES {
            cmdbuf.push(self.gpu.buffer(
                order.len() as u64 * 20 + 16 + cpu.leafs_len().div_ceil(32) as u64 * 4,
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                false,
            )?);
            out.push(self.gpu.buffer(
                total.max(1) as u64 * 4,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                false,
            )?);
        }
        Ok((WorldCull {
            marks: marks_b,
            surfs: surfs_b,
            src: src_b,
            surf_frame: frame_b,
            out,
            cmdbuf,
            nmarks: marks.len() as u32,
            cmd_tex: order,
            cmd_first,
            cats,
            opaque_fb,
            framecount: 0,
        }, water_marks))
    }
}
