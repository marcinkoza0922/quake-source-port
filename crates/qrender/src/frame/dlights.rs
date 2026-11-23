//! Real-time dynamic lights (r_shadow_realtime_dlight). Instead of being
//! added to the lightmaps (lightmap.comp) and to alias models' light colour
//! (R_SetupAliasLighting), the frame's dynamic lights are lit per pixel by
//! `dyn_lights` in common.glsl, and the most important ones get cube shadow
//! maps (`ShadowMaps`, six D16 layers each). This module picks the lights,
//! gathers their shadow casters and records the shadow map faces.

use super::*;

use crate::rtlight::{self, RtLight};

/// A shadowed light: its cube face matrices, world casters and face UBOs.
pub(super) struct ShadowLight {
    pub(super) origin: [f32; 3],
    pub(super) radius: f32,
    pub(super) key: u32,
    /// First of its six shadow map layers.
    pub(super) layer: u32,
    pub(super) vp: [Mat4; 6],
    pub(super) world: Option<WorldDraw>,
    /// Dynamic UBO offset of each face (its `viewproj`).
    pub(super) ubo: [u32; 6],
}

/// This frame's per-pixel lights for the views of `passes`.
pub(super) fn select(dlights: &[DLight], passes: &[ViewPass], st: &Settings, shadows: usize) -> Vec<RtLight> {
    let views: Vec<_> = passes.iter().filter_map(|p| p.view.as_ref()).map(|v| (v.eye, v.frustum)).collect();
    rtlight::select(dlights, &views, st, shadows)
}

impl Renderer {
    /// The shadowed lights of `sel` with their world casters: surfaces in
    /// the PVS of the light's leaf within its radius (liquids don't cast).
    pub(super) fn build_shadow_lights(&mut self, sel: &[RtLight], dynw: &mut Dyn, time: f64, alphas: [f32; 4]) -> Vec<ShadowLight> {
        let mut out = Vec::new();
        let mut scratch = LightmapRequests::default();
        for (d, layer) in sel.iter().filter_map(|s| Some((s.light, s.layer?))) {
            let (o, r) = (d.origin, d.radius);
            let world = self.world.as_mut().map(|wg| {
                let surfs = wg.cpu.portal_surfaces(o);
                let near: Vec<u32> = surfs
                    .iter()
                    .copied()
                    .filter(|&si| {
                        let s = &wg.cpu.surfaces[si as usize];
                        (dot(o, s.normal) - s.dist).abs() < r && rtlight::box_touches_sphere(s.mins, s.maxs, o, r)
                    })
                    .collect();
                // shadow casters need no lightmaps
                scratch.begin(wg.cpu.lm_surfs.len());
                build_world_draw(wg, &mut self.buckets, &mut self.world_draw_scratch, dynw, &near, false, time, alphas, 1.0, ident(), &mut scratch)
            });
            out.push(ShadowLight {
                origin: o,
                radius: r,
                key: d.key,
                layer,
                vp: rtlight::face_vps(o, r),
                world,
                ubo: [0; 6],
            });
        }
        out
    }

    /// Renders every shadowed light's six faces (world, brush entities and
    /// alias models within its radius, except its own entity and the view
    /// model), then leaves the maps shader-readable. Without shadowed lights
    /// only the layout transition is recorded.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_shadows(
        &self,
        cmd: vk::CommandBuffer,
        set: vk::DescriptorSet,
        dynbuf: vk::Buffer,
        lights: &[ShadowLight],
        world_draws: &[WorldDraw],
        alias_draws: &[AliasDraw],
    ) {
        let Some(sm) = &self.shadow_maps else { return };
        let dev = &self.gpu.device;
        let p = &self.pipes;
        let rng = gpu::range(vk::ImageAspectFlags::DEPTH, 1, sm.layers.len() as u32);
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        if lights.is_empty() {
            gpu::barrier(dev, cmd, sm.image.image, rng, vk::ImageLayout::UNDEFINED, read);
            return;
        }
        gpu::barrier(dev, cmd, sm.image.image, rng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL);
        let gfx = vk::PipelineBindPoint::GRAPHICS;
        let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let extent = vk::Extent2D { width: sm.size, height: sm.size };
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent };
        let vp = vk::Viewport { x: 0.0, y: 0.0, width: sm.size as f32, height: sm.size as f32, min_depth: 0.0, max_depth: 1.0 };
        for l in lights {
            let touches = |c: [f32; 3], r: f32| rtlight::spheres_touch(c, r, l.origin, l.radius);
            let brushes: Vec<&WorldDraw> = world_draws
                .iter()
                .filter(|wd| wd.bounds[3] > 0.0 && wd.alpha >= 1.0 && touches([wd.bounds[0], wd.bounds[1], wd.bounds[2]], wd.bounds[3]))
                .collect();
            let models: Vec<&AliasDraw> = alias_draws
                .iter()
                .filter(|a| {
                    !a.viewmodel && !a.portal_only && (l.key == 0 || a.key != l.key) && a.push.light[3] >= 1.0 && touches(a.origin, a.radius)
                })
                .collect();
            for face in 0..6 {
                let Some(&view) = sm.layers.get(l.layer as usize + face) else { continue };
                let datt = vk::RenderingAttachmentInfo::default()
                    .image_view(view)
                    .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
                let ri = vk::RenderingInfo::default().render_area(area).layer_count(1).depth_attachment(&datt);
                let draw_world = |wd: &WorldDraw| {
                    let push = WorldPush { model: wd.model, params: [1.0, 0.0, 0.0, 0.0] };
                    // SAFETY: recording; buffers live for the frame.
                    unsafe {
                        dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                        dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[push]));
                        for r in [wd.opaque, wd.sky] {
                            if r.count > 0 {
                                dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                            }
                        }
                    }
                };
                // SAFETY: recording command buffer; the layer view, pipelines,
                // set and buffers are alive for the frame.
                unsafe {
                    dev.cmd_begin_rendering(cmd, &ri);
                    dev.cmd_set_viewport(cmd, 0, &[vp]);
                    dev.cmd_set_scissor(cmd, 0, &[area]);
                    dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[set], &[l.ubo[face]]);
                    // slope-scaled bias against acne (with the lookup's normal offset)
                    dev.cmd_set_depth_bias(cmd, 2.0, 0.0, 2.0);
                    if let Some(wg) = &self.world {
                        dev.cmd_bind_pipeline(cmd, gfx, p.shadow_world);
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                        if let Some(wd) = &l.world {
                            draw_world(wd);
                        }
                        for wd in &brushes {
                            draw_world(wd);
                        }
                    }
                    if !models.is_empty() {
                        dev.cmd_bind_pipeline(cmd, gfx, p.shadow_alias);
                    }
                    for a in &models {
                        let Some(Model::Alias(am)) = self.models.get(a.model) else { continue };
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                        dev.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                        dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[a.push]));
                        dev.cmd_draw_indexed(cmd, a.num_indices, 1, a.first_index, 0, 0);
                    }
                    dev.cmd_end_rendering(cmd);
                }
            }
        }
        gpu::barrier(dev, cmd, sm.image.image, rng, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL, read);
    }
}
