//! Bind group layouts and render pipelines.

use super::{DEPTH_FORMAT, SCENE_FORMAT};
use crate::FteBlend;

const SCENE_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/material.wgsl"));
const SKY_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/sky.wgsl"));
const SCREEN_SRC: &str = include_str!("shaders/screen.wgsl");
const AO_SRC: &str = include_str!("shaders/ao.wgsl");
/// SSAO target: visibility and linear depth.
pub(super) const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

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
    scene_layout: wgpu::PipelineLayout,
    sky_layout: wgpu::PipelineLayout,
    scene_module: wgpu::ShaderModule,
    sky_module: wgpu::ShaderModule,
    /// Scene pipelines for the current MSAA sample count.
    pub(super) scene: ScenePipes,
    pub(super) ui: wgpu::RenderPipeline,
    pub(super) post: wgpu::RenderPipeline,
}

/// Pipelines drawing into the (possibly multisampled) scene target.
pub(super) struct ScenePipes {
    pub(super) samples: u32,
    pub(super) world: wgpu::RenderPipeline,
    pub(super) world_alpha: wgpu::RenderPipeline,
    pub(super) water: wgpu::RenderPipeline,
    pub(super) water_alpha: wgpu::RenderPipeline,
    pub(super) sky: wgpu::RenderPipeline,
    pub(super) alias: wgpu::RenderPipeline,
    pub(super) alias_alpha: wgpu::RenderPipeline,
    /// FTE blend modes × (triangles, lines).
    pub(super) poly: Vec<wgpu::RenderPipeline>,
    /// SSAO: scene depth + colour (alpha mask) -> AO target.
    pub(super) ao_gen_bgl: wgpu::BindGroupLayout,
    pub(super) ao_gen: wgpu::RenderPipeline,
    /// SSAO: scene depth + AO target, multiplied into the scene colour.
    pub(super) ao_apply_bgl: wgpu::BindGroupLayout,
    pub(super) ao_apply: wgpu::RenderPipeline,
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
fn load_entry(binding: u32, sample_type: wgpu::TextureSampleType, multisampled: bool) -> wgpu::BindGroupLayoutEntry {
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
    samples: u32,
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
        multisample: wgpu::MultisampleState { count: d.samples, ..Default::default() },
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
        let scene_module = module(SCENE_SRC);
        let sky_module = module(SKY_SRC);
        let screen = module(SCREEN_SRC);
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let ui_vb = [vb::<UiV>(&UI_ATTRS)];
        let scene = ScenePipes::new(dev, &draw_bgl, &scene_layout, &sky_layout, &scene_module, &sky_module, 1);
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
            samples: 1,
        });
        Pipes {
            frame_bgl,
            mat_bgl,
            sky_bgl,
            draw_bgl,
            scene_layout,
            sky_layout,
            scene_module,
            sky_module,
            scene,
            ui,
            post,
        }
    }

    /// Rebuilds the scene pipelines for `samples` (MSAA) if it changed.
    pub(super) fn set_samples(&mut self, dev: &wgpu::Device, samples: u32) {
        if samples != self.scene.samples {
            self.scene = ScenePipes::new(
                dev,
                &self.draw_bgl,
                &self.scene_layout,
                &self.sky_layout,
                &self.scene_module,
                &self.sky_module,
                samples,
            );
        }
    }
}

impl ScenePipes {
    fn new(
        dev: &wgpu::Device,
        draw_bgl: &wgpu::BindGroupLayout,
        scene_layout: &wgpu::PipelineLayout,
        sky_layout: &wgpu::PipelineLayout,
        scene: &wgpu::ShaderModule,
        sky_m: &wgpu::ShaderModule,
        samples: u32,
    ) -> ScenePipes {
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let world_vb = [vb::<crate::world::WorldVertex>(&WORLD_ATTRS)];
        let alias_vb = [vb::<AliasV>(&ALIAS_ATTRS)];
        let poly_vb = [vb::<crate::FteVertex>(&POLY_ATTRS)];
        let scene_pipe = |vs, fs, buffers, blend, write| {
            pipeline(dev, Desc {
                layout: scene_layout,
                module: scene,
                vs,
                fs,
                buffers,
                topology: tri,
                blend,
                depth: Some((write, true)),
                format: SCENE_FORMAT,
                samples,
            })
        };
        let world = scene_pipe("vs_world", "fs_world", &world_vb, None, true);
        let world_alpha = scene_pipe("vs_world", "fs_world", &world_vb, alpha, false);
        let water = scene_pipe("vs_world", "fs_water", &world_vb, None, true);
        let water_alpha = scene_pipe("vs_world", "fs_water", &world_vb, alpha, false);
        let alias = scene_pipe("vs_alias", "fs_alias", &alias_vb, None, true);
        let alias_alpha = scene_pipe("vs_alias", "fs_alias", &alias_vb, alpha, false);
        let sky = pipeline(dev, Desc {
            layout: sky_layout,
            module: sky_m,
            vs: "vs_sky",
            fs: "fs_sky",
            buffers: &world_vb,
            topology: tri,
            blend: None,
            depth: Some((true, true)),
            format: SCENE_FORMAT,
            samples,
        });
        let mut poly = Vec::new();
        for b in FteBlend::ALL {
            for lines in [false, true] {
                debug_assert_eq!(poly.len(), poly_index(b, lines));
                poly.push(pipeline(dev, Desc {
                    layout: scene_layout,
                    module: scene,
                    vs: "vs_poly",
                    fs: "fs_poly",
                    buffers: &poly_vb,
                    topology: if lines { wgpu::PrimitiveTopology::LineList } else { tri },
                    blend: Some(blend_of(b)),
                    depth: Some((false, true)),
                    format: SCENE_FORMAT,
                    samples,
                }));
            }
        }
        let ms = samples > 1;
        let unfilt = wgpu::TextureSampleType::Float { filterable: false };
        let bgl = |entries: &[wgpu::BindGroupLayoutEntry]| {
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries })
        };
        let ao_gen_bgl = bgl(&[load_entry(0, wgpu::TextureSampleType::Depth, ms), load_entry(1, unfilt, ms)]);
        let ao_apply_bgl = bgl(&[load_entry(0, wgpu::TextureSampleType::Depth, ms), load_entry(1, unfilt, false)]);
        let (depth_t, color_t) = if ms {
            ("texture_depth_multisampled_2d", "texture_multisampled_2d<f32>")
        } else {
            ("texture_depth_2d", "texture_2d<f32>")
        };
        // binding 1 is the scene colour for fs_ao but the AO target for fs_apply
        let ao_module = |color_t: &str| {
            dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ao"),
                source: wgpu::ShaderSource::Wgsl(AO_SRC.replace("DEPTH_T", depth_t).replace("COLOR_T", color_t).into()),
            })
        };
        let (gen_module, apply_module) = (ao_module(color_t), ao_module("texture_2d<f32>"));
        let ao_layout = |g: &wgpu::BindGroupLayout| {
            dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(draw_bgl), Some(g)],
                immediate_size: 0,
            })
        };
        let ao_pipe = |layout: &wgpu::PipelineLayout, module, fs, blend, format, samples| {
            pipeline(dev, Desc {
                layout,
                module,
                vs: "vs_full",
                fs,
                buffers: &[],
                topology: tri,
                blend,
                depth: None,
                format,
                samples,
            })
        };
        let ao_gen = ao_pipe(&ao_layout(&ao_gen_bgl), &gen_module, "fs_ao", None, AO_FORMAT, 1);
        // scene colour *= AO; alpha kept
        use wgpu::{BlendComponent as C, BlendFactor as F};
        let mul = wgpu::BlendState {
            color: C { src_factor: F::Dst, dst_factor: F::Zero, operation: wgpu::BlendOperation::Add },
            alpha: C { src_factor: F::Zero, dst_factor: F::One, operation: wgpu::BlendOperation::Add },
        };
        let ao_apply = ao_pipe(&ao_layout(&ao_apply_bgl), &apply_module, "fs_apply", Some(mul), SCENE_FORMAT, samples);
        ScenePipes {
            samples,
            world,
            world_alpha,
            water,
            water_alpha,
            sky,
            alias,
            alias_alpha,
            poly,
            ao_gen_bgl,
            ao_gen,
            ao_apply_bgl,
            ao_apply,
        }
    }
}
