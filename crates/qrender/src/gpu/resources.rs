//! Buffers and images. One that fails halfway frees its parts.

use super::*;

impl Gpu {
    pub fn buffer(&mut self, size: u64, mut usage: vk::BufferUsageFlags, host: bool) -> Result<Buffer> {
        let size = size.max(16);
        if self.rt.is_some() {
            // ray tracing reads geometry and model buffers by address
            usage |= vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        }
        self.journal.injected()?;
        let ci = vk::BufferCreateInfo::default().size(size).usage(usage);
        // SAFETY: valid device/create info.
        let buf = unsafe { self.device.create_buffer(&ci, None)? };
        // SAFETY: buf just created.
        let req = unsafe { self.device.get_buffer_memory_requirements(buf) };
        let flags = if host {
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        } else {
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        };
        let mem = match self.alloc.alloc(&self.device, &self.mem_props, req, flags, true) {
            Ok(m) => m,
            Err(e) => {
                // SAFETY: just created, never used.
                unsafe { self.device.destroy_buffer(buf, None) };
                return Err(e);
            }
        };
        self.journal.add(Made::Buffer(buf, mem));
        let b = Buffer { buf, mem, size };
        // SAFETY: memory satisfies requirements.
        if let Err(e) = unsafe { self.device.bind_buffer_memory(buf, mem.memory, mem.offset) } {
            self.free_buffer(b);
            return Err(e.into());
        }
        Ok(b)
    }

    /// Frees a buffer that lives as long as the renderer (teardown only;
    /// `b` must not be used afterwards).
    pub fn free_buffer_ref(&mut self, b: &Buffer) {
        self.journal.forget_buffer(b.buf);
        // SAFETY: caller guarantees the GPU no longer uses it.
        unsafe { self.device.destroy_buffer(b.buf, None) };
        self.alloc.free(&b.mem);
    }

    pub fn free_buffer(&mut self, b: Buffer) {
        self.free_buffer_ref(&b);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image(
        &mut self,
        w: u32,
        h: u32,
        layers: u32,
        mips: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
    ) -> Result<Image> {
        self.image_ms(w, h, layers, mips, format, usage, aspect, 1)
    }

    /// [`Gpu::image`] with a sample count (1 = single-sampled).
    #[allow(clippy::too_many_arguments)]
    pub fn image_ms(
        &mut self,
        w: u32,
        h: u32,
        layers: u32,
        mips: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        samples: u32,
    ) -> Result<Image> {
        self.journal.injected()?;
        let ci = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: w,
                height: h,
                depth: 1,
            })
            .mip_levels(mips)
            .array_layers(layers)
            .samples(vk::SampleCountFlags::from_raw(samples.max(1)))
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        // SAFETY: valid device/create info.
        let image = unsafe { self.device.create_image(&ci, None)? };
        // SAFETY: image just created.
        let req = unsafe { self.device.get_image_memory_requirements(image) };
        let flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        let mem = match self.alloc.alloc(&self.device, &self.mem_props, req, flags, false) {
            Ok(m) => m,
            Err(e) => {
                // SAFETY: just created, never used.
                unsafe { self.device.destroy_image(image, None) };
                return Err(e);
            }
        };
        let vci = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(
                if layers > 1 || usage.contains(vk::ImageUsageFlags::STORAGE) {
                    vk::ImageViewType::TYPE_2D_ARRAY
                } else {
                    vk::ImageViewType::TYPE_2D
                },
            )
            .format(format)
            .subresource_range(range(aspect, mips, layers));
        // SAFETY: memory satisfies requirements; the view is made once bound.
        let view = unsafe {
            self.device
                .bind_image_memory(image, mem.memory, mem.offset)
                .and_then(|()| self.device.create_image_view(&vci, None))
        };
        let view = match view {
            Ok(v) => v,
            Err(e) => {
                // SAFETY: just created, never used.
                unsafe { self.device.destroy_image(image, None) };
                self.alloc.free(&mem);
                return Err(e.into());
            }
        };
        self.journal.add(Made::Image(image, view, mem));
        Ok(Image {
            image,
            view,
            mem,
            width: w,
            height: h,
            layers,
            mips,
        })
    }

    pub fn free_image(&mut self, i: Image) {
        self.journal.forget_image(i.image);
        // SAFETY: caller guarantees the GPU no longer uses it.
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
        }
        self.alloc.free(&i.mem);
    }
}
