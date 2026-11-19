//! Entity draws: brush models, alias models (posed on the CPU), sprites.

use super::math::*;
use super::models::{AliasModel, Model, SpriteModel};
use super::pipes::{AliasV, DrawU, poly_index};
use super::scene::{Buckets, Cmd, Lists, Pipe, Src, world_cmds};
use super::{NO_TEX, Renderer};
use crate::convert::group_index;
use crate::{FteBlend, FteVertex, RenderEntity, RenderFrame};

impl Renderer {
    pub(super) fn entity_cmds(
        &mut self,
        fr: &RenderFrame,
        vs: &ViewSetup,
        vbit: u8,
        alphas: [f32; 4],
        lists: &mut Lists,
        b: &mut Buckets,
    ) {
        let styles: Vec<i32> = (0..256).map(|i| fr.lightstyles.get(i).copied().unwrap_or(256)).collect();
        for e in fr.entities {
            if (e.views != 0 && e.views & vbit == 0) || e.portal_only || self.hidden_entities.contains(&e.key) {
                continue;
            }
            let dist = dot(sub(e.origin, vs.eye), vs.fwd);
            match self.models.get(e.model.0 as usize) {
                Some(Model::Brush(sm)) => {
                    let sm = *sm;
                    self.brush_cmds(fr, e, sm, vs, alphas, dist, lists, b);
                }
                Some(Model::Alias(am)) => {
                    let light = self.alias_light(fr, e, am, &styles);
                    // vertices are posed in world space: prev = M_prev · M_cur⁻¹
                    let prev = self.motion.prev_transform(e).map(|m| mul(&m, &affine_inv(entity_rotation(e.angles), e.origin)));
                    alias_cmds(fr, e, e.model.0 as usize, am, vs, light, dist, prev, lists, b);
                }
                Some(Model::Sprite(sm)) => sprite_cmds(fr, e, sm, vs, dist, lists, b),
                None => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn brush_cmds(
        &mut self,
        fr: &RenderFrame,
        e: &RenderEntity,
        sm: u32,
        vs: &ViewSetup,
        alphas: [f32; 4],
        dist: f32,
        lists: &mut Lists,
        b: &mut Buckets,
    ) {
        let Some(wr) = self.world.as_mut() else { return };
        let Some(sub_) = wr.cpu.submodels.get(sm as usize).cloned() else { return };
        let rot = entity_rotation(e.angles);
        if e.angles.iter().any(|&a| a != 0.0) {
            let r = (0..3).map(|i| sub_.mins[i].abs().max(sub_.maxs[i].abs()).powi(2)).sum::<f32>().sqrt();
            if !sphere_in_frustum(&vs.frustum, e.origin, r) {
                return;
            }
        } else {
            let mn = [0, 1, 2].map(|i| sub_.mins[i] + e.origin[i]);
            let mx = [0, 1, 2].map(|i| sub_.maxs[i] + e.origin[i]);
            if !crate::world::box_in_frustum(&vs.frustum, mn, mx) {
                return;
            }
        }
        // eye in model space (R^T * (eye - origin))
        let d = sub(vs.eye, e.origin);
        let le = [0, 1, 2].map(|c| rot[0][c] * d[0] + rot[1][c] * d[1] + rot[2][c] * d[2]);
        let surfs: Vec<u32> = (sub_.first_surface..sub_.first_surface + sub_.num_surfaces)
            .filter(|&si| {
                wr.cpu.surfaces.get(si as usize).is_some_and(|s| (dot(le, s.normal) - s.dist < 0.0) == s.flipped)
            })
            .collect();
        let alpha = e.alpha.clamp(0.0, 1.0);
        let model = affine(rot, e.origin);
        let first = b.opaque.len();
        world_cmds(wr, &surfs, e.frame != 0, fr.time, alphas, alpha, model, dist, lists, b);
        if lists.views.is_empty() && alpha >= 1.0 {
            // shadow casters (first view): the submodel's bounding sphere
            let mid = [0, 1, 2].map(|i| 0.5 * (sub_.mins[i] + sub_.maxs[i]));
            let half = [0, 1, 2].map(|i| 0.5 * (sub_.maxs[i] - sub_.mins[i]));
            let c = xform(&model, mid);
            let sphere = [c[0], c[1], c[2], dot(half, half).sqrt()];
            let casters: Vec<_> = b.opaque[first..].iter().map(|c| (*c, sphere, e.key)).collect();
            lists.casters.extend(casters);
        }
        if let Some(prev) = self.motion.prev_transform(e) {
            let du = lists.draw(DrawU { model, pad: vel_pad(&prev), ..Default::default() });
            b.vel.extend(b.opaque[first..].iter().map(|c| Cmd { du, ..*c }));
        }
    }

    /// R_SetupAliasLighting: (light colour / 200).
    fn alias_light(&self, fr: &RenderFrame, e: &RenderEntity, am: &AliasModel, styles: &[i32]) -> [f32; 3] {
        let wl = self.world.as_ref().map(|w| &w.cpu);
        let mut lc = wl
            .and_then(|w| {
                w.light_point(e.origin, styles)
                    .or_else(|| w.light_point([e.origin[0], e.origin[1], e.origin[2] + am.maxs_z * 0.5], styles))
            })
            .unwrap_or([128.0; 3]);
        // real-time dynamic lights shade models per pixel instead
        if fr.settings.dynamic && !fr.settings.rt_dlights {
            for dl in fr.dlights {
                let d = sub(e.origin, dl.origin);
                let add = dl.radius - dot(d, d).sqrt();
                if add > 0.0 {
                    for (c, dc) in lc.iter_mut().zip(dl.color) {
                        *c += add * dc;
                    }
                }
            }
        }
        let minimum = if e.viewmodel {
            72.0
        } else if e.player {
            24.0
        } else {
            0.0
        };
        let sum = lc[0] + lc[1] + lc[2];
        if sum < minimum {
            lc = lc.map(|c| c + (minimum - sum) / 3.0);
        }
        let sum = lc[0] + lc[1] + lc[2];
        if sum > 288.0 {
            lc = lc.map(|c| c * 288.0 / sum);
        }
        lc.map(|c| c / 200.0)
    }
}

fn shadedot(n: V3, sv: V3) -> f32 {
    let d = dot(n, sv);
    if d < 0.0 { 1.0 + d * (13.0 / 44.0) } else { 1.0 + d }
}

/// Decodes one packed pose vertex: (position in model units, normal).
fn decode(am: &AliasModel, dyn_verts: Option<&[u32]>, idx: usize) -> (V3, V3) {
    if am.md3 {
        let a0 = am.poses.get(idx).copied().unwrap_or(0);
        let a1 = am.poses.get(idx + 1).copied().unwrap_or(0);
        let p = [(a0 & 0xffff) as f32, (a0 >> 16) as f32, (a1 & 0xffff) as f32].map(|x| x - 32768.0);
        const K: f32 = std::f32::consts::TAU / 255.0;
        let (lat, lng) = (((a1 >> 16) & 0xff) as f32 * K, ((a1 >> 24) & 0xff) as f32 * K);
        (p, [lng.cos() * lat.sin(), lng.sin() * lat.sin(), lat.cos()])
    } else {
        let a = match dyn_verts {
            Some(d) => d.get(idx).copied().unwrap_or(0),
            None => am.poses.get(idx).copied().unwrap_or(0),
        };
        let n = anorms();
        (
            [(a & 0xff) as f32, ((a >> 8) & 0xff) as f32, ((a >> 16) & 0xff) as f32],
            n[((a >> 24) as usize).min(n.len() - 1)],
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn alias_cmds(
    fr: &RenderFrame,
    e: &RenderEntity,
    mi: usize,
    am: &AliasModel,
    vs: &ViewSetup,
    light: [f32; 3],
    dist: f32,
    prev: Option<Mat4>,
    lists: &mut Lists,
    b: &mut Buckets,
) {
    if am.frames.is_empty() || am.surfaces.is_empty() {
        return;
    }
    let dynp = if am.md3 { None } else { e.dyn_pose };
    let radius = dynp.map_or(am.radius, |d| d.radius);
    if !e.viewmodel && !sphere_in_frustum(&vs.frustum, e.origin, radius) {
        return;
    }
    let t = fr.time + e.syncbase as f64;
    let pose = |f: u32| {
        let (first, n, iv) = &am.frames[(f as usize).min(am.frames.len() - 1)];
        first + group_index(iv, *n as usize, t) as u32
    };
    let (pf1, pf2) = (pose(e.prev_frame), pose(e.frame));
    let rot = entity_rotation(e.angles);
    let model = match dynp {
        Some(d) => affine([[d.scale[0], 0.0, 0.0], [0.0, d.scale[1], 0.0], [0.0, 0.0, d.scale[2]]], d.origin),
        None => mul(
            &affine(rot, e.origin),
            &affine([[am.scale[0], 0.0, 0.0], [0.0, am.scale[1], 0.0], [0.0, 0.0, am.scale[2]]], am.scale_origin),
        ),
    };
    // R_SetupAliasLighting shade vector, in model space like the GLSL shader
    let q = ((e.angles[1] * (16.0 / 360.0)) as i32 & 15) as f32;
    let ang = q / 16.0 * std::f32::consts::TAU;
    let sv = norm([(-ang).cos(), (-ang).sin(), 1.0]);
    let blend = e.lerp.clamp(0.0, 1.0);
    let base = lists.alias_v.len() as u32;
    let stride = if am.md3 { 2 } else { 1 };
    let dyn_verts = dynp.map(|_| fr.dyn_verts);
    // normals: the model matrix is R·S, its inverse transpose R·S⁻¹ = M·S⁻²
    let sc = dynp.map_or(am.scale, |d| d.scale).map(|s| if s != 0.0 { 1.0 / (s * s) } else { 1.0 });
    for s in &am.surfaces {
        for j in s.first_vert..s.first_vert + s.vert_count {
            let v = am.verts[j as usize];
            let (ia, ib) = match dynp {
                Some(d) => ((d.ofs + v.vert) as usize, (d.ofs + v.vert) as usize),
                None => (
                    (s.pose_base + (pf1 * s.num_verts + v.vert) * stride) as usize,
                    (s.pose_base + (pf2 * s.num_verts + v.vert) * stride) as usize,
                ),
            };
            let (pa, na) = decode(am, dyn_verts, ia);
            let (pb, nb) = decode(am, dyn_verts, ib);
            let p: V3 = std::array::from_fn(|k| pa[k] + (pb[k] - pa[k]) * blend);
            let sh = shadedot(na, sv) + (shadedot(nb, sv) - shadedot(na, sv)) * blend;
            let n: V3 = std::array::from_fn(|k| (na[k] + (nb[k] - na[k]) * blend) * sc[k]);
            lists.alias_v.push(AliasV {
                pos: xform(&model, p),
                st: v.st,
                shade: if e.fullbright { [1.0; 3] } else { light.map(|c| c * sh) },
                nrm: norm(xform_dir(&model, n)),
            });
        }
    }
    let alpha = e.alpha.clamp(0.0, 1.0);
    let [top, bot] = e.colormap.unwrap_or([1, 6]);
    let xray = fr.settings.goggles.mode == 2 && !e.viewmodel && e.heat == crate::HEAT_BODY;
    // ragdoll poses have their own transform; not blurred
    let vel_du = prev.filter(|_| dynp.is_none()).map(|p| lists.draw(DrawU { pad: vel_pad(&p), ..Default::default() }));
    for s in &am.surfaces {
        if s.num_indices == 0 {
            continue;
        }
        let skin = if s.skins.is_empty() {
            [NO_TEX; 3]
        } else {
            let (imgs, iv) = &s.skins[e.skin as usize % s.skins.len()];
            imgs[group_index(iv, imgs.len(), t)]
        };
        let draw = DrawU {
            color: [1.0, 1.0, 1.0, alpha],
            params: [
                f32::from(s.holey),
                f32::from(skin[1] != NO_TEX),
                f32::from(e.colormap.is_some() && skin[2] != NO_TEX),
                // thermal goggles heat
                f32::from(e.heat.min(crate::HEAT_FIRE)),
            ],
            ix: [u32::from(top.min(13)), u32::from(bot.min(13)), 0, 0],
            // w: fullbright model, no per-pixel dynamic lights
            extra: [0.0, 0.0, 0.0, f32::from(e.fullbright)],
            ..Default::default()
        };
        let du = lists.draw(draw);
        let translucent = alpha < 1.0;
        let cmd = Cmd {
            pipe: if translucent { Pipe::AliasAlpha } else { Pipe::Alias },
            src: Src::Alias(mi),
            mat: skin,
            first: s.first_index,
            count: s.num_indices,
            base_vertex: base as i32,
            du,
            viewmodel: e.viewmodel,
        };
        if xray {
            // thermal goggles: warm bodies seen through walls, after the
            // other models and before the viewmodel
            let du = lists.draw(DrawU { params: [draw.params[0], draw.params[1], draw.params[2], 4.0], ..draw });
            b.trans.push((-0.25, Cmd { pipe: Pipe::AliasXray, du, ..cmd }));
        }
        if translucent && !e.viewmodel {
            b.trans.push((dist, cmd));
        } else if e.viewmodel {
            // drawn after the world so its compressed depth range can't poke through walls
            b.trans.push((-0.5, cmd));
        } else {
            if let Some(du) = vel_du {
                b.vel.push(Cmd { du, ..cmd });
            }
            if lists.views.is_empty() {
                // shadow caster of real-time dynamic lights (first view)
                lists.casters.push((cmd, [e.origin[0], e.origin[1], e.origin[2], radius], e.key));
            }
            b.opaque.push(cmd);
        }
    }
}

/// `DrawU::pad` carrying the velocity pass's previous transform (`DrawU.prev`).
fn vel_pad(prev: &Mat4) -> [f32; 32] {
    let mut pad = [0.0; 32];
    pad[..16].copy_from_slice(prev);
    pad
}

fn sprite_cmds(fr: &RenderFrame, e: &RenderEntity, sm: &SpriteModel, vs: &ViewSetup, dist: f32, lists: &mut Lists, b: &mut Buckets) {
    if sm.frames.is_empty() || !sphere_in_frustum(&vs.frustum, e.origin, sm.radius.max(16.0)) {
        return;
    }
    let f = &sm.frames[e.frame as usize % sm.frames.len()];
    if f.images.is_empty() {
        return;
    }
    let (slot, [l, r, dn, upv]) = f.images[group_index(&f.intervals, f.images.len(), fr.time + e.syncbase as f64)];
    let (sr, su) = match sm.kind {
        qformats::spr::SPR_VP_PARALLEL_UPRIGHT => {
            let fw = norm([vs.fwd[0], vs.fwd[1], 0.0]);
            ([fw[1], -fw[0], 0.0], [0.0, 0.0, 1.0])
        }
        qformats::spr::SPR_FACING_UPRIGHT => {
            let fw = norm([e.origin[0] - vs.eye[0], e.origin[1] - vs.eye[1], 0.0]);
            ([fw[1], -fw[0], 0.0], [0.0, 0.0, 1.0])
        }
        qformats::spr::SPR_ORIENTED => {
            let (_, r, u) = angle_vectors(e.angles);
            (r, u)
        }
        qformats::spr::SPR_VP_PARALLEL_ORIENTED => {
            let (s, c) = e.angles[2].to_radians().sin_cos();
            (
                [0, 1, 2].map(|i| vs.right[i] * c + vs.up[i] * s),
                [0, 1, 2].map(|i| vs.right[i] * -s + vs.up[i] * c),
            )
        }
        _ => (vs.right, vs.up),
    };
    let alpha = (e.alpha.clamp(0.0, 1.0) * 255.0) as u8;
    let p = |x: f32, y: f32| [0, 1, 2].map(|i| e.origin[i] + sr[i] * x + su[i] * y);
    let base = lists.poly_v.len() as u32;
    let first = lists.poly_i.len() as u32;
    for (pos, uv) in [(p(l, dn), [0.0, 1.0]), (p(l, upv), [0.0, 0.0]), (p(r, upv), [1.0, 0.0]), (p(r, dn), [1.0, 1.0])] {
        lists.poly_v.push(FteVertex { pos, uv, color: [255, 255, 255, alpha] });
    }
    lists.poly_i.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    // thermal goggles: sprites are hot (explosions, flames)
    let du = lists.draw(DrawU { params: [0.0; 4], ix: [0, 1, 0, 0], ..Default::default() });
    b.trans.push((
        dist,
        Cmd {
            pipe: Pipe::Poly(poly_index(FteBlend::Blend, false)),
            src: Src::Poly,
            mat: [slot, NO_TEX, NO_TEX],
            first,
            count: 6,
            base_vertex: base as i32,
            du,
            viewmodel: false,
        },
    ));
}
