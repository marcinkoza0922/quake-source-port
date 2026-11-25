use super::*;

impl Pipelines {
    /// Pipelines to use for single-sampled portal/teleporter views.
    pub fn portal_pipes(&self) -> ScenePipes {
        if self.samples > 1 {
            self.portal
        } else {
            ScenePipes {
                world: self.world,
                world_alpha: self.world_alpha,
                water: self.water,
                water_alpha: self.water_alpha,
                sky: self.sky,
                alias: self.alias,
                alias_alpha: self.alias_alpha,
                particle: self.particle,
                fte: self.fte,
                fte_quad: self.fte_quad,
            }
        }
    }

    pub(crate) fn graphics_pipes(&self) -> Vec<vk::Pipeline> {
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
            self.post_fast,
            self.ao,
            self.ao_source,
            self.ao_down,
            self.ao_apply,
            self.droplets,
            self.vel_alias,
            self.vel_world,
            self.bloom_down,
            self.bloom_up,
            self.shadow_world,
            self.shadow_alias,
            self.portal_alias_mirror,
            self.water_rt,
            self.water_rt_alpha,
            self.rt_depth_world,
            self.water_rt_half,
        ];
        v.extend(self.fte);
        v.extend(self.fte_quad);
        v.extend(self.wire_world);
        v.extend(self.wire_alias);
        v.extend(self.wire_sprite);
        v.extend(self.portal.all().filter(|&p| p != vk::Pipeline::null()));
        v.extend(self.bloom_glow.all().filter(|&p| p != vk::Pipeline::null()));
        v
    }

    /// Sets every graphics pipeline to null (destroyed by the caller).
    pub(super) fn forget_graphics(&mut self) {
        let null = vk::Pipeline::null();
        for p in [
            &mut self.world,
            &mut self.world_alpha,
            &mut self.water,
            &mut self.water_alpha,
            &mut self.sky,
            &mut self.alias,
            &mut self.alias_alpha,
            &mut self.alias_xray,
            &mut self.poly,
            &mut self.particle,
            &mut self.ui,
            &mut self.post,
            &mut self.post_fast,
            &mut self.ao,
            &mut self.ao_source,
            &mut self.ao_down,
            &mut self.ao_apply,
            &mut self.droplets,
            &mut self.vel_alias,
            &mut self.vel_world,
            &mut self.bloom_down,
            &mut self.bloom_up,
            &mut self.shadow_world,
            &mut self.shadow_alias,
            &mut self.portal_alias_mirror,
            &mut self.water_rt,
            &mut self.water_rt_alpha,
            &mut self.rt_depth_world,
            &mut self.water_rt_half,
        ] {
            *p = null;
        }
        self.fte = [null; 17];
        self.fte_quad = [null; 8];
        self.wire_world = [null; 2];
        self.wire_alias = [null; 2];
        self.wire_sprite = [null; 2];
        self.portal = ScenePipes::null();
        self.bloom_glow = BloomPipes::null();
    }

    /// (Re)creates the graphics pipelines for an output format and scene
    /// sample count. The device must be idle. After an error there are none
    /// and `samples` is 0.
    pub fn build(&mut self, gpu: &Gpu, post_format: vk::Format, samples: u32) -> Result<()> {
        for p in self.graphics_pipes() {
            // SAFETY: device idle (caller contract); null is allowed.
            unsafe { gpu.device.destroy_pipeline(p, None) };
        }
        self.forget_graphics();
        self.samples = 0;
        gpu.journal.pipelines.borrow_mut().clear();
        let r = self.build_all(gpu, post_format, samples);
        let made = gpu.journal.pipelines.take();
        if r.is_err() {
            for p in made {
                // SAFETY: made just now, never used.
                unsafe { gpu.device.destroy_pipeline(p, None) };
            }
            self.forget_graphics();
        }
        r
    }

    fn build_all(&mut self, gpu: &Gpu, post_format: vk::Format, samples: u32) -> Result<()> {
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
            fte_quad_vs,
            fte_fs,
            ui_vs,
            ui_fs,
            post_vs,
            post_fs,
            ao_fs,
            ao_source_fs,
            ao_down_fs,
            ao_apply_fs,
            droplets_fs,
            vel_alias_vs,
            vel_world_vs,
            vel_fs,
            bloom_fs,
            shadow_world_fs,
            shadow_alias_fs,
            bloom: bloom_sh,
        } = self.sh;
        let layout = self.layout;
        let f32x3 = vk::Format::R32G32B32_SFLOAT;
        let f32x2 = vk::Format::R32G32_SFLOAT;
        let wb = [bind(0, std::mem::size_of::<crate::world::WorldVertex>() as u32, false)];
        let wa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, f32x2, 12),
            attr(2, 0, f32x3, 20),
            attr(3, 0, f32x2, 32),
            attr(4, 0, f32x3, 40),
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
            depth_only: false,
            no_color_write: false,
            shadow: false,
            mirror: false,
            wire: false,
            frag_spec_bool: None,
        };
        let g = |d: Desc| graphics(gpu, layout, &d);
        let fb = [bind(0, 24, false)];
        let fa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, f32x2, 12),
            attr(2, 0, vk::Format::R8G8B8A8_UNORM, 20),
        ];
        let fqb = [bind(0, std::mem::size_of::<crate::FteQuad>() as u32, true)];
        let fqa = [
            attr(0, 0, f32x3, 0),
            attr(1, 0, vk::Format::R32_SFLOAT, 12),
            attr(2, 0, vk::Format::R32_SFLOAT, 16),
            attr(3, 0, vk::Format::R8G8B8A8_UNORM, 20),
            attr(4, 0, vk::Format::R32G32B32A32_SFLOAT, 24),
        ];
        let fte_supersample = self.fte_supersample;
        let build_scene = |pipe_samples, pipe_ss| -> Result<ScenePipes> {
            let sd = |vs, fs, bindings, attrs, blend, dt, dw| Desc {
                samples: pipe_samples,
                sample_shading: pipe_ss,
                ..d(vs, fs, bindings, attrs, blend, dt, dw)
            };
            let sg = |desc: Desc| graphics(gpu, layout, &desc);
            let mut fte = [vk::Pipeline::null(); 17];
            for (i, slot) in fte.iter_mut().enumerate() {
                let overlay = i == 16;
                let b = if overlay { crate::FteBlend::Overlay } else { crate::FteBlend::ALL[i % 8] };
                let (src, dst) = fte_factors(b);
                *slot = sg(Desc {
                    sample_shading: pipe_ss && fte_supersample,
                    topology: if i >= 8 { vk::PrimitiveTopology::LINE_LIST } else { vk::PrimitiveTopology::TRIANGLE_LIST },
                    ..sd(fte_vs, fte_fs, &fb, &fa, Blend::Factors(src, dst), !overlay, false)
                })?;
            }
            let mut fte_quad = [vk::Pipeline::null(); 8];
            for (i, slot) in fte_quad.iter_mut().enumerate() {
                let (src, dst) = fte_factors(crate::FteBlend::ALL[i]);
                *slot = sg(Desc {
                    sample_shading: pipe_ss && fte_supersample,
                    ..sd(fte_quad_vs, fte_fs, &fqb, &fqa, Blend::Factors(src, dst), true, false)
                })?;
            }
            Ok(ScenePipes {
                world: sg(sd(world_vs, world_fs, &wb, &wa, Blend::None, true, true))?,
                world_alpha: sg(sd(world_vs, world_fs, &wb, &wa, Blend::Alpha, true, false))?,
                water: sg(sd(world_vs, water_fs, &wb, &wa, Blend::None, true, true))?,
                water_alpha: sg(sd(world_vs, water_fs, &wb, &wa, Blend::Alpha, true, false))?,
                sky: sg(sd(world_vs, sky_fs, &wb, &wa, Blend::None, true, true))?,
                alias: sg(Desc { cull: true, ..sd(alias_vs, alias_fs, &ab, &aa, Blend::None, true, true) })?,
                alias_alpha: sg(Desc { cull: true, ..sd(alias_vs, alias_fs, &ab, &aa, Blend::Alpha, true, false) })?,
                particle: sg(sd(part_vs, part_fs, &qb, &qa, Blend::Alpha, true, false))?,
                fte,
                fte_quad,
            })
        };
        let scene = build_scene(samples, supersample)?;
        // ray-traced reflections: main-view water and half-res single-sampled trace pass
        if let Some(rt) = &self.rt {
            self.water_rt = g(d(world_vs, rt.water_fs, &wb, &wa, Blend::None, true, true))?;
            self.water_rt_alpha = g(d(world_vs, rt.water_fs, &wb, &wa, Blend::Alpha, true, false))?;
            self.rt_depth_world = g(Desc {
                samples: 1,
                sample_shading: false,
                no_color_write: true,
                ..d(world_vs, rt.depth_fs, &wb, &wa, Blend::None, true, true)
            })?;
            self.water_rt_half = g(Desc {
                samples: 1,
                sample_shading: false,
                ..d(world_vs, rt.water_half_fs, &wb, &wa, Blend::None, true, true)
            })?;
        }
        // reflection views: single-sampled alias models with mirrored winding
        self.portal_alias_mirror = g(Desc { cull: true, samples: 1, sample_shading: false, mirror: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::None, true, true) })?;
        // portal views are single-sampled
        self.portal = if samples > 1 { build_scene(1, false)? } else { ScenePipes::null() };
        // thermal goggles: hot bodies behind walls, added on top
        let one = vk::BlendFactor::ONE;
        self.alias_xray = g(Desc { cull: true, behind: true, ..d(alias_vs, alias_fs, &ab, &aa, Blend::Factors(one, one), true, false) })?;
        if self.wire_enabled {
            for (i, depth) in [true, false].into_iter().enumerate() {
                self.wire_world[i] = g(Desc {
                    wire: true, ..d(world_vs, world_fs, &wb, &wa, Blend::Alpha, depth, false)
                })?;
                self.wire_alias[i] = g(Desc {
                    wire: true, cull: depth, ..d(alias_vs, alias_fs, &ab, &aa, Blend::Alpha, depth, false)
                })?;
                self.wire_sprite[i] = g(Desc {
                    wire: true, ..d(poly_vs, poly_fs, &pb, &pa, Blend::Alpha, depth, false)
                })?;
            }
        }
        let poly = g(d(poly_vs, poly_fs, &pb, &pa, Blend::Alpha, true, true))?;
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
        let post_desc = Desc {
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
            depth_only: false,
            no_color_write: false,
            shadow: false,
            mirror: false,
            wire: false,
            frag_spec_bool: Some(false),
        };
        let post = graphics(gpu, layout, &post_desc)?;
        self.post_fast = graphics(gpu, layout, &Desc { frag_spec_bool: Some(true), ..post_desc })?;
        // post-effect passes: single-sampled, no depth attachment
        let fx = |vs, fs, bindings, attrs, color, cull| Desc {
            color,
            depth: false,
            samples: 1,
            cull,
            sample_shading: false,
            ..d(vs, fs, bindings, attrs, Blend::None, false, false)
        };
        self.ao_source = graphics(gpu, layout, &fx(post_vs, ao_source_fs, &[], &[], AO_SOURCE_FORMAT, false))?;
        self.ao_down = graphics(gpu, layout, &fx(post_vs, ao_down_fs, &[], &[], AO_SOURCE_FORMAT, false))?;
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
        self.droplets = graphics(gpu, layout, &fx(post_vs, droplets_fs, &[], &[], DROPLET_FORMAT, false))?;
        self.vel_alias = graphics(gpu, layout, &fx(vel_alias_vs, vel_fs, &ab, &aa, VEL_FORMAT, true))?;
        self.vel_world = graphics(gpu, layout, &fx(vel_world_vs, vel_fs, &wb, &wa, VEL_FORMAT, false))?;
        // r_shadow_realtime_dlight: two-sided casters, single-sampled
        let shadow = |vs, fs, bindings, attrs| Desc {
            samples: 1,
            sample_shading: false,
            shadow: true,
            ..d(vs, fs, bindings, attrs, Blend::None, true, true)
        };
        self.shadow_world = graphics(gpu, layout, &shadow(world_vs, shadow_world_fs, &wb, &wa))?;
        self.shadow_alias = graphics(gpu, layout, &shadow(alias_vs, shadow_alias_fs, &ab, &aa))?;
        // narrow single-sampled half/quarter-res bloom glow pass into level 0
        let bg = |vs, fs, bindings, attrs, blend, cull| Desc {
            color: BLOOM_FORMAT,
            samples: 1,
            sample_shading: false,
            cull,
            ..d(vs, fs, bindings, attrs, blend, true, false)
        };
        let mut bloom_fte = [vk::Pipeline::null(); 9];
        let glow_blends = [crate::FteBlend::Blend, crate::FteBlend::AddA, crate::FteBlend::AddC, crate::FteBlend::Premul];
        for (i, slot) in bloom_fte.iter_mut().enumerate() {
            let overlay = i == 8;
            let b = if overlay { crate::FteBlend::Overlay } else { glow_blends[i % 4] };
            let (src, dst) = fte_factors(b);
            *slot = g(Desc {
                depth_test: !overlay,
                topology: if (4..8).contains(&i) { vk::PrimitiveTopology::LINE_LIST } else { vk::PrimitiveTopology::TRIANGLE_LIST },
                ..bg(fte_vs, bloom_sh.fte, &fb, &fa, Blend::Factors(src, dst), false)
            })?;
        }
        let mut bloom_fte_quad = [vk::Pipeline::null(); 4];
        for (i, slot) in bloom_fte_quad.iter_mut().enumerate() {
            let (src, dst) = fte_factors(glow_blends[i]);
            *slot = g(bg(fte_quad_vs, bloom_sh.fte, &fqb, &fqa, Blend::Factors(src, dst), false))?;
        }
        self.bloom_glow = BloomPipes {
            depth: g(Desc {
                depth_only: true,
                samples: 1,
                sample_shading: false,
                ..d(post_vs, bloom_sh.depth, &[], &[], Blend::None, true, true)
            })?,
            world: g(bg(world_vs, bloom_sh.world, &wb, &wa, Blend::None, false))?,
            world_alpha: g(bg(world_vs, bloom_sh.world, &wb, &wa, Blend::Alpha, false))?,
            water: g(bg(world_vs, bloom_sh.water, &wb, &wa, Blend::None, false))?,
            water_alpha: g(bg(world_vs, bloom_sh.water, &wb, &wa, Blend::Alpha, false))?,
            alias: g(bg(alias_vs, bloom_sh.alias, &ab, &aa, Blend::None, true))?,
            alias_alpha: g(bg(alias_vs, bloom_sh.alias, &ab, &aa, Blend::Alpha, true))?,
            poly: g(bg(poly_vs, bloom_sh.poly, &pb, &pa, Blend::Alpha, false))?,
            particle: g(bg(part_vs, bloom_sh.part, &qb, &qa, Blend::Alpha, false))?,
            fte: bloom_fte,
            fte_quad: bloom_fte_quad,
        };
        self.bloom_down = graphics(gpu, layout, &fx(post_vs, bloom_fs, &[], &[], BLOOM_FORMAT, false))?;
        self.bloom_up = graphics(gpu, layout, &Desc { blend: Blend::Factors(one, one), ..fx(post_vs, bloom_fs, &[], &[], BLOOM_FORMAT, false) })?;
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
        ) = (
            scene.world,
            scene.world_alpha,
            scene.water,
            scene.water_alpha,
            scene.sky,
            scene.alias,
            scene.alias_alpha,
            poly,
            scene.particle,
            ui,
            post,
        );
        self.fte = scene.fte;
        self.fte_quad = scene.fte_quad;
        self.post_format = post_format;
        self.samples = samples;
        Ok(())
    }
}
