//! The window's swapchain.
use super::*;

impl Renderer {
    /// (Re)creates the swapchain and what depends on it. False when the window
    /// has no area (minimised on Windows): the old one is kept, to try again.
    pub(super) fn create_swapchain(&mut self, vsync: bool) -> Result<bool> {
        self.needs_resize = true; // until done
        let (Some(sf), Some(scf)) = (&self.gpu.surface_fn, &self.gpu.swapchain_fn) else {
            return Ok(true);
        };
        let pd = self.gpu.pdev;
        let surface = self.gpu.surface;
        if surface == vk::SurfaceKHR::null() {
            return Err(Error::lost(crate::Lost::Surface, "no window surface"));
        }
        // SAFETY: valid pdev/surface.
        let caps = unsafe { sf.get_physical_device_surface_capabilities(pd, surface)? };
        let Some(extent) = swap_extent(&caps, (self.surface_size)()) else {
            return Ok(false);
        };
        self.idle();
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
            .or(formats.first())
            .copied()
            .ok_or_else(|| Error::new("surface has no formats"))?;
        let mode = if vsync {
            vk::PresentModeKHR::FIFO
        } else if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else {
            vk::PresentModeKHR::FIFO
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
        let sc = unsafe { scf.create_swapchain(&ci, None) };
        // retired by the call, even when it fails
        if let Some(s) = self.swap.take() {
            self.destroy_swapchain(s);
        }
        // (owned at once, so a failure below neither leaks it nor draws to it)
        self.swap = Some(Swapchain {
            sc: sc?,
            images: Vec::new(),
            views: Vec::new(),
            render_sems: Vec::new(),
            vsync,
            format: fmt.format,
            readable: caps
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC),
        });
        let swap = self.swap.as_mut().unwrap();
        let scf = self
            .gpu
            .swapchain_fn
            .as_ref()
            .ok_or_else(|| Error::new("no swapchain"))?;
        // SAFETY: valid swapchain.
        swap.images = unsafe { scf.get_swapchain_images(swap.sc)? };
        for &img in &swap.images {
            let vci = vk::ImageViewCreateInfo::default()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(fmt.format)
                .subresource_range(gpu::range(vk::ImageAspectFlags::COLOR, 1, 1));
            // SAFETY: valid swapchain image.
            unsafe {
                swap.views.push(self.gpu.device.create_image_view(&vci, None)?);
                swap.render_sems.push(
                    self.gpu
                        .device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                );
            }
        }
        if self.pipes.post_format != fmt.format {
            self.pipes.build(&self.gpu, fmt.format, self.msaa)?;
        }
        self.width = extent.width;
        self.height = extent.height;
        self.create_targets()?;
        self.needs_resize = false;
        Ok(true)
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

/// Size for a surface's swapchain: its own, or the window's when it takes any.
/// `None` when that has no area.
pub(crate) fn swap_extent(caps: &vk::SurfaceCapabilitiesKHR, (w, h): (u32, u32)) -> Option<vk::Extent2D> {
    let e = if caps.current_extent.width != u32::MAX {
        caps.current_extent
    } else {
        if w == 0 || h == 0 {
            return None;
        }
        let (min, max) = (caps.min_image_extent, caps.max_image_extent);
        vk::Extent2D { width: w.clamp(min.width, max.width.max(min.width)), height: h.clamp(min.height, max.height.max(min.height)) }
    };
    (e.width > 0 && e.height > 0).then_some(e)
}
