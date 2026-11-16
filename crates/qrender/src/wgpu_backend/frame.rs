//! Frame recording: uploads the CPU draw lists, draws the scene views into
//! the scene target, post-processes them onto the output, then the 2D layer.

use super::pipes::{DrawU, UNIFORM_STRIDE, UiV};
use super::scene::{Lists, Pipe, Src};
use super::ssao::AoView;

/// Which of each view's draws a scene pass records (split around SSAO).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    All,
    Opaque,
    Rest,
}
use super::{DEPTH_FORMAT, MSAA_SAMPLES, NO_TEX, Renderer, SCENE_FORMAT, Targets};
use crate::{Draw2D, Error, RenderFrame, Result};

/// A GPU buffer that grows to fit each frame's data.
#[derive(Default)]
pub(super) struct GrowBuf(Option<wgpu::Buffer>);

impl GrowBuf {
    /// Uploads `data`; true when the buffer was (re)created.
    fn upload(&mut self, dev: &wgpu::Device, queue: &wgpu::Queue, usage: wgpu::BufferUsages, data: &[u8]) -> bool {
        let need = (data.len() as u64).max(256).next_multiple_of(256);
        let mut created = false;
        if self.0.as_ref().is_none_or(|b| b.size() < need) {
            if let Some(old) = self.0.take() {
                old.destroy();
            }
            self.0 = Some(dev.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: need.next_power_of_two(),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            created = true;
        }
        if !data.is_empty()
            && let Some(b) = &self.0
        {
            queue.write_buffer(b, 0, data);
        }
        created
    }

    fn get(&self) -> &wgpu::Buffer {
        self.0.as_ref().expect("uploaded before use")
    }
}

#[derive(Default)]
pub(super) struct Buffers {
    world_idx: GrowBuf,
    alias_v: GrowBuf,
    poly_v: GrowBuf,
    poly_i: GrowBuf,
    ui_v: GrowBuf,
    draw_u: GrowBuf,
    frame_u: GrowBuf,
    /// Group 0 of scene pipelines (frame uniforms, lightmap, palette).
    pub(super) frame_group: Option<wgpu::BindGroup>,
    pub(super) draw_group: Option<wgpu::BindGroup>,
    /// Scene colour as a material (post pass input).
    post_group: Option<wgpu::BindGroup>,
    lists: Lists,
}

fn ui_quads(r: &Renderer, fr: &RenderFrame) -> (Vec<UiV>, Vec<(u32, u32, u32)>) {
    let mut v: Vec<UiV> = Vec::with_capacity(fr.draw2d.len() * 6);
    let mut batches: Vec<(u32, u32, u32)> = Vec::new();
    let mut quad = |x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4], tex: u32| {
        let c = color.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        let p = |px: f32, py: f32, u: f32, vv: f32| UiV { pos: [px, py], uv: [u, vv], color: c };
        let first = v.len() as u32;
        v.extend_from_slice(&[
            p(x, y, uv[0], uv[1]),
            p(x + w, y, uv[2], uv[1]),
            p(x + w, y + h, uv[2], uv[3]),
            p(x, y, uv[0], uv[1]),
            p(x + w, y + h, uv[2], uv[3]),
            p(x, y + h, uv[0], uv[3]),
        ]);
        match batches.last_mut() {
            Some(b) if b.0 == tex => b.2 += 6,
            _ => batches.push((tex, first, 6)),
        }
    };
    for d in fr.draw2d {
        match *d {
            Draw2D::Pic { x, y, w, h, pic, uv, color } => {
                if let Some(p) = r.pics.get(pic.0 as usize) {
                    quad(x, y, w, h, uv, color, p.slot);
                }
            }
            Draw2D::Char { x, y, size, ch, color } => {
                if ch == b' ' {
                    continue;
                }
                if let Some(p) = r.conchars.and_then(|c| r.pics.get(c.0 as usize)) {
                    let (cx, cy) = ((ch & 15) as f32 / 16.0, (ch >> 4) as f32 / 16.0);
                    let e = 1.0 / 2048.0;
                    let uv = [cx + e, cy + e, cx + 1.0 / 16.0 - e, cy + 1.0 / 16.0 - e];
                    quad(x, y, size, size, uv, color, p.slot);
                }
            }
            Draw2D::Fill { x, y, w, h, color } => quad(x, y, w, h, [0.0; 4], color, NO_TEX),
            Draw2D::Tile { x, y, w, h, pic, scale } => {
                if let Some(p) = r.pics.get(pic.0 as usize) {
                    let (pw, ph) = (p.width as f32 * scale.max(1e-3), p.height as f32 * scale.max(1e-3));
                    quad(x, y, w, h, [x / pw, y / ph, (x + w) / pw, (y + h) / ph], [1.0; 4], p.slot);
                }
            }
        }
    }
    (v, batches)
}

impl Renderer {
    fn create_targets(&mut self) {
        let (w, h) = (self.width, self.height);
        let scene = crate::projection::scene_extent((w, h), self.render_scale);
        let tex = |(width, height), samples, format, usage| {
            self.dev.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        use wgpu::TextureUsages as U;
        let color = tex(scene, 1, SCENE_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let ms_view = (self.msaa > 1)
            .then(|| tex(scene, self.msaa, SCENE_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING).create_view(&Default::default()));
        // sampled by SSAO
        let depth = tex(scene, self.msaa, DEPTH_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let out = self.surface.is_none().then(|| {
            let o = tex((w, h), 1, wgpu::TextureFormat::Rgba8Unorm, U::RENDER_ATTACHMENT | U::COPY_SRC);
            let bpr = (w * 4).next_multiple_of(256);
            let buf = self.dev.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: (bpr * h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (o, buf, bpr)
        });
        let color_view = color.create_view(&Default::default());
        // a scaled scene is filtered up/down to the output (vkQuake r_scale)
        let post_sampler = if scene == (w, h) { &self.samplers.clamp } else { &self.samplers.linear };
        self.bufs.post_group = Some(self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipes.mat_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.black) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.black) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(post_sampler) },
            ],
        }));
        let depth_view = depth.create_view(&Default::default());
        let ao = self.ssao.map(|q| self.create_ao_targets(q, scene, ms_view.as_ref().unwrap_or(&color_view), &depth_view));
        self.targets = Some(Targets { scene, color, color_view, ms_view, depth_view, ao, out });
        self.frame_ready = false;
    }

    fn uniform_group(&self, layout: &wgpu::BindGroupLayout, buf: &wgpu::Buffer, size: usize) -> wgpu::BindGroup {
        self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(size as u64),
                }),
            }],
        })
    }

    /// Draws one frame (and presents it when windowed).
    pub fn render(&mut self, fr: &RenderFrame) -> Result<()> {
        self.set_world_filter(fr.settings.texture_linear);
        let scale = fr.settings.render_scale.clamp(0.25, 2.0);
        if scale != self.render_scale {
            self.render_scale = scale;
            self.needs_resize = true;
        }
        let msaa = if fr.settings.msaa > 1 { MSAA_SAMPLES } else { 1 };
        if msaa != self.msaa {
            self.msaa = msaa;
            self.pipes.set_samples(&self.dev, msaa);
            self.needs_resize = true;
        }
        let ssao = (fr.settings.ssao > 0.0).then(|| fr.settings.ssao_quality.min(2));
        if ssao != self.ssao {
            self.ssao = ssao;
            self.needs_resize = true;
        }
        if self.needs_resize {
            self.needs_resize = false;
            if let Some((s, cfg)) = &mut self.surface {
                cfg.width = self.width;
                cfg.height = self.height;
                s.configure(&self.dev, cfg);
            }
            if let Some(t) = self.targets.take() {
                t.color.destroy();
            }
            self.bufs.post_group = None;
        }
        if self.targets.is_none() {
            self.create_targets();
        }
        if let Some(wr) = &self.world {
            let mut styles = [256.0f32; 256];
            for (i, v) in fr.lightstyles.iter().take(256).enumerate() {
                styles[i] = *v as f32;
            }
            let dl = if fr.settings.dynamic { fr.dlights } else { &[] };
            self.lightmap.update(&self.queue, &wr.cpu, &styles, dl);
        }
        let mut lists = std::mem::take(&mut self.bufs.lists);
        lists.world_idx.clear();
        lists.alias_v.clear();
        lists.poly_v.clear();
        lists.poly_i.clear();
        lists.draw_u.clear();
        lists.frame_u.clear();
        lists.views.clear();
        let (w, h) = (self.width, self.height);
        if !fr.splits.is_empty() {
            for (k, s) in fr.splits.iter().enumerate() {
                self.build_view(fr, &s.view, s.rect, s.blend, 1u8 << k.min(7), &mut lists);
            }
        } else if let Some(v) = &fr.view {
            self.build_view(fr, v, [0, 0, w, h], fr.blend, 1, &mut lists);
        }
        let st = &fr.settings;
        let (tw, th) = self.targets.as_ref().map_or((w, h), |t| t.scene);
        let post: Vec<DrawU> = lists
            .views
            .iter()
            .map(|v| {
                let ([x, y], [sw, sh]) = (v.ofs, v.scene);
                DrawU {
                    color: v.blend.map(|c| c.clamp(0.0, 1.0)),
                    // z: underwater warp clock (< 0 off); w: output aspect
                    params: [st.gamma.clamp(0.1, 4.0), st.contrast.max(0.0), v.warp, w as f32 / h.max(1) as f32],
                    extra: [x as f32 / tw as f32, y as f32 / th as f32, (x + sw) as f32 / tw as f32, (y + sh) as f32 / th as f32],
                    ..Default::default()
                }
            })
            .collect();
        let post_du: Vec<u32> = post.into_iter().map(|d| lists.draw(d)).collect();
        let ao_views = self.ao_views(&mut lists, st);
        let ui_du = lists.draw(DrawU { color: [1.0 / w as f32, 1.0 / h as f32, 0.0, 0.0], ..Default::default() });
        let (ui_v, ui_batches) = ui_quads(self, fr);
        let mut need_sky = false;
        for v in &lists.views {
            for c in &v.cmds {
                if c.pipe == Pipe::Sky {
                    need_sky = true;
                } else {
                    self.ensure_material(c.mat);
                }
            }
        }
        for &(slot, _, _) in &ui_batches {
            self.ensure_material([slot, NO_TEX, NO_TEX]);
        }
        if need_sky {
            self.ensure_sky();
        }
        self.upload(&lists, &ui_v);
        let result = self.encode(&lists, &post_du, &ao_views, ui_du, &ui_batches);
        self.bufs.lists = lists;
        result
    }

    fn upload(&mut self, l: &Lists, ui_v: &[UiV]) {
        use crate::bytes_of;
        use wgpu::BufferUsages as B;
        let (d, q) = (&self.dev, &self.queue);
        let b = &mut self.bufs;
        b.world_idx.upload(d, q, B::INDEX, bytes_of(&l.world_idx));
        b.alias_v.upload(d, q, B::VERTEX, bytes_of(&l.alias_v));
        b.poly_v.upload(d, q, B::VERTEX, bytes_of(&l.poly_v));
        b.poly_i.upload(d, q, B::INDEX, bytes_of(&l.poly_i));
        b.ui_v.upload(d, q, B::VERTEX, bytes_of(ui_v));
        if b.draw_u.upload(d, q, B::UNIFORM, bytes_of(&l.draw_u)) {
            b.draw_group = None;
        }
        if b.frame_u.upload(d, q, B::UNIFORM, bytes_of(&l.frame_u)) {
            b.frame_group = None;
        }
        if self.bufs.draw_group.is_none() {
            let g = self.uniform_group(&self.pipes.draw_bgl, self.bufs.draw_u.get(), std::mem::size_of::<DrawU>());
            self.bufs.draw_group = Some(g);
        }
        if self.bufs.frame_group.is_none() {
            let g = self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pipes.frame_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: self.bufs.frame_u.get(),
                            offset: 0,
                            size: wgpu::BufferSize::new(std::mem::size_of::<super::pipes::FrameU>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.lightmap.view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.palette_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.samplers.linear) },
                ],
            });
            self.bufs.frame_group = Some(g);
        }
    }

    fn encode(&mut self, l: &Lists, post_du: &[u32], ao_views: &[AoView], ui_du: u32, ui_batches: &[(u32, u32, u32)]) -> Result<()> {
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
                    store: if last { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
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
