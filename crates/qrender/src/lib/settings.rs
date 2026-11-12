use super::*;

impl Renderer {
    /// Highest MSAA sample count the device supports for scene targets.
    pub fn max_msaa(&self) -> u32 {
        self.gpu.max_samples
    }

    /// Highest anisotropy the device supports (1 = none).
    pub fn max_anisotropy(&self) -> f32 {
        self.gpu.max_anisotropy
    }

    /// Applies MSAA / render scale / texture filtering from `st`, rebuilding
    /// targets, pipelines or samplers when they changed.
    pub(super) fn apply_settings(&mut self, st: &Settings) -> Result<()> {
        let msaa = match st.msaa {
            0 | 1 => 1,
            n => n.next_power_of_two().min(self.gpu.max_samples).max(1),
        };
        let scale = st.render_scale.clamp(0.25, 2.0);
        let supersample = st.supersample && self.gpu.sample_shading;
        if msaa != self.msaa || supersample != self.pipes.supersample {
            self.idle();
            self.msaa = msaa;
            self.pipes.supersample = supersample;
            let fmt = self.pipes.post_format;
            self.pipes.build(&self.gpu, fmt, msaa)?;
            self.destroy_targets();
        }
        if scale != self.render_scale {
            self.render_scale = scale;
            self.idle();
            self.destroy_targets();
        }
        // TexMgr_ApplySettings: gl_texturemode (r_texfilter picks the default),
        // anisotropy, lod bias; softemu 2/3 or lightmap banding force nearest
        // with linear mips and at least 8x anisotropy.
        let mut mode = st.texture_mode.unwrap_or(if st.texture_linear { 5 } else { 2 });
        let mut aniso = st.anisotropy;
        if mode != 0 && (st.softemu >= 2 || st.lightmap_banding > 0.0) {
            mode = 2;
            aniso = aniso.max(8.0);
        }
        let aniso = aniso.clamp(1.0, self.gpu.max_anisotropy);
        let bias = st.lod_bias.unwrap_or(if supersample && msaa > 1 { -(msaa as f32).log2() / 2.0 } else { 0.0 });
        let filter = (mode, aniso, bias);
        if filter != self.world_filter {
            self.idle();
            let (_, lin, mipped, lin_mips) = TEXTURE_MODES[mode];
            let f = if lin { vk::Filter::LINEAR } else { vk::Filter::NEAREST };
            let ci = vk::SamplerCreateInfo::default()
                .mag_filter(f)
                .min_filter(f)
                .mipmap_mode(if lin_mips { vk::SamplerMipmapMode::LINEAR } else { vk::SamplerMipmapMode::NEAREST })
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .mip_lod_bias(bias)
                .anisotropy_enable(aniso > 1.0 && mipped)
                .max_anisotropy(aniso)
                .max_lod(if mipped { vk::LOD_CLAMP_NONE } else { 0.0 });
            // SAFETY: valid device.
            let new = unsafe { self.gpu.device.create_sampler(&ci, None)? };
            let old = std::mem::replace(&mut self.sampler_world, new);
            if old != self.sampler_repeat {
                // SAFETY: device idle; descriptors are rewritten below.
                unsafe { self.gpu.device.destroy_sampler(old, None) };
            }
            self.world_filter = filter;
            for i in 0..self.textures.len() {
                if let Some(Tex { image: Some(img), sampling: Sampling::World, .. }) = &self.textures[i] {
                    self.write_texture_descriptor(i as u32, img.view, new);
                }
            }
        }
        // GLPalette_UpdateLookupTable: auto metric is OKLab except in banded
        // mode on maps without coloured light.
        if st.softemu > 0 {
            let lit = self.world.as_ref().is_some_and(|w| w.cpu.lit);
            let metric = if st.softemu_metric < 0 {
                if st.softemu != 3 || lit { 2 } else { 0 }
            } else {
                st.softemu_metric.min(2)
            };
            if metric != self.lut_metric {
                self.idle();
                let lut = convert::palette_lut(&self.palette, metric as u32);
                self.palette_buf.bytes_mut()[1024..1024 + lut.len()].copy_from_slice(&lut);
                self.lut_metric = metric;
            }
        }
        Ok(())
    }
}
