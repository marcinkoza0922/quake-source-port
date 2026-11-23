//! r_bloom: narrow single-sampled glow pass directly into level 0, followed by
//! the downsample/upsample blur chain (`bloom.frag`) summed back into level 0
//! for `post.frag`.

use crate::frame::types::{AliasDraw, FteDraw, FteQuadDraw, GUN_PORTAL_MODE, PolyPush, Range, SpriteDraw, Trans, WorldDraw, WorldPush};
use crate::frame::{GEL_FAST_MODE, GEL_MODE};
use crate::gpu;
use crate::pipelines::{Pipelines, bloom_fte_index};
use crate::{BloomTargets, Model, NO_TEX, Settings, WorldGpu, bytes_of};
use ash::vk;

/// Strength at r_bloom_intensity 1: the levels are averaged, so a small
/// lamp's halo is faint without it.
const BLOOM_GAIN: f32 = 3.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct BloomDepthPush {
    scale: [f32; 2],
    max_px: [i32; 2],
    msaa: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BloomFtePush {
    fogmix: [f32; 4],
    tex: u32,
    msaa: u32,
    scale: [f32; 2],
    max_px: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BloomPush {
    uv_scale: [f32; 2],
    texel: [f32; 2],
    uv_max: [f32; 2],
    src: u32,
    scene: u32,
    mode: u32,
    bright: f32,
    weight: f32,
}

/// What post.frag needs to add the bloom: level 0's slot, the UV scale from
/// the view's [0, 1] to its region in level 0, and the strength (intensity
/// over the summed level weights).
pub(super) struct BloomResult {
    pub(super) slot: u32,
    pub(super) uv: [f32; 2],
    pub(super) amount: f32,
}

/// Inputs for the narrow glow pass (the view's already-culled draw lists).
pub(super) struct GlowScene<'a> {
    pub(super) world: Option<&'a WorldGpu>,
    pub(super) models: &'a [Model],
    pub(super) dynbuf: vk::Buffer,
    pub(super) world_draws: &'a [WorldDraw],
    pub(super) alias_draws: &'a [AliasDraw],
    pub(super) sprite_draws: &'a [SpriteDraw],
    pub(super) trans: &'a [(f32, Trans)],
    pub(super) particles: Range,
    pub(super) fte: Option<&'a FteDraw>,
    pub(super) fte_quads: Option<&'a FteQuadDraw>,
    pub(super) soft_enabled: bool,
    pub(super) samples: u32,
}

/// Records the depth downsample, narrow single-sampled glow pass into level 0,
/// optional `r_bloom_bright` overlay, and the blur chain for one view.
/// Scene depth must be in `DEPTH_STENCIL_READ_ONLY_OPTIMAL` and scene colour
/// in `SHADER_READ_ONLY_OPTIMAL`; the bloom levels are left shader-readable.
#[allow(clippy::too_many_arguments)]
pub(super) fn record(
    dev: &ash::Device,
    cmd: vk::CommandBuffer,
    p: &Pipelines,
    set: vk::DescriptorSet,
    ubo_ofs: u32,
    b: &BloomTargets,
    region: [u32; 2],
    scene: [u32; 2],
    st: &Settings,
    gs: &GlowScene<'_>,
) -> BloomResult {
    let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
    let drng = gpu::range(vk::ImageAspectFlags::DEPTH, 1, 1);
    let gfx = vk::PipelineBindPoint::GRAPHICS;
    let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
    let fx = region[0] as f32 / scene[0].max(1) as f32;
    let fy = region[1] as f32 / scene[1].max(1) as f32;
    let nlevels = b.levels.len();
    let mut vps_arr = [[1u32; 2]; 8];
    for (dst, &(_, _, w, h)) in vps_arr.iter_mut().zip(&b.levels) {
        *dst = [((w as f32 * fx).round() as u32).clamp(1, w), ((h as f32 * fy).round() as u32).clamp(1, h)];
    }
    let vps = &vps_arr[..nlevels];
    let weight = st.bloom_size.clamp(0.5, 2.0);
    let bright = st.bloom_bright.clamp(0.0, 1.0);

    record_glow_pass(dev, cmd, p, set, ubo_ofs, b, region, vps[0], st, gs, crng, drng);

    let pass = |level: usize, load: bool, pipe: vk::Pipeline, push: BloomPush| {
        let (img, _, w, h) = &b.levels[level];
        let [vw, vh] = vps[level];
        let old = if load { vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL } else { vk::ImageLayout::UNDEFINED };
        gpu::barrier(dev, cmd, img.image, crng, old, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let att = [vk::RenderingAttachmentInfo::default()
            .image_view(img.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(if load { vk::AttachmentLoadOp::LOAD } else { vk::AttachmentLoadOp::CLEAR })
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } })];
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: *w, height: *h } };
        let ri = vk::RenderingInfo::default().render_area(area).layer_count(1).color_attachments(&att);
        let vp = vk::Viewport { x: 0.0, y: 0.0, width: vw as f32, height: vh as f32, min_depth: 0.0, max_depth: 1.0 };
        let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: vw, height: vh } };
        // SAFETY: recording command buffer; the level image, pipelines and
        // set are alive for the frame and in the layouts named above.
        unsafe {
            dev.cmd_begin_rendering(cmd, &ri);
            dev.cmd_set_viewport(cmd, 0, &[vp]);
            dev.cmd_set_scissor(cmd, 0, &[scissor]);
            dev.cmd_bind_pipeline(cmd, gfx, pipe);
            dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[set], &[0]);
            dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[push]));
            dev.cmd_draw(cmd, 3, 1, 0, 0);
            dev.cmd_end_rendering(cmd);
        }
        gpu::barrier(dev, cmd, img.image, crng, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    };

    // Optional DarkPlaces-style bright scene contribution added onto level 0.
    if bright > 0.0 {
        let [vw0, vh0] = vps[0];
        let uv_scale = [fx / vw0 as f32, fy / vh0 as f32];
        pass(0, true, p.bloom_up, BloomPush {
            uv_scale,
            texel: uv_scale,
            uv_max: [(region[0] as f32 - 0.5) / scene[0].max(1) as f32, (region[1] as f32 - 0.5) / scene[1].max(1) as f32],
            src: 0,
            scene: b.scene_slot,
            mode: 0,
            bright,
            weight: 1.0,
        });
    }

    // Downsample levels 1..N (level 0 is already filled by the narrow glow pass).
    for level in 1..b.levels.len() {
        let (_, prev_slot, pw, ph) = b.levels[level - 1];
        let [pvw, pvh] = vps[level - 1];
        let [vw, vh] = vps[level];
        let uv_scale = [pvw as f32 / pw as f32 / vw as f32, pvh as f32 / ph as f32 / vh as f32];
        pass(level, false, p.bloom_down, BloomPush {
            uv_scale,
            texel: [uv_scale[0] * 0.5, uv_scale[1] * 0.5],
            uv_max: [(pvw as f32 - 0.5) / pw as f32, (pvh as f32 - 0.5) / ph as f32],
            src: prev_slot,
            scene: b.scene_slot,
            mode: 1,
            bright: 0.0,
            weight: 1.0,
        });
    }

    // level i += weight × tent(level i + 1), from the smallest up
    for level in (0..b.levels.len().saturating_sub(1)).rev() {
        let (_, s, w, h) = b.levels[level + 1];
        let [sw, sh] = vps[level + 1];
        let [vw, vh] = vps[level];
        pass(level, true, p.bloom_up, BloomPush {
            uv_scale: [sw as f32 / w as f32 / vw as f32, sh as f32 / h as f32 / vh as f32],
            texel: [1.0 / w as f32, 1.0 / h as f32],
            uv_max: [(sw as f32 - 0.5) / w as f32, (sh as f32 - 0.5) / h as f32],
            src: s,
            scene: b.scene_slot,
            mode: 2,
            bright: 0.0,
            weight,
        });
    }
    let total: f32 = (0..b.levels.len()).map(|i| weight.powi(i as i32)).sum();
    let (_, slot, w, h) = b.levels[0];
    BloomResult {
        slot,
        uv: [vps[0][0] as f32 / w as f32, vps[0][1] as f32 / h as f32],
        amount: st.bloom.clamp(0.0, 4.0) * BLOOM_GAIN / total.max(1.0),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_glow_pass(
    dev: &ash::Device,
    cmd: vk::CommandBuffer,
    p: &Pipelines,
    set: vk::DescriptorSet,
    ubo_ofs: u32,
    b: &BloomTargets,
    region: [u32; 2],
    vp0: [u32; 2],
    st: &Settings,
    gs: &GlowScene<'_>,
    crng: vk::ImageSubresourceRange,
    drng: vk::ImageSubresourceRange,
) {
    let gfx = vk::PipelineBindPoint::GRAPHICS;
    let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
    let (img0, _, w0, h0) = &b.levels[0];
    let [vw0, vh0] = vp0;
    let area_rt = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: *w0, height: *h0 } };
    let vp = vk::Viewport { x: 0.0, y: 0.0, width: vw0 as f32, height: vh0 as f32, min_depth: 0.0, max_depth: 1.0 };
    let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: vw0, height: vh0 } };
    let scale = [region[0] as f32 / vw0 as f32, region[1] as f32 / vh0 as f32];
    let max_px = [region[0].max(1) as i32 - 1, region[1].max(1) as i32 - 1];
    let bg = &p.bloom_glow;

    // 1. Downsample scene depth (sample 0) into single-sampled b.depth.
    gpu::barrier(dev, cmd, b.depth.image, drng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL);
    let datt_w = vk::RenderingAttachmentInfo::default()
        .image_view(b.depth.view)
        .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE);
    let ri_d = vk::RenderingInfo::default().render_area(area_rt).layer_count(1).depth_attachment(&datt_w);
    // SAFETY: recording command buffer; b.depth, pipelines, and descriptor set are alive for this frame.
    unsafe {
        dev.cmd_begin_rendering(cmd, &ri_d);
        dev.cmd_set_viewport(cmd, 0, &[vp]);
        dev.cmd_set_scissor(cmd, 0, &[scissor]);
        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
        dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[set], &[ubo_ofs]);
        dev.cmd_bind_pipeline(cmd, gfx, bg.depth);
        dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[BloomDepthPush { scale, max_px, msaa: gs.samples }]));
        dev.cmd_draw(cmd, 3, 1, 0, 0);
        dev.cmd_end_rendering(cmd);
    }
    gpu::barrier(dev, cmd, b.depth.image, drng, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);

    // 2. Render only glowing draws into b.levels[0] tested against b.depth.
    gpu::barrier(dev, cmd, img0.image, crng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let catt = [vk::RenderingAttachmentInfo::default()
        .image_view(img0.view)
        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } })];
    let datt_r = vk::RenderingAttachmentInfo::default()
        .image_view(b.depth.view)
        .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
        .load_op(vk::AttachmentLoadOp::LOAD)
        .store_op(vk::AttachmentStoreOp::DONT_CARE);
    let ri_g = vk::RenderingInfo::default()
        .render_area(area_rt)
        .layer_count(1)
        .color_attachments(&catt)
        .depth_attachment(&datt_r);

    // SAFETY: all buffers, images, pipelines, and descriptor sets belong to the current frame.
    unsafe {
        dev.cmd_begin_rendering(cmd, &ri_g);
        dev.cmd_set_viewport(cmd, 0, &[vp]);
        dev.cmd_set_scissor(cmd, 0, &[scissor]);
        dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[set], &[ubo_ofs]);
        let push = |data: &[u8]| dev.cmd_push_constants(cmd, p.layout, all, 0, data);

        let draw_world = |wd: &WorldDraw, cats: &[(Range, vk::Pipeline)]| {
            if cats.iter().all(|(r, _)| r.count == 0) {
                return;
            }
            dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(gs.dynbuf), 0, vk::IndexType::UINT32);
            push(bytes_of(&[WorldPush { model: wd.model, params: [wd.alpha, 0.0, 0.0, 0.0] }]));
            if wd.zfix {
                dev.cmd_set_depth_bias(cmd, -6.0, 0.0, -1.0);
            } else {
                dev.cmd_set_depth_bias(cmd, -2.0, 0.0, -1.0);
            }
            for (r, pipe) in cats {
                if r.count > 0 {
                    dev.cmd_bind_pipeline(cmd, gfx, *pipe);
                    dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(gs.dynbuf), r.ofs, r.count, 20);
                }
            }
        };

        let hdr_on = gs.world.is_some_and(|wg| wg.cpu.has_hdr) && st.hdr_lightmaps;
        if let Some(wg) = gs.world {
            dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
            for wd in gs.world_draws.iter().filter(|wd| wd.main_visible && wd.alpha >= 1.0) {
                let opaque_r = if hdr_on { wd.opaque } else { wd.opaque_fb };
                if wd.cbuf.is_some() {
                    // GPU-culled world: lava = [1], tele = [3] (water/slime never glow).
                    draw_world(wd, &[(opaque_r, bg.world), (wd.water[1], bg.water), (wd.water[3], bg.water)]);
                } else {
                    draw_world(wd, &[(opaque_r, bg.world), (wd.water[0], bg.water)]);
                }
            }
        }

        let draw_alias = |a: &AliasDraw, pipe: vk::Pipeline| {
            if a.push.texs[1] == NO_TEX || a.portal_only || !a.main_visible {
                return;
            }
            let Some(Model::Alias(am)) = gs.models.get(a.model) else { return };
            dev.cmd_bind_pipeline(cmd, gfx, pipe);
            dev.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
            dev.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
            push(bytes_of(&[a.push]));
            dev.cmd_draw_indexed(cmd, a.num_indices, 1, a.first_index, 0, 0);
        };
        dev.cmd_set_depth_bias(cmd, -2.0, 0.0, -1.0);
        for a in gs.alias_draws.iter().filter(|a| a.push.light[3] >= 1.0 && !a.viewmodel) {
            draw_alias(a, bg.alias);
        }

        // Translucent glowing liquids (lava/tele).
        if let Some(wg) = gs.world {
            dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
            for wd in gs.world_draws.iter().filter(|wd| wd.main_visible && wd.alpha >= 1.0) {
                if wd.cbuf.is_some() {
                    draw_world(wd, &[(wd.water_alpha[1], bg.water_alpha), (wd.water_alpha[3], bg.water_alpha)]);
                } else {
                    draw_world(wd, &[(wd.water_alpha[0], bg.water_alpha)]);
                }
            }
        }

        // Sorted translucent entities.
        for (_, tr) in gs.trans {
            match *tr {
                Trans::World(i) => {
                    if let Some(wg) = gs.world {
                        let wd = &gs.world_draws[i];
                        let opaque_r = if hdr_on { wd.opaque } else { wd.opaque_fb };
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                        draw_world(
                            wd,
                            &[
                                (opaque_r, bg.world_alpha),
                                (wd.water[0], bg.water_alpha),
                                (wd.water_alpha[0], bg.water_alpha),
                            ],
                        );
                    }
                }
                Trans::Alias(i) => {
                    dev.cmd_set_depth_bias(cmd, -2.0, 0.0, -1.0);
                    draw_alias(&gs.alias_draws[i], bg.alias_alpha);
                }
                Trans::Sprite(i) => {
                    let s = &gs.sprite_draws[i];
                    dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                    dev.cmd_bind_pipeline(cmd, gfx, bg.poly);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[gs.dynbuf], &[s.vofs]);
                    push(bytes_of(&[PolyPush { color: [1.0, 1.0, 1.0, s.alpha], tex: s.tex }]));
                    dev.cmd_draw(cmd, 6, 1, 0, 0);
                }
            }
        }

        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
        if gs.particles.count > 0 {
            dev.cmd_bind_pipeline(cmd, gfx, bg.particle);
            dev.cmd_bind_vertex_buffers(cmd, 0, &[gs.dynbuf], &[gs.particles.ofs]);
            dev.cmd_draw(cmd, 6, gs.particles.count, 0, 0);
        }

        if let Some((vofs, iofs, batches)) = gs.fte {
            let mut bound = false;
            let mut last_pipe = usize::MAX;
            for (_, slot, batch, mode) in batches {
                if *mode == GEL_MODE || *mode == GEL_FAST_MODE {
                    continue;
                }
                let Some(gp) = bloom_fte_index(batch.blend, batch.lines) else { continue };
                if !bound {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[gs.dynbuf], &[*vofs]);
                    dev.cmd_bind_index_buffer(cmd, gs.dynbuf, *iofs, vk::IndexType::UINT32);
                    bound = true;
                }
                if gp != last_pipe {
                    dev.cmd_bind_pipeline(cmd, gfx, bg.fte[gp]);
                    last_pipe = gp;
                }
                let soft = if gs.soft_enabled && batch.soft && *mode != GUN_PORTAL_MODE {
                    1.0 / st.soft_particle_distance
                } else {
                    0.0
                };
                push(bytes_of(&[BloomFtePush {
                    fogmix: [0.0, soft, 0.0, *mode],
                    tex: *slot,
                    msaa: gs.samples,
                    scale,
                    max_px,
                }]));
                dev.cmd_draw_indexed(cmd, batch.num_indices, 1, batch.first_index, 0, 0);
            }
        }

        if let Some((qofs, batches)) = gs.fte_quads {
            let mut bound = false;
            let mut last_pipe = usize::MAX;
            for (_, slot, batch, mode) in batches {
                let Some(gp) = bloom_fte_index(batch.blend, false) else { continue };
                if gp >= 4 {
                    continue;
                }
                if !bound {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[gs.dynbuf], &[*qofs]);
                    bound = true;
                }
                if gp != last_pipe {
                    dev.cmd_bind_pipeline(cmd, gfx, bg.fte_quad[gp]);
                    last_pipe = gp;
                }
                let soft = if gs.soft_enabled && batch.soft { 1.0 / st.soft_particle_distance } else { 0.0 };
                push(bytes_of(&[BloomFtePush {
                    fogmix: [0.0, soft, 0.0, *mode],
                    tex: *slot,
                    msaa: gs.samples,
                    scale,
                    max_px,
                }]));
                dev.cmd_draw(cmd, 6, batch.num_indices, 0, batch.first_index);
            }
        }

        let mut vm_vp_set = false;
        for a in gs.alias_draws.iter().filter(|a| a.viewmodel && a.push.texs[1] != NO_TEX) {
            if !vm_vp_set {
                dev.cmd_set_viewport(cmd, 0, &[vk::Viewport { max_depth: 0.3, ..vp }]);
                dev.cmd_set_depth_bias(cmd, -2.0, 0.0, -1.0);
                vm_vp_set = true;
            }
            draw_alias(a, if a.push.light[3] < 1.0 { bg.alias_alpha } else { bg.alias });
        }
        dev.cmd_end_rendering(cmd);
    }
    gpu::barrier(dev, cmd, img0.image, crng, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
}
