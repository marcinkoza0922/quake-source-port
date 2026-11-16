//! Enhanced water (r_enhancedwater): ripples, caustics and splashes, driven
//! by the shared CPU side (`crate::water::WaterFx`) like the Vulkan backend.
//! Ripple events go into one uniform buffer shared by every view
//! (`WaterU`, frame group binding 4); the water and world shaders in
//! `material.wgsl` evaluate the rings, swell and caustics. Splash droplets
//! and spray puffs are drawn as soft quads with the particles.

use super::math::{V3, dot, sub};
use super::models::Model;
use super::scene::{Buckets, Cmd, Lists, Pipe, Src};
use super::{NO_TEX, Renderer};
use crate::water::{MAX_RIPPLES, WaterFx};
use crate::{FteBlend, FteVertex, Particle, PicId, RenderFrame};

/// Water uniforms (see `WaterU` in common.wgsl).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct WaterU {
    /// x ripple strength, y caustics strength, z ripple count, w quality.
    params: [f32; 4],
    /// xy centre, z liquid surface height, w start time.
    ripples: [[f32; 4]; MAX_RIPPLES],
    /// Amplitudes, 4 per vec4.
    amp: [[f32; 4]; MAX_RIPPLES / 4],
}

pub(super) struct Water {
    pub(super) fx: WaterFx,
    /// This frame's splash droplets.
    parts: Vec<Particle>,
    pub(super) buf: wgpu::Buffer,
    /// Tiling Worley caustic texture (linear, repeat).
    pub(super) caustic: wgpu::TextureView,
    pub(super) repeat: wgpu::Sampler,
    /// Soft blob for spray puffs and droplets.
    splash_pic: Option<PicId>,
}

impl Water {
    pub(super) fn new(dev: &wgpu::Device, queue: &wgpu::Queue, palette: &crate::convert::Palette) -> Water {
        let mut fx = WaterFx::default();
        fx.set_palette(palette);
        let buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("water"),
            size: std::mem::size_of::<WaterU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let caustic = super::textures::make_texture(dev, queue, 128, 128, &[&crate::water::caustic_texture(128)]).1;
        let repeat = dev.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Water { fx, parts: Vec::new(), buf, caustic, repeat, splash_pic: None }
    }
}

impl Renderer {
    /// Registers the spray picture (after the renderer exists).
    pub(super) fn init_water(&mut self) {
        let blob = crate::water::spray_blob(32);
        self.water.splash_pic = self.register_pic_rgba_linear(32, 32, &blob).ok();
    }

    /// Steps ripples/splashes for this frame and uploads the water uniforms.
    pub(super) fn update_water(&mut self, fr: &RenderFrame) {
        let st = &fr.settings;
        let primary = fr.splits.first().map(|s| s.view).or(fr.view);
        let w = &mut self.water;
        w.parts.clear();
        if st.water_ripples <= 0.0 && !st.water_splashes {
            w.fx.reset();
        } else if let (Some(wr), Some(view)) = (&self.world, primary) {
            let models = &self.models;
            let movers = fr.entities.iter().filter_map(|e| {
                let alias = matches!(models.get(e.model.0 as usize), Some(Model::Alias(_)));
                (alias && e.key != 0 && !e.viewmodel).then_some((e.key, e.origin))
            });
            w.parts = w.fx.update(
                &wr.cpu,
                fr.time,
                movers,
                fr.player,
                Some(view.origin),
                fr.shot_impacts,
                st.water_ripples > 0.0,
                st.water_splashes,
                st.water_quality.min(1),
            );
        }
        let n = w.fx.ripples.len().min(MAX_RIPPLES);
        let mut u = WaterU {
            params: [
                st.water_ripples.clamp(0.0, 4.0),
                st.water_caustics.clamp(0.0, 4.0),
                if st.water_ripples > 0.0 || st.water_caustics > 0.0 { n as f32 } else { 0.0 },
                st.water_quality.min(1) as f32,
            ],
            ripples: [[0.0; 4]; MAX_RIPPLES],
            amp: [[0.0; 4]; MAX_RIPPLES / 4],
        };
        for (i, r) in w.fx.ripples.iter().take(n).enumerate() {
            u.ripples[i] = [r.pos[0], r.pos[1], r.pos[2], r.t0];
            u.amp[i / 4][i % 4] = r.amp;
        }
        self.queue.write_buffer(&w.buf, 0, crate::bytes_of(&[u]));
    }

    /// Spray puffs and splash droplets as soft camera-facing quads.
    /// `px`: world units per pixel at distance 1 (keeps distant drops ~1.5 px).
    pub(super) fn splash_cmds(&self, fr: &RenderFrame, eye: V3, axes: [V3; 3], px: f32, lists: &mut Lists, b: &mut Buckets) {
        let Some(slot) = self.water.splash_pic.and_then(|p| self.pics.get(p.0 as usize)).map(|p| p.slot) else { return };
        let [fwd, right, up] = axes;
        let drops = self.water.parts.iter().map(|d| {
            let c = self.palette[d.color as usize];
            let dist = dot(sub(d.origin, eye), fwd).max(1.0);
            crate::water::SprayPuff {
                pos: d.origin,
                radius: (0.75f32).max(0.75 * px * dist),
                color: [c[0], c[1], c[2], (d.alpha.clamp(0.0, 1.0) * 255.0) as u8],
            }
        });
        let (base, first) = (lists.poly_v.len() as u32, lists.poly_i.len() as u32);
        for p in self.water.fx.spray(fr.time).chain(drops) {
            if p.color[3] == 0 {
                continue;
            }
            let n = lists.poly_v.len() as u32 - base;
            for (sr, su, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                let pos = std::array::from_fn(|k| p.pos[k] + (right[k] * sr + up[k] * su) * p.radius);
                lists.poly_v.push(FteVertex { pos, uv, color: p.color });
            }
            lists.poly_i.extend_from_slice(&[n, n + 1, n + 2, n, n + 2, n + 3]);
        }
        let count = lists.poly_i.len() as u32 - first;
        if count == 0 {
            return;
        }
        // soft like Vulkan's spray batch
        let soft = self.soft_particles(&fr.settings);
        let du = lists.draw(super::scene::poly_params(FteBlend::Blend, soft));
        let pipe = if soft.is_some() { Pipe::SoftPoly(FteBlend::Blend as usize) } else { Pipe::Poly(super::pipes::poly_index(FteBlend::Blend, false)) };
        let cmd = Cmd { pipe, src: Src::Poly, mat: [slot, NO_TEX, NO_TEX], first, count, base_vertex: base as i32, du, viewmodel: false };
        if soft.is_some() {
            b.soft.push(cmd);
        } else {
            b.trans.push((-1.5, cmd));
        }
    }
}
