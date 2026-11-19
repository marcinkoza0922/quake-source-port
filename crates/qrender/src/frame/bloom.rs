//! r_bloom chain recording: the scene's glow attachment is filtered down
//! through [`BloomTargets::levels`] and summed back up into level 0, which
//! post.frag adds to the scene.

use crate::gpu;
use crate::pipelines::Pipelines;
use crate::{BloomTargets, Settings, bytes_of};
use ash::vk;

/// Strength at r_bloom_intensity 1: the levels are averaged, so a small
/// lamp's halo is faint without it.
const BLOOM_GAIN: f32 = 3.0;

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

/// Records the chain for one view. `region`/`scene` are the view's part of
/// the scene targets and their full size. The glow and scene colour must be
/// shader-readable; the levels are left shader-readable.
#[allow(clippy::too_many_arguments)]
pub(super) fn record(
    dev: &ash::Device,
    cmd: vk::CommandBuffer,
    p: &Pipelines,
    set: vk::DescriptorSet,
    b: &BloomTargets,
    region: [u32; 2],
    scene: [u32; 2],
    st: &Settings,
) -> BloomResult {
    let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
    let gfx = vk::PipelineBindPoint::GRAPHICS;
    let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
    // each level's share of this view (split views use the top-left part)
    let fx = region[0] as f32 / scene[0].max(1) as f32;
    let fy = region[1] as f32 / scene[1].max(1) as f32;
    let vps: Vec<[u32; 2]> = b
        .levels
        .iter()
        .map(|&(_, _, w, h)| [((w as f32 * fx).round() as u32).clamp(1, w), ((h as f32 * fy).round() as u32).clamp(1, h)])
        .collect();
    let weight = st.bloom_size.clamp(0.5, 2.0);
    let bright = st.bloom_bright.clamp(0.0, 1.0);
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
    // source UV per destination pixel, the source's valid UV extent and slot
    let mut src = (
        [fx / vps[0][0] as f32, fy / vps[0][1] as f32],
        [(region[0] as f32 - 0.5) / scene[0].max(1) as f32, (region[1] as f32 - 0.5) / scene[1].max(1) as f32],
        b.glow_slot,
    );
    for level in 0..b.levels.len() {
        let (uv_scale, uv_max, slot) = src;
        pass(level, false, p.bloom_down, BloomPush {
            uv_scale,
            // taps half a destination pixel apart: the 13-tap filter covers
            // the footprint whatever the source/destination ratio
            texel: [uv_scale[0] * 0.5, uv_scale[1] * 0.5],
            uv_max,
            src: slot,
            scene: b.scene_slot,
            mode: u32::from(level != 0),
            bright: if level == 0 { bright } else { 0.0 },
            weight: 1.0,
        });
        let (_, s, w, h) = b.levels[level];
        let [vw, vh] = vps[level];
        if let Some(&[nw, nh]) = vps.get(level + 1) {
            src = (
                [vw as f32 / w as f32 / nw as f32, vh as f32 / h as f32 / nh as f32],
                [(vw as f32 - 0.5) / w as f32, (vh as f32 - 0.5) / h as f32],
                s,
            );
        }
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
