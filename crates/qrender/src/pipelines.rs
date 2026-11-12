//! Descriptor layouts and pipelines.

use crate::Result;
use crate::gpu::Gpu;
use ash::vk;

macro_rules! spv {
    ($n:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $n, ".spv"))
    };
}

pub const MAX_TEXTURES: u32 = 4096;
pub const COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
pub const LM_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

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

struct Desc<'a> {
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    bindings: &'a [vk::VertexInputBindingDescription],
    attrs: &'a [vk::VertexInputAttributeDescription],
    blend: Blend,
    depth_test: bool,
    depth_write: bool,
    color: vk::Format,
    depth: bool,
    samples: u32,
    /// Cull back faces (alias models). Quake triangles are clockwise-front in
    /// GL; our Vulkan projection flips Y, so they arrive counter-clockwise.
    cull: bool,
    /// Per-sample shading (supersampling) when multisampled.
    sample_shading: bool,
    topology: vk::PrimitiveTopology,
}

fn attr(
    loc: u32,
    binding: u32,
    format: vk::Format,
    offset: u32,
) -> vk::VertexInputAttributeDescription {
    vk::VertexInputAttributeDescription {
        location: loc,
        binding,
        format,
        offset,
    }
}

fn bind(binding: u32, stride: u32, instance: bool) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
        binding,
        stride,
        input_rate: if instance {
            vk::VertexInputRate::INSTANCE
        } else {
            vk::VertexInputRate::VERTEX
        },
    }
}

fn graphics(gpu: &Gpu, layout: vk::PipelineLayout, d: &Desc) -> Result<vk::Pipeline> {
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(d.vs)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(d.fs)
            .name(c"main"),
    ];
    let vi = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(d.bindings)
        .vertex_attribute_descriptions(d.attrs);
    let ia = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(d.topology);
    let vp = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rs = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(if d.cull { vk::CullModeFlags::BACK } else { vk::CullModeFlags::NONE })
        .front_face(vk::FrontFace::CLOCKWISE)
        .depth_bias_enable(d.depth)
        .line_width(1.0);
    let ms = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::from_raw(d.samples.max(1)))
        .sample_shading_enable(d.sample_shading && d.samples > 1)
        .min_sample_shading(1.0);
    let ds = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(d.depth_test)
        .depth_write_enable(d.depth_write)
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
    let (src, dst, src_a, dst_a) = match d.blend {
        Blend::Factors(s, t) => (s, t, vk::BlendFactor::ZERO, vk::BlendFactor::ONE),
        _ => (
            vk::BlendFactor::SRC_ALPHA,
            vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            vk::BlendFactor::ONE,
            vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        ),
    };
    let att = [vk::PipelineColorBlendAttachmentState {
        blend_enable: (d.blend != Blend::None).into(),
        src_color_blend_factor: src,
        dst_color_blend_factor: dst,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: src_a,
        dst_alpha_blend_factor: dst_a,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: vk::ColorComponentFlags::RGBA,
    }];
    let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&att);
    // scene pipelines take a dynamic depth bias (gl_zfix for brush entities)
    let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR, vk::DynamicState::DEPTH_BIAS];
    let n = if d.depth { 3 } else { 2 };
    let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states[..n]);
    let formats = [d.color];
    let mut ri = vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&formats);
    if d.depth {
        ri = ri.depth_attachment_format(DEPTH_FORMAT);
    }
    let ci = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vi)
        .input_assembly_state(&ia)
        .viewport_state(&vp)
        .rasterization_state(&rs)
        .multisample_state(&ms)
        .depth_stencil_state(&ds)
        .color_blend_state(&cb)
        .dynamic_state(&dy)
        .layout(layout)
        .push_next(&mut ri)];
    // SAFETY: all referenced state lives until the call returns.
    let p = unsafe {
        gpu.device
            .create_graphics_pipelines(vk::PipelineCache::null(), &ci, None)
    }
    .map_err(|(_, e)| e)?;
    Ok(p[0])
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
            post_format,
            samples,
            supersample: false,
            sh,
            modules,
        };
        p.build(gpu, post_format, samples)?;
        Ok(p)
    }

    fn graphics_pipes(&self) -> Vec<vk::Pipeline> {
        let mut v = vec![
            self.world,
            self.world_alpha,
            self.water,
            self.water_alpha,
            self.sky,
            self.alias,
            self.alias_alpha,
            self.poly,
            self.particle,
            self.ui,
            self.post,
        ];
        v.extend(self.fte);
        v
    }

    /// (Re)creates the graphics pipelines for an output format and scene
    /// sample count. The device must be idle.
    pub fn build(&mut self, gpu: &Gpu, post_format: vk::Format, samples: u32) -> Result<()> {
        for p in self.graphics_pipes() {
            if p != vk::Pipeline::null() {
                // SAFETY: device idle (caller contract).
                unsafe { gpu.device.destroy_pipeline(p, None) };
            }
        }
        let supersample = self.supersample && gpu.sample_shading;
        let Shaders {
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
        } = self.sh;
        let layout = self.layout;
        let f32x3 = vk::Format::R32G32B32_SFLOAT;
        let f32x2 = vk::Format::R32G32_SFLOAT;
        let wb = [bind(0, 32, false)];
        let wa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, f32x2, 12),
            attr(2, 0, f32x3, 20),
        ];
        let ab = [bind(0, 12, false)];
        let aa = [attr(0, 0, vk::Format::R32_UINT, 0), attr(1, 0, f32x2, 4)];
        let pb = [bind(0, 20, false)];
        let pa = [attr(0, 0, f32x3, 0), attr(1, 0, f32x2, 12)];
        let qb = [bind(0, 16, true)];
        let qa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, vk::Format::R8G8B8A8_UNORM, 12),
        ];
        let ub = [bind(0, 24, false)];
        let ua = [
            attr(0, 0, f32x2, 0),
            attr(1, 0, f32x2, 8),
            attr(2, 0, vk::Format::R8G8B8A8_UNORM, 16),
            attr(3, 0, vk::Format::R32_UINT, 20),
        ];
        let d = |vs, fs, bindings, attrs, blend, dt, dw| Desc {
            vs,
            fs,
            bindings,
            attrs,
            blend,
            depth_test: dt,
            depth_write: dw,
            color: COLOR_FORMAT,
            depth: true,
            samples,
            cull: false,
            sample_shading: supersample,
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        };
        let g = |d: Desc| graphics(gpu, layout, &d);
        let world = g(d(world_vs, world_fs, &wb, &wa, Blend::None, true, true))?;
        let world_alpha = g(d(world_vs, world_fs, &wb, &wa, Blend::Alpha, true, false))?;
        let water = g(d(world_vs, water_fs, &wb, &wa, Blend::None, true, true))?;
        let water_alpha = g(d(world_vs, water_fs, &wb, &wa, Blend::Alpha, true, false))?;
        let sky = g(d(world_vs, sky_fs, &wb, &wa, Blend::None, true, true))?;
        let alias = g(Desc { cull: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::None, true, true) })?;
        let alias_alpha = g(Desc { cull: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::Alpha, true, false) })?;
        let poly = g(d(poly_vs, poly_fs, &pb, &pa, Blend::Alpha, true, true))?;
        let particle = g(d(part_vs, part_fs, &qb, &qa, Blend::Alpha, true, false))?;
        let fb = [bind(0, 24, false)];
        let fa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, f32x2, 12),
            attr(2, 0, vk::Format::R8G8B8A8_UNORM, 20),
        ];
        let mut fte = [vk::Pipeline::null(); 16];
        for (i, slot) in fte.iter_mut().enumerate() {
            let (src, dst) = fte_factors(crate::FteBlend::ALL[i % 8]);
            *slot = g(Desc {
                topology: if i >= 8 { vk::PrimitiveTopology::LINE_LIST } else { vk::PrimitiveTopology::TRIANGLE_LIST },
                ..d(fte_vs, fte_fs, &fb, &fa, Blend::Factors(src, dst), true, false)
            })?;
        }
        // UI is drawn in the output pass, after post-processing
        let ui = graphics(
            gpu,
            layout,
            &Desc {
                color: post_format,
                depth: false,
                samples: 1,
                ..d(ui_vs, ui_fs, &ub, &ua, Blend::Alpha, false, false)
            },
        )?;
        let post = graphics(
            gpu,
            layout,
            &Desc {
                vs: post_vs,
                fs: post_fs,
                bindings: &[],
                attrs: &[],
                blend: Blend::None,
                depth_test: false,
                depth_write: false,
                color: post_format,
                depth: false,
                samples: 1,
                cull: false,
                sample_shading: false,
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            },
        )?;
        (
            self.world,
            self.world_alpha,
            self.water,
            self.water_alpha,
            self.sky,
            self.alias,
            self.alias_alpha,
            self.poly,
            self.particle,
            self.ui,
            self.post,
        ) = (world, world_alpha, water, water_alpha, sky, alias, alias_alpha, poly, particle, ui, post);
        self.fte = fte;
        self.post_format = post_format;
        self.samples = samples;
        Ok(())
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
