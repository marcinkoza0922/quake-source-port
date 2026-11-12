use super::*;

pub(super) struct Desc<'a> {
    pub(super) vs: vk::ShaderModule,
    pub(super) fs: vk::ShaderModule,
    pub(super) bindings: &'a [vk::VertexInputBindingDescription],
    pub(super) attrs: &'a [vk::VertexInputAttributeDescription],
    pub(super) blend: Blend,
    pub(super) depth_test: bool,
    pub(super) depth_write: bool,
    pub(super) color: vk::Format,
    pub(super) depth: bool,
    pub(super) samples: u32,
    /// Cull back faces (alias models). Quake triangles are clockwise-front in
    /// GL; our Vulkan projection flips Y, so they arrive counter-clockwise.
    pub(super) cull: bool,
    /// Per-sample shading (supersampling) when multisampled.
    pub(super) sample_shading: bool,
    pub(super) topology: vk::PrimitiveTopology,
}

pub(super) fn attr(
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

pub(super) fn bind(binding: u32, stride: u32, instance: bool) -> vk::VertexInputBindingDescription {
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

pub(super) fn graphics(gpu: &Gpu, layout: vk::PipelineLayout, d: &Desc) -> Result<vk::Pipeline> {
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
