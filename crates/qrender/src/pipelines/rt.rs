//! Ray-traced reflections (r_water_reflections 2): set 1 of the graphics
//! layout, the alias posing compute pipeline and the ray-query water
//! shaders. Created only on devices with ray queries.

use super::*;

/// Storage buffers of graphics set 1 after the TLAS (binding 0): world
/// vertices, triangle indices, triangle textures, geometry bases, texture
/// animation table, alias draws, alias triangle attributes.
pub const RT_SET_BUFFERS: u32 = 7;
/// Storage buffers of the posing compute set: alias poses, per-frame
/// dynamic buffer, alias draws, output positions, output attributes.
pub const RT_COMP_BUFFERS: u32 = 5;

pub struct RtPipes {
    /// Graphics set 1 (fragment stage).
    pub set_layout: vk::DescriptorSetLayout,
    pub comp_set_layout: vk::DescriptorSetLayout,
    pub comp_layout: vk::PipelineLayout,
    /// rt_pose.comp: alias models posed into world space.
    pub pose: vk::Pipeline,
    /// water_rt.frag, half-res water_rt_half.frag, and rt_depth.frag.
    pub(super) water_fs: vk::ShaderModule,
    pub(super) water_half_fs: vk::ShaderModule,
    pub(super) depth_fs: vk::ShaderModule,
}

impl RtPipes {
    pub(super) fn new(gpu: &Gpu, modules: &mut Vec<vk::ShaderModule>) -> Result<RtPipes> {
        let dev = &gpu.device;
        let frag = vk::ShaderStageFlags::FRAGMENT;
        let mut bindings = vec![vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
            .descriptor_count(1)
            .stage_flags(frag)];
        bindings.extend((1..=RT_SET_BUFFERS).map(|i| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(i)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(frag)
        }));
        // SAFETY: valid device and create info.
        let set_layout = unsafe {
            dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)?
        };
        let cbind: Vec<_> = (0..RT_COMP_BUFFERS)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        // SAFETY: valid device and create info.
        let comp_set_layout = unsafe {
            dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&cbind), None)?
        };
        let sl = [comp_set_layout];
        let pcr = [vk::PushConstantRange { stage_flags: vk::ShaderStageFlags::COMPUTE, offset: 0, size: 8 }];
        // SAFETY: valid device.
        let comp_layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&sl).push_constant_ranges(&pcr),
                None,
            )?
        };
        let mut m = |spv: &[u8]| -> Result<vk::ShaderModule> {
            let s = gpu.shader(spv)?;
            modules.push(s);
            Ok(s)
        };
        let pose_cs = m(spv!("rt_pose.comp"))?;
        let water_fs = m(spv!("water_rt.frag"))?;
        let water_half_fs = m(spv!("water_rt_half.frag"))?;
        let depth_fs = m(spv!("rt_depth.frag"))?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(pose_cs)
            .name(c"main");
        let cpci = [vk::ComputePipelineCreateInfo::default().stage(stage).layout(comp_layout)];
        // SAFETY: valid module/layout.
        let pose = unsafe { dev.create_compute_pipelines(vk::PipelineCache::null(), &cpci, None) }
            .map_err(|(_, e)| e)?[0];
        Ok(RtPipes { set_layout, comp_set_layout, comp_layout, pose, water_fs, water_half_fs, depth_fs })
    }

    pub(super) fn destroy(&self, dev: &ash::Device) {
        // SAFETY: device idle at teardown.
        unsafe {
            dev.destroy_pipeline(self.pose, None);
            dev.destroy_pipeline_layout(self.comp_layout, None);
            dev.destroy_descriptor_set_layout(self.comp_set_layout, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}
