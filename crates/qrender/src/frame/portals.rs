use super::*;
pub(super) struct PortalDraw {
    pub portal: crate::portals::Portal,
    pub world: WorldDraw,
    pub ubo: u32,
    pub scissor: vk::Rect2D,
    /// The view this portal is seen in (`None`: the main view).
    pub parent: Option<usize>,
    /// Nesting depth (`0`: directly visible from the main view).
    pub depth: u32,
    /// FTE batches drawn in this view (particles, gun portal ovals).
    pub fte: Vec<(usize, u32, crate::FteBatch, f32)>,
}

/// Portal viewport extent inside the portal render target for a scene region.
pub(super) fn portal_viewport(t: &Targets, [scene_w, scene_h]: [u32; 2]) -> [u32; 2] {
    [
        ((scene_w as f32 * t.portal_w as f32 / t.scene_w.max(1) as f32).round() as u32)
            .clamp(1, t.portal_w.max(1)),
        ((scene_h as f32 * t.portal_h as f32 / t.scene_h.max(1) as f32).round() as u32)
            .clamp(1, t.portal_h.max(1)),
    ]
}

/// Whether water ripples/caustics, particles and tiny distant models are skipped
/// for a portal destination view at `quality` (`0` low, `1` medium, `2` high) and `depth`.
pub(super) fn skip_portal_extras(quality: u32, depth: u32) -> bool {
    quality == 0 || (quality == 1 && depth >= 1)
}

fn portal_budgets(set: &crate::Settings) -> (usize, u32, f32) {
    match set.portal_quality.min(2) {
        0 => ((set.gun_portal_max as usize).min(2), set.gun_portal_depth.min(1), set.gun_portal_dist.min(2048.0)),
        1 => ((set.gun_portal_max as usize).min(4), set.gun_portal_depth.min(3), set.gun_portal_dist.min(4096.0)),
        _ => (set.gun_portal_max as usize, set.gun_portal_depth, set.gun_portal_dist),
    }
}
impl Renderer {
    /// Destination views for this frame: live add-on portals first (nearest,
    /// at most `gun_portal_max`, within `gun_portal_dist`), then teleporter
    /// windows, four at most in all.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_portals(
        &mut self,
        view: Option<&ViewSetup>,
        extent: [u32; 2],
        gun: &[crate::GunPortal],
        set: &crate::Settings,
        dynw: &mut Dyn,
        time: f64,
        alphas: [f32; 4],
        lm: &mut LightmapRequests,
        worlds: &mut PortalWorldCache,
    ) -> Vec<PortalDraw> {
        let (Some(v), Some(w)) = (view, self.world.as_mut()) else {
            return Vec::new();
        };
        let (portal_extent, texture_extent) = self
            .targets
            .as_ref()
            .map_or((extent, extent), |t| (portal_viewport(t, extent), [t.portal_w, t.portal_h]));
        let (gun_max, gun_depth, gun_dist) = portal_budgets(set);
        let dist2 = |p: &crate::portals::Portal| dot(sub(p.center, v.eye), sub(p.center, v.eye));
        let facing = |p: &crate::portals::Portal| {
            dot(sub(v.eye, p.center), p.normal) > 1.0 && box_in_frustum(&v.frustum, p.mins, p.maxs)
        };
        let mut guns: Vec<_> = gun
            .iter()
            .enumerate()
            .filter_map(|(i, g)| Some(crate::portals::gun(i, g, g.exit.filter(|_| g.live)?)))
            .filter(|p| facing(p) && dist2(p) < gun_dist * gun_dist)
            .collect();
        guns.sort_by(|a, b| dist2(a).total_cmp(&dist2(b)));
        guns.truncate(gun_max);
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
        // portals seen through portals, breadth first up to
        // `gun_depth` levels (children follow their parents)
        let mut views: Vec<(crate::portals::Portal, Option<usize>, u32)> =
            portals.into_iter().map(|p| (p, None, 0)).collect();
        let mut k = 0;
        while k < views.len() && views.len() < crate::MAX_PORTAL_VIEWS {
            let (outer, _, depth) = views[k].clone();
            if outer.gun.is_some() && depth < gun_depth {
                let eye = outer.to_destination(v.eye);
                let clip = outer.clip();
                let mut kids: Vec<_> = gun
                    .iter()
                    .enumerate()
                    .filter_map(|(i, g)| Some(crate::portals::gun(i, g, g.exit.filter(|_| g.live)?)))
                    .filter(|p| {
                        let d = sub(eye, p.center);
                        p.center != outer.destination
                            && dot(d, p.normal) > 1.0
                            && dot(d, d) < gun_dist * gun_dist
                            && dot([clip[0], clip[1], clip[2]], p.center) + clip[3] > 0.0
                    })
                    .map(|p| outer.through(&p))
                    .filter(|p| box_in_frustum(&v.frustum, p.mins, p.maxs))
                    .collect();
                kids.sort_by(|a, b| dist2(a).total_cmp(&dist2(b)));
                for p in kids {
                    if views.len() < crate::MAX_PORTAL_VIEWS {
                        views.push((p, Some(k), depth + 1));
                    }
                }
            }
            k += 1;
        }
        views
            .into_iter()
            .filter_map(|(portal, parent, depth)| {
                let ubo = dynw.push(&[0u8; 2048], 256)? as u32;
                let surfs = w.cpu.portal_surfaces(portal.pvs);
                let world = worlds.get_or_build(&surfs, || build_world_draw(
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
                ));
                let scissor = super::portal_scissor::aperture(&v.vp, &portal, portal_extent, texture_extent);
                Some(PortalDraw { portal, world, ubo, scissor, parent, depth, fte: Vec::new() })
            })
            .collect()
    }
    /// Record destination views into single-sampled reduced-scale portal targets,
    /// nested views first so their parents can sample them.
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_portals(
        &self,
        cmd: vk::CommandBuffer,
        portals: &[PortalDraw],
        particles: Range,
        fte: Option<(u64, u64)>,
        brushes: &[WorldDraw],
        aliases: &[AliasDraw],
        fi: usize,
        scene_extent: [u32; 2],
        scene: usize,
        view: Option<&ViewSetup>,
        portal_quality: u32,
    ) {
        let Some(t) = &self.targets else { return };
        let Some(w) = &self.world else { return };
        let d = &self.gpu.device;
        let p = &self.pipes;
        let pp = p.portal_pipes();
        let f = &self.frames[fi];
        let gfx = vk::PipelineBindPoint::GRAPHICS;
        let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
        let [width, height] = portal_viewport(t, scene_extent);
        let portal_depth = t.portal_depth.as_ref().unwrap_or(&t.depth);
        // SAFETY: caller is recording a command buffer outside a rendering instance;
        // all resources belong to this renderer and survive its submission.
        unsafe {
            if !portals.is_empty() && t.portal_depth.is_some() {
                gpu::barrier(
                    d,
                    cmd,
                    portal_depth.image,
                    gpu::range(vk::ImageAspectFlags::DEPTH, 1, 1),
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                );
            }
            for (i, portal) in portals.iter().enumerate().rev() {
                let timestamp = |stage: usize| {
                    if let Some(timing) = &self.gpu_times {
                        let query = timing.destination_base(fi, scene)
                            + (i * timing::DESTINATION_TS + stage) as u32;
                        d.cmd_write_timestamp(cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE, timing.pool, query);
                    }
                };
                timestamp(0);
                let img = &t.portals[i].0;
                gpu::barrier(
                    d,
                    cmd,
                    img.image,
                    crng,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                );
                let colors = [vk::RenderingAttachmentInfo::default()
                    .image_view(img.view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    })];
                let depth = vk::RenderingAttachmentInfo::default()
                    .image_view(portal_depth.view)
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
                d.cmd_set_scissor(cmd, 0, &[portal.scissor]);
                d.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[f.set], &[portal.ubo]);
                d.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                d.cmd_bind_vertex_buffers(cmd, 0, &[w.vbuf.buf], &[0]);
                timestamp(1);
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
                    for (range, pipe) in [(wd.opaque, pp.world), (wd.sky, pp.sky)].into_iter().chain(
                        wd.water
                            .iter()
                            .chain(&wd.water_alpha)
                            .map(|r| (*r, pp.water)),
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
                timestamp(2);
                let clip = portal.portal.clip();
                let skip_extras = skip_portal_extras(portal_quality, portal.depth);
                for a in aliases.iter().filter(|a| !a.viewmodel) {
                    if dot([clip[0], clip[1], clip[2]], a.origin) + clip[3] + a.radius < 0.0 {
                        continue;
                    }
                    if let Some(vs) = view {
                        let cull_pos = portal.portal.from_destination(a.origin);
                        if !sphere_in_frustum(&vs.frustum, cull_pos, a.radius) {
                            continue;
                        }
                        if skip_extras {
                            let dist = sub(cull_pos, vs.eye);
                            let dist2 = dot(dist, dist);
                            if dist2 > 256.0 * 256.0 && a.radius * a.radius < dist2 * (0.012 * 0.012) {
                                continue;
                            }
                        }
                    }
                    let Some(Model::Alias(am)) = self.models.get(a.model) else {
                        continue;
                    };
                    d.cmd_bind_pipeline(cmd, gfx, pp.alias);
                    d.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                    d.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                    d.cmd_push_constants(
                        cmd,
                        p.layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        bytes_of(&[a.push]),
                    );
                    d.cmd_draw_indexed(cmd, a.num_indices, 1, a.first_index, 0, 0);
                }
                timestamp(3);
                if !skip_extras && particles.count > 0 {
                    d.cmd_bind_pipeline(cmd, gfx, pp.particle);
                    d.cmd_bind_vertex_buffers(cmd, 0, &[f.dynbuf.buf], &[particles.ofs]);
                    d.cmd_draw(cmd, 6, particles.count, 0, 0);
                }
                if let Some((vofs, iofs)) = fte.filter(|_| !portal.fte.is_empty()) {
                    d.cmd_bind_vertex_buffers(cmd, 0, &[f.dynbuf.buf], &[vofs]);
                    d.cmd_bind_index_buffer(cmd, f.dynbuf.buf, iofs, vk::IndexType::UINT32);
                    for (pipe, slot, b, mode) in &portal.fte {
                        d.cmd_bind_pipeline(cmd, gfx, pp.fte[*pipe]);
                        let fogmix = if matches!(b.blend, crate::FteBlend::Blend | crate::FteBlend::BlendColour) { 1.0 } else { 0.0 };
                        d.cmd_push_constants(
                            cmd,
                            p.layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0,
                            bytes_of(&[PolyPush { color: [fogmix, 0.0, 1.0, *mode], tex: *slot }]),
                        );
                        d.cmd_draw_indexed(cmd, b.num_indices, 1, b.first_index, 0, 0);
                    }
                }
                timestamp(4);
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
                timestamp(5);
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
        soft: false, gel: false,
    };
    (crate::pipelines::fte_index(crate::FteBlend::Blend, false), slot, batch, GUN_PORTAL_MODE)
}
