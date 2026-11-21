//! Per-object motion blur (r_motionblur), a port of the Vulkan backend's
//! velocity pass. Entities that moved since the last frame are redrawn into a
//! velocity target (`shaders/vel.wgsl`) with their previous transform, only
//! where they are the visible surface; the post pass then smears along the
//! strongest nearby motion. Alias models are posed on the CPU in world space
//! with the current pose, so their previous position is
//! `M_prev · M_cur⁻¹ · p` (the Vulkan backend also uses the current pose).

use super::fx_pipes::VEL_FORMAT;
use super::math::{Mat4, V3, affine, dot, entity_rotation, sub};
use super::pipes::UNIFORM_STRIDE;
use super::scene::{Lists, Src};
use super::{Renderer, Targets};
use crate::{RenderEntity, RenderFrame};
use std::collections::HashMap;

/// Last and this frame's entity transforms by entity key.
#[derive(Default)]
pub(super) struct Motion {
    prev: HashMap<u32, (V3, V3)>,
    cur: HashMap<u32, (V3, V3)>,
    prev_time: f64,
    /// `prev` is usable this frame (motion blur on, last frame recent).
    active: bool,
    /// Blur length factor: r_motionblur × the motion over 1/60 s.
    pub(super) scale: f32,
}

impl Motion {
    /// Records this frame's transforms; call before building the views.
    pub(super) fn begin(&mut self, fr: &RenderFrame) {
        let st = &fr.settings;
        let on = st.motion_blur > 0.0 && (fr.view.is_some() || !fr.splits.is_empty());
        let dt = fr.time - self.prev_time;
        self.active = on && dt > 0.0 && dt < 0.25;
        self.scale = if self.active { st.motion_blur.min(4.0) / (60.0 * dt as f32) } else { 0.0 };
        self.cur.clear();
        if on {
            for e in fr.entities {
                if e.key != 0 && !e.viewmodel && !e.follows_camera && !e.portal_only {
                    self.cur.insert(e.key, (e.origin, e.angles));
                }
            }
        }
    }

    /// Makes this frame's transforms the previous ones.
    pub(super) fn end(&mut self, time: f64) {
        std::mem::swap(&mut self.prev, &mut self.cur);
        self.prev_time = time;
    }

    /// Last frame's model transform of `e`, when it moved (and didn't teleport).
    pub(super) fn prev_transform(&self, e: &RenderEntity) -> Option<Mat4> {
        if !self.active || e.key == 0 || e.viewmodel || e.follows_camera || e.alpha < 1.0 {
            return None;
        }
        let &(po, pa) = self.prev.get(&e.key)?;
        if po == e.origin && pa == e.angles {
            return None;
        }
        let d = sub(e.origin, po);
        if dot(d, d) > 256.0 * 256.0 {
            return None;
        }
        Some(affine(entity_rotation(pa), po))
    }
}

/// The velocity target and the bind group reading scene depth for it.
pub(super) struct VelTarget {
    view: wgpu::TextureView,
    /// Velocity pixels per scene pixel.
    scale: f32,
    group: wgpu::BindGroup,
}

impl VelTarget {
    pub(super) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

impl Renderer {
    /// Velocity target for a scene of `scene` pixels: half (quality 0) or
    /// full resolution; `depth` is the scene depth view.
    pub(super) fn create_vel_target(&self, quality: u32, scene: (u32, u32), depth: &wgpu::TextureView) -> Option<VelTarget> {
        let fx = self.pipes.scene.fx.as_ref()?;
        let scale = if quality == 0 { 0.5 } else { 1.0 };
        let size = (((scene.0 as f32 * scale) as u32).max(1), ((scene.1 as f32 * scale) as u32).max(1));
        let tex = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("velocity"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VEL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let group = self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &fx.vel_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) }],
        });
        Some(VelTarget { view: tex.create_view(&Default::default()), scale, group })
    }

    /// `FrameU::vel`: scene px per velocity px and the largest scene pixel.
    pub(super) fn vel_frame_params(&self) -> [f32; 4] {
        match &self.targets {
            Some(t) if t.vel.is_some() => {
                let k = t.vel.as_ref().map_or(1.0, |v| v.scale);
                [1.0 / k, 0.0, t.scene.0 as f32 - 1.0, t.scene.1 as f32 - 1.0]
            }
            _ => [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Draws every view's moving entities into the velocity target; false
    /// when nothing moved (the post pass then skips the blur).
    pub(super) fn encode_velocity(&self, enc: &mut wgpu::CommandEncoder, t: &Targets, l: &Lists) -> bool {
        let b = &self.bufs;
        let (Some(vel), Some(frame_group), Some(draw_group), Some(p)) = (&t.vel, &b.frame_group, &b.draw_group, &self.pipes.scene.fx) else {
            return false;
        };
        if l.views.iter().all(|v| v.vel.is_empty()) {
            return false;
        }
        let ts = self.pass_times(super::timing::VELOCITY);
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("velocity"),
            timestamp_writes: super::timing::writes(&ts),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &vel.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_bind_group(1, &vel.group, &[]);
        for v in &l.views {
            let k = vel.scale;
            let [x, y] = v.ofs.map(|o| (o as f32 * k) as u32);
            let [w, h] = v.scene.map(|s| ((s as f32 * k) as u32).max(1));
            if v.vel.is_empty() || v.scene[0] == 0 || v.scene[1] == 0 {
                continue;
            }
            pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
            pass.set_bind_group(0, frame_group, &[v.fu * UNIFORM_STRIDE as u32]);
            for c in &v.vel {
                match c.src {
                    Src::World => {
                        let Some(wr) = &self.world else { continue };
                        pass.set_pipeline(&p.vel_world);
                        pass.set_vertex_buffer(0, wr.vbuf.slice(..));
                        pass.set_index_buffer(b.world_idx.get().slice(..), wgpu::IndexFormat::Uint32);
                    }
                    Src::Alias(mi) => {
                        let Some(super::models::Model::Alias(am)) = self.models.get(mi) else { continue };
                        pass.set_pipeline(&p.vel_alias);
                        pass.set_vertex_buffer(0, b.alias_v.get().slice(..));
                        pass.set_index_buffer(am.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    }
                    Src::Poly => continue,
                }
                pass.set_bind_group(2, draw_group, &[c.du * UNIFORM_STRIDE as u32]);
                pass.draw_indexed(c.first..c.first + c.count, c.base_vertex, 0..1);
            }
        }
        true
    }
}
