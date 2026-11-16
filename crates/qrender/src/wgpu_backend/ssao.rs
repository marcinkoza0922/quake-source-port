//! Screen-space ambient occlusion (r_ssao), a port of the Vulkan backend's
//! AO passes (`shaders/ao.wgsl`). The scene pass is split after the opaque
//! draws (`ViewDraw::split`); AO is generated from scene depth into a half-
//! (quality 0/1) or full-resolution (quality 2) target and multiplied into
//! the scene colour with a depth-aware blur, then the translucent draws
//! resume. With MSAA both passes read sample 0 of the multisampled targets
//! directly, so no sample-0 copy (Vulkan's aosource) is needed.

use super::fx_pipes::AO_FORMAT;
use super::pipes::{DrawU, UNIFORM_STRIDE};
use super::scene::Lists;
use super::{Renderer, Targets};

/// The AO target and the bind groups that read the scene for it.
pub(super) struct AoTargets {
    view: wgpu::TextureView,
    /// AO target size.
    size: (u32, u32),
    /// AO pixels per scene pixel.
    scale: f32,
    gen_group: wgpu::BindGroup,
    apply_group: wgpu::BindGroup,
}

/// One view's AO uniform record and its rectangle in the AO target.
pub(super) struct AoView {
    du: u32,
    rect: [u32; 4],
}

fn tex_group(dev: &wgpu::Device, layout: &wgpu::BindGroupLayout, a: &wgpu::TextureView, b: &wgpu::TextureView) -> wgpu::BindGroup {
    dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(a) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(b) },
        ],
    })
}

impl Renderer {
    /// AO target for a scene of `scene` pixels; `color`/`depth` are the views
    /// the scene is drawn into (multisampled when MSAA is on).
    pub(super) fn create_ao_targets(
        &self,
        quality: u32,
        scene: (u32, u32),
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> AoTargets {
        let scale = if quality < 2 { 0.5 } else { 1.0 };
        let size = (((scene.0 as f32 * scale) as u32).max(1), ((scene.1 as f32 * scale) as u32).max(1));
        let tex = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("ssao"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: AO_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let p = &self.pipes.scene;
        let gen_group = tex_group(&self.dev, &p.fx.ao_gen_bgl, depth, color);
        let apply_group = tex_group(&self.dev, &p.fx.ao_apply_bgl, depth, &view);
        AoTargets { view, size, scale, gen_group, apply_group }
    }

    /// Per-view AO uniform records (empty when SSAO is off).
    pub(super) fn ao_views(&self, l: &mut Lists, st: &crate::Settings) -> Vec<AoView> {
        let Some(ao) = self.targets.as_ref().and_then(|t| t.ao.as_ref()) else { return Vec::new() };
        let (aw, ah) = ao.size;
        let mut out = Vec::with_capacity(l.views.len());
        for i in 0..l.views.len() {
            let v = &l.views[i];
            let x = ((v.ofs[0] as f32 * ao.scale) as u32).min(aw - 1);
            let y = ((v.ofs[1] as f32 * ao.scale) as u32).min(ah - 1);
            let w = ((v.scene[0] as f32 * ao.scale).round() as u32).clamp(1, aw - x);
            let h = ((v.scene[1] as f32 * ao.scale).round() as u32).clamp(1, ah - y);
            let mut pad = [0.0; 32];
            pad[0] = v.fog;
            let du = l.draw(DrawU {
                color: [x as f32, y as f32, w as f32, h as f32],
                params: [v.tan[0], v.tan[1], st.ssao_radius.clamp(4.0, 256.0), st.ssao.min(4.0)],
                extra: [v.ofs[0] as f32, v.ofs[1] as f32, v.scene[0] as f32, v.scene[1] as f32],
                ix: [st.ssao_quality.min(2), 0, 0, 0],
                pad,
                ..Default::default()
            });
            out.push(AoView { du, rect: [x, y, w, h] });
        }
        out
    }

    /// Generates AO for every view and multiplies it into the scene colour.
    /// Runs between the opaque and translucent scene passes.
    pub(super) fn encode_ao(&self, enc: &mut wgpu::CommandEncoder, t: &Targets, l: &Lists, views: &[AoView]) {
        let (Some(ao), Some(draw_group)) = (&t.ao, &self.bufs.draw_group) else { return };
        let p = &self.pipes.scene;
        {
            let ts = self.pass_times(super::timing::AO_GENERATE);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssao"),
                timestamp_writes: super::timing::writes(&ts),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ao.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 0.0, b: 0.0, a: 0.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&p.fx.ao_gen);
            pass.set_bind_group(1, &ao.gen_group, &[]);
            for (v, a) in l.views.iter().zip(views) {
                if v.scene[0] == 0 || v.scene[1] == 0 {
                    continue;
                }
                let [x, y, w, h] = a.rect;
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                pass.set_bind_group(0, draw_group, &[a.du * UNIFORM_STRIDE as u32]);
                pass.draw(0..3, 0..1);
            }
        }
        let ts = self.pass_times(super::timing::AO_APPLY);
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ssao apply"),
            timestamp_writes: super::timing::writes(&ts),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: t.ms_view.as_ref().unwrap_or(&t.color_view),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&p.fx.ao_apply);
        pass.set_bind_group(1, &ao.apply_group, &[]);
        for (v, a) in l.views.iter().zip(views) {
            let ([x, y], [w, h]) = (v.ofs, v.scene);
            if w == 0 || h == 0 {
                continue;
            }
            pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
            pass.set_scissor_rect(x, y, w, h);
            pass.set_bind_group(0, draw_group, &[a.du * UNIFORM_STRIDE as u32]);
            pass.draw(0..3, 0..1);
        }
    }
}
