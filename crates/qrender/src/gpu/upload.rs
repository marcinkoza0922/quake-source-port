use super::*;

impl Gpu {
    pub fn begin_upload(&mut self) -> Result<Upload> {
        let ai = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.upload_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: valid pool.
        let cmd = unsafe { self.device.allocate_command_buffers(&ai)? }[0];
        let bi = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        // SAFETY: fresh command buffer.
        unsafe { self.device.begin_command_buffer(cmd, &bi)? };
        Ok(Upload {
            cmd,
            staging: Vec::new(),
        })
    }

    /// Submits the upload and waits for completion.
    pub fn end_upload(&mut self, up: Upload) -> Result<()> {
        // SAFETY: recording command buffer; queue use is single threaded.
        unsafe {
            self.device.end_command_buffer(up.cmd)?;
            let cmds = [up.cmd];
            let si = [vk::SubmitInfo::default().command_buffers(&cmds)];
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)?;
            self.device.queue_submit(self.queue, &si, fence)?;
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.upload_pool, &cmds);
        }
        for b in up.staging {
            self.free_buffer(b);
        }
        Ok(())
    }

    /// Copies `data` into a device-local buffer via staging.
    pub fn upload_buffer(
        &mut self,
        up: &mut Upload,
        dst: vk::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut st = self.buffer(data.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        st.bytes_mut()[..data.len()].copy_from_slice(data);
        let r = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: offset,
            size: data.len() as u64,
        }];
        // SAFETY: recording; both buffers live until end_upload.
        unsafe { self.device.cmd_copy_buffer(up.cmd, st.buf, dst, &r) };
        up.staging.push(st);
        Ok(())
    }

    /// Uploads RGBA8 mip chain into `img` (layer 0) and transitions it to shader-read.
    pub fn upload_image(&mut self, up: &mut Upload, img: &Image, mips: &[&[u8]]) -> Result<()> {
        let total: usize = mips.iter().map(|m| m.len()).sum();
        let mut st = self.buffer(total as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        let mut regions = Vec::new();
        let mut off = 0usize;
        for (lvl, m) in mips.iter().enumerate() {
            st.bytes_mut()[off..off + m.len()].copy_from_slice(m);
            regions.push(vk::BufferImageCopy {
                buffer_offset: off as u64,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: lvl as u32,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_extent: vk::Extent3D {
                    width: (img.width >> lvl).max(1),
                    height: (img.height >> lvl).max(1),
                    depth: 1,
                },
                ..Default::default()
            });
            off += m.len();
        }
        let rng = range(vk::ImageAspectFlags::COLOR, img.mips, img.layers);
        barrier(
            &self.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        // SAFETY: image in TRANSFER_DST layout; staging lives until end_upload.
        unsafe {
            self.device.cmd_copy_buffer_to_image(
                up.cmd,
                st.buf,
                img.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            )
        };
        barrier(
            &self.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        up.staging.push(st);
        Ok(())
    }

    /// Uploads the first `rows` rows of an RGBA8 single-mip image from
    /// `data` (full-width rows) and clears the rest to transparent black,
    /// then transitions it to shader-read. Cheaper than [`Self::upload_image`]
    /// when only the top of a large image holds anything.
    pub fn upload_image_rows(&mut self, up: &mut Upload, img: &Image, data: &[u8], rows: u32) -> Result<()> {
        let rows = rows.min(img.height);
        let rng = range(vk::ImageAspectFlags::COLOR, img.mips, img.layers);
        barrier(&self.device, up.cmd, img.image, rng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
        if rows < img.height {
            let clear = vk::ClearColorValue { float32: [0.0; 4] };
            // SAFETY: image in TRANSFER_DST layout.
            unsafe { self.device.cmd_clear_color_image(up.cmd, img.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &clear, &[rng]) };
            barrier(&self.device, up.cmd, img.image, rng, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
        }
        if rows > 0 {
            let len = (img.width * rows * 4) as usize;
            let mut st = self.buffer(len as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
            st.bytes_mut()[..len].copy_from_slice(&data[..len]);
            let region = [vk::BufferImageCopy {
                image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
                image_extent: vk::Extent3D { width: img.width, height: rows, depth: 1 },
                ..Default::default()
            }];
            // SAFETY: image in TRANSFER_DST layout; staging lives until end_upload.
            unsafe { self.device.cmd_copy_buffer_to_image(up.cmd, st.buf, img.image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &region) };
            up.staging.push(st);
        }
        barrier(&self.device, up.cmd, img.image, rng, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        Ok(())
    }

    pub fn destroy(&mut self) {
        // SAFETY: device idle; all resources freed by the caller.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.upload_pool, None);
            self.alloc.destroy(&self.device);
            self.device.destroy_device(None);
            if let Some(s) = &self.surface_fn {
                s.destroy_surface(self.surface, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}
