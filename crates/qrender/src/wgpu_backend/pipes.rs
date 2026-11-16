//! Bind group layouts and render pipelines.

use super::{DEPTH_FORMAT, SCENE_FORMAT};
use crate::FteBlend;

const SCENE_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/material.wgsl"));
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
    pub(super) pad: [f32; 20],
}

/// Vertex of CPU-posed alias models (world space).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct AliasV {
    pub(super) pos: [f32; 3],
    pub(super) st: [f32; 2],
    pub(super) shade: [f32; 3],
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
    pub(super) world: wgpu::RenderPipeline,
    pub(super) world_alpha: wgpu::RenderPipeline,
    pub(super) water: wgpu::RenderPipeline,
    pub(super) water_alpha: wgpu::RenderPipeline,
    pub(super) sky: wgpu::RenderPipeline,
    pub(super) alias: wgpu::RenderPipeline,
    pub(super) alias_alpha: wgpu::RenderPipeline,
    /// FTE blend modes × (triangles, lines).
    pub(super) poly: Vec<wgpu::RenderPipeline>,
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

fn blend_of(b: FteBlend) -> wgpu::BlendState {
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

struct Desc<'a> {
    layout: &'a wgpu::PipelineLayout,
    module: &'a wgpu::ShaderModule,
    vs: &'a str,
    fs: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    topology: wgpu::PrimitiveTopology,
    blend: Option<wgpu::BlendState>,
    /// (depth write, depth test); None = no depth attachment.
    depth: Option<(bool, bool)>,
    format: wgpu::TextureFormat,
}

fn pipeline(dev: &wgpu::Device, d: Desc) -> wgpu::RenderPipeline {
    dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(d.fs),
        layout: Some(d.layout),
        vertex: wgpu::VertexState {
            module: d.module,
            entry_point: Some(d.vs),
            compilation_options: Default::default(),
            buffers: d.buffers,
        },
        primitive: wgpu::PrimitiveState { topology: d.topology, cull_mode: None, ..Default::default() },
        depth_stencil: d.depth.map(|(write, test)| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(if test { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always }),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: d.module,
            entry_point: Some(d.fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: d.format,
                blend: d.blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

const WORLD_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x2];
const ALIAS_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3];
const POLY_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Unorm8x4];
const UI_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];

fn vb<T>(attrs: &[wgpu::VertexAttribute]) -> Option<wgpu::VertexBufferLayout<'_>> {
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
        let module = |src: &str| {
            dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(src.into()),
            })
        };
        let scene = module(SCENE_SRC);
        let sky_m = module(SKY_SRC);
        let screen = module(SCREEN_SRC);
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let world_vb = [vb::<crate::world::WorldVertex>(&WORLD_ATTRS)];
        let alias_vb = [vb::<AliasV>(&ALIAS_ATTRS)];
        let poly_vb = [vb::<crate::FteVertex>(&POLY_ATTRS)];
        let ui_vb = [vb::<UiV>(&UI_ATTRS)];
        let scene_pipe = |vs, fs, buffers, blend, write| {
            pipeline(dev, Desc {
                layout: &scene_layout,
                module: &scene,
                vs,
                fs,
                buffers,
                topology: tri,
                blend,
                depth: Some((write, true)),
                format: SCENE_FORMAT,
            })
        };
        let world = scene_pipe("vs_world", "fs_world", &world_vb, None, true);
        let world_alpha = scene_pipe("vs_world", "fs_world", &world_vb, alpha, false);
        let water = scene_pipe("vs_world", "fs_water", &world_vb, None, true);
        let water_alpha = scene_pipe("vs_world", "fs_water", &world_vb, alpha, false);
        let alias = scene_pipe("vs_alias", "fs_alias", &alias_vb, None, true);
        let alias_alpha = scene_pipe("vs_alias", "fs_alias", &alias_vb, alpha, false);
        let sky = pipeline(dev, Desc {
            layout: &sky_layout,
            module: &sky_m,
            vs: "vs_sky",
            fs: "fs_sky",
            buffers: &world_vb,
            topology: tri,
            blend: None,
            depth: Some((true, true)),
            format: SCENE_FORMAT,
        });
        let mut poly = Vec::new();
        for b in FteBlend::ALL {
            for lines in [false, true] {
                debug_assert_eq!(poly.len(), poly_index(b, lines));
                poly.push(pipeline(dev, Desc {
                    layout: &scene_layout,
                    module: &scene,
                    vs: "vs_poly",
                    fs: "fs_poly",
                    buffers: &poly_vb,
                    topology: if lines { wgpu::PrimitiveTopology::LineList } else { tri },
                    blend: Some(blend_of(b)),
                    depth: Some((false, true)),
                    format: SCENE_FORMAT,
                }));
            }
        }
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
        });
        let post = pipeline(dev, Desc {
            layout: &screen_layout,
            module: &screen,
            vs: "vs_post",
            fs: "fs_post",
            buffers: &[],
            topology: tri,
            blend: None,
            depth: None,
            format: out_format,
        });
        Pipes {
            frame_bgl,
            mat_bgl,
            sky_bgl,
            draw_bgl,
            world,
            world_alpha,
            water,
            water_alpha,
            sky,
            alias,
            alias_alpha,
            poly,
            ui,
            post,
        }
    }
}
