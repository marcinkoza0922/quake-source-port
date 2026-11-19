//! r_bloom for the wgpu backend, a port of the Vulkan backend's
//! `frame/bloom.rs` (`shaders/bloom.wgsl`). The scene pipelines write what
//! glows to a second colour attachment (`*_glow` fragment entry points);
//! after the scene it is filtered down a chain of half-size levels and summed
//! back up into level 0, which `fs_post` adds to the scene. Split views share
//! the targets at their own offsets, so every pass clamps its taps to the
//! view's rectangle.

use super::pipes::{DrawU, UNIFORM_STRIDE};
use super::scene::Lists;
use super::{GLOW_FORMAT, Renderer, Targets};

const BLOOM_SRC: &str = include_str!("shaders/bloom.wgsl");
/// Chain levels.
const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Most levels (half resolution down to 1/128).
const BLOOM_LEVELS: usize = 7;
/// Strength at r_bloom_intensity 1 (the Vulkan backend's `BLOOM_GAIN`).
const BLOOM_GAIN: f32 = 3.0;

/// Chain pipelines (created when bloom is first turned on).
pub(super) struct BloomPipes {
    bgl: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
}

impl BloomPipes {
    pub(super) fn new(dev: &wgpu::Device, draw_bgl: &wgpu::BindGroupLayout) -> BloomPipes {
        use wgpu::TextureViewDimension as D;
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: D::D2,
                multisampled: false,
            },
            count: None,
        };
        let bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom"),
            entries: &[
                tex(0),
                tex(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom"),
            bind_group_layouts: &[Some(draw_bgl), Some(&bgl)],
            immediate_size: 0,
        });
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_SRC.into()),
        });
        let pipe = |blend| {
            super::pipes::pipeline(dev, super::pipes::Desc {
                layout: &layout,
                module: &module,
                vs: "vs_bloom",
                fs: "fs_bloom",
                buffers: &[],
                topology: wgpu::PrimitiveTopology::TriangleList,
                blend,
                depth: None,
                format: BLOOM_FORMAT,
                samples: 1,
                glow: false,
            })
        };
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let down = pipe(None);
        let up = pipe(Some(wgpu::BlendState { color: add, alpha: add }));
        BloomPipes { bgl, down, up }
    }
}

/// The glow attachment and the chain.
pub(super) struct BloomTargets {
    pub(super) glow_view: wgpu::TextureView,
    /// Multisampled glow drawn into and resolved to `glow_view` (MSAA on).
    pub(super) glow_ms_view: Option<wgpu::TextureView>,
    /// Level views and sizes.
    levels: Vec<(wgpu::TextureView, [u32; 2])>,
    /// Bind group reading the source of level `i` (the glow, then level
    /// `i - 1`) with the scene colour; `[levels.len()..]` read level `i + 1`
    /// for the upsample into level `i`.
    down_groups: Vec<wgpu::BindGroup>,
    up_groups: Vec<wgpu::BindGroup>,
}

impl BloomTargets {
    /// Level 0, which the post pass adds to the scene.
    pub(super) fn view(&self) -> &wgpu::TextureView {
        &self.levels[0].0
    }
}

/// One chain pass of every view: target level, its draws (du, rect) and
/// whether it adds onto the level (upsample).
pub(super) struct BloomPass {
    level: usize,
    up: bool,
    draws: Vec<(u32, [u32; 4])>,
}

/// Per-view post parameters: level 0 uv rectangle and strength.
pub(super) struct BloomPost {
    pub(super) uv: [f32; 4],
    pub(super) amount: f32,
}

/// `rect` of a view (in a `from`-sized target) scaled to a `to`-sized one.
fn scale_rect(rect: [u32; 4], from: [u32; 2], to: [u32; 2]) -> [u32; 4] {
    let fx = to[0] as f32 / from[0].max(1) as f32;
    let fy = to[1] as f32 / from[1].max(1) as f32;
    let x = ((rect[0] as f32 * fx) as u32).min(to[0] - 1);
    let y = ((rect[1] as f32 * fy) as u32).min(to[1] - 1);
    let w = ((rect[2] as f32 * fx).round() as u32).clamp(1, to[0] - x);
    let h = ((rect[3] as f32 * fy).round() as u32).clamp(1, to[1] - y);
    [x, y, w, h]
}

/// DrawU of a pass drawing `dst` (target pixels) from `src` (in a
/// `src_size` texture); `tap` is the tap spacing in source pixels, or None
/// for half a destination pixel (downsampling).
fn pass_draw(dst: [u32; 4], src: [u32; 4], src_size: [u32; 2], tap: Option<f32>, mode: f32, bright: f32, weight: f32) -> DrawU {
    let (sw, sh) = (src_size[0] as f32, src_size[1] as f32);
    let kx = src[2] as f32 / dst[2] as f32;
    let ky = src[3] as f32 / dst[3] as f32;
    let scale = [kx / sw, ky / sh];
    let offset = [(src[0] as f32 - dst[0] as f32 * kx) / sw, (src[1] as f32 - dst[1] as f32 * ky) / sh];
    let texel = match tap {
        Some(t) => [t / sw, t / sh],
        None => [scale[0] * 0.5, scale[1] * 0.5],
    };
    let mut model = [0.0; 16];
    model[..4].copy_from_slice(&[scale[0], scale[1], offset[0], offset[1]]);
    model[4..8].copy_from_slice(&[
        (src[0] as f32 + 0.5) / sw,
        (src[1] as f32 + 0.5) / sh,
        (src[0] + src[2]) as f32 / sw - 0.5 / sw,
        (src[1] + src[3]) as f32 / sh - 0.5 / sh,
    ]);
    model[8..12].copy_from_slice(&[texel[0], texel[1], mode, bright]);
    model[12] = weight;
    DrawU { model, ..Default::default() }
}

impl Renderer {
    /// Glow attachment at `scene` size and the chain, from half (`quality` 1)
    /// or a quarter (0) of the output-capped scene down to about 8 pixels.
    pub(super) fn create_bloom_targets(&self, quality: u32, scene: (u32, u32), color: &wgpu::TextureView) -> Option<BloomTargets> {
        let pipes = self.bloom_pipes.as_ref()?;
        let tex = |label, (width, height): (u32, u32), samples, format| {
            self.dev
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let glow_view = tex("glow", scene, 1, GLOW_FORMAT);
        let glow_ms_view = (self.msaa > 1).then(|| tex("glow ms", scene, self.msaa, GLOW_FORMAT));
        let div = if quality == 0 { 4 } else { 2 };
        let base = (scene.0.min(self.width), scene.1.min(self.height));
        let (mut lw, mut lh) = ((base.0 / div).max(1), (base.1 / div).max(1));
        let mut levels = Vec::new();
        loop {
            levels.push((tex("bloom", (lw, lh), 1, BLOOM_FORMAT), [lw, lh]));
            if levels.len() >= BLOOM_LEVELS || lw.min(lh) < 16 {
                break;
            }
            (lw, lh) = ((lw / 2).max(1), (lh / 2).max(1));
        }
        let group = |src: &wgpu::TextureView| {
            self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipes.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(color) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.samplers.linear) },
                ],
            })
        };
        let down_groups = std::iter::once(&glow_view).chain(levels.iter().map(|l| &l.0).take(levels.len() - 1)).map(group).collect();
        let up_groups = levels.iter().skip(1).map(|l| group(&l.0)).collect();
        Some(BloomTargets { glow_view, glow_ms_view, levels, down_groups, up_groups })
    }

    /// Per-view chain records and post parameters (empty when bloom is off).
    pub(super) fn bloom_views(&self, l: &mut Lists, st: &crate::Settings) -> (Vec<BloomPass>, Vec<BloomPost>) {
        let Some(t) = &self.targets else { return Default::default() };
        let Some(b) = &t.bloom else { return Default::default() };
        let scene = [t.scene.0, t.scene.1];
        let n = b.levels.len();
        let weight = st.bloom_size.clamp(0.5, 2.0);
        let bright = st.bloom_bright.clamp(0.0, 1.0);
        let mut passes: Vec<BloomPass> = (0..n).map(|level| BloomPass { level, up: false, draws: Vec::new() }).collect();
        passes.extend((0..n.saturating_sub(1)).rev().map(|level| BloomPass { level, up: true, draws: Vec::new() }));
        let total: f32 = (0..n).map(|i| weight.powi(i as i32)).sum();
        let amount = st.bloom.clamp(0.0, 4.0) * BLOOM_GAIN / total.max(1.0);
        let mut post = Vec::with_capacity(l.views.len());
        for vi in 0..l.views.len() {
            let v = &l.views[vi];
            let view = [v.ofs[0], v.ofs[1], v.scene[0].max(1), v.scene[1].max(1)];
            let rects: Vec<[u32; 4]> = b.levels.iter().map(|(_, size)| scale_rect(view, scene, *size)).collect();
            for level in 0..n {
                let (src, src_size) = if level == 0 { (view, scene) } else { (rects[level - 1], b.levels[level - 1].1) };
                // mode 0 reads the glow (+ bright scene colours), 1 a level
                let (mode, bright) = if level == 0 { (0.0, bright) } else { (1.0, 0.0) };
                let du = l.draw(pass_draw(rects[level], src, src_size, None, mode, bright, 1.0));
                passes[level].draws.push((du, rects[level]));
            }
            for (k, level) in (0..n.saturating_sub(1)).rev().enumerate() {
                let du = l.draw(pass_draw(rects[level], rects[level + 1], b.levels[level + 1].1, Some(1.0), 2.0, 0.0, weight));
                passes[n + k].draws.push((du, rects[level]));
            }
            let [x, y, w, h] = rects[0];
            let [lw, lh] = b.levels[0].1;
            let (lw, lh) = (lw as f32, lh as f32);
            post.push(BloomPost { uv: [x as f32 / lw, y as f32 / lh, (x + w) as f32 / lw, (y + h) as f32 / lh], amount });
        }
        (passes, post)
    }

    /// Records the chain for every view (after the scene, before post).
    pub(super) fn encode_bloom(&self, enc: &mut wgpu::CommandEncoder, t: &Targets, passes: &[BloomPass]) {
        let (Some(b), Some(pipes), Some(draw_group)) = (&t.bloom, &self.bloom_pipes, &self.bufs.draw_group) else { return };
        for p in passes {
            let (view, _) = &b.levels[p.level];
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if p.up { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(if p.up { &pipes.up } else { &pipes.down });
            pass.set_bind_group(1, if p.up { &b.up_groups[p.level] } else { &b.down_groups[p.level] }, &[]);
            for &(du, [x, y, w, h]) in &p.draws {
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                pass.set_bind_group(0, draw_group, &[du * UNIFORM_STRIDE as u32]);
                pass.draw(0..3, 0..1);
            }
        }
    }
}
