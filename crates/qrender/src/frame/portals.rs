use super::*;
pub(super) struct PortalDraw {
    pub portal: crate::portals::Portal,
    pub world: WorldDraw,
    pub ubo: u32,
}
impl Renderer {
    /// Destination views for this frame: live add-on portals first (nearest,
    /// at most `gun_portal_max`, within `gun_portal_dist`), then teleporter
    /// windows, four at most in all.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_portals(
        &mut self,
        view: Option<&ViewSetup>,
        gun: &[crate::GunPortal],
        set: &crate::Settings,
        dynw: &mut Dyn,
        time: f64,
        alphas: [f32; 4],
        lm: &mut Vec<u32>,
    ) -> Vec<PortalDraw> {
        let (Some(v), Some(w)) = (view, self.world.as_mut()) else {
            return Vec::new();
        };
        let dist2 = |p: &crate::portals::Portal| dot(sub(p.center, v.eye), sub(p.center, v.eye));
        let facing = |p: &crate::portals::Portal| {
            dot(sub(v.eye, p.center), p.normal) > 1.0 && box_in_frustum(&v.frustum, p.mins, p.maxs)
        };
        let mut guns: Vec<_> = gun
            .iter()
            .enumerate()
            .filter_map(|(i, g)| Some(crate::portals::gun(i, g, g.exit.filter(|_| g.live)?)))
            .filter(|p| facing(p) && dist2(p) < set.gun_portal_dist * set.gun_portal_dist)
            .collect();
        guns.sort_by(|a, b| dist2(a).total_cmp(&dist2(b)));
        guns.truncate(set.gun_portal_max as usize);
        let mut portals: Vec<_> = if set.teleport_windows {
            w.portals
                .iter()
                .enumerate()
                .filter(|(i, p)| self.isolated_portal.is_none_or(|index| index == *i) && facing(p))
                .map(|(_, p)| p.clone())
                .collect()
        } else {
            Vec::new()
        };
        portals.sort_by(|a, b| dist2(a).total_cmp(&dist2(b)));
        guns.extend(portals);
        let mut portals = guns;
        portals.truncate(4);
        portals
            .into_iter()
            .filter_map(|portal| {
                let ubo = dynw.push(&[0u8; 2048], 256)? as u32;
                let surfs = w.cpu.portal_surfaces(portal.pvs);
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
        [width, height]: [u32; 2],
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
                        width,
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
                        width: width as f32,
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

/// One add-on portal oval: a quad just off the wall (uv 0..1 across it for
/// the oval mask), drawn in `fte.frag`'s portal mode with the view texture
/// `slot` (`NO_TEX`: flat colour).
pub(super) fn gun_portal_quad(
    g: &crate::GunPortal,
    slot: u32,
    v: &mut Vec<crate::FteVertex>,
    i: &mut Vec<u32>,
) -> (usize, u32, crate::FteBatch, f32) {
    let (n, u) = (g.normal, g.up);
    let r = [u[1] * n[2] - u[2] * n[1], u[2] * n[0] - u[0] * n[2], u[0] * n[1] - u[1] * n[0]];
    let [hw, hh] = g.half;
    let [cr, cg, cb] = g.color;
    let base = v.len() as u32;
    let first = i.len() as u32;
    for (s, t) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        let (a, b) = (s * 2.0 - 1.0, 1.0 - t * 2.0);
        let pos = std::array::from_fn(|k| g.origin[k] + n[k] * 0.6 + r[k] * a * hw + u[k] * b * hh);
        v.push(crate::FteVertex { pos, uv: [s, t], color: [cr, cg, cb, 255] });
    }
    i.extend([0, 1, 2, 0, 2, 3].map(|k| base + k));
    let batch = crate::FteBatch {
        pic: crate::PicId(0),
        blend: crate::FteBlend::Blend,
        lines: false,
        first_index: first,
        num_indices: 6,
        soft: false,
    };
    (crate::pipelines::fte_index(crate::FteBlend::Blend, false), slot, batch, GUN_PORTAL_MODE)
}
