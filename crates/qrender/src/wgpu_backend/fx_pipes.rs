//! Pipelines of the screen-space effects that read the scene targets: SSAO
//! (`shaders/ao.wgsl`) and per-object motion velocity (`shaders/vel.wgsl`).
//! Built per MSAA sample count, since they bind the (multisampled) scene
//! depth and colour.

use super::SCENE_FORMAT;
use super::pipes::{ALIAS_ATTRS, AliasV, Desc, POLY_ATTRS, SCENE_SRC, WORLD_ATTRS, load_entry, pipeline, vb};
use crate::FteBlend;

const AO_SRC: &str = include_str!("shaders/ao.wgsl");
const SOFT_SRC: &str = include_str!("shaders/soft.wgsl");
const VEL_SRC: &str = concat!(include_str!("shaders/common.wgsl"), include_str!("shaders/vel.wgsl"));
/// SSAO target: visibility and linear depth.
pub(super) const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;
/// Motion target: screen displacement per frame (view uv units).
pub(super) const VEL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

pub(super) struct FxPipes {
    /// SSAO: scene depth + colour (alpha mask) -> AO target.
    pub(super) ao_gen_bgl: wgpu::BindGroupLayout,
    pub(super) ao_gen: wgpu::RenderPipeline,
    /// SSAO: scene depth + AO target, multiplied into the scene colour.
    pub(super) ao_apply_bgl: wgpu::BindGroupLayout,
    pub(super) ao_apply: wgpu::RenderPipeline,
    /// Velocity: scene depth (occlusion test).
    pub(super) vel_bgl: wgpu::BindGroupLayout,
    pub(super) vel_world: wgpu::RenderPipeline,
    pub(super) vel_alias: wgpu::RenderPipeline,
    /// Soft particles: scene depth, bound while it is a read-only attachment.
    pub(super) soft_bgl: wgpu::BindGroupLayout,
    /// Soft FTE triangles per blend mode (`FteBlend::ALL` order).
    pub(super) soft: Vec<wgpu::RenderPipeline>,
}

impl FxPipes {
    pub(super) fn new(
        dev: &wgpu::Device,
        [frame_bgl, mat_bgl, draw_bgl]: [&wgpu::BindGroupLayout; 3],
        samples: u32,
        glow: bool,
    ) -> FxPipes {
        let ms = samples > 1;
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let depth = wgpu::TextureSampleType::Depth;
        let unfilt = wgpu::TextureSampleType::Float { filterable: false };
        let bgl = |entries: &[wgpu::BindGroupLayoutEntry]| {
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries })
        };
        let layout = |groups: &[&wgpu::BindGroupLayout]| {
            let g: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|g| Some(*g)).collect();
            dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &g, immediate_size: 0 })
        };
        let (depth_t, color_t) = if ms {
            ("texture_depth_multisampled_2d", "texture_multisampled_2d<f32>")
        } else {
            ("texture_depth_2d", "texture_2d<f32>")
        };
        let module = |label, src: String| {
            dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(src.into()) })
        };
        let fx_pipe = |layout: &wgpu::PipelineLayout, module, vs, fs, buffers, blend, format, samples| {
            pipeline(dev, Desc { layout, module, vs, fs, buffers, topology: tri, blend, depth: None, format, samples, glow: false })
        };

        let ao_gen_bgl = bgl(&[load_entry(0, depth, ms), load_entry(1, unfilt, ms)]);
        let ao_apply_bgl = bgl(&[load_entry(0, depth, ms), load_entry(1, unfilt, false)]);
        // binding 1 is the scene colour for fs_ao but the AO target for fs_apply
        let ao_src = |color_t: &str| AO_SRC.replace("DEPTH_T", depth_t).replace("COLOR_T", color_t);
        let gen_module = module("ao", ao_src(color_t));
        let apply_module = module("ao apply", ao_src("texture_2d<f32>"));
        let ao_gen = fx_pipe(&layout(&[draw_bgl, &ao_gen_bgl]), &gen_module, "vs_full", "fs_ao", &[], None, AO_FORMAT, 1);
        // scene colour *= AO; alpha kept
        use wgpu::{BlendComponent as C, BlendFactor as F};
        let mul = wgpu::BlendState {
            color: C { src_factor: F::Dst, dst_factor: F::Zero, operation: wgpu::BlendOperation::Add },
            alpha: C { src_factor: F::Zero, dst_factor: F::One, operation: wgpu::BlendOperation::Add },
        };
        let apply_layout = layout(&[draw_bgl, &ao_apply_bgl]);
        let ao_apply = fx_pipe(&apply_layout, &apply_module, "vs_full", "fs_apply", &[], Some(mul), SCENE_FORMAT, samples);

        let vel_bgl = bgl(&[load_entry(0, depth, ms)]);
        let vel_module = module("vel", VEL_SRC.replace("DEPTH_T", depth_t));
        let vel_layout = layout(&[frame_bgl, &vel_bgl, draw_bgl]);
        let world_vb = [vb::<crate::world::WorldVertex>(&WORLD_ATTRS)];
        let alias_vb = [vb::<AliasV>(&ALIAS_ATTRS)];
        let vel_world = fx_pipe(&vel_layout, &vel_module, "vs_vel", "fs_vel", &world_vb, None, VEL_FORMAT, 1);
        let vel_alias = fx_pipe(&vel_layout, &vel_module, "vs_vel", "fs_vel", &alias_vb, None, VEL_FORMAT, 1);

        let soft_bgl = bgl(&[load_entry(0, depth, ms)]);
        let soft_module = module("soft", format!("{SCENE_SRC}{}", SOFT_SRC.replace("DEPTH_T", depth_t)));
        let soft_layout = layout(&[frame_bgl, mat_bgl, draw_bgl, &soft_bgl]);
        let poly_vb = [vb::<crate::FteVertex>(&POLY_ATTRS)];
        let soft = FteBlend::ALL
            .iter()
            .map(|&b| {
                pipeline(dev, Desc {
                    layout: &soft_layout,
                    module: &soft_module,
                    vs: "vs_poly",
                    fs: if glow { "fs_poly_soft_glow" } else { "fs_poly_soft" },
                    buffers: &poly_vb,
                    topology: tri,
                    blend: Some(super::pipes::blend_of(b)),
                    depth: Some((false, true)),
                    format: SCENE_FORMAT,
                    samples,
                    glow,
                })
            })
            .collect();
        FxPipes { ao_gen_bgl, ao_gen, ao_apply_bgl, ao_apply, vel_bgl, vel_world, vel_alias, soft_bgl, soft }
    }
}
