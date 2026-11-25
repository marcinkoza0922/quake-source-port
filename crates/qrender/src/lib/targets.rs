use super::*;

/// Most bloom chain levels (half resolution down to 1/128).
const BLOOM_LEVELS: usize = 7;

impl Renderer {
    pub(super) fn destroy_targets(&mut self) {
        if let Some(t) = self.targets.take() {
            for (img, slot) in t.portals {
                self.textures[slot as usize] = None;
                self.gpu.free_image(img);
            }
            if let Some(d) = t.portal_depth {
                self.gpu.free_image(d);
            }
            self.textures[t.color_slot as usize] = None;
            self.textures[t.ao_slot as usize] = None;
            self.textures[t.vel_slot as usize] = None;
            if let Some(src) = t.ao_source {
                self.textures[src.slot as usize] = None;
                for v in src.mip_views {
                    // SAFETY: device idle (destroy_targets); view belongs to src.img.
                    unsafe { self.gpu.device.destroy_image_view(v, None) };
                }
                self.gpu.free_image(src.img);
            }
            if let Some((img, slot)) = t.droplets {
                self.textures[slot as usize] = None;
                self.gpu.free_image(img);
            }
            if let Some(rt) = t.rt_refl {
                self.textures[rt.slot as usize] = None;
                self.gpu.free_image(rt.color);
            }
            if let Some(d) = t.half_depth {
                self.gpu.free_image(d);
            }
            self.gpu.free_image(t.ao);
            self.gpu.free_image(t.vel);
            self.gpu.free_image(t.color);
            self.gpu.free_image(t.depth);
            if let Some(m) = t.color_ms {
                self.gpu.free_image(m);
            }
            if let Some(b) = t.bloom {
                self.textures[b.scene_slot as usize] = None;
                for (img, slot, _, _) in b.levels {
                    self.textures[slot as usize] = None;
                    self.gpu.free_image(img);
                }
            }
            if let Some((i, b)) = t.out {
                self.gpu.free_image(i);
                self.gpu.free_buffer(b);
            }
        }
    }

    pub(super) fn create_targets(&mut self) -> Result<()> {
        self.idle();
        self.destroy_targets();
        let (w, h) = (self.width.max(1), self.height.max(1));
        let scale = self.render_scale;
        let (sw, shh) = crate::projection::scene_extent((w, h), scale);
        let color = self.gpu.image(
            sw,
            shh,
            1,
            1,
            COLOR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let color_ms = if self.msaa > 1 {
            Some(self.gpu.image_ms(
                sw,
                shh,
                1,
                1,
                COLOR_FORMAT,
                // sampled by the SSAO pass (scene alpha masks the sky)
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
                self.msaa,
            )?)
        } else {
            None
        };
        let depth = self.gpu.image_ms(
            sw,
            shh,
            1,
            1,
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
            self.msaa,
        )?;
        // Soft particles read scene depth with texelFetch while the depth
        // attachment is read-only.
        let dinfo = [vk::DescriptorImageInfo {
            sampler: self.sampler_clamp,
            image_view: depth.view,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        }];
        for f in &self.frames {
            let w = [vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(if self.msaa > 1 { 7 } else { 6 })
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&dinfo)];
            // SAFETY: device idle (create_targets); handles valid.
            unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
            if let Some(ms) = &color_ms {
                let cinfo = [vk::DescriptorImageInfo {
                    sampler: self.sampler_clamp,
                    image_view: ms.view,
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                }];
                let w = [vk::WriteDescriptorSet::default()
                    .dst_set(f.set)
                    .dst_binding(8)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&cinfo)];
                // SAFETY: as above.
                unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
            }
        }
        let slot = self.alloc_slot()?;
        let post_sampler = if (scale - 1.0).abs() < 1e-3 {
            self.sampler_clamp
        } else {
            self.sampler_linear
        };
        self.write_texture_descriptor(slot, color.view, post_sampler);
        let out = if self.swap.is_none() || self.keep_out {
            let img = self.gpu.image(
                w,
                h,
                1,
                1,
                // the post pipelines' format (the swapchain's when windowed)
                if self.swap.is_some() { self.pipes.post_format } else { COLOR_FORMAT },
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                vk::ImageAspectFlags::COLOR,
            )?;
            let buf =
                self.gpu
                    .buffer((w * h * 4) as u64, vk::BufferUsageFlags::TRANSFER_DST, true)?;
            Some((img, buf))
        } else {
            None
        };
        self.textures[slot as usize] = Some(Tex {
            image: None,
            level: false,
            sampling: Sampling::Clamp,
        });
        let fx_target = |r: &mut Self, tw, th, format, sampler| -> Result<(Image, u32)> {
            let img = r.gpu.image(
                tw,
                th,
                1,
                1,
                format,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
            )?;
            let slot = r.alloc_slot()?;
            r.write_texture_descriptor(slot, img.view, sampler);
            r.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Clamp });
            Ok((img, slot))
        };
        let (base_w, base_h) = (sw.min(w), shh.min(h));
        let (ao_w, ao_h) = if self.ssao_quality < 2 {
            ((base_w / 2).max(1), (base_h / 2).max(1))
        } else {
            (base_w.max(1), base_h.max(1))
        };
        let (ao, ao_slot) = fx_target(self, ao_w, ao_h, pipelines::AO_FORMAT, self.sampler_clamp)?;
        let (vel_w, vel_h) = if self.motion_blur_quality == 0 {
            ((base_w / 2).max(1), (base_h / 2).max(1))
        } else {
            (base_w.max(1), base_h.max(1))
        };
        let (vel, vel_slot) = fx_target(self, vel_w, vel_h, pipelines::VEL_FORMAT, self.sampler_clamp)?;
        let (portal_w, portal_h) = match self.portal_quality {
            0 => ((base_w / 2).max(1), (base_h / 2).max(1)),
            1 => (((base_w * 3) / 4).max(1), ((base_h * 3) / 4).max(1)),
            _ => (base_w.max(1), base_h.max(1)),
        };
        let mut portals = Vec::new();
        let mut portal_depth = None;
        if self.gun_targets || self.world.as_ref().is_some_and(|w| !w.portals.is_empty()) {
            let portal_sampler = if (portal_w, portal_h) == (sw, shh) {
                self.sampler_clamp
            } else {
                self.sampler_linear
            };
            for _ in 0..crate::MAX_PORTAL_VIEWS {
                portals.push(fx_target(self, portal_w, portal_h, COLOR_FORMAT, portal_sampler)?);
            }
            portal_depth = Some(self.gpu.image(
                portal_w,
                portal_h,
                1,
                1,
                DEPTH_FORMAT,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
            )?);
        }
        let bloom = match self.bloom {
            Some(quality) => Some(self.create_bloom_targets((base_w, base_h), quality, color.view)?),
            None => None,
        };
        let half_depth = if bloom.is_some() {
            Some(self.gpu.image(
                (base_w / 2).max(1),
                (base_h / 2).max(1),
                1,
                1,
                DEPTH_FORMAT,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
            )?)
        } else {
            None
        };
        self.targets = Some(Targets {
            portals,
            portal_depth,
            portal_w,
            portal_h,
            color,
            color_ms,
            depth,
            scene_w: sw,
            scene_h: shh,
            color_slot: slot,
            ao_source: None,
            droplets: None,
            rt_refl: None,
            half_depth,
            ao,
            ao_w,
            ao_h,
            ao_slot,
            vel,
            vel_w,
            vel_h,
            vel_slot,
            bloom,
            out,
        });
        Ok(())
    }

    /// Reduced-resolution bloom depth target and blur chain, from half
    /// (`quality` 1) or a quarter (0) of the output-capped `base` down to
    /// about 8 pixels.
    fn create_bloom_targets(&mut self, base: (u32, u32), quality: u32, color_view: vk::ImageView) -> Result<BloomTargets> {
        let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED;
        let slot = |r: &mut Self, view, sampler| -> Result<u32> {
            let s = r.alloc_slot()?;
            r.write_texture_descriptor(s, view, sampler);
            r.textures[s as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Clamp });
            Ok(s)
        };
        let scene_slot = slot(self, color_view, self.sampler_linear)?;
        let div = if quality == 0 { 4 } else { 2 };
        let (mut lw, mut lh) = ((base.0 / div).max(1), (base.1 / div).max(1));
        let mut levels = Vec::new();
        loop {
            let img = self.gpu.image(lw, lh, 1, 1, pipelines::BLOOM_FORMAT, usage, vk::ImageAspectFlags::COLOR)?;
            let s = slot(self, img.view, self.sampler_linear)?;
            levels.push((img, s, lw, lh));
            if levels.len() >= BLOOM_LEVELS || lw.min(lh) < 16 {
                break;
            }
            (lw, lh) = ((lw / 2).max(1), (lh / 2).max(1));
        }
        Ok(BloomTargets { scene_slot, levels })
    }

    /// Allocate the SAO linear-depth mip chain on the first SSAO frame. Keep
    /// it until a resize/sample/AO-quality change destroys scene targets.
    pub(super) fn ensure_ao_source(&mut self) -> Result<()> {
        if !self.targets.as_ref().is_some_and(|t| t.ao_source.is_none()) {
            return Ok(());
        }
        self.idle();
        let (ao_w, ao_h) = {
            let t = self.targets.as_ref().unwrap();
            (t.ao_w, t.ao_h)
        };
        let mips = (1 + ao_w.min(ao_h).max(1).ilog2()).min(5);
        let img = self.gpu.image(
            ao_w,
            ao_h,
            1,
            mips,
            pipelines::AO_SOURCE_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let mut mip_views = Vec::with_capacity(mips as usize);
        for m in 0..mips {
            let vci = vk::ImageViewCreateInfo::default()
                .image(img.image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(pipelines::AO_SOURCE_FORMAT)
                .subresource_range(gpu::range_mip(vk::ImageAspectFlags::COLOR, m, 1, 1));
            // SAFETY: valid image and mip level `m < mips`.
            mip_views.push(unsafe { self.gpu.device.create_image_view(&vci, None)? });
        }
        let slot = self.alloc_slot()?;
        self.write_texture_descriptor(slot, img.view, self.sampler_clamp);
        self.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Clamp });
        self.targets.as_mut().unwrap().ao_source = Some(AoSource { img, slot, mip_views });
        Ok(())
    }

    /// Allocate the reduced-resolution droplet field target on the first frame
    /// with active screen droplets (capped at 960×540 so 4K does 1/16 the work).
    pub(super) fn ensure_droplets(&mut self) -> Result<()> {
        if !self.targets.as_ref().is_some_and(|t| t.droplets.is_none()) {
            return Ok(());
        }
        self.idle();
        let dw = (self.width / 2).clamp(1, 960);
        let dh = (self.height / 2).clamp(1, 540);
        let img = self.gpu.image(
            dw,
            dh,
            1,
            1,
            pipelines::DROPLET_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let slot = self.alloc_slot()?;
        self.write_texture_descriptor(slot, img.view, self.sampler_linear);
        self.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Linear });
        self.targets.as_mut().unwrap().droplets = Some((img, slot));
        Ok(())
    }

    /// Allocate the half-resolution single-sampled ray-traced water reflection
    /// targets on the first RT frame (sharing `Targets::half_depth` with bloom).
    pub(super) fn ensure_rt_refl(&mut self) -> Result<()> {
        if !self.targets.as_ref().is_some_and(|t| t.rt_refl.is_none()) {
            return Ok(());
        }
        self.idle();
        let (sw, sh, need_depth) = {
            let t = self.targets.as_ref().unwrap();
            (t.scene_w.min(self.width.max(1)), t.scene_h.min(self.height.max(1)), t.half_depth.is_none())
        };
        let rw = (sw / 2).max(1);
        let rh = (sh / 2).max(1);
        let color = self.gpu.image(
            rw,
            rh,
            1,
            1,
            COLOR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        if need_depth {
            let depth = self.gpu.image(
                rw,
                rh,
                1,
                1,
                DEPTH_FORMAT,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
            )?;
            self.targets.as_mut().unwrap().half_depth = Some(depth);
        }
        let slot = self.alloc_slot()?;
        self.write_texture_descriptor(slot, color.view, self.sampler_linear);
        self.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Linear });
        self.targets.as_mut().unwrap().rt_refl = Some(RtReflTargets { color, slot, width: rw, height: rh });
        Ok(())
    }
}
