use super::*;

impl Pipelines {
    pub(super) fn graphics_pipes(&self) -> Vec<vk::Pipeline> {
        let mut v = vec![
            self.world,
            self.world_alpha,
            self.water,
            self.water_alpha,
            self.sky,
            self.alias,
            self.alias_alpha,
            self.alias_xray,
            self.poly,
            self.particle,
            self.ui,
            self.post,
            self.ao,
            self.ao_apply,
            self.vel_alias,
            self.vel_world,
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
            ao_fs,
            ao_apply_fs,
            vel_alias_vs,
            vel_world_vs,
            vel_fs,
        } = self.sh;
        let layout = self.layout;
        let f32x3 = vk::Format::R32G32B32_SFLOAT;
        let f32x2 = vk::Format::R32G32_SFLOAT;
        let wb = [bind(0, 40, false)];
        let wa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, f32x2, 12),
            attr(2, 0, f32x3, 20),
            attr(3, 0, f32x2, 32),
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
            behind: false,
        };
        let g = |d: Desc| graphics(gpu, layout, &d);
        let world = g(d(world_vs, world_fs, &wb, &wa, Blend::None, true, true))?;
        let world_alpha = g(d(world_vs, world_fs, &wb, &wa, Blend::Alpha, true, false))?;
        let water = g(d(world_vs, water_fs, &wb, &wa, Blend::None, true, true))?;
        let water_alpha = g(d(world_vs, water_fs, &wb, &wa, Blend::Alpha, true, false))?;
        let sky = g(d(world_vs, sky_fs, &wb, &wa, Blend::None, true, true))?;
        let alias = g(Desc { cull: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::None, true, true) })?;
        let alias_alpha = g(Desc { cull: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::Alpha, true, false) })?;
        // thermal goggles: hot bodies behind walls, added on top
        let one = vk::BlendFactor::ONE;
        self.alias_xray = g(Desc { cull: true, behind: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::Factors(one, one), true, false) })?;
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
                behind: false,
            },
        )?;
        // post-effect passes: single-sampled, no depth attachment
        let fx = |vs, fs, bindings, attrs, color, cull| Desc {
            color,
            depth: false,
            samples: 1,
            cull,
            sample_shading: false,
            ..d(vs, fs, bindings, attrs, Blend::None, false, false)
        };
        self.ao = graphics(gpu, layout, &fx(post_vs, ao_fs, &[], &[], AO_FORMAT, false))?;
        self.ao_apply = graphics(
            gpu,
            layout,
            &Desc {
                depth: false,
                sample_shading: false,
                ..d(post_vs, ao_apply_fs, &[], &[], Blend::Factors(vk::BlendFactor::ZERO, vk::BlendFactor::SRC_COLOR), false, false)
            },
        )?;
        self.vel_alias = graphics(gpu, layout, &fx(vel_alias_vs, vel_fs, &ab, &aa, VEL_FORMAT, true))?;
        self.vel_world = graphics(gpu, layout, &fx(vel_world_vs, vel_fs, &wb, &wa, VEL_FORMAT, false))?;
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
}
