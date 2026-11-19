use super::*;
use pipelines::SHADOW_FORMAT;

/// Cube shadow maps of real-time dynamic lights (r_shadow_realtime_dlight):
/// a 2D array with six layers per shadowed light, one per cube face. The
/// shaders pick the face themselves and project with the same matrices the
/// faces were rendered with (`Lights::shadow_vp` in common.glsl).
pub(super) struct ShadowMaps {
    pub(super) image: Image,
    /// One depth view per layer, for rendering.
    pub(super) layers: Vec<vk::ImageView>,
    /// Face size in pixels.
    pub(super) size: u32,
    /// Lights the array has room for.
    pub(super) lights: usize,
}

impl Renderer {
    /// (Re)creates the shadow maps for `lights` lights of `size`² faces and
    /// points binding 11 of every frame's set at them.
    pub(super) fn create_shadow_maps(&mut self, lights: usize, size: u32) -> Result<()> {
        self.idle();
        self.destroy_shadow_maps();
        let n = (lights.max(1) * 6) as u32;
        let image = self.gpu.image(
            size,
            size,
            n,
            1,
            SHADOW_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
        )?;
        let mut layers = Vec::with_capacity(n as usize);
        for l in 0..n {
            let vci = vk::ImageViewCreateInfo::default()
                .image(image.image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(SHADOW_FORMAT)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: l,
                    layer_count: 1,
                });
            // SAFETY: image bound to memory; valid create info.
            layers.push(unsafe { self.gpu.device.create_image_view(&vci, None)? });
        }
        let info = [vk::DescriptorImageInfo {
            sampler: self.sampler_shadow,
            image_view: image.view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        for f in &self.frames {
            let w = [vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(11)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&info)];
            // SAFETY: device idle; handles valid.
            unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
        }
        self.shadow_maps = Some(ShadowMaps { image, layers, size, lights });
        Ok(())
    }

    pub(super) fn destroy_shadow_maps(&mut self) {
        if let Some(s) = self.shadow_maps.take() {
            for v in s.layers {
                // SAFETY: callers idle the device first.
                unsafe { self.gpu.device.destroy_image_view(v, None) };
            }
            self.gpu.free_image(s.image);
        }
    }
}
