use super::*;

impl Renderer {
    pub(super) fn destroy_targets(&mut self) {
        if let Some(t) = self.targets.take() {
            self.textures[t.color_slot as usize] = None;
            self.textures[t.ao_slot as usize] = None;
            self.textures[t.vel_slot as usize] = None;
            self.gpu.free_image(t.ao);
            self.gpu.free_image(t.vel);
            self.gpu.free_image(t.color);
            self.gpu.free_image(t.depth);
            if let Some(m) = t.color_ms {
                self.gpu.free_image(m);
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
        let sw = ((w as f32 * scale).round() as u32).clamp(1, 16384);
        let shh = ((h as f32 * scale).round() as u32).clamp(1, 16384);
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
        let out = if self.swap.is_none() {
            let img = self.gpu.image(
                w,
                h,
                1,
                1,
                COLOR_FORMAT,
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
        let fx_target = |r: &mut Self, format| -> Result<(Image, u32)> {
            let img = r.gpu.image(
                sw,
                shh,
                1,
                1,
                format,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
            )?;
            let slot = r.alloc_slot()?;
            r.write_texture_descriptor(slot, img.view, r.sampler_clamp);
            r.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Clamp });
            Ok((img, slot))
        };
        let (ao, ao_slot) = fx_target(self, pipelines::AO_FORMAT)?;
        let (vel, vel_slot) = fx_target(self, pipelines::VEL_FORMAT)?;
        self.targets = Some(Targets {
            color,
            color_ms,
            depth,
            scene_w: sw,
            scene_h: shh,
            color_slot: slot,
            ao,
            ao_slot,
            vel,
            vel_slot,
            out,
        });
        Ok(())
    }

    pub(super) fn create_swapchain(&mut self, vsync: bool) -> Result<()> {
        self.idle();
        let (Some(sf), Some(scf)) = (&self.gpu.surface_fn, &self.gpu.swapchain_fn) else {
            return Ok(());
        };
        let pd = self.gpu.pdev;
        let surface = self.gpu.surface;
        // SAFETY: valid pdev/surface.
        let caps = unsafe { sf.get_physical_device_surface_capabilities(pd, surface)? };
        // SAFETY: valid pdev/surface.
        let formats = unsafe { sf.get_physical_device_surface_formats(pd, surface)? };
        // SAFETY: valid pdev/surface.
        let modes = unsafe { sf.get_physical_device_surface_present_modes(pd, surface)? };
        let fmt = formats
            .iter()
            .find(|f| {
                matches!(
                    f.format,
                    vk::Format::B8G8R8A8_UNORM | vk::Format::R8G8B8A8_UNORM
                )
            })
            .copied()
            .unwrap_or(formats[0]);
        let mode = if vsync {
            vk::PresentModeKHR::FIFO
        } else if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else {
            vk::PresentModeKHR::FIFO
        };
        let (w, h) = (self.surface_size)();
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: w.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: h.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        let mut count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let old = self
            .swap
            .as_ref()
            .map_or(vk::SwapchainKHR::null(), |s| s.sc);
        let ci = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(count)
            .image_format(fmt.format)
            .image_color_space(fmt.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | (caps.supported_usage_flags & vk::ImageUsageFlags::TRANSFER_SRC),
            )
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(mode)
            .clipped(true)
            .old_swapchain(old);
        // SAFETY: valid surface and create info.
        let sc = unsafe { scf.create_swapchain(&ci, None)? };
        if let Some(s) = self.swap.take() {
            self.destroy_swapchain(s);
        }
        let scf = self
            .gpu
            .swapchain_fn
            .as_ref()
            .ok_or_else(|| Error::new("no swapchain"))?;
        // SAFETY: valid swapchain.
        let images = unsafe { scf.get_swapchain_images(sc)? };
        let mut views = Vec::new();
        let mut render_sems = Vec::new();
        for &img in &images {
            let vci = vk::ImageViewCreateInfo::default()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(fmt.format)
                .subresource_range(gpu::range(vk::ImageAspectFlags::COLOR, 1, 1));
            // SAFETY: valid swapchain image.
            unsafe {
                views.push(self.gpu.device.create_image_view(&vci, None)?);
                render_sems.push(
                    self.gpu
                        .device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                );
            }
        }
        self.swap = Some(Swapchain {
            sc,
            images,
            views,
            render_sems,
            vsync,
            format: fmt.format,
            readable: caps
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC),
        });
        if self.pipes.post_format != fmt.format {
            self.pipes.build(&self.gpu, fmt.format, self.msaa)?;
        }
        self.width = extent.width;
        self.height = extent.height;
        self.create_targets()?;
        self.needs_resize = false;
        Ok(())
    }

    pub(super) fn destroy_swapchain(&self, s: Swapchain) {
        // SAFETY: device idle; resources owned by `s`.
        unsafe {
            for v in s.views {
                self.gpu.device.destroy_image_view(v, None);
            }
            for sem in s.render_sems {
                self.gpu.device.destroy_semaphore(sem, None);
            }
            if let Some(f) = &self.gpu.swapchain_fn {
                f.destroy_swapchain(s.sc, None);
            }
        }
        let _ = s.images;
    }
}
