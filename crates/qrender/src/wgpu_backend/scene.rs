//! Per-view draw lists: world surfaces from the PVS, entities, particles.

use super::level::WorldRes;
use super::math::*;
use super::pipes::{AliasV, DrawU, FrameU, poly_index};
use super::{NO_TEX, Renderer};
use crate::{FteBlend, FteVertex, RenderFrame, TextureKind, View};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Pipe {
    World,
    WorldAlpha,
    Water,
    WaterAlpha,
    Sky,
    Alias,
    AliasAlpha,
    Poly(usize),
}

/// Where a draw's vertices and indices come from.
#[derive(Clone, Copy, Debug)]
pub(super) enum Src {
    /// World vertex buffer + this frame's world index buffer.
    World,
    /// This frame's alias vertices + the model's index buffer.
    Alias(usize),
    /// This frame's poly vertices and indices.
    Poly,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cmd {
    pub(super) pipe: Pipe,
    pub(super) src: Src,
    pub(super) mat: [u32; 3],
    pub(super) first: u32,
    pub(super) count: u32,
    pub(super) base_vertex: i32,
    pub(super) du: u32,
    /// First-person weapon: compressed depth range.
    pub(super) viewmodel: bool,
}

pub(super) struct ViewDraw {
    pub(super) fu: u32,
    /// Output rectangle of the view.
    pub(super) rect: [u32; 4],
    /// Top-left of the view in the (render-scaled) scene target.
    pub(super) ofs: [u32; 2],
    /// 3D scene size in the scene target (above the reserved status bar rows).
    pub(super) scene: [u32; 2],
    /// The same 3D area in output pixels.
    pub(super) out: [u32; 2],
    pub(super) blend: [f32; 4],
    /// Underwater screen warp clock in seconds; negative = off.
    pub(super) warp: f32,
    /// Tangents of the half field of view (x, y), for SSAO.
    pub(super) tan: [f32; 2],
    /// Fog density (/64) for SSAO.
    pub(super) fog: f32,
    /// `cmds[..split]` are drawn before SSAO: opaque world, sky and opaque
    /// models except liquids and the viewmodel (as in the Vulkan backend).
    pub(super) split: usize,
    pub(super) cmds: Vec<Cmd>,
}

/// CPU-side data of one frame, uploaded before encoding.
#[derive(Default)]
pub(super) struct Lists {
    pub(super) world_idx: Vec<u32>,
    pub(super) alias_v: Vec<AliasV>,
    pub(super) poly_v: Vec<FteVertex>,
    pub(super) poly_i: Vec<u32>,
    pub(super) draw_u: Vec<DrawU>,
    pub(super) frame_u: Vec<FrameU>,
    pub(super) views: Vec<ViewDraw>,
}

impl Lists {
    pub(super) fn draw(&mut self, d: DrawU) -> u32 {
        self.draw_u.push(d);
        self.draw_u.len() as u32 - 1
    }
}

/// Draw buckets of one view, merged in drawing order at the end.
#[derive(Default)]
pub(super) struct Buckets {
    pub(super) opaque: Vec<Cmd>,
    pub(super) sky: Vec<Cmd>,
    pub(super) alpha_liquids: Vec<Cmd>,
    /// Translucent draws with their view distance (drawn far to near).
    pub(super) trans: Vec<(f32, Cmd)>,
}

pub(super) fn liquid_alphas(st: &crate::Settings) -> [f32; 4] {
    [
        st.wateralpha,
        st.lavaalpha.unwrap_or(st.wateralpha),
        st.slimealpha.unwrap_or(st.wateralpha),
        st.telealpha.unwrap_or(st.wateralpha),
    ]
    .map(|a| a.clamp(0.0, 1.0))
}

/// Brush surfaces `surfs` drawn with `model`, bucketed by (animated) texture.
#[allow(clippy::too_many_arguments)]
pub(super) fn world_cmds(
    wr: &mut WorldRes,
    surfs: &[u32],
    alt: bool,
    time: f64,
    alphas: [f32; 4],
    ent_alpha: f32,
    model: Mat4,
    dist: f32,
    lists: &mut Lists,
    out: &mut Buckets,
) {
    let nb = wr.buckets.len();
    for &si in surfs {
        if wr.cpu.hidden_surfaces.contains(&si) {
            continue;
        }
        let Some(s) = wr.cpu.surfaces.get(si as usize) else { continue };
        if s.num_indices == 0 {
            continue;
        }
        let t = (wr.cpu.resolve_texture(s.texture, alt, time, si) as usize).min(nb - 1);
        wr.buckets[t].push(si);
    }
    for t in 0..nb {
        if wr.buckets[t].is_empty() {
            continue;
        }
        let first = lists.world_idx.len() as u32;
        for &si in &wr.buckets[t] {
            let s = &wr.cpu.surfaces[si as usize];
            lists
                .world_idx
                .extend_from_slice(&wr.cpu.indices[s.first_index as usize..(s.first_index + s.num_indices) as usize]);
        }
        wr.buckets[t].clear();
        let count = lists.world_idx.len() as u32 - first;
        let [base, fb, flags] = wr.materials[t];
        let (pipe, alpha) = match wr.kinds[t] {
            TextureKind::Sky => (Pipe::Sky, 1.0),
            TextureKind::Liquid => {
                let a = alphas[(flags & 0xf).clamp(1, 4) as usize - 1] * ent_alpha;
                (if a < 1.0 { Pipe::WaterAlpha } else { Pipe::Water }, a)
            }
            _ => (if ent_alpha < 1.0 { Pipe::WorldAlpha } else { Pipe::World }, ent_alpha),
        };
        let du = lists.draw(DrawU {
            model,
            color: [alpha, 0.0, 0.0, 0.0],
            params: [
                f32::from(flags & 0x10 != 0),
                f32::from(fb != NO_TEX),
                f32::from(flags & 0x20 != 0),
                0.0,
            ],
            ..Default::default()
        });
        let cmd = Cmd { pipe, src: Src::World, mat: [base, fb, NO_TEX], first, count, base_vertex: 0, du, viewmodel: false };
        match pipe {
            Pipe::Sky => out.sky.push(cmd),
            Pipe::WaterAlpha if ent_alpha >= 1.0 => out.alpha_liquids.push(cmd),
            Pipe::WaterAlpha | Pipe::WorldAlpha => out.trans.push((dist, cmd)),
            _ => out.opaque.push(cmd),
        }
    }
}

impl Renderer {
    /// Builds the draw list of one view into `lists`.
    pub(super) fn build_view(
        &mut self,
        fr: &RenderFrame,
        view: &View,
        rect: [u32; 4],
        blend: [f32; 4],
        vbit: u8,
        lists: &mut Lists,
    ) {
        let (w, h) = (self.width, self.height);
        let tsize = self.targets.as_ref().map_or((w, h), |t| t.scene);
        let (sw, sh) = crate::projection::view_extent(rect, view.reserve_bottom, (w, h), tsize);
        let (ow, oh) = crate::projection::view_extent(rect, view.reserve_bottom, (w, h), (w, h));
        let ofs = [
            ((rect[0] as f32 * tsize.0 as f32 / w.max(1) as f32).round() as u32).min(tsize.0 - sw),
            ((rect[1] as f32 * tsize.1 as f32 / h.max(1) as f32).round() as u32).min(tsize.1 - sh),
        ];
        let vs = setup_view(view, sw, sh);
        let st = &fr.settings;
        let alphas = liquid_alphas(st);
        let fog = fr.fog;
        lists.frame_u.push(FrameU {
            viewproj: vs.vp,
            eye: [vs.eye[0], vs.eye[1], vs.eye[2], fr.time as f32],
            fog: [fog.color[0], fog.color[1], fog.color[2], fog.density.max(0.0) / 64.0],
            vright: [vs.right[0], vs.right[1], vs.right[2], 0.0],
            vup: [vs.up[0], vs.up[1], vs.up[2], 0.0],
            vfwd: [vs.fwd[0], vs.fwd[1], vs.fwd[2], 0.0],
            sky: [st.skyfog, if self.skybox.is_some() { 1.0 } else { 0.0 }, st.skyalpha, 0.0],
            alphas,
            pad: [0.0; 20],
        });
        let fu = lists.frame_u.len() as u32 - 1;
        let mut b = Buckets::default();
        if let Some(wr) = self.world.as_mut() {
            let need_novis = wr.cpu.needs_transparent_novis(alphas);
            wr.cpu.set_transparent_liquids(need_novis);
            let pvs = wr.cpu.portal_surfaces(vs.eye);
            let surfs: Vec<u32> = pvs
                .iter()
                .copied()
                .filter(|&si| {
                    let s = &wr.cpu.surfaces[si as usize];
                    if !crate::world::box_in_frustum(&vs.frustum, s.mins, s.maxs) {
                        return false;
                    }
                    s.kind == TextureKind::Sky || ((dot(vs.eye, s.normal) - s.dist < 0.0) == s.flipped)
                })
                .collect();
            world_cmds(wr, &surfs, false, fr.time, alphas, 1.0, ident(), 0.0, lists, &mut b);
        }
        self.entity_cmds(fr, &vs, vbit, alphas, lists, &mut b);
        self.particle_cmds(fr, &vs, lists, &mut b);
        b.trans.sort_by(|a, c| c.0.total_cmp(&a.0));
        let (mut cmds, late): (Vec<Cmd>, Vec<Cmd>) =
            b.opaque.into_iter().partition(|c| !c.viewmodel && c.pipe != Pipe::Water);
        cmds.extend(b.sky);
        let split = cmds.len();
        cmds.extend(late);
        cmds.extend(b.alpha_liquids);
        cmds.extend(b.trans.into_iter().map(|(_, c)| c));
        let warp = match view.waterwarp {
            crate::WaterWarp::Screen(t) => t.max(0.0) % 1000.0,
            _ => -1.0,
        };
        let tan = crate::projection::half_fov_tangents(view, (sw, sh));
        let fog = fog.density.max(0.0) / 64.0;
        let out = [ow, oh];
        lists.views.push(ViewDraw { fu, rect, ofs, scene: [sw, sh], out, blend, warp, tan, fog, split, cmds });
    }

    /// Classic particles (camera-facing dots) and FTE particle geometry.
    fn particle_cmds(&self, fr: &RenderFrame, vs: &ViewSetup, lists: &mut Lists, b: &mut Buckets) {
        let poly_du = lists.draw(DrawU::default());
        if !fr.particles.is_empty() {
            let base = lists.poly_v.len() as u32;
            let first = lists.poly_i.len() as u32;
            for p in fr.particles {
                let c = self.palette[p.color as usize];
                let color = [c[0], c[1], c[2], (p.alpha.clamp(0.0, 1.0) * 255.0) as u8];
                let d = dot(sub(p.origin, vs.eye), vs.fwd);
                let scale = if d < 20.0 { 1.0 } else { 1.0 + d * 0.004 } * 0.75;
                let n = lists.poly_v.len() as u32 - base;
                for (sr, su, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                    let pos = std::array::from_fn(|k| p.origin[k] + (vs.right[k] * sr + vs.up[k] * su) * scale);
                    lists.poly_v.push(FteVertex { pos, uv, color });
                }
                lists.poly_i.extend_from_slice(&[n, n + 1, n + 2, n, n + 2, n + 3]);
            }
            b.trans.push((
                -1.0,
                Cmd {
                    pipe: Pipe::Poly(poly_index(FteBlend::Blend, false)),
                    src: Src::Poly,
                    mat: [self.dot_tex, NO_TEX, NO_TEX],
                    first,
                    count: lists.poly_i.len() as u32 - first,
                    base_vertex: base as i32,
                    du: poly_du,
                    viewmodel: false,
                },
            ));
        }
        if !fr.fte_batches.is_empty() && !fr.fte_indices.is_empty() {
            let base = lists.poly_v.len() as u32;
            let first = lists.poly_i.len() as u32;
            lists.poly_v.extend_from_slice(fr.fte_vertices);
            lists.poly_i.extend_from_slice(fr.fte_indices);
            for (k, bt) in fr.fte_batches.iter().enumerate() {
                let Some(p) = self.pics.get(bt.pic.0 as usize) else { continue };
                let end = bt.first_index.saturating_add(bt.num_indices);
                if end as usize > fr.fte_indices.len() {
                    continue;
                }
                b.trans.push((
                    -2.0 - k as f32 * 1e-3,
                    Cmd {
                        pipe: Pipe::Poly(poly_index(bt.blend, bt.lines)),
                        src: Src::Poly,
                        mat: [p.slot, NO_TEX, NO_TEX],
                        first: first + bt.first_index,
                        count: bt.num_indices,
                        base_vertex: base as i32,
                        du: poly_du,
                        viewmodel: false,
                    },
                ));
            }
        }
    }
}
