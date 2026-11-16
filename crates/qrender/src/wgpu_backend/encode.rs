//! Command recording of a frame: the scene views (split around SSAO), the
//! motion velocity pass, the post pass onto the output, the 2D layer, and the
//! read-back copies.

use super::pipes::UNIFORM_STRIDE;
use super::scene::{Lists, Pipe, Src};
use super::ssao::AoView;
use super::{NO_TEX, Renderer, Targets};
use crate::{Error, Result};

/// Which of each view's draws a scene pass records (split around SSAO).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    All,
    Opaque,
    Rest,
}

impl Renderer {
    pub(super) fn encode(&mut self, l: &Lists, post_du: &[u32], ao_views: &[AoView], ui_du: u32, ui_batches: &[(u32, u32, u32)]) -> Result<()> {
        let frame_tex = match &self.surface {
            Some((s, cfg)) => match s.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
                _ => {
                    s.configure(&self.dev, cfg);
                    return Ok(());
                }
            },
            None => None,
        };
        let t = self.targets.as_ref().ok_or_else(|| Error::new("no targets"))?;
        let out_view = match (&frame_tex, &t.out) {
            (Some(ft), _) => ft.texture.create_view(&Default::default()),
            (None, Some((o, _, _))) => o.create_view(&Default::default()),
            (None, None) => return Err(Error::new("no output")),
        };
        let b = &self.bufs;
        let (Some(draw_group), Some(post_group)) = (&b.draw_group, &b.post_group) else {
            return Err(Error::new("bind groups missing"));
        };
        let mut enc = self.dev.create_command_encoder(&Default::default());
        if !l.views.is_empty() {
            if ao_views.is_empty() {
                self.scene_pass(&mut enc, t, l, Part::All);
            } else {
                self.scene_pass(&mut enc, t, l, Part::Opaque);
                self.encode_ao(&mut enc, t, l, ao_views);
                self.scene_pass(&mut enc, t, l, Part::Rest);
            }
            self.encode_velocity(&mut enc, t, l);
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("output"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipes.post);
            pass.set_bind_group(1, post_group, &[]);
            for (v, &du) in l.views.iter().zip(post_du) {
                let ([x, y, _, _], [ow, oh]) = (v.rect, v.out);
                if v.scene[0] == 0 || v.scene[1] == 0 || ow == 0 || oh == 0 {
                    continue;
                }
                pass.set_viewport(x as f32, y as f32, ow as f32, oh as f32, 0.0, 1.0);
                pass.set_bind_group(0, draw_group, &[du * UNIFORM_STRIDE as u32]);
                pass.draw(0..3, 0..1);
            }
            if !ui_batches.is_empty() {
                pass.set_viewport(0.0, 0.0, self.width as f32, self.height as f32, 0.0, 1.0);
                pass.set_pipeline(&self.pipes.ui);
                pass.set_bind_group(0, draw_group, &[ui_du * UNIFORM_STRIDE as u32]);
                pass.set_vertex_buffer(0, b.ui_v.get().slice(..));
                for &(slot, first, count) in ui_batches {
                    let Some(mat) = self.bind_groups.get(&[slot, NO_TEX, NO_TEX]) else { continue };
                    pass.set_bind_group(1, mat, &[]);
                    pass.draw(first..first + count, 0..1);
                }
            }
        }
        if let (None, Some((o, buf, bpr))) = (&frame_tex, &t.out) {
            enc.copy_texture_to_buffer(
                o.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(*bpr), rows_per_image: None },
                },
                o.size(),
            );
        }
        if let Some(ft) = &frame_tex {
            self.record_capture(&mut enc, &ft.texture);
        }
        self.queue.submit([enc.finish()]);
        self.start_capture_map();
        if let Some(ft) = frame_tex {
            self.queue.present(ft);
        }
        self.frame_ready = true;
        Ok(())
    }

    /// Records the scene pass for `part` of every view's draws.
    fn scene_pass(&self, enc: &mut wgpu::CommandEncoder, t: &Targets, l: &Lists, part: Part) {
        let b = &self.bufs;
        let (Some(frame_group), Some(draw_group)) = (&b.frame_group, &b.draw_group) else { return };
        let (first, last) = (part != Part::Rest, part != Part::Opaque);
        let keep_depth = t.vel.is_some() && l.views.iter().any(|v| !v.vel.is_empty());
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            // MSAA draws into ms_view and resolves into color_view at the end
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: t.ms_view.as_ref().unwrap_or(&t.color_view),
                depth_slice: None,
                resolve_target: t.ms_view.as_ref().filter(|_| last).map(|_| &t.color_view),
                ops: wgpu::Operations {
                    load: if first { wgpu::LoadOp::Clear(wgpu::Color::BLACK) } else { wgpu::LoadOp::Load },
                    store: if t.ms_view.is_some() && last { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if first { wgpu::LoadOp::Clear(1.0) } else { wgpu::LoadOp::Load },
                    // kept for SSAO (between the parts) and the velocity pass
                    store: if last && !keep_depth { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        for v in &l.views {
            let ([x, y], [sw, sh]) = (v.ofs, v.scene);
            if sw == 0 || sh == 0 {
                continue;
            }
            pass.set_scissor_rect(x, y, sw, sh);
            pass.set_bind_group(0, frame_group, &[v.fu * UNIFORM_STRIDE as u32]);
            let mut vm = None;
            let cmds = match part {
                Part::All => &v.cmds[..],
                Part::Opaque => &v.cmds[..v.split],
                Part::Rest => &v.cmds[v.split..],
            };
            for c in cmds {
                if vm != Some(c.viewmodel) {
                    vm = Some(c.viewmodel);
                    let maxz = if c.viewmodel { 0.3 } else { 1.0 };
                    pass.set_viewport(x as f32, y as f32, sw as f32, sh as f32, 0.0, maxz);
                }
                let p = &self.pipes.scene;
                pass.set_pipeline(match c.pipe {
                    Pipe::World => &p.world,
                    Pipe::WorldAlpha => &p.world_alpha,
                    Pipe::Water => &p.water,
                    Pipe::WaterAlpha => &p.water_alpha,
                    Pipe::Sky => &p.sky,
                    Pipe::Alias => &p.alias,
                    Pipe::AliasAlpha => &p.alias_alpha,
                    Pipe::Poly(i) => &p.poly[i],
                });
                let mat = if c.pipe == Pipe::Sky { self.sky_group.as_ref() } else { self.bind_groups.get(&c.mat) };
                let Some(mat) = mat else { continue };
                pass.set_bind_group(1, mat, &[]);
                pass.set_bind_group(2, draw_group, &[c.du * UNIFORM_STRIDE as u32]);
                match c.src {
                    Src::World => {
                        let Some(wr) = &self.world else { continue };
                        pass.set_vertex_buffer(0, wr.vbuf.slice(..));
                        pass.set_index_buffer(b.world_idx.get().slice(..), wgpu::IndexFormat::Uint32);
                    }
                    Src::Alias(mi) => {
                        let Some(super::models::Model::Alias(am)) = self.models.get(mi) else { continue };
                        pass.set_vertex_buffer(0, b.alias_v.get().slice(..));
                        pass.set_index_buffer(am.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    }
                    Src::Poly => {
                        pass.set_vertex_buffer(0, b.poly_v.get().slice(..));
                        pass.set_index_buffer(b.poly_i.get().slice(..), wgpu::IndexFormat::Uint32);
                    }
                }
                pass.draw_indexed(c.first..c.first + c.count, c.base_vertex, 0..1);
            }
        }
    }
}
