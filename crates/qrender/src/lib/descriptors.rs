use super::*;

impl Renderer {
    /// Rewrites every non-texture descriptor (after resource replacement; device idle).
    pub(super) fn write_static_descriptors(&mut self) {
        let dev = &self.gpu.device;
        let Some(lightmap) = &self.lightmap else {
            return;
        };
        let (samples, surfs, materials) = match &self.world {
            Some(w) => (w.samples.buf, w.surfs.buf, w.materials.buf),
            None => (self.dummy.buf, self.dummy.buf, self.dummy.buf),
        };
        let whole = |b: vk::Buffer| {
            [vk::DescriptorBufferInfo {
                buffer: b,
                offset: 0,
                range: vk::WHOLE_SIZE,
            }]
        };
        let lm_img = [vk::DescriptorImageInfo {
            sampler: self.sampler_linear,
            image_view: lightmap.view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        let lm_storage = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: lightmap.view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        for (fi, f) in self.frames.iter().enumerate() {
            let d = self.dummy.buf;
            let cull_bufs = match &self.world {
                Some(w) => [
                    w.cull.marks.buf,
                    w.cull.surfs.buf,
                    w.cull.src.buf,
                    w.cull.out[fi].buf,
                    f.dynbuf.buf,
                    w.cull.surf_frame.buf,
                    w.cull.cmdbuf[fi].buf,
                ],
                None => [d, d, d, d, f.dynbuf.buf, d, d],
            };
            let cull_info: Vec<_> = cull_bufs.iter().map(|&b| whole(b)).collect();
            let cull_writes: Vec<_> = cull_info
                .iter()
                .enumerate()
                .map(|(i, info)| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(f.cull_set)
                        .dst_binding(i as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(info)
                })
                .collect();
            // SAFETY: device idle or sets not in use; all handles valid.
            unsafe { dev.update_descriptor_sets(&cull_writes, &[]) };
            let ubo = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: 0,
                range: 1024,
            }];
            let lmdyn = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: OFS_LMDYN,
                range: 4096,
            }];
            let vis = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: OFS_VIS,
                range: MAX_VIS * 4,
            }];
            let alias = whole(self.alias_buf.buf);
            let dynv = whole(f.dynbuf.buf);
            let mats = whole(materials);
            let pal = whole(self.palette_buf.buf);
            let smp = whole(samples);
            let srf = whole(surfs);
            let w = |set, b, ty| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(b)
                    .descriptor_type(ty)
            };
            let sb = vk::DescriptorType::STORAGE_BUFFER;
            let writes = [
                w(f.set, 0, vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&ubo),
                w(f.set, 2, sb).buffer_info(&alias),
                w(f.set, 3, vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&lm_img),
                w(f.set, 4, sb).buffer_info(&mats),
                w(f.set, 5, sb).buffer_info(&pal),
                w(f.set, 9, sb).buffer_info(&dynv),
                w(f.comp_set, 0, sb).buffer_info(&smp),
                w(f.comp_set, 1, sb).buffer_info(&srf),
                w(f.comp_set, 2, sb).buffer_info(&lmdyn),
                w(f.comp_set, 3, sb).buffer_info(&vis),
                w(f.comp_set, 4, vk::DescriptorType::STORAGE_IMAGE).image_info(&lm_storage),
            ];
            // SAFETY: device idle or sets not in use; all handles valid.
            unsafe { dev.update_descriptor_sets(&writes, &[]) };
        }
    }

    pub(super) fn write_texture_descriptor(&self, slot: u32, view: vk::ImageView, sampler: vk::Sampler) {
        let info = [vk::DescriptorImageInfo {
            sampler,
            image_view: view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        for f in &self.frames {
            let w = [vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(1)
                .dst_array_element(slot)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&info)];
            // SAFETY: callers wait for device idle before registering textures.
            unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
        }
    }

    pub(super) fn sampler_for(&self, s: Sampling) -> vk::Sampler {
        match s {
            Sampling::World => self.sampler_world,
            Sampling::Nearest => self.sampler_repeat,
            Sampling::Clamp => self.sampler_clamp,
            Sampling::Linear => self.sampler_linear,
        }
    }
}
