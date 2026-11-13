use super::*;

impl Renderer {
    pub(super) fn init(mut gpu: Gpu, palette: &Palette, width: u32, height: u32) -> Result<Renderer> {
        let pipes = Pipelines::new(&gpu, COLOR_FORMAT, 1)?;
        let dev = gpu.device.clone();
        let sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC,
                descriptor_count: 8,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: (MAX_TEXTURES + 4) * FRAMES as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 96,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 4,
            },
        ];
        let pci = vk::DescriptorPoolCreateInfo::default()
            .max_sets(4 * FRAMES as u32)
            .pool_sizes(&sizes);
        // SAFETY: valid device.
        let desc_pool = unsafe { dev.create_descriptor_pool(&pci, None)? };
        let cpci = vk::CommandPoolCreateInfo::default()
            .queue_family_index(gpu.qfam)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: valid device.
        let cmd_pool = unsafe { dev.create_command_pool(&cpci, None)? };
        let mut frames = Vec::new();
        for _ in 0..FRAMES {
            let ai = vk::CommandBufferAllocateInfo::default()
                .command_pool(cmd_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let layouts = [pipes.set_layout, pipes.comp_set_layout, pipes.cull_set_layout];
            let dai = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(desc_pool)
                .set_layouts(&layouts);
            // SAFETY: valid device/pools.
            let (cmd, fence, acquire, sets) = unsafe {
                (
                    dev.allocate_command_buffers(&ai)?[0],
                    dev.create_fence(
                        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )?,
                    dev.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                    dev.allocate_descriptor_sets(&dai)?,
                )
            };
            let dynbuf = gpu.buffer(
                DYN_SIZE,
                vk::BufferUsageFlags::UNIFORM_BUFFER
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::INDEX_BUFFER
                    | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_SRC,
                true,
            )?;
            frames.push(FrameRes {
                cmd,
                fence,
                acquire,
                dynbuf,
                set: sets[0],
                comp_set: sets[1],
                cull_set: sets[2],
            });
        }
        let sampler =
            |filter: vk::Filter, mode: vk::SamplerAddressMode, mip: vk::SamplerMipmapMode| {
                let ci = vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .mipmap_mode(mip)
                    .address_mode_u(mode)
                    .address_mode_v(mode)
                    .address_mode_w(mode)
                    .max_lod(vk::LOD_CLAMP_NONE);
                // SAFETY: valid device.
                unsafe { dev.create_sampler(&ci, None) }
            };
        let sampler_repeat = sampler(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerMipmapMode::LINEAR,
        )?;
        let sampler_clamp = sampler(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerMipmapMode::NEAREST,
        )?;
        let sampler_linear = sampler(
            vk::Filter::LINEAR,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerMipmapMode::NEAREST,
        )?;
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let mut palette_buf = gpu.buffer(1024 + convert::LUT_BYTES as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
        let pal: Vec<u32> = palette
            .iter()
            .map(|c| c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | 0xff << 24)
            .collect();
        palette_buf.bytes_mut()[..1024].copy_from_slice(bytes_of(&pal));
        let dummy = gpu.buffer(256, storage, false)?;
        let alias_buf = gpu.buffer(4 << 20, storage, false)?;
        let lightmap = Some(Self::make_lightmap(&mut gpu, 16, 1)?);
        let mut r = Renderer {
            gpu,
            pipes,
            desc_pool,
            cmd_pool,
            frames,
            cur: 0,
            gpu_times: None,
            swap: None,
            surface_size: Box::new(move || (width, height)),
            width,
            height,
            targets: None,
            sampler_repeat,
            sampler_clamp,
            sampler_linear,
            sampler_world: sampler_repeat,
            world_filter: (usize::MAX, 0.0, 0.0),
            lut_metric: -1,
            msaa: 1,
            render_scale: 1.0,
            prev_xforms: Default::default(),
            prev_xform_time: 0.0,
            water_fx: {
                let mut w = water::WaterFx::default();
                w.set_palette(palette);
                w
            },
            textures: Vec::new(),
            palette: *palette,
            palette_buf,
            lightmap,
            dummy,
            world: None,
            models: Vec::new(),
            pics: Vec::new(),
            splash_pic: None,
            conchars: None,
            alias_cpu: Vec::new(),
            alias_buf,
            sky_layers: [NO_TEX; 2],
            skybox: None,
            buckets: Vec::new(),
            needs_resize: false,
            frame_count: 0,
            capture_requested: false,
            capture: None,
        };
        r.write_static_descriptors();
        r.splash_pic = r.register_pic_rgba_linear(32, 32, &water::spray_blob(32)).ok();
        Ok(r)
    }

    pub(super) fn make_lightmap(gpu: &mut Gpu, size: u32, layers: u32) -> Result<Image> {
        let img = gpu.image(
            size,
            size,
            layers,
            1,
            LM_FORMAT,
            vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageAspectFlags::COLOR,
        )?;
        let up = gpu.begin_upload()?;
        let rng = gpu::range(vk::ImageAspectFlags::COLOR, 1, layers);
        gpu::barrier(
            &gpu.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
        let c = vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        };
        // SAFETY: image in GENERAL layout, recording command buffer.
        unsafe {
            gpu.device.cmd_clear_color_image(
                up.cmd,
                img.image,
                vk::ImageLayout::GENERAL,
                &c,
                &[rng],
            )
        };
        gpu::barrier(
            &gpu.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::GENERAL,
        );
        gpu.end_upload(up)?;
        Ok(img)
    }
}
