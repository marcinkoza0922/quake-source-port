//! Descriptor layouts and pipelines.

use crate::Result;
use crate::gpu::Gpu;
use ash::vk;
mod desc;
use desc::*;
mod build;

macro_rules! spv {
    ($n:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $n, ".spv"))
    };
}

pub const MAX_TEXTURES: u32 = 4096;
pub const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
pub const LM_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// SSAO target (visibility, view depth) and per-object velocity target.
pub const AO_FORMAT: vk::Format = vk::Format::R16G16_SFLOAT;
pub const VEL_FORMAT: vk::Format = vk::Format::R16G16_SFLOAT;

pub struct Pipelines {
    pub set_layout: vk::DescriptorSetLayout,
    pub layout: vk::PipelineLayout,
    pub comp_set_layout: vk::DescriptorSetLayout,
    pub comp_layout: vk::PipelineLayout,
    pub lightmap: vk::Pipeline,
    /// World surface culling (cull.comp): 7 storage buffers + push constants.
    pub cull_set_layout: vk::DescriptorSetLayout,
    pub cull_layout: vk::PipelineLayout,
    pub cull: vk::Pipeline,
    pub world: vk::Pipeline,
    pub world_alpha: vk::Pipeline,
    pub water: vk::Pipeline,
    pub water_alpha: vk::Pipeline,
    pub sky: vk::Pipeline,
    pub alias: vk::Pipeline,
    pub alias_alpha: vk::Pipeline,
    pub poly: vk::Pipeline,
    pub particle: vk::Pipeline,
    /// FTE particles: 8 blend modes of triangles, then the same as lines
    /// (see [`fte_index`]).
    pub fte: [vk::Pipeline; 16],
    pub ui: vk::Pipeline,
    pub post: vk::Pipeline,
    /// SSAO pass (fullscreen, scene resolution).
    pub ao: vk::Pipeline,
    /// Multiplies the blurred AO into the scene colour (scene samples).
    pub ao_apply: vk::Pipeline,
    /// Velocity pass for moving alias models / brush entities.
    pub vel_alias: vk::Pipeline,
    pub vel_world: vk::Pipeline,
    pub post_format: vk::Format,
    /// MSAA sample count of the scene pipelines.
    pub samples: u32,
    /// Scene pipelines shade per sample (set before `build`).
    pub supersample: bool,
    sh: Shaders,
    modules: Vec<vk::ShaderModule>,
}

#[derive(Clone, Copy)]
struct Shaders {
    world_vs: vk::ShaderModule,
    world_fs: vk::ShaderModule,
    water_fs: vk::ShaderModule,
    sky_fs: vk::ShaderModule,
    alias_vs: vk::ShaderModule,
    alias_fs: vk::ShaderModule,
    poly_vs: vk::ShaderModule,
    poly_fs: vk::ShaderModule,
    part_vs: vk::ShaderModule,
    part_fs: vk::ShaderModule,
    fte_vs: vk::ShaderModule,
    fte_fs: vk::ShaderModule,
    ui_vs: vk::ShaderModule,
    ui_fs: vk::ShaderModule,
    post_vs: vk::ShaderModule,
    post_fs: vk::ShaderModule,
    ao_fs: vk::ShaderModule,
    ao_apply_fs: vk::ShaderModule,
    vel_alias_vs: vk::ShaderModule,
    vel_world_vs: vk::ShaderModule,
    vel_fs: vk::ShaderModule,
}

#[derive(Clone, Copy, PartialEq)]
enum Blend {
    None,
    Alpha,
    /// Colour factors (src, dst); destination alpha is preserved.
    Factors(vk::BlendFactor, vk::BlendFactor),
}

/// Colour blend factors of an FTE blend mode.
fn fte_factors(b: crate::FteBlend) -> (vk::BlendFactor, vk::BlendFactor) {
    use crate::FteBlend as F;
    use vk::BlendFactor as B;
    match b {
        F::Blend => (B::SRC_ALPHA, B::ONE_MINUS_SRC_ALPHA),
        F::BlendColour => (B::SRC_COLOR, B::ONE_MINUS_SRC_COLOR),
        F::AddA => (B::SRC_ALPHA, B::ONE),
        F::AddC => (B::SRC_COLOR, B::ONE),
        F::Subtract => (B::SRC_ALPHA, B::ONE_MINUS_SRC_COLOR),
        F::InvModA => (B::ZERO, B::ONE_MINUS_SRC_ALPHA),
        F::InvModC => (B::ZERO, B::ONE_MINUS_SRC_COLOR),
        F::Premul => (B::ONE, B::ONE_MINUS_SRC_ALPHA),
    }
}

/// Index of an FTE pipeline in [`Pipelines::fte`].
pub fn fte_index(b: crate::FteBlend, lines: bool) -> usize {
    crate::FteBlend::ALL.iter().position(|x| *x == b).unwrap_or(0) + if lines { 8 } else { 0 }
}

impl Pipelines {
    pub fn new(gpu: &Gpu, post_format: vk::Format, samples: u32) -> Result<Pipelines> {
        let dev = &gpu.device;
        let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let b = |i: u32, ty: vk::DescriptorType, n: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(i)
                .descriptor_type(ty)
                .descriptor_count(n)
                .stage_flags(all)
        };
        let bindings = [
            b(0, vk::DescriptorType::UNIFORM_BUFFER, 1),
            b(1, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, MAX_TEXTURES),
            b(2, vk::DescriptorType::STORAGE_BUFFER, 1),
            b(3, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 1),
            b(4, vk::DescriptorType::STORAGE_BUFFER, 1),
            b(5, vk::DescriptorType::STORAGE_BUFFER, 1),
            // scene depth for soft particles: single-sampled / MSAA (only the
            // one matching the targets is written)
            b(6, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 1),
            b(7, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 1),
            // MSAA scene colour, read by SSAO (alpha = AO mask)
            b(8, vk::DescriptorType::COMBINED_IMAGE_SAMPLER, 1),
            // the per-frame dynamic buffer, for CPU-built alias poses
            b(9, vk::DescriptorType::STORAGE_BUFFER, 1),
        ];
        let flags = [
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::PARTIALLY_BOUND,
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::empty(),
            vk::DescriptorBindingFlags::PARTIALLY_BOUND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND,
            vk::DescriptorBindingFlags::PARTIALLY_BOUND,
            vk::DescriptorBindingFlags::empty(),
        ];
        let mut fi = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&flags);
        let ci = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings)
            .push_next(&mut fi);
        // SAFETY: valid device and create info.
        let set_layout = unsafe { dev.create_descriptor_set_layout(&ci, None)? };
        let pcr = [vk::PushConstantRange {
            stage_flags: all,
            offset: 0,
            size: 128,
        }];
        let sl = [set_layout];
        let lci = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&sl)
            .push_constant_ranges(&pcr);
        // SAFETY: valid device.
        let layout = unsafe { dev.create_pipeline_layout(&lci, None)? };

        let cb = |i: u32, ty: vk::DescriptorType| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(i)
                .descriptor_type(ty)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
        };
        let cbind = [
            cb(0, vk::DescriptorType::STORAGE_BUFFER),
            cb(1, vk::DescriptorType::STORAGE_BUFFER),
            cb(2, vk::DescriptorType::STORAGE_BUFFER),
            cb(3, vk::DescriptorType::STORAGE_BUFFER),
            cb(4, vk::DescriptorType::STORAGE_IMAGE),
        ];
        let cci = vk::DescriptorSetLayoutCreateInfo::default().bindings(&cbind);
        // SAFETY: valid device.
        let comp_set_layout = unsafe { dev.create_descriptor_set_layout(&cci, None)? };
        let csl = [comp_set_layout];
        let clci = vk::PipelineLayoutCreateInfo::default().set_layouts(&csl);
        // SAFETY: valid device.
        let comp_layout = unsafe { dev.create_pipeline_layout(&clci, None)? };

        let cullbind: Vec<_> = (0..7).map(|i| cb(i, vk::DescriptorType::STORAGE_BUFFER)).collect();
        let cci = vk::DescriptorSetLayoutCreateInfo::default().bindings(&cullbind);
        // SAFETY: valid device.
        let cull_set_layout = unsafe { dev.create_descriptor_set_layout(&cci, None)? };
        let csl2 = [cull_set_layout];
        let cpcr = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::COMPUTE, offset: 0, size: 112 }];
        let clci = vk::PipelineLayoutCreateInfo::default().set_layouts(&csl2).push_constant_ranges(&cpcr);
        // SAFETY: valid device.
        let cull_layout = unsafe { dev.create_pipeline_layout(&clci, None)? };

        let mut modules = Vec::new();
        let mut m = |spv: &[u8]| -> Result<vk::ShaderModule> {
            let s = gpu.shader(spv)?;
            modules.push(s);
            Ok(s)
        };
        let lm_cs = m(spv!("lightmap.comp"))?;
        let world_vs = m(spv!("world.vert"))?;
        let world_fs = m(spv!("world.frag"))?;
        let water_fs = m(spv!("water.frag"))?;
        let sky_fs = m(spv!("sky.frag"))?;
        let alias_vs = m(spv!("alias.vert"))?;
        let alias_fs = m(spv!("alias.frag"))?;
        let poly_vs = m(spv!("poly.vert"))?;
        let poly_fs = m(spv!("poly.frag"))?;
        let part_vs = m(spv!("particle.vert"))?;
        let part_fs = m(spv!("particle.frag"))?;
        let fte_vs = m(spv!("fte.vert"))?;
        let fte_fs = m(spv!("fte.frag"))?;
        let ui_vs = m(spv!("ui.vert"))?;
        let ui_fs = m(spv!("ui.frag"))?;
        let post_vs = m(spv!("post.vert"))?;
        let post_fs = m(spv!("post.frag"))?;
        let ao_fs = m(spv!("ao.frag"))?;
        let ao_apply_fs = m(spv!("aoapply.frag"))?;
        let vel_alias_vs = m(spv!("velalias.vert"))?;
        let vel_world_vs = m(spv!("velworld.vert"))?;
        let vel_fs = m(spv!("vel.frag"))?;

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(lm_cs)
            .name(c"main");
        let cpci = [vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(comp_layout)];
        // SAFETY: valid module/layout.
        let lightmap =
            unsafe { dev.create_compute_pipelines(vk::PipelineCache::null(), &cpci, None) }
                .map_err(|(_, e)| e)?[0];
        let cull_cs = m(spv!("cull.comp"))?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(cull_cs)
            .name(c"main");
        let cpci = [vk::ComputePipelineCreateInfo::default().stage(stage).layout(cull_layout)];
        // SAFETY: valid module/layout.
        let cull = unsafe { dev.create_compute_pipelines(vk::PipelineCache::null(), &cpci, None) }
            .map_err(|(_, e)| e)?[0];

        let sh = Shaders {
            world_vs,
            world_fs,
            water_fs,
            sky_fs,
            alias_vs,
            alias_fs,
            poly_vs,
            poly_fs,
            part_vs,
            part_fs,
            fte_vs,
            fte_fs,
            ui_vs,
            ui_fs,
            post_vs,
            post_fs,
            ao_fs,
            ao_apply_fs,
            vel_alias_vs,
            vel_world_vs,
            vel_fs,
        };
        let null = vk::Pipeline::null();
        let mut p = Pipelines {
            set_layout,
            layout,
            comp_set_layout,
            comp_layout,
            lightmap,
            cull_set_layout,
            cull_layout,
            cull,
            world: null,
            world_alpha: null,
            water: null,
            water_alpha: null,
            sky: null,
            alias: null,
            alias_alpha: null,
            poly: null,
            particle: null,
            fte: [null; 16],
            ui: null,
            post: null,
            ao: null,
            ao_apply: null,
            vel_alias: null,
            vel_world: null,
            post_format,
            samples,
            supersample: false,
            sh,
            modules,
        };
        p.build(gpu, post_format, samples)?;
        Ok(p)
    }

    pub fn destroy(&mut self, dev: &ash::Device) {
        // SAFETY: device idle at teardown.
        unsafe {
            for p in self.graphics_pipes().into_iter().chain([self.lightmap, self.cull]) {
                dev.destroy_pipeline(p, None);
            }
            for m in self.modules.drain(..) {
                dev.destroy_shader_module(m, None);
            }
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_pipeline_layout(self.comp_layout, None);
            dev.destroy_pipeline_layout(self.cull_layout, None);
            dev.destroy_descriptor_set_layout(self.cull_set_layout, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
            dev.destroy_descriptor_set_layout(self.comp_set_layout, None);
        }
    }
}
