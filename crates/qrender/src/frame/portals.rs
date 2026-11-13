use super::*;
pub(super) struct PortalDraw {
    pub portal: crate::portals::Portal,
    pub world: WorldDraw,
    pub ubo: u32,
}
impl Renderer {
    pub(super) fn prepare_portals(
        &mut self,
        view: Option<&ViewSetup>,
        dynw: &mut Dyn,
        time: f64,
        alphas: [f32; 4],
        lm: &mut Vec<u32>,
    ) -> Vec<PortalDraw> {
        let (Some(v), Some(w)) = (view, self.world.as_mut()) else {
            return Vec::new();
        };
        let mut portals: Vec<_> = w
            .portals
            .iter()
            .filter(|p| {
                dot(sub(v.eye, p.center), p.normal) > 1.0
                    && box_in_frustum(&v.frustum, p.mins, p.maxs)
            })
            .cloned()
            .collect();
        portals.sort_by(|a, b| {
            dot(sub(a.center, v.eye), sub(a.center, v.eye))
                .total_cmp(&dot(sub(b.center, v.eye), sub(b.center, v.eye)))
        });
        portals.truncate(4);
        portals
            .into_iter()
            .filter_map(|portal| {
                let ubo = dynw.push(&[0u8; 2048], 256)? as u32;
                let surfs = w.cpu.portal_surfaces(portal.destination);
                let world = build_world_draw(
                    w,
                    &mut self.buckets,
                    dynw,
                    &surfs,
                    false,
                    time,
                    alphas,
                    1.0,
                    ident(),
                    lm,
                );
                Some(PortalDraw { portal, world, ubo })
            })
            .collect()
    }
    /// Record bounded, nonrecursive destination views using the scene's MSAA/depth scratch.
    pub(super) unsafe fn draw_portals(
        &self,
        cmd: vk::CommandBuffer,
        portals: &[PortalDraw],
        brushes: &[WorldDraw],
        aliases: &[AliasDraw],
        fi: usize,
        height: u32,
    ) {
        let Some(t) = &self.targets else { return };
        let Some(w) = &self.world else { return };
        let d = &self.gpu.device;
        let p = &self.pipes;
        let f = &self.frames[fi];
        let gfx = vk::PipelineBindPoint::GRAPHICS;
        let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
        // SAFETY: caller is recording a command buffer outside a rendering instance;
        // all resources belong to this renderer and survive its submission.
        unsafe {
            for (i, portal) in portals.iter().enumerate() {
                let img = &t.portals[i].0;
                gpu::barrier(
                    d,
                    cmd,
                    img.image,
                    crng,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                );
                let mut color = vk::RenderingAttachmentInfo::default()
                    .image_view(img.view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    });
                if let Some(ms) = &t.color_ms {
                    color = color
                        .image_view(ms.view)
                        .resolve_mode(vk::ResolveModeFlags::AVERAGE)
                        .resolve_image_view(img.view)
                        .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                }
                let colors = [color];
                let depth = vk::RenderingAttachmentInfo::default()
                    .image_view(t.depth.view)
                    .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    });
                let area = vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: t.scene_w,
                        height,
                    },
                };
                d.cmd_begin_rendering(
                    cmd,
                    &vk::RenderingInfo::default()
                        .render_area(area)
                        .layer_count(1)
                        .color_attachments(&colors)
                        .depth_attachment(&depth),
                );
                d.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: t.scene_w as f32,
                        height: height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                d.cmd_set_scissor(cmd, 0, &[area]);
                d.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[f.set], &[portal.ubo]);
                d.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                d.cmd_bind_vertex_buffers(cmd, 0, &[w.vbuf.buf], &[0]);
                for wd in std::iter::once(&portal.world).chain(brushes.iter().skip(1)) {
                    d.cmd_bind_index_buffer(
                        cmd,
                        wd.ibuf.unwrap_or(f.dynbuf.buf),
                        0,
                        vk::IndexType::UINT32,
                    );
                    d.cmd_push_constants(
                        cmd,
                        p.layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        bytes_of(&[WorldPush {
                            model: wd.model,
                            params: [wd.alpha, 0.0, 0.0, 0.0],
                        }]),
                    );
                    for (range, pipe) in [(wd.opaque, p.world), (wd.sky, p.sky)].into_iter().chain(
                        wd.water
                            .iter()
                            .chain(&wd.water_alpha)
                            .map(|r| (*r, p.water)),
                    ) {
                        if range.count == 0 {
                            continue;
                        }
                        d.cmd_bind_pipeline(cmd, gfx, pipe);
                        d.cmd_draw_indexed_indirect(
                            cmd,
                            wd.cbuf.unwrap_or(f.dynbuf.buf),
                            range.ofs,
                            range.count,
                            20,
                        );
                    }
                }
                for a in aliases.iter().filter(|a| !a.viewmodel) {
                    let Some(Model::Alias(am)) = self.models.get(a.model) else {
                        continue;
                    };
                    d.cmd_bind_pipeline(cmd, gfx, p.alias);
                    d.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                    d.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                    d.cmd_push_constants(
                        cmd,
                        p.layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        bytes_of(&[a.push]),
                    );
                    d.cmd_draw_indexed(cmd, am.num_indices, 1, 0, 0, 0);
                }
                d.cmd_end_rendering(cmd);
                gpu::barrier(
                    d,
                    cmd,
                    img.image,
                    crng,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
                let mb = [vk::MemoryBarrier::default()
                    .src_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )];
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    vk::DependencyFlags::empty(),
                    &mb,
                    &[],
                    &[],
                );
            }
        }
    }
}
