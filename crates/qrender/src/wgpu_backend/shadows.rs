//! Real-time dynamic lights (r_shadow_realtime_dlight) for the wgpu backend,
//! a port of the Vulkan backend's `frame/dlights.rs`. The selected lights
//! (`crate::rtlight`) go into a uniform the scene shaders read
//! (`dyn_lights` in common.wgsl); the shadowed ones get six Depth16 layers of
//! a 2D array each, rendered here before the scene: their world surfaces
//! (PVS of the light's leaf within its radius), plus the first view's opaque
//! brush and alias entities within reach, except the light's own entity.

use super::math::{dot, ident};
use super::pipes::{ALIAS_ATTRS, AliasV, FrameU, UNIFORM_STRIDE, WORLD_ATTRS, vb};
use super::scene::{Buckets, Cmd, Lists, Pipe, Src, liquid_alphas, world_cmds};
use super::{NO_TEX, Renderer};
use crate::rtlight::{self, LightsGpu, RtLight};
use crate::{RenderFrame, Settings};

const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;

/// Shadow maps, the light list uniform and the depth-only pipelines.
pub(super) struct ShadowRes {
    /// Every layer (sampled by the scene shaders).
    pub(super) array_view: wgpu::TextureView,
    /// One view per layer (rendered).
    layers: Vec<wgpu::TextureView>,
    size: u32,
    lights: usize,
    pub(super) sampler: wgpu::Sampler,
    pub(super) buf: wgpu::Buffer,
    /// 1×1 depth array bound instead of the maps while rendering them.
    pub(super) placeholder: wgpu::TextureView,
    world: wgpu::RenderPipeline,
    alias: wgpu::RenderPipeline,
}

/// One shadowed light this frame.
pub(super) struct ShadowPass {
    origin: [f32; 3],
    radius: f32,
    key: u32,
    layer: u32,
    /// World caster draws (opaque and sky surfaces).
    cmds: Vec<Cmd>,
    /// Frame uniform record of each face (the first view's with the face matrix).
    fu: [u32; 6],
}

impl Renderer {
    /// Creates the shadow resources, or recreates the maps when the budget
    /// (lights, face size) for `st` changed.
    pub(super) fn ensure_shadows(&mut self, st: &Settings) {
        let (lights, size) = rtlight::shadow_budget(st);
        if self.shadows.as_ref().is_some_and(|s| (s.lights, s.size) == (lights, size)) {
            return;
        }
        let tex = self.dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow maps"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: (lights * 6) as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let array_view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = (0..(lights * 6) as u32)
            .map(|l| {
                tex.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: l,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let old = self.shadows.take();
        let (sampler, buf, placeholder, world, alias) = match old {
            Some(o) => (o.sampler, o.buf, o.placeholder, o.world, o.alias),
            None => {
                let sampler = self.dev.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("shadow compare"),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    compare: Some(wgpu::CompareFunction::LessEqual),
                    ..Default::default()
                });
                let buf = self.dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rt lights"),
                    size: std::mem::size_of::<LightsGpu>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let placeholder = self
                    .dev
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("shadow placeholder"),
                        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: SHADOW_FORMAT,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2Array),
                        ..Default::default()
                    });
                let world_vb = [vb::<crate::world::WorldVertex>(&WORLD_ATTRS)];
                let alias_vb = [vb::<AliasV>(&ALIAS_ATTRS)];
                let pipe = |vs: &str, fs: &str, buffers: &[Option<wgpu::VertexBufferLayout>]| {
                    self.dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(fs),
                        layout: Some(&self.pipes.scene_layout),
                        vertex: wgpu::VertexState {
                            module: &self.pipes.scene_module,
                            entry_point: Some(vs),
                            compilation_options: Default::default(),
                            buffers,
                        },
                        // two-sided casters
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: SHADOW_FORMAT,
                            depth_write_enabled: Some(true),
                            depth_compare: Some(wgpu::CompareFunction::LessEqual),
                            stencil: Default::default(),
                            // slope-scaled bias against acne (with the lookup's normal offset)
                            bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
                        }),
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &self.pipes.scene_module,
                            entry_point: Some(fs),
                            compilation_options: Default::default(),
                            targets: &[],
                        }),
                        multiview_mask: None,
                        cache: None,
                    })
                };
                let world = pipe("vs_world", "fs_shadow_world", &world_vb);
                let alias = pipe("vs_alias", "fs_shadow_alias", &alias_vb);
                (sampler, buf, placeholder, world, alias)
            }
        };
        self.shadows = Some(ShadowRes { array_view, layers, size, lights, sampler, buf, placeholder, world, alias });
        // the frame bind group holds the maps
        self.bufs.frame_group = None;
    }

    /// Selects this frame's lights, writes their uniform, and builds the
    /// shadowed ones' world casters and face uniforms (after the views).
    pub(super) fn build_shadows(&mut self, fr: &RenderFrame, lists: &mut Lists) -> Vec<ShadowPass> {
        let st = &fr.settings;
        let (w, h) = (self.width, self.height);
        let views: Vec<(crate::View, [u32; 4])> = if fr.splits.is_empty() {
            fr.view.map(|v| (v, [0, 0, w, h])).into_iter().collect()
        } else {
            fr.splits.iter().map(|s| (s.view, s.rect)).collect()
        };
        let frusta: Vec<_> = views
            .iter()
            .map(|(v, r)| (v.origin, crate::projection::ViewFrustum::for_extent(v, r[2].max(1), r[3].max(1)).planes))
            .collect();
        let Some(sr) = &self.shadows else { return Vec::new() };
        let shadows = if st.dlight_shadows { sr.lights } else { 0 };
        let sel: Vec<RtLight> = rtlight::select(fr.dlights, &frusta, st, shadows);
        self.queue.write_buffer(&sr.buf, 0, crate::bytes_of(&[rtlight::lights_gpu(&sel, st.shadow_quality, sr.size)]));
        let Some(first) = lists.views.first().map(|v| lists.frame_u[v.fu as usize]) else { return Vec::new() };
        let alphas = liquid_alphas(st);
        let mut out = Vec::new();
        for (d, layer) in sel.iter().filter_map(|s| Some((s.light, s.layer?))) {
            let (o, r) = (d.origin, d.radius);
            let mut b = Buckets::default();
            if let Some(wr) = self.world.as_mut() {
                let pvs = wr.cpu.portal_surfaces(o);
                let near: Vec<u32> = pvs
                    .iter()
                    .copied()
                    .filter(|&si| {
                        let s = &wr.cpu.surfaces[si as usize];
                        (dot(o, s.normal) - s.dist).abs() < r && rtlight::box_touches_sphere(s.mins, s.maxs, o, r)
                    })
                    .collect();
                world_cmds(wr, &near, false, fr.time, alphas, 1.0, ident(), 0.0, lists, &mut b);
            }
            // liquids don't cast
            let cmds: Vec<Cmd> = b.opaque.into_iter().chain(b.sky).filter(|c| matches!(c.pipe, Pipe::World | Pipe::Sky)).collect();
            let fu = rtlight::face_vps(o, r).map(|vp| {
                lists.frame_u.push(FrameU { viewproj: vp, ..first });
                lists.frame_u.len() as u32 - 1
            });
            out.push(ShadowPass { origin: o, radius: r, key: d.key, layer, cmds, fu });
        }
        for p in &out {
            for c in &p.cmds {
                self.ensure_material(c.mat);
            }
        }
        out
    }

    /// Renders every shadowed light's six faces (before the scene passes).
    pub(super) fn encode_shadows(&self, enc: &mut wgpu::CommandEncoder, l: &Lists, passes: &[ShadowPass]) {
        let (Some(sr), Some(frame_group), Some(draw_group)) = (&self.shadows, &self.bufs.shadow_frame_group, &self.bufs.draw_group) else {
            return;
        };
        let b = &self.bufs;
        for p in passes {
            let casters: Vec<&Cmd> = l
                .casters
                .iter()
                .filter(|(_, s, key)| (p.key == 0 || *key != p.key) && rtlight::spheres_touch([s[0], s[1], s[2]], s[3], p.origin, p.radius))
                .map(|(c, _, _)| c)
                .collect();
            for face in 0..6 {
                let Some(view) = sr.layers.get(p.layer as usize + face) else { continue };
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
                pass.set_bind_group(0, frame_group, &[p.fu[face] * UNIFORM_STRIDE as u32]);
                for c in p.cmds.iter().chain(casters.iter().copied()) {
                    let Some(mat) = self.bind_groups.get(&c.mat).or_else(|| self.bind_groups.get(&[NO_TEX; 3])) else { continue };
                    match c.src {
                        Src::World => {
                            let Some(wr) = &self.world else { continue };
                            pass.set_pipeline(&sr.world);
                            pass.set_vertex_buffer(0, wr.vbuf.slice(..));
                            pass.set_index_buffer(b.world_idx.get().slice(..), wgpu::IndexFormat::Uint32);
                        }
                        Src::Alias(mi) => {
                            let Some(super::models::Model::Alias(am)) = self.models.get(mi) else { continue };
                            pass.set_pipeline(&sr.alias);
                            pass.set_vertex_buffer(0, b.alias_v.get().slice(..));
                            pass.set_index_buffer(am.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                        }
                        Src::Poly => continue,
                    }
                    pass.set_bind_group(1, mat, &[]);
                    pass.set_bind_group(2, draw_group, &[c.du * UNIFORM_STRIDE as u32]);
                    pass.draw_indexed(c.first..c.first + c.count, c.base_vertex, 0..1);
                }
            }
        }
    }
}
