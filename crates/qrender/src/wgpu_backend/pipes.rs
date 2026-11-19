//! Bind group layouts and render pipelines.

use super::{DEPTH_FORMAT, GLOW_FORMAT, SCENE_FORMAT};
use crate::FteBlend;
pub(super) use super::scene_pipes::ScenePipes;

pub(super) const SCENE_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/material.wgsl"));
const SKY_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/sky.wgsl"));
const SCREEN_SRC: &str = include_str!("shaders/screen.wgsl");

/// Bytes per per-draw / per-view uniform record (dynamic offset alignment).
pub(super) const UNIFORM_STRIDE: u64 = 256;

/// Per-draw uniform record (see `DrawU` in the shaders).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct DrawU {
    pub(super) model: [f32; 16],
    pub(super) color: [f32; 4],
    pub(super) params: [f32; 4],
    pub(super) extra: [f32; 4],
    pub(super) ix: [u32; 4],
    pub(super) pad: [f32; 32],
}

impl Default for DrawU {
    fn default() -> Self {
        Self { model: super::math::ident(), color: [1.0; 4], params: [0.0; 4], extra: [0.0; 4], ix: [0; 4], pad: [0.0; 32] }
    }
}

/// Per-view uniform record (see `FrameU` in the shaders).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FrameU {
    pub(super) viewproj: [f32; 16],
    pub(super) eye: [f32; 4],
    pub(super) fog: [f32; 4],
    pub(super) vright: [f32; 4],
    pub(super) vup: [f32; 4],
    pub(super) vfwd: [f32; 4],
    pub(super) sky: [f32; 4],
    pub(super) alphas: [f32; 4],
    /// Motion velocity pass: scene px per velocity px, max scene px x, y.
    pub(super) vel: [f32; 4],
    /// Screen dither, texture dither (r_softemu / r_dither).
    pub(super) soft: [f32; 4],
    /// World dither level, alias mode, goggles mode.
    pub(super) softf: [u32; 4],
    /// Skybox wind: xyz direction (world), w loop phase.
    pub(super) wind: [f32; 4],
    pub(super) pad: [f32; 4],
}

/// Vertex of CPU-posed alias models (world space).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct AliasV {
    pub(super) pos: [f32; 3],
    pub(super) st: [f32; 2],
    pub(super) shade: [f32; 3],
    /// World-space normal (per-pixel dynamic lights).
    pub(super) nrm: [f32; 3],
}

/// 2D vertex in output pixels.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct UiV {
    pub(super) pos: [f32; 2],
    pub(super) uv: [f32; 2],
    pub(super) color: [u8; 4],
}

pub(super) struct Pipes {
    pub(super) frame_bgl: wgpu::BindGroupLayout,
    pub(super) mat_bgl: wgpu::BindGroupLayout,
    pub(super) sky_bgl: wgpu::BindGroupLayout,
    pub(super) draw_bgl: wgpu::BindGroupLayout,
    /// Post pass inputs: scene, motion velocity, sampler, palette, softemu LUT.
    pub(super) post_bgl: wgpu::BindGroupLayout,
    pub(super) scene_layout: wgpu::PipelineLayout,
    sky_layout: wgpu::PipelineLayout,
    pub(super) scene_module: wgpu::ShaderModule,
    sky_module: wgpu::ShaderModule,
    /// Scene pipelines for the current MSAA sample count.
    pub(super) scene: ScenePipes,
    pub(super) ui: wgpu::RenderPipeline,
    pub(super) post: wgpu::RenderPipeline,
}


pub(super) fn poly_index(b: FteBlend, lines: bool) -> usize {
    b as usize * 2 + usize::from(lines)
}

fn tex_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

/// Unfilterable texture read with `textureLoad` (optionally multisampled).
pub(super) fn load_entry(binding: u32, sample_type: wgpu::TextureSampleType, multisampled: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniform_entry(binding: u32, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

pub(super) fn blend_of(b: FteBlend) -> wgpu::BlendState {
    use wgpu::BlendFactor as F;
    let (s, d) = match b {
        // Overlay (drawn over the world without depth test in Vulkan) blends like Blend
        FteBlend::Blend | FteBlend::Overlay => (F::SrcAlpha, F::OneMinusSrcAlpha),
        FteBlend::BlendColour => (F::Src, F::OneMinusSrc),
        FteBlend::AddA => (F::SrcAlpha, F::One),
        FteBlend::AddC => (F::Src, F::One),
        FteBlend::Subtract => (F::SrcAlpha, F::OneMinusSrc),
        FteBlend::InvModA => (F::Zero, F::OneMinusSrcAlpha),
        FteBlend::InvModC => (F::Zero, F::OneMinusSrc),
        FteBlend::Premul => (F::One, F::OneMinusSrcAlpha),
    };
    let c = wgpu::BlendComponent { src_factor: s, dst_factor: d, operation: wgpu::BlendOperation::Add };
    wgpu::BlendState { color: c, alpha: c }
}

pub(super) struct Desc<'a> {
    pub(super) layout: &'a wgpu::PipelineLayout,
    pub(super) module: &'a wgpu::ShaderModule,
    pub(super) vs: &'a str,
    pub(super) fs: &'a str,
    pub(super) buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    pub(super) topology: wgpu::PrimitiveTopology,
    pub(super) blend: Option<wgpu::BlendState>,
    /// (depth write, depth test); None = no depth attachment.
    pub(super) depth: Option<(bool, bool)>,
    pub(super) format: wgpu::TextureFormat,
    pub(super) samples: u32,
    /// Also write the bloom glow ([`GLOW_FORMAT`], blended like the colour).
    pub(super) glow: bool,
}

pub(super) fn pipeline(dev: &wgpu::Device, d: Desc) -> wgpu::RenderPipeline {
    pipeline_with(dev, d, wgpu::CompareFunction::LessEqual, None)
}

/// [`pipeline`] with the depth test's comparison (when testing) and face culling.
pub(super) fn pipeline_with(dev: &wgpu::Device, d: Desc, compare: wgpu::CompareFunction, cull: Option<wgpu::Face>) -> wgpu::RenderPipeline {
    dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(d.fs),
        layout: Some(d.layout),
        vertex: wgpu::VertexState {
            module: d.module,
            entry_point: Some(d.vs),
            compilation_options: Default::default(),
            buffers: d.buffers,
        },
        primitive: wgpu::PrimitiveState { topology: d.topology, cull_mode: cull, ..Default::default() },
        depth_stencil: d.depth.map(|(write, test)| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(if test { compare } else { wgpu::CompareFunction::Always }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState { count: d.samples, ..Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: d.module,
            entry_point: Some(d.fs),
            compilation_options: Default::default(),
            targets: &[
                Some(wgpu::ColorTargetState { format: d.format, blend: d.blend, write_mask: wgpu::ColorWrites::ALL }),
                Some(wgpu::ColorTargetState { format: GLOW_FORMAT, blend: d.blend, write_mask: wgpu::ColorWrites::ALL }),
            ][..if d.glow { 2 } else { 1 }],
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) const WORLD_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x2];
pub(super) const ALIAS_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x3];
pub(super) const POLY_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4];
const UI_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];

pub(super) fn vb<T>(attrs: &[wgpu::VertexAttribute]) -> Option<wgpu::VertexBufferLayout<'_>> {
    Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<T>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: attrs,
    })
}

impl Pipes {
    pub(super) fn new(dev: &wgpu::Device, out_format: wgpu::TextureFormat) -> Pipes {
        use wgpu::TextureViewDimension as D;
        let bgl = |entries: &[wgpu::BindGroupLayoutEntry]| {
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries })
        };
        let frame_bgl = bgl(&[
            uniform_entry(0, std::mem::size_of::<FrameU>() as u64),
            tex_entry(1, D::D2Array),
            tex_entry(2, D::D2),
            sampler_entry(3),
            // water: ripple uniforms, caustic texture, repeat sampler
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<super::water::WaterU>() as u64),
                },
                count: None,
            },
            tex_entry(5, D::D2),
            sampler_entry(6),
            // r_shadow_realtime_dlight: light list, shadow maps, compare sampler
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<crate::rtlight::LightsGpu>() as u64),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 8,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: D::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 9,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ]);
        let mat_bgl = bgl(&[tex_entry(0, D::D2), tex_entry(1, D::D2), tex_entry(2, D::D2), sampler_entry(3)]);
        let sky_bgl = bgl(&[
            tex_entry(0, D::D2),
            tex_entry(1, D::D2),
            tex_entry(2, D::D2),
            tex_entry(3, D::D2),
            tex_entry(4, D::D2),
            tex_entry(5, D::D2),
            sampler_entry(6),
        ]);
        let draw_bgl = bgl(&[uniform_entry(0, std::mem::size_of::<DrawU>() as u64)]);
        let post_bgl = bgl(&[
            tex_entry(0, D::D2),
            tex_entry(1, D::D2),
            sampler_entry(3),
            tex_entry(4, D::D2),
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            tex_entry(6, D::D2),
            sampler_entry(7),
        ]);
        let layout = |groups: &[&wgpu::BindGroupLayout]| {
            let g: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|g| Some(*g)).collect();
            dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &g,
                immediate_size: 0,
            })
        };
        let scene_layout = layout(&[&frame_bgl, &mat_bgl, &draw_bgl]);
        let sky_layout = layout(&[&frame_bgl, &sky_bgl, &draw_bgl]);
        let screen_layout = layout(&[&draw_bgl, &mat_bgl]);
        let post_layout = layout(&[&draw_bgl, &post_bgl]);
        let module = |src: &str| {
            dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let scene_module = module(SCENE_SRC);
        let sky_module = module(SKY_SRC);
        let screen = module(SCREEN_SRC);
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let ui_vb = [vb::<UiV>(&UI_ATTRS)];
        let scene = ScenePipes::new(dev, &frame_bgl, &mat_bgl, &draw_bgl, &scene_layout, &sky_layout, &scene_module, &sky_module, 1, false);
        let ui = pipeline(dev, Desc {
            layout: &screen_layout,
            module: &screen,
            vs: "vs_ui",
            fs: "fs_ui",
            buffers: &ui_vb,
            topology: tri,
            blend: alpha,
            depth: None,
            format: out_format,
            samples: 1,
            glow: false,
        });
        let post = pipeline(dev, Desc {
            layout: &post_layout,
            module: &screen,
            vs: "vs_post",
            fs: "fs_post",
            buffers: &[],
            topology: tri,
            blend: None,
            depth: None,
            format: out_format,
            samples: 1,
            glow: false,
        });
        Pipes {
            frame_bgl,
            mat_bgl,
            sky_bgl,
            draw_bgl,
            post_bgl,
            scene_layout,
            sky_layout,
            scene_module,
            sky_module,
            scene,
            ui,
            post,
        }
    }

    /// Rebuilds the scene pipelines for `samples` (MSAA) and the bloom glow
    /// attachment if either changed.
    pub(super) fn set_scene(&mut self, dev: &wgpu::Device, samples: u32, glow: bool) {
        if samples != self.scene.samples || glow != self.scene.glow {
            self.scene = ScenePipes::new(
                dev,
                &self.frame_bgl,
                &self.mat_bgl,
                &self.draw_bgl,
                &self.scene_layout,
                &self.sky_layout,
                &self.scene_module,
                &self.sky_module,
                samples,
                glow,
            );
        }
    }
}

