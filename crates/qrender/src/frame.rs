//! Per-frame scene building and command recording.

use crate::convert::group_index;
use crate::world::{NO_LM, box_in_frustum, dot};
use crate::*;
mod portals;
mod portal_scissor;
use portals::gun_portal_quad;
/// fte.frag mode of add-on gel batches ([`crate::FteBatch::gel`]).
const GEL_MODE: f32 = 9.0;
const GEL_FAST_MODE: f32 = 10.0;
mod types;
use types::*;
pub(super) use types::UiVert;
mod math;
pub use math::*;
mod world_draw;
mod mapper;
use world_draw::*;
mod lightmaps;
mod bloom;
mod dlights;
mod rt;
pub(super) use lightmaps::LightmapRequests;

type Mat4 = [f32; 16];

fn vel_viewport(t: &Targets, [scene_w, scene_h]: [u32; 2]) -> [u32; 2] {
    [
        ((scene_w as f32 * t.vel_w as f32 / t.scene_w.max(1) as f32).round() as u32).clamp(1, t.vel_w),
        ((scene_h as f32 * t.vel_h as f32 / t.scene_h.max(1) as f32).round() as u32).clamp(1, t.vel_h),
    ]
}

impl Renderer {
    /// Draws and presents one frame.
    pub fn render(&mut self, fr: &RenderFrame) -> Result<()> {
        if let Some(s) = &self.swap
            && (self.needs_resize || s.vsync != fr.settings.vsync)
        {
            self.create_swapchain(fr.settings.vsync)?;
        }
        self.apply_settings(&fr.settings)?;
        // r_water_reflections: 1 planar (portal views), 2 ray traced
        let refl_mode = self.reflection_mode(&fr.settings);
        let rt_enabled = refl_mode == 2 && fr.settings.goggles.mode != 2 && (fr.view.is_some() || !fr.splits.is_empty());
        if rt_enabled {
            self.ensure_rt()?;
        }
        self.gun_targets |= fr.gun_portals.iter().any(|g| g.live && g.exit.is_some()) || refl_mode == 1;
        if self.targets.as_ref().is_some_and(|t| t.portals.is_empty())
            && (self.gun_targets || self.world.as_ref().is_some_and(|w| !w.portals.is_empty()))
        {
            self.create_targets()?;
        }
        if self.targets.is_none() && (self.swap.is_some() || self.width > 0) {
            self.create_targets()?;
        }
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        if rt_enabled {
            self.ensure_rt_refl()?;
        }
        if fr.settings.ssao > 0.0 && (fr.view.is_some() || !fr.splits.is_empty()) {
            self.ensure_ao_source()?;
        }
        let droplets_active = fr.view.is_some_and(|v| v.water_screen.drops_age() >= 0.0)
            || fr.splits.iter().any(|s| s.view.water_screen.drops_age() >= 0.0);
        if droplets_active {
            self.ensure_droplets()?;
        }
        self.prepare_shader(fr.settings.shader_scale != 0)?;
        let dev = self.gpu.device.clone();
        let fi = self.cur;
        let (fence, acquire, cmd) = (
            self.frames[fi].fence,
            self.frames[fi].acquire,
            self.frames[fi].cmd,
        );
        let wait = std::time::Instant::now();
        // SAFETY: fence owned by this frame.
        unsafe { dev.wait_for_fences(&[fence], true, u64::MAX)? };
        for b in std::mem::take(&mut self.frames[fi].staging) {
            self.gpu.free_buffer(b);
        }
        self.fence_wait_ns = wait.elapsed().as_nanos() as u64;
        self.init_gpu_times(fr.splits.len().max(1))?;
        self.collect_gpu_times(fi);
        let mut img_idx = 0u32;
        let present = self.presenting();
        if present && let (Some(s), Some(f)) = (&self.swap, &self.gpu.swapchain_fn) {
            // SAFETY: valid swapchain/semaphore.
            match unsafe { f.acquire_next_image(s.sc, u64::MAX, acquire, vk::Fence::null()) } {
                Ok((i, _)) => img_idx = i,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.needs_resize = true;
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        }
        // SAFETY: fence not in use (waited).
        unsafe { dev.reset_fences(&[fence])? };

        let (w, h) = (self.width, self.height);
        let mut dynw = Dyn {
            ptr: self.frames[fi].dynbuf.mem.ptr,
            ofs: OFS_BUMP,
            cap: self.frames[fi].dynbuf.size,
        };
        let st = &fr.settings;
        self.mapper_stats = MapperStats::default();
        let mut teleport_ripples = [[0.0; 4]; MAX_TELEPORT_RIPPLES];
        let ripple_count = if st.teleport_windows {
            fr.teleport_ripples.len().min(MAX_TELEPORT_RIPPLES)
        } else {
            0
        };
        teleport_ripples[..ripple_count].copy_from_slice(&fr.teleport_ripples[..ripple_count]);
        let mut alphas = [
            st.wateralpha,
            st.lavaalpha.unwrap_or(st.wateralpha),
            st.slimealpha.unwrap_or(st.wateralpha),
            st.telealpha.unwrap_or(st.wateralpha),
        ]
        .map(|a| a.clamp(0.0, 1.0));
        if let Some(wg) = &mut self.world {
            if st.wateralpha_vised_only && !wg.cpu.liquid_seethrough(0) {
                alphas[0] = 1.0;
            }
            let need_novis = wg.cpu.needs_transparent_novis(alphas);
            wg.cpu.set_transparent_liquids(need_novis);
        }
        let time = fr.time;
        let mut styles = [256.0f32; 256];
        let mut style_ints = [256i32; 256];
        for (i, &v) in fr.lightstyles.iter().take(256).enumerate() {
            styles[i] = v as f32;
            style_ints[i] = v;
        }

        // views: split-screen cells, or the one view over the whole output
        let views_in: Vec<(Option<View>, [u32; 4], [f32; 4])> = if fr.splits.is_empty() {
            vec![(fr.view, [0, 0, w, h], fr.blend)]
        } else {
            fr.splits.iter().map(|s| (Some(s.view), s.rect, s.blend)).collect()
        };
        let multi = views_in.len() > 1;
        let primary = views_in[0].0;

        // liquid ripples/splashes (CPU side); droplets join the particles
        let mut water_parts = Vec::new();
        if st.water_ripples <= 0.0 && !st.water_splashes {
            self.water_fx.reset();
        } else if let Some(wg) = &self.world
            && primary.is_some()
        {
            let models = &self.models;
            let movers = fr.entities.iter().filter_map(|e| {
                let alias = matches!(models.get(e.model.0 as usize), Some(Model::Alias(_)));
                // the local player's own body is tracked as `fr.player`
                let own_body = e.portal_only || e.follows_camera;
                (alias && e.key != 0 && e.key & crate::ATTACHED_KEY == 0 && !e.viewmodel && !own_body).then_some((e.key, e.origin))
            });
            water_parts = self.water_fx.update(
                &wg.cpu,
                time,
                movers,
                fr.player,
                primary.map(|v| v.origin),
                fr.shot_impacts,
                st.water_ripples > 0.0,
                st.water_splashes,
                st.water_quality.min(1),
            );
        }

        // per-object motion blur: last frame's transforms by entity key
        let mb_on = st.motion_blur > 0.0 && primary.is_some();
        let prev_xforms = std::mem::take(&mut self.prev_xforms);
        let mb_dt = time - self.prev_xform_time;
        let prev_ok = mb_on && mb_dt > 0.0 && mb_dt < 0.25;
        let mb_scale = if prev_ok { st.motion_blur.min(4.0) / (60.0 * mb_dt as f32) } else { 0.0 };
        let mb_quality = st.motion_blur_quality.min(1);
        let mut xforms = std::mem::take(&mut self.cur_xforms);
        xforms.clear();
        if mb_on {
            for e in fr.entities {
                if e.key != 0
                    && !e.viewmodel
                    && !e.follows_camera
                    && !e.portal_only
                    && !self.hidden_entities.contains(&e.key)
                {
                    xforms.insert(e.key, (e.origin, e.angles));
                }
            }
        }
        // last frame's model transform rows, when the entity moved perceptibly
        let prev_rows = |vs: &ViewSetup, view_px: [f32; 2], e: &RenderEntity, center: [f32; 3], local_mid: [f32; 3], radius: f32, local: Option<&Mat4>| -> Option<[[f32; 4]; 3]> {
            if !prev_ok || e.key == 0 || e.viewmodel || e.follows_camera {
                return None;
            }
            let &(po, pa) = prev_xforms.get(&e.key)?;
            if po == e.origin && pa == e.angles {
                return None;
            }
            let d = sub(e.origin, po);
            if dot(d, d) > 256.0 * 256.0 {
                return None; // teleported
            }
            let prot = entity_rotation(pa);
            let prev_center = if local_mid == [0.0; 3] {
                po
            } else {
                add(po, rotate3(&prot, local_mid))
            };
            if !vel_entity_visible(vs, center, prev_center, e.angles, pa, radius, mb_scale, view_px, mb_quality) {
                return None;
            }
            let m = affine(prot, po);
            Some(rows3(&local.map_or(m, |l| mul(&m, l))))
        };
        // CPU-built alias poses (ragdolls), indexed in u32s from the buffer start
        let dyn_base = if fr.dyn_verts.is_empty() { None } else { dynw.push(fr.dyn_verts, 4).map(|o| (o / 4) as u32) };
        // lightmaps to update: the union over all views
        let mut lm_vis = std::mem::take(&mut self.lightmap_requests);
        lm_vis.begin(self.world.as_ref().map_or(0, |w| w.cpu.lm_surfs.len()));
        // scene target size (render scale)
        let (sw, sh) = self
            .targets
            .as_ref()
            .map_or((w, h), |t| (t.scene_w, t.scene_h));
        // classic particles are view-independent: uploaded once
        let mut particle_range: Option<Range> = None;
        let fog = fr.fog;
        let mut passes: Vec<ViewPass> = Vec::with_capacity(views_in.len());
        let mut primary_water_bounds: Option<([f32; 3], [f32; 3])> = None;
        let mut primary_eye_z = 0.0f32;
        for (vi, &(view_in, rect, blend)) in views_in.iter().enumerate() {
        let vbit = 1u8 << vi.min(7);
        let mut world_draws: Vec<WorldDraw> = Vec::new();
        let mut vel_draws: Vec<VelDraw> = Vec::new();
        let mut alias_draws: Vec<AliasDraw> = Vec::new();
        let mut sprite_draws: Vec<SpriteDraw> = Vec::new();
        let mut trans: Vec<(f32, Trans)> = Vec::new();
        let mut cull: Option<CullFrame> = None;
        let mut particles = Range::default();
        // FTE particles: (vertex offset, index offset, batches with texture slots)
        let mut fte: Option<FteDraw> = None;
        let mut fte_quads: Option<FteQuadDraw> = None;
        // this view's scene region (top-left of the target) and its 3D height
        let (scene_w, scene_h) = crate::projection::view_extent(
            rect, view_in.map_or(0, |v| v.reserve_bottom), (w, h), (sw, sh),
        );
        let view = view_in.map(|mut v| {
            if !multi
                && let (Some(index), Some(distance), Some(w)) =
                    (self.isolated_portal, self.isolated_distance, self.world.as_ref())
            {
                let p = &w.portals[index];
                v.origin = std::array::from_fn(|k| p.center[k] + p.normal[k] * distance);
                v.angles = [0.0, (-p.normal[1]).atan2(-p.normal[0]).to_degrees(), 0.0];
            }
            setup_view(&v, scene_w, scene_h)
        });
        let view_px = [
            w.max(1) as f32 * (scene_w as f32 / sw.max(1) as f32),
            h.max(1) as f32 * (scene_h as f32 / sh.max(1) as f32),
        ];
        let mut portal_draws = if fr.settings.teleport_windows || !fr.gun_portals.is_empty() || refl_mode == 1 {
            self.prepare_portals(view.as_ref(), [scene_w, scene_h], fr.gun_portals, &fr.settings, &mut dynw, time, alphas, &mut lm_vis, ripple_count > 0)
        } else {
            Vec::new()
        };

        if vi == 0 && let Some(wg) = self.world.as_mut() {
            wg.cpu.lock_pvs(st.mapper.lock_pvs, view.as_ref().map(|vs| vs.eye));
        }
        let measure = vi == 0 && (st.mapper.speeds || st.mapper.wire != 0);
        let mut leaves = 0;
        if let Some(vs) = &view {
            if let Some(wg) = self.world.as_mut() {
                if let Some(index) = self.isolated_portal.filter(|_| !multi) {
                    let p = &wg.portals[index];
                    let surfs: Vec<_> = wg.cpu.surfaces.iter().enumerate().filter_map(|(i, s)| {
                        let normal = if s.flipped { s.normal.map(|x| -x) } else { s.normal };
                        (s.mins == p.mins && s.maxs == p.maxs && normal == p.normal)
                            .then_some(i as u32)
                    }).collect();
                    world_draws.push(build_world_draw(
                        wg, &mut self.buckets, &mut dynw, &surfs, false, time,
                        alphas, 1.0, ident(), &mut lm_vis,
                    ));
                } else if measure {
                    let (surfs, count) = wg.cpu.view_surfaces(vs.eye, &vs.frustum);
                    leaves = count;
                    world_draws.push(build_world_draw(
                        wg, &mut self.buckets, &mut dynw, &surfs, false, time,
                        alphas, 1.0, ident(), &mut lm_vis,
                    ));
                } else if wg.cpu.hidden_surfaces.is_empty() {
                    if let Some((wd, cf)) = gpu_world_draw(wg, &mut dynw, vs, time, alphas, fi) {
                        world_draws.push(wd);
                        cull = Some(cf);
                    }
                } else {
                    let surfs = wg.cpu.surfaces_for_leaf(wg.cpu.view_leaf(vs.eye));
                    world_draws.push(build_world_draw(
                        wg, &mut self.buckets, &mut dynw, &surfs, false, time,
                        alphas, 1.0, ident(), &mut lm_vis,
                    ));
                }
            }
            // Only activate ray-traced reflections when at least one reflecting
            // liquid (water/slime) surface is visible in this view's PVS & frustum.
            let mut water_bounds: Option<([f32; 3], [f32; 3])> = None;
            if rt_enabled && self.isolated_portal.is_none()
                && let Some(wg) = self.world.as_ref()
            {
                let use_bits = cull.is_some();
                for &(li, si) in &wg.water_marks {
                    if use_bits && wg.scratch_bits.get((li >> 5) as usize).is_none_or(|&w| w & (1 << (li & 31)) == 0) {
                        continue;
                    }
                    let s = &wg.cpu.surfaces[si as usize];
                    if wg.cpu.hidden_surfaces.contains(&si) {
                        continue;
                    }
                    let dd = dot(vs.eye, s.normal) - s.dist;
                    if (dd < 0.0) != s.flipped {
                        continue;
                    }
                    if !box_in_frustum(&vs.frustum, s.mins, s.maxs) {
                        continue;
                    }
                    water_bounds = Some(match water_bounds {
                        Some((mn, mx)) => (
                            [mn[0].min(s.mins[0]), mn[1].min(s.mins[1]), mn[2].min(s.mins[2])],
                            [mx[0].max(s.maxs[0]), mx[1].max(s.maxs[1]), mx[2].max(s.maxs[2])],
                        ),
                        None => (s.mins, s.maxs),
                    });
                }
                if wg.has_brush_water {
                    for e in fr.entities {
                        if let Some(Model::Brush(sm)) = self.models.get(e.model.0 as usize)
                            && wg.brush_has_water.get(*sm as usize).copied().unwrap_or(false)
                            && let Some(sub_) = wg.cpu.submodels.get(*sm as usize)
                        {
                            let mn = [0, 1, 2].map(|i| sub_.mins[i] + e.origin[i]);
                            let mx = [0, 1, 2].map(|i| sub_.maxs[i] + e.origin[i]);
                            if box_in_frustum(&vs.frustum, mn, mx) {
                                water_bounds = Some(match water_bounds {
                                    Some((a, b)) => (
                                        [a[0].min(mn[0]), a[1].min(mn[1]), a[2].min(mn[2])],
                                        [b[0].max(mx[0]), b[1].max(mx[1]), b[2].max(mx[2])],
                                    ),
                                    None => (mn, mx),
                                });
                            }
                        }
                    }
                }
            }
            if vi == 0 {
                primary_water_bounds = water_bounds;
                primary_eye_z = vs.eye[2];
            }
            let rt_on = water_bounds.is_some();
            if rt_on && vi == 0 && let Some(wg) = self.world.as_mut() {
                // ray-traced reflections reach surfaces outside the view:
                // keep the lightmaps of the eye's PVS current
                for &si in wg.cpu.portal_surfaces(vs.eye).iter() {
                    let lm = wg.draw_surfaces[si as usize].lm;
                    if lm != NO_LM {
                        lm_vis.push(lm);
                    }
                }
            }
            let mut brush_surfs: Vec<u32> = Vec::new();
            for e in fr.entities {
                if self.hidden_entities.contains(&e.key) || (e.views != 0 && e.views & vbit == 0) { continue; }
                let Some(m) = self.models.get(e.model.0 as usize) else {
                    continue;
                };
                if st.mapper.lightmap && !matches!(m, Model::Brush(_)) {
                    continue;
                }
                let rot = entity_rotation(e.angles);
                let dist = dot(sub(e.origin, vs.eye), vs.fwd);
                match m {
                    Model::Brush(sm) => {
                        let Some(wg) = self.world.as_ref() else {
                            continue;
                        };
                        let Some(sub_) = wg.cpu.submodels.get(*sm as usize) else {
                            continue;
                        };
                        let rotated = e.angles.iter().any(|&a| a != 0.0);
                        let local_mid = [0, 1, 2].map(|i| 0.5 * (sub_.mins[i] + sub_.maxs[i]));
                        let half = [0, 1, 2].map(|i| 0.5 * (sub_.maxs[i] - sub_.mins[i]));
                        let radius = dot(half, half).sqrt();
                        let center = add(e.origin, rotate3(&rot, local_mid));
                        let main_visible = if rotated {
                            let r = [0, 1, 2]
                                .map(|i| sub_.mins[i].abs().max(sub_.maxs[i].abs()))
                                .iter()
                                .map(|x| x * x)
                                .sum::<f32>()
                                .sqrt();
                            sphere_in_frustum(&vs.frustum, e.origin, r)
                        } else {
                            let mn = [0, 1, 2].map(|i| sub_.mins[i] + e.origin[i]);
                            let mx = [0, 1, 2].map(|i| sub_.maxs[i] + e.origin[i]);
                            box_in_frustum(&vs.frustum, mn, mx)
                        };
                        let portal_visible = !portal_draws.is_empty()
                            && portal_draws.iter().any(|pd| {
                                pd.frustum.is_some_and(|f| {
                                    let c = pd.portal.clip();
                                    dot([c[0], c[1], c[2]], center) + c[3] + radius >= 0.0
                                        && sphere_in_frustum(&f, center, radius)
                                })
                            });
                        if !main_visible && !portal_visible {
                            // Offscreen brush entities still participate in ray-traced reflections
                            // via `fr.entities`, so keep their lightmaps current when RT is on.
                            if rt_on && vi == 0 {
                                for si in sub_.first_surface..sub_.first_surface + sub_.num_surfaces {
                                    if let Some(ds) = wg.draw_surfaces.get(si as usize)
                                        && ds.lm != NO_LM
                                    {
                                        lm_vis.push(ds.lm);
                                    }
                                }
                            }
                            continue;
                        }
                        // eye in model space (R^T * (eye - origin))
                        let d = sub(vs.eye, e.origin);
                        let le = [0, 1, 2]
                            .map(|c| rot[0][c] * d[0] + rot[1][c] * d[1] + rot[2][c] * d[2]);
                        brush_surfs.clear();
                        for si in sub_.first_surface..sub_.first_surface + sub_.num_surfaces {
                            let Some(s) = wg.cpu.surfaces.get(si as usize) else {
                                break;
                            };
                            if s.num_indices == 0 {
                                continue;
                            }
                            if !portal_visible {
                                let dd = dot(le, s.normal) - s.dist;
                                if (dd < 0.0) != s.flipped {
                                    continue;
                                }
                            }
                            brush_surfs.push(si);
                        }
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        let mut wd = build_world_draw(
                            wg,
                            &mut self.buckets,
                            &mut dynw,
                            &brush_surfs,
                            e.frame != 0,
                            time,
                            alphas,
                            alpha,
                            affine(rot, e.origin),
                            &mut lm_vis,
                        );
                        wd.zfix = true;
                        wd.main_visible = main_visible;
                        wd.bounds = [center[0], center[1], center[2], radius];
                        if main_visible {
                            if alpha < 1.0 {
                                trans.push((dist, Trans::World(world_draws.len())));
                            } else if !e.portal_only
                                && let Some(r) = prev_rows(vs, view_px, e, center, local_mid, radius, None)
                            {
                                vel_draws.push(VelDraw::World(world_draws.len(), r));
                            }
                        }
                        world_draws.push(wd);
                    }
                    Model::Alias(am) => {
                        if am.frames.is_empty() || am.surfaces.is_empty() {
                            continue;
                        }
                        let dynp = if am.md3 || am.iqm {
                            None
                        } else {
                            match e.dyn_pose {
                                Some(d) => match dyn_base {
                                    Some(base) => Some((base + d.ofs, d)),
                                    None => continue,
                                },
                                None => None,
                            }
                        };
                        let radius = dynp.map_or(am.radius, |(_, d)| d.radius);
                        let main_visible = e.viewmodel || sphere_in_frustum(&vs.frustum, e.origin, radius);
                        // Keep offscreen models only when visible in a portal or relevant to visible RT water.
                        if !main_visible {
                            let for_rt = rt_on && vi == 0 && rt::rt_alias_relevant(e.origin, radius, water_bounds, vs.eye[2]);
                            let in_portal = !for_rt
                                && !portal_draws.is_empty()
                                && portal_draws.iter().any(|pd| {
                                    pd.frustum.is_some_and(|f| {
                                        let c = pd.portal.clip();
                                        dot([c[0], c[1], c[2]], e.origin) + c[3] + radius >= 0.0
                                            && sphere_in_frustum(&f, e.origin, radius)
                                    })
                                });
                            if !for_rt && !in_portal {
                                continue;
                            }
                        }
                        let t = time + e.syncbase as f64;
                        let pose = |f: u32| {
                            let (first, n, iv) = &am.frames[(f as usize).min(am.frames.len() - 1)];
                            first + group_index(iv, *n as usize, t) as u32
                        };
                        let pf1 = pose(e.prev_frame);
                        let pf2 = pose(e.frame);
                        let local = affine(
                            [
                                [am.scale[0], 0.0, 0.0],
                                [0.0, am.scale[1], 0.0],
                                [0.0, 0.0, am.scale[2]],
                            ],
                            am.scale_origin,
                        );
                        let model = match dynp {
                            Some((_, d)) => affine(
                                [[d.scale[0], 0.0, 0.0], [0.0, d.scale[1], 0.0], [0.0, 0.0, d.scale[2]]],
                                d.origin,
                            ),
                            None => mul(&affine(rot, e.origin), &local),
                        };
                        // R_SetupAliasLighting
                        let wl = self.world.as_ref().map(|w| &w.cpu);
                        let (mut lc, ldir) = if st.fullbright {
                            ([100.0; 3], None) // ironwail: lightcolor 0.5 (x200)
                        } else {
                            wl.and_then(|w| {
                                w.light_point_dir(e.origin, &style_ints).or_else(|| {
                                    w.light_point_dir(
                                        [e.origin[0], e.origin[1], e.origin[2] + am.maxs_z * 0.5],
                                        &style_ints,
                                    )
                                })
                            })
                            .unwrap_or(([128.0; 3], None))
                        };
                        // real-time dynamic lights shade models per pixel instead
                        if st.dynamic && !st.fullbright && !st.rt_dlights {
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
                            if wl.is_some_and(|w| w.has_hdr) && st.hdr_lightmaps {
                                // Soft rolloff for HDR lightmaps so models reflect strong light without hard clipping.
                                let excess = sum - 288.0;
                                let target = 288.0 + 192.0 * (excess / (excess + 192.0));
                                lc = lc.map(|c| c * target / sum);
                            } else {
                                lc = lc.map(|c| c * 288.0 / sum);
                            }
                        }
                        let sv = if st.deluxemapping
                            && let Some(wd) = ldir
                        {
                            // Transform world-space baked light direction into model space (R^T * wd).
                            let local_d = [0, 1, 2].map(|c| rot[0][c] * wd[0] + rot[1][c] * wd[1] + rot[2][c] * wd[2]);
                            norm(local_d)
                        } else {
                            let q = ((e.angles[1] * (16.0 / 360.0)) as i32 & 15) as f32;
                            let ang = q / 16.0 * std::f32::consts::TAU;
                            norm([(-ang).cos(), (-ang).sin(), 1.0])
                        };
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        let mut base_flags = 0u32;
                        if e.fullbright {
                            base_flags |= 2;
                        }
                        let [top, bot] = e.colormap.unwrap_or([1, 6]);
                        if e.colormap.is_some() {
                            base_flags |= 4;
                        }
                        if dynp.is_some() {
                            base_flags |= 16;
                        }
                        if am.md3 {
                            base_flags |= 32;
                        }
                        if am.iqm {
                            base_flags |= 64;
                        }
                        base_flags |= (e.heat.min(3) as u32) << 8;
                        let stride = if am.md3 { 2 } else { 1 };
                        let prev_r = if main_visible && !e.portal_only && (alpha >= 1.0 || e.viewmodel) {
                            prev_rows(vs, view_px, e, e.origin, [0.0; 3], radius, Some(&local))
                        } else {
                            None
                        };
                        for surf in &am.surfaces {
                            if surf.num_indices == 0 {
                                continue;
                            }
                            let (p1, p2) = if am.iqm {
                                let bone_stride = am.num_bones * 12;
                                (am.bone_base + pf1 * bone_stride, am.bone_base + pf2 * bone_stride)
                            } else {
                                match dynp {
                                    Some((p, _)) => (p, p),
                                    None => (
                                        surf.verts_base + pf1 * surf.num_verts * stride,
                                        surf.verts_base + pf2 * surf.num_verts * stride,
                                    ),
                                }
                            };
                            let skin = if surf.skins.is_empty() {
                                [NO_TEX; 3]
                            } else {
                                let (imgs, iv) = &surf.skins[e.skin as usize % surf.skins.len()];
                                imgs[group_index(iv, imgs.len(), t)]
                            };
                            let push = AliasPush {
                                model,
                                light: [lc[0] / 200.0, lc[1] / 200.0, lc[2] / 200.0, alpha],
                                shadevec: [sv[0], sv[1], sv[2], e.lerp.clamp(0.0, 1.0)],
                                poses: [p1, p2, top.min(13) as u32, bot.min(13) as u32],
                                texs: [skin[0], skin[1], skin[2], base_flags | u32::from(surf.holey)],
                            };
                            if e.portal_only || !main_visible {
                                // offscreen main-view models (drawn only in portal views if visible there)
                            } else if alpha < 1.0 && !e.viewmodel {
                                trans.push((dist, Trans::Alias(alias_draws.len())));
                            } else if let Some(r) = prev_r {
                                vel_draws.push(VelDraw::Alias(alias_draws.len(), r));
                            }
                            alias_draws.push(AliasDraw {
                                model: e.model.0 as usize,
                                first_index: surf.first_index,
                                num_indices: surf.num_indices,
                                push,
                                origin: e.origin,
                                radius,
                                main_visible,
                                viewmodel: e.viewmodel,
                                portal_only: e.portal_only,
                                key: e.key,
                            });
                        }
                    }
                    Model::Sprite(sm) => {
                        if sm.frames.is_empty()
                            || !sphere_in_frustum(&vs.frustum, e.origin, sm.radius.max(16.0))
                        {
                            continue;
                        }
                        let f = &sm.frames[e.frame as usize % sm.frames.len()];
                        if f.images.is_empty() {
                            continue;
                        }
                        let (slot, [l, r, dn, upv]) = f.images
                            [group_index(&f.intervals, f.images.len(), time + e.syncbase as f64)];
                        let (sr, su) = match sm.kind {
                            qformats::spr::SPR_VP_PARALLEL_UPRIGHT => {
                                let fw = norm([vs.fwd[0], vs.fwd[1], 0.0]);
                                ([fw[1], -fw[0], 0.0], [0.0, 0.0, 1.0])
                            }
                            qformats::spr::SPR_FACING_UPRIGHT => {
                                let fw =
                                    norm([e.origin[0] - vs.eye[0], e.origin[1] - vs.eye[1], 0.0]);
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
                        let p =
                            |x: f32, y: f32| [0, 1, 2].map(|i| e.origin[i] + sr[i] * x + su[i] * y);
                        let v = |pos: [f32; 3], u: f32, vv: f32| [pos[0], pos[1], pos[2], u, vv];
                        let quad = [
                            v(p(l, dn), 0.0, 1.0),
                            v(p(l, upv), 0.0, 0.0),
                            v(p(r, upv), 1.0, 0.0),
                            v(p(l, dn), 0.0, 1.0),
                            v(p(r, upv), 1.0, 0.0),
                            v(p(r, dn), 1.0, 1.0),
                        ];
                        let Some(vofs) = dynw.push(&quad, 16) else {
                            continue;
                        };
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        trans.push((dist, Trans::Sprite(sprite_draws.len())));
                        sprite_draws.push(SpriteDraw {
                            vofs,
                            tex: slot,
                            alpha,
                        });
                    }
                }
            }
            if let Some(r) = particle_range {
                particles = r;
            } else if !fr.particles.is_empty() {
                if let Some((ofs, ptr)) = dynw.alloc_uninit::<([f32; 3], u32)>(fr.particles.len(), 16) {
                    for (idx, p) in fr.particles.iter().enumerate() {
                        let c = self.palette[p.color as usize];
                        let a = (p.alpha.clamp(0.0, 1.0) * 255.0) as u32;
                        let rec = (
                            p.origin,
                            c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | a << 24,
                        );
                        // SAFETY: `alloc_uninit` reserved `fr.particles.len()` aligned elements.
                        unsafe { ptr.add(idx).write(rec) };
                    }
                    particles = Range {
                        ofs,
                        count: fr.particles.len() as u32,
                    };
                }
                particle_range = Some(particles);
            }
            // water spray puffs: camera-facing quads appended to the FTE geometry
            let mut spray_v: Vec<FteVertex> = Vec::new();
            let mut spray_i: Vec<u32> = Vec::new();
            let mut spray_b: Vec<FteBatch> = Vec::new();
            if let Some(pic) = self.splash_pic {
                let base = fr.fte_vertices.len() as u32;
                // splash droplets: small soft dots (classic particles are too
                // coarse), kept at least ~1.5 px wide in the distance
                let px = 2.0 * vs.tan[1] / scene_h.max(1) as f32;
                let drops = water_parts.iter().map(|d| {
                    let c = self.palette[d.color as usize];
                    let dist = dot(sub(d.origin, vs.eye), vs.fwd).max(1.0);
                    crate::water::SprayPuff {
                        pos: d.origin,
                        radius: (0.75f32).max(0.75 * px * dist),
                        color: [c[0], c[1], c[2], (d.alpha.clamp(0.0, 1.0) * 255.0) as u8],
                    }
                });
                for p in self.water_fx.spray(time).chain(drops) {
                    if p.color[3] == 0 {
                        continue;
                    }
                    let (r, u) = (vs.right, vs.up);
                    let corner = |sr: f32, su: f32| {
                        std::array::from_fn(|k| p.pos[k] + (r[k] * sr + u[k] * su) * p.radius)
                    };
                    let n = base + spray_v.len() as u32;
                    for (sr, su, uv) in [(-1.0, -1.0, [0.0, 1.0]), (1.0, -1.0, [1.0, 1.0]), (1.0, 1.0, [1.0, 0.0]), (-1.0, 1.0, [0.0, 0.0])] {
                        spray_v.push(FteVertex { pos: corner(sr, su), uv, color: p.color });
                    }
                    spray_i.extend_from_slice(&[n, n + 1, n + 2, n, n + 2, n + 3]);
                }
                if !spray_i.is_empty() {
                    spray_b.push(FteBatch {
                        pic,
                        blend: FteBlend::Blend,
                        lines: false,
                        first_index: fr.fte_indices.len() as u32,
                        num_indices: spray_i.len() as u32,
                        soft: true, gel: false,
                    });
                }
            }
            let mut gun_v: Vec<FteVertex> = Vec::new();
            let mut gun_i: Vec<u32> = Vec::new();
            let mut gun_batches = Vec::new();
            // the view texture of gun portal `gi` as seen in view `parent`
            let slot_in = |parent: Option<usize>, gi: usize| {
                portal_draws
                    .iter()
                    .position(|d| d.parent == parent && d.portal.gun == Some(gi))
                    .and_then(|k| Some(self.targets.as_ref()?.portals.get(k)?.1))
                    .unwrap_or(NO_TEX)
            };
            if !fr.gun_portals.is_empty() {
                let v_base = (fr.fte_vertices.len() + spray_v.len()) as u32;
                let i_base = (fr.fte_indices.len() + spray_i.len()) as u32;
                gun_v.reserve(fr.gun_portals.len() * 4);
                gun_i.reserve(fr.gun_portals.len() * 6);
                for (gi, g) in fr.gun_portals.iter().enumerate() {
                    gun_batches.push((gi, gun_portal_quad(g, slot_in(None, gi), v_base, i_base, &mut gun_v, &mut gun_i)));
                }
            }
            let total_indices = fr.fte_indices.len() + spray_i.len() + gun_i.len();
            if !(fr.fte_batches.is_empty() && spray_b.is_empty() && gun_batches.is_empty()) && total_indices > 0 {
                let gel_mode = if st.gel_quality == 0 { GEL_FAST_MODE } else { GEL_MODE };
                let mut parts: Vec<(usize, u32, crate::FteBatch, f32)> =
                    Vec::with_capacity(fr.fte_batches.len() + spray_b.len());
                for b in fr.fte_batches.iter().chain(&spray_b) {
                    let Some(pic) = self.pics.get(b.pic.0 as usize) else { continue };
                    let pipe = crate::pipelines::fte_index(b.blend, b.lines);
                    let slot = pic.slot;
                    let mode = if b.gel { gel_mode } else { b.blend.shader_mode() };
                    // Merge adjacent batches with identical state and contiguous indices.
                    if let Some((lp, ls, lb, lm)) = parts.last_mut()
                        && *lp == pipe
                        && *ls == slot
                        && *lm == mode
                        && lb.soft == b.soft
                        && lb.blend == b.blend
                        && lb.first_index + lb.num_indices == b.first_index
                    {
                        lb.num_indices += b.num_indices;
                    } else {
                        parts.push((pipe, slot, *b, mode));
                    }
                }
                // portal views: the same particles (unless gated by portal quality) and
                // every gun portal but the one they look out of, with nested views
                let per_view: Vec<Vec<_>> = portal_draws
                    .iter()
                    .enumerate()
                    .map(|(k, pd)| {
                        let exit = pd.portal.destination;
                        let mut b = if portals::skip_portal_extras(st.portal_quality.min(2), pd.depth) {
                            Vec::new()
                        } else {
                            parts.iter().map(|&(p, s, mut b, m)| { b.soft = false; (p, s, b, m) }).collect()
                        };
                        for &(gi, (p, _, gb, m)) in &gun_batches {
                            if fr.gun_portals[gi].origin != exit {
                                b.push((p, slot_in(Some(k), gi), gb, m));
                            }
                        }
                        b
                    })
                    .collect();
                for (d, b) in portal_draws.iter_mut().zip(per_view) {
                    d.fte = b;
                }
                let batches: Vec<_> = parts.into_iter().chain(gun_batches.into_iter().map(|(_, b)| b)).collect();
                if let (Some(v), Some(i)) = (
                    dynw.push_slices(&[fr.fte_vertices, &spray_v, &gun_v], 16),
                    dynw.push_slices(&[fr.fte_indices, &spray_i, &gun_i], 4),
                ) {
                    fte = Some((v, i, batches));
                }
            }
            if !fr.fte_quad_batches.is_empty() && !fr.fte_quads.is_empty() {
                let mut qbatches: Vec<(usize, u32, crate::FteBatch, f32)> =
                    Vec::with_capacity(fr.fte_quad_batches.len());
                for b in fr.fte_quad_batches {
                    let Some(pic) = self.pics.get(b.pic.0 as usize) else { continue };
                    let pipe = crate::pipelines::fte_index(b.blend, false).min(7);
                    let slot = pic.slot;
                    let mode = b.blend.shader_mode();
                    if let Some((lp, ls, lb, lm)) = qbatches.last_mut()
                        && *lp == pipe
                        && *ls == slot
                        && *lm == mode
                        && lb.soft == b.soft
                        && lb.blend == b.blend
                        && lb.first_index + lb.num_indices == b.first_index
                    {
                        lb.num_indices += b.num_indices;
                    } else {
                        qbatches.push((pipe, slot, *b, mode));
                    }
                }
                if !qbatches.is_empty()
                    && let Some(qofs) = dynw.push(fr.fte_quads, 16)
                {
                    for pd in &mut portal_draws {
                        if !portals::skip_portal_extras(st.portal_quality.min(2), pd.depth) {
                            pd.fte_quads = qbatches
                                .iter()
                                .map(|&(p, s, mut b, m)| {
                                    b.soft = false;
                                    (p, s, b, m)
                                })
                                .collect();
                        }
                    }
                    fte_quads = Some((qofs, qbatches));
                }
            }
            trans.sort_by(|a, b| b.0.total_cmp(&a.0));
        }
        if measure {
            self.mapper_stats = mapper::stats(&world_draws, &alias_draws, &sprite_draws, particles, fte.as_ref(), fte_quads.as_ref(), leaves);
        }
        passes.push(ViewPass {
            speed_blur: view_in.map_or(0.0, |v| v.speed_blur.clamp(0.0, 1.0)),
            warp: match view_in.map(|v| v.waterwarp) {
                Some(crate::WaterWarp::Screen(t)) => t.max(0.0) % 1000.0,
                _ => -1.0,
            },
            water: view_in.map(|v| v.water_screen).unwrap_or_default(),
            view,
            rect,
            blend,
            scene_w,
            scene_h,
            ubo: 0,
            world_draws,
            alias_draws,
            sprite_draws,
            trans,
            vel_draws,
            cull,
            particles,
            fte,
            fte_quads,
            portal_draws,
        });
        }

        let rt_build = if primary_water_bounds.is_some() {
            passes.first().and_then(|p| self.prepare_rt(fi, fr, &p.alias_draws, time, primary_water_bounds, primary_eye_z))
        } else {
            None
        };
        let rt_half_on = rt_build.is_some() && self.targets.as_ref().is_some_and(|t| t.rt_refl.is_some());
        let (water_pipe, water_alpha_pipe) = match (rt_build.is_some(), rt_half_on) {
            (true, false) => (self.pipes.water_rt, self.pipes.water_rt_alpha),
            _ => (self.pipes.water, self.pipes.water_alpha),
        };

        // UI vertices (split into scissor ranges whenever Draw2D::Clip appears)
        let mut ui = std::mem::take(&mut self.ui_verts);
        let mut ui_ranges = std::mem::take(&mut self.ui_ranges);
        ui.clear();
        ui_ranges.clear();
        let mut cur_clip: Option<[f32; 4]> = None;
        let mut cur_start: u32 = 0;
        let quad = |ui: &mut Vec<UiVert>,
                    x: f32,
                    y: f32,
                    w: f32,
                    h: f32,
                    uv: [f32; 4],
                    color: [f32; 4],
                    tex: u32| {
            let c = pack_color(color);
            let v = |px: f32, py: f32, u: f32, vv: f32| UiVert {
                pos: [px, py],
                uv: [u, vv],
                color: c,
                tex,
            };
            ui.extend_from_slice(&[
                v(x, y, uv[0], uv[1]),
                v(x + w, y, uv[2], uv[1]),
                v(x + w, y + h, uv[2], uv[3]),
                v(x, y, uv[0], uv[1]),
                v(x + w, y + h, uv[2], uv[3]),
                v(x, y + h, uv[0], uv[3]),
            ]);
        };
        for d in fr.draw2d {
            match *d {
                Draw2D::Pic {
                    x,
                    y,
                    w,
                    h,
                    pic,
                    uv,
                    color,
                } => {
                    if let Some(p) = self.pics.get(pic.0 as usize) {
                        quad(&mut ui, x, y, w, h, uv, color, p.slot);
                    }
                }
                Draw2D::Char {
                    x,
                    y,
                    size,
                    ch,
                    color,
                } => {
                    if ch == b' ' {
                        continue;
                    }
                    if let Some(p) = self.conchars.and_then(|c| self.pics.get(c.0 as usize)) {
                        let (cx, cy) = ((ch & 15) as f32 / 16.0, (ch >> 4) as f32 / 16.0);
                        let e = 1.0 / 2048.0;
                        let uv = [cx + e, cy + e, cx + 1.0 / 16.0 - e, cy + 1.0 / 16.0 - e];
                        quad(&mut ui, x, y, size, size, uv, color, p.slot);
                    }
                }
                Draw2D::Fill { x, y, w, h, color } => {
                    quad(&mut ui, x, y, w, h, [0.0; 4], color, NO_TEX)
                }
                Draw2D::Tile {
                    x,
                    y,
                    w,
                    h,
                    pic,
                    scale,
                } => {
                    if let Some(p) = self.pics.get(pic.0 as usize) {
                        let (pw, ph) = (
                            p.width as f32 * scale.max(1e-3),
                            p.height as f32 * scale.max(1e-3),
                        );
                        let uv = [x / pw, y / ph, (x + w) / pw, (y + h) / ph];
                        quad(&mut ui, x, y, w, h, uv, [1.0; 4], p.slot);
                    }
                }
                Draw2D::Line { x0, y0, x1, y1, width, color } => {
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    let len = (dx * dx + dy * dy).sqrt();
                    if len >= 0.01 {
                        let hw = width.max(0.5) * 0.5;
                        let (nx, ny) = (-dy / len * hw, dx / len * hw);
                        let c = pack_color(color);
                        let v = |px: f32, py: f32| UiVert { pos: [px, py], uv: [0.0, 0.0], color: c, tex: NO_TEX };
                        ui.extend_from_slice(&[
                            v(x0 - nx, y0 - ny),
                            v(x0 + nx, y0 + ny),
                            v(x1 + nx, y1 + ny),
                            v(x0 - nx, y0 - ny),
                            v(x1 + nx, y1 + ny),
                            v(x1 - nx, y1 - ny),
                        ]);
                    }
                }
                Draw2D::Clip(rect) => {
                    let end = ui.len() as u32;
                    if end > cur_start {
                        ui_ranges.push((cur_clip, cur_start, end - cur_start));
                        cur_start = end;
                    }
                    cur_clip = rect;
                }
            }
        }
        if (ui.len() as u32) > cur_start {
            ui_ranges.push((cur_clip, cur_start, ui.len() as u32 - cur_start));
        }
        let ui_ofs = if ui.is_empty() {
            None
        } else {
            dynw.push(&ui, 16)
        };

        // r_shadow_realtime_dlight: per-pixel lights and their shadow casters
        let rt_sel = {
            let shadows = if st.dlight_shadows { self.shadow_maps.as_ref().map_or(0, |s| s.lights) } else { 0 };
            dlights::select(fr.dlights, &passes, st, shadows)
        };
        let mut shadow_lights = self.build_shadow_lights(&rt_sel, &mut dynw, time, alphas);

        // UBO per view (view 0 at offset 0) + lightmap dynamic data
        for (vi, pass) in passes.iter_mut().enumerate() {
            let Some(vs) = &pass.view else { continue };
            let sky = self.skybox;
            let soft = crate::softemu::scene_params(st);
            let mut ubo = FrameUbo {
                portal_clip: [0.0; 4],
                portal_plane: [[0.0; 4]; 4],
                portal_min: [[0.0; 4]; 4],
                portal_max: [[0.0; 4]; 4],
                viewproj: vs.vp,
                eye: [vs.eye[0], vs.eye[1], vs.eye[2], time as f32],
                fog: [
                    fog.color[0],
                    fog.color[1],
                    fog.color[2],
                    fog.density.max(0.0) / 64.0,
                ],
                vright: [vs.right[0], vs.right[1], vs.right[2], 0.0],
                vup: [vs.up[0], vs.up[1], vs.up[2], 0.0],
                vfwd: [vs.fwd[0], vs.fwd[1], vs.fwd[2], 0.0],
                sky: [
                    st.skyfog,
                    if sky.is_some() { 1.0 } else { 0.0 },
                    st.skyalpha,
                    st.particle_style as f32,
                ],
                skybox0: sky.map_or([NO_TEX; 4], |s| [s[0], s[1], s[2], s[3]]),
                skybox1: [
                    sky.map_or(NO_TEX, |s| s[4]),
                    sky.map_or(NO_TEX, |s| s[5]),
                    self.sky_layers[0],
                    self.sky_layers[1],
                ],
                alphas,
                soft: [soft.0, soft.1, st.goggles.intensity.clamp(0.25, 3.0), st.goggles.noise.clamp(0.0, 2.0)],
                softf: [soft.2 | (u32::from(st.fullbright) << 4) | st.mapper.lightmap_flag(), soft.3, st.goggles.mode.min(2), self.caustic_tex],
                water: [
                    st.water_ripples.clamp(0.0, 4.0),
                    st.water_caustics.clamp(0.0, 4.0),
                    if st.water_ripples > 0.0 || st.water_caustics > 0.0 { self.water_fx.ripples.len() as f32 } else { 0.0 },
                    st.water_quality.min(1) as f32,
                ],
                portal_uv: if let Some(t) = &self.targets {
                    let [pw, ph] = portals::portal_viewport(t, [pass.scene_w, pass.scene_h]);
                    let (ptw, pth) = (t.portal_w.max(1) as f32, t.portal_h.max(1) as f32);
                    [
                        pw as f32 / (pass.scene_w.max(1) as f32 * ptw),
                        ph as f32 / (pass.scene_h.max(1) as f32 * pth),
                        (pw as f32 - 0.5).max(0.0) / ptw,
                        (ph as f32 - 0.5).max(0.0) / pth,
                    ]
                } else {
                    [0.0; 4]
                },
                vel_uv: if let Some(t) = &self.targets {
                    let [vw, vh] = vel_viewport(t, [pass.scene_w, pass.scene_h]);
                    [
                        pass.scene_w.max(1) as f32 / vw as f32,
                        pass.scene_h.max(1) as f32 / vh as f32,
                        (pass.scene_w.max(1) - 1) as f32,
                        (pass.scene_h.max(1) - 1) as f32,
                    ]
                } else {
                    [0.0; 4]
                },
                wind: if sky.is_some() { st.sky_wind } else { [0.0; 4] },
                ripples: {
                    let mut r = [[0.0; 4]; crate::water::MAX_RIPPLES];
                    for (d, w) in r.iter_mut().zip(&self.water_fx.ripples) {
                        *d = [w.pos[0], w.pos[1], w.pos[2], w.t0];
                    }
                    r
                },
                ripple_amp: {
                    let mut a = [[0.0; 4]; crate::water::MAX_RIPPLES / 4];
                    for (i, w) in self.water_fx.ripples.iter().enumerate() {
                        a[i / 4][i % 4] = w.amp;
                    }
                    a
                },
                refl: {
                    let amount = if st.water_reflections > 0 { st.water_reflection_amount.clamp(0.0, 1.5) } else { 0.0 };
                    let opaque_bits = rt_build.map_or(0, |b| b.opaque_tris);
                    let (slot_bits, scale_bits) = if rt_half_on
                        && let Some(rt_t) = self.targets.as_ref().and_then(|t| t.rt_refl.as_ref())
                    {
                        let [rw, rh] = rt::rt_refl_viewport(rt_t, [pass.scene_w, pass.scene_h], [sw, sh]);
                        (
                            rt_t.slot,
                            rt::pack_unorm2x16(rw as f32 / rt_t.width.max(1) as f32, rh as f32 / rt_t.height.max(1) as f32),
                        )
                    } else {
                        (0, 0)
                    };
                    [amount, f32::from_bits(opaque_bits), f32::from_bits(slot_bits), f32::from_bits(scale_bits)]
                },
                teleport_fx: [ripple_count as u32, 0, 0, 0],
                teleport_ripples,
                hdr_deluxe: match &self.world {
                    Some(wg) => [
                        if wg.cpu.has_deluxe && st.deluxemapping { wg.cpu.atlas_layers as f32 } else { 0.0 },
                        if wg.cpu.has_hdr && st.hdr_lightmaps { 1.0 } else { 0.0 },
                        if wg.cpu.has_deluxe && st.deluxemapping { st.deluxemapping_specular.clamp(0.0, 2.0) } else { 0.0 },
                        if wg.cpu.has_deluxe && st.deluxemapping { 0.35 } else { 0.0 },
                    ],
                    None => [0.0; 4],
                },
            };
            for (i, portal) in pass.portal_draws.iter_mut().enumerate().filter(|(i, _)| *i < 4) {
                ubo.portal_plane[i] = portal.portal.plane();
                // only teleporter sheets sample their view in water.frag
                let slot = if portal.portal.gun.is_some() { 0.0 } else { self.targets.as_ref().unwrap().portals[i].1 as f32 };
                ubo.portal_min[i] = [portal.portal.mins[0], portal.portal.mins[1], portal.portal.mins[2], slot];
                // w: 1 teleporter sheet / gun portal, 2 reflection (water.frag)
                let kind = if portal.portal.reflect { 2.0 } else { 1.0 };
                ubo.portal_max[i] = [portal.portal.maxs[0], portal.portal.maxs[1], portal.portal.maxs[2], kind];
            }
            let portal_uv_self = self.targets.as_ref().map_or([0.0; 4], |t| {
                let [pw, ph] = portals::portal_viewport(t, [pass.scene_w, pass.scene_h]);
                let (ptw, pth) = (t.portal_w.max(1) as f32, t.portal_h.max(1) as f32);
                [
                    1.0 / ptw,
                    1.0 / pth,
                    (pw as f32 - 0.5).max(0.0) / ptw,
                    (ph as f32 - 0.5).max(0.0) / pth,
                ]
            });
            for portal in &pass.portal_draws {
                let mut pu = ubo;
                pu.portal_min = [[0.0; 4]; 4];
                pu.portal_clip = portal.portal.clip();
                pu.portal_uv = portal_uv_self;
                if portals::skip_portal_extras(st.portal_quality.min(2), portal.depth) {
                    pu.water = [0.0; 4];
                }
                pu.viewproj = mul(&vs.vp, &portal.portal.to_source());
                let eye = portal.portal.to_destination(vs.eye);
                pu.eye[..3].copy_from_slice(&eye);
                let rotate = |v: [f32; 3]| { let q = portal.portal.rotate(v); [q[0], q[1], q[2], 0.0] };
                pu.vright = rotate(vs.right);
                pu.vup = rotate(vs.up);
                pu.vfwd = rotate(vs.fwd);
                dynw.write_at(portal.ubo as u64, &[pu]);
            }
            if vi == 0 {
                // shadow map faces: the first view's uniforms with the face's matrix
                for sl in shadow_lights.iter_mut() {
                    for (f, ofs) in sl.ubo.iter_mut().enumerate() {
                        let mut fu = ubo;
                        fu.viewproj = sl.vp[f];
                        *ofs = dynw.push(&[fu], 256).unwrap_or(0) as u32;
                    }
                }
                dynw.write_at(0, &[ubo]);
            } else {
                pass.ubo = dynw.push(&[ubo], 256).unwrap_or(0) as u32;
            }
        }
        let shadow_size = self.shadow_maps.as_ref().map_or(16, |s| s.size);
        dynw.write_at(OFS_LIGHTS, &[crate::rtlight::lights_gpu(&rt_sel, st.shadow_quality, shadow_size)]);
        let mut lm_cpu_range = None;
        if passes.iter().any(|p| p.view.is_some()) {
            let deluxe_ofs = self
                .world
                .as_ref()
                .filter(|w| w.cpu.has_deluxe)
                .map_or(0, |w| w.cpu.atlas_layers);
            let mut lmd = LmDyn {
                styles,
                num_dlights: 0,
                pad: [deluxe_ofs, u32::from(st.hdr_lightmaps), 0],
                dl: [[0.0; 8]; 64],
            };
            // real-time dynamic lights are lit per pixel, not in the lightmaps
            if st.dynamic && !st.rt_dlights {
                for (i, d) in fr.dlights.iter().take(64).enumerate() {
                    lmd.dl[i] = [
                        d.origin[0],
                        d.origin[1],
                        d.origin[2],
                        d.radius,
                        d.color[0],
                        d.color[1],
                        d.color[2],
                        d.minlight,
                    ];
                    lmd.num_dlights = i as u32 + 1;
                }
            }
            dynw.write_at(OFS_LMDYN, &[lmd]);
            // Portal views can request more lightmaps than the fixed GPU
            // cull list holds. Keep their complete union in the bump buffer;
            // the reserved list belongs only to the GPU-culled world.
            let lm_vis = lm_vis.sorted();
            if !lm_vis.is_empty() {
                let ofs = dynw.push(lm_vis, 4).ok_or_else(|| Error::new("lightmap update buffer full"))?;
                lm_cpu_range = Some((ofs / 4, lm_vis.len()));
            }
        }

        self.lightmap_requests = lm_vis;
        self.prev_xforms = xforms;
        self.cur_xforms = prev_xforms;
        self.cur_xforms.clear();
        self.prev_xform_time = time;

        // ---- record ----
        let tsq = self.gpu_times.as_ref().map(|t| t.pool);
        let ts_stride = self.gpu_times.as_ref().map_or(0, |t| t.stride());
        if let Some(t) = &mut self.gpu_times {
            t.written[fi] = passes.len();
            t.destination_counts[fi].clear();
            t.destination_counts[fi].extend(passes.iter().map(|p| p.portal_draws.len()));
        }
        let ts_base = fi as u32 * ts_stride;
        // SAFETY: recording into `cmd`; query index within this frame's pool region.
        let timestamp = |k: u32| unsafe {
            if let Some(q) = tsq {
                dev.cmd_write_timestamp(cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE, q, ts_base + k);
            }
        };
        let has_output = present || self.targets.as_ref().is_some_and(|t| t.out.is_some());
        if self.targets.is_none() || self.lightmap.is_none() || !has_output {
            return Ok(());
        }
        let pic_uploads = std::mem::take(&mut self.pending_pic_uploads);
        let f = &self.frames[fi];
        let dynbuf = f.dynbuf.buf;
        let set = [f.set];
        let comp_set = [f.comp_set];
        let p = &self.pipes;
        let Some(t) = &self.targets else {
            return Ok(());
        };
        let Some(lightmap) = &self.lightmap else {
            return Ok(());
        };
        // SAFETY: all recorded handles are alive until this frame's fence signals;
        // the command buffer is not in use (fence waited).
        unsafe {
            dev.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            dev.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            if let Some(q) = tsq {
                dev.cmd_reset_query_pool(cmd, q, ts_base, ts_stride);
            }
            timestamp(0);
            for up in &pic_uploads {
                self.gpu.record_image_rows_upload(
                    cmd,
                    up.image,
                    up.width,
                    up.height,
                    up.rows,
                    up.staging.as_ref().map(|b| b.buf),
                );
            }
            if let Some(b) = rt_build {
                self.record_rt(cmd, fi, b);
            }
            // post: gamma/contrast/polyblend to the output
            let (out_img, out_view) = match (self.swap.as_ref().filter(|_| present), &t.out) {
                (Some(s), _) => (s.images[img_idx as usize], s.views[img_idx as usize]),
                (None, Some((o, _))) => (o.image, o.view),
                _ => return Ok(()),
            };
            // with a slang preset, post (and UI in scope 1) render to its
            // input at scene size; the chain then draws onto the output
            let slang_src = self.slang.as_ref().and_then(|s| s.src.as_ref()).map(|i| (i.image, i.view, i.width, i.height));
            let ui_in_src = st.shader_scope == 1;
            let (post_img, post_view, pw, ph) = slang_src.unwrap_or((out_img, out_view, w, h));
            let post_area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: pw, height: ph } };
            let vp_full = vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 };
            let npasses = passes.len();
            let gfx = vk::PipelineBindPoint::GRAPHICS;
            let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let push = |data: &[u8]| dev.cmd_push_constants(cmd, p.layout, all, 0, data);
            let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
            let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
            for (pi, pass) in passes.iter_mut().enumerate() {
            let ts = |k: u32| timestamp(pi as u32 * timing::VIEW_TS as u32 + k);
            if pi > 0 {
                // the previous view's draws read the shared targets and cull buffers
                let mb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::MEMORY_WRITE | vk::AccessFlags::MEMORY_READ)
                    .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::PipelineStageFlags::ALL_COMMANDS,
                    vk::DependencyFlags::empty(),
                    &mb,
                    &[],
                    &[],
                );
            }
            let view = pass.view.as_ref();
            let cull = &pass.cull;
            let (scene_w, scene_h, rect, blend) = (pass.scene_w, pass.scene_h, pass.rect, pass.blend);
            let ubo_ofs = pass.ubo;
            let portal_draws = &pass.portal_draws;
            let world_draws = &mut pass.world_draws;
            let alias_draws = &mut pass.alias_draws;
            let sprite_draws = &mut pass.sprite_draws;
            let trans = &mut pass.trans;
            let vel_draws = &mut pass.vel_draws;
            let mut particles = pass.particles;
            let mut fte = pass.fte.take();
            let mut fte_quads = pass.fte_quads.take();
            let ao_on = st.ssao > 0.0 && view.is_some();
            let bloom_on = t.bloom.is_some() && view.is_some() && st.goggles.mode != 2;
            // the velocity and narrow bloom glow passes read scene depth after the scene pass
            // (SSAO runs inside the scene, between opaque and translucent draws)
            let keep_depth = !vel_draws.is_empty() || bloom_on;
            let lm_rng = gpu::range(vk::ImageAspectFlags::COLOR, 1, lightmap.layers);
            if let Some(c) = &cull {
                let cs = vk::PipelineBindPoint::COMPUTE;
                dev.cmd_copy_buffer(
                    cmd,
                    dynbuf,
                    c.cmdbuf,
                    &[vk::BufferCopy { src_offset: c.stage_ofs, dst_offset: 0, size: c.stage_len }],
                );
                let tb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &tb,
                    &[],
                    &[],
                );
                dev.cmd_bind_pipeline(cmd, cs, p.cull);
                dev.cmd_bind_descriptor_sets(cmd, cs, p.cull_layout, 0, &[f.cull_set], &[]);
                dev.cmd_push_constants(cmd, p.cull_layout, vk::ShaderStageFlags::COMPUTE, 0, bytes_of(&[c.push]));
                dev.cmd_dispatch(cmd, c.groups, 1, 1);
                let mb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(
                        vk::AccessFlags::INDIRECT_COMMAND_READ
                            | vk::AccessFlags::INDEX_READ
                            | vk::AccessFlags::SHADER_READ,
                    )];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::DRAW_INDIRECT
                        | vk::PipelineStageFlags::VERTEX_INPUT
                        | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &mb,
                    &[],
                    &[],
                );
            }
            ts(1);
            let lm_cpu = pi == 0 && lm_cpu_range.is_some();
            if (lm_cpu || cull.is_some()) && self.world.is_some() {
                gpu::barrier(
                    &dev,
                    cmd,
                    lightmap.image,
                    lm_rng,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                );
                dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, p.lightmap);
                dev.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::COMPUTE,
                    p.comp_layout,
                    0,
                    &comp_set,
                    &[],
                );
                if lm_cpu {
                    let (base, count) = lm_cpu_range.unwrap();
                    // Vulkan guarantees at least 65535 workgroups per axis.
                    for first in (0..count).step_by(65535) {
                        let n = (count - first).min(65535) as u32;
                        dev.cmd_push_constants(cmd, p.comp_layout, vk::ShaderStageFlags::COMPUTE,
                            0, bytes_of(&[(base + first as u64) as u32, n]));
                        dev.cmd_dispatch(cmd, n, 1, 1);
                    }
                }
                if let Some(c) = &cull {
                    if lm_cpu {
                        // Portal and world lists can contain the same surface.
                        gpu::barrier(&dev, cmd, lightmap.image, lm_rng,
                            vk::ImageLayout::GENERAL, vk::ImageLayout::GENERAL);
                    }
                    dev.cmd_push_constants(cmd, p.comp_layout, vk::ShaderStageFlags::COMPUTE,
                        0, bytes_of(&[(OFS_VIS / 4) as u32, MAX_VIS as u32]));
                    dev.cmd_dispatch_indirect(cmd, c.cmdbuf, c.disp_ofs);
                }
                gpu::barrier(
                    &dev,
                    cmd,
                    lightmap.image,
                    lm_rng,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                );
            }
            if pi == 0 {
                self.record_shadows(cmd, set[0], dynbuf, &shadow_lights, world_draws, alias_draws);
            }
            ts(2);
            let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
            let drng = gpu::range(vk::ImageAspectFlags::DEPTH, 1, 1);
            gpu::barrier(
                &dev,
                cmd,
                t.color.image,
                crng,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            gpu::barrier(
                &dev,
                cmd,
                t.depth.image,
                drng,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            );
            if let Some(ms) = &t.color_ms {
                gpu::barrier(
                    &dev,
                    cmd,
                    ms.image,
                    crng,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                );
            }
            let area = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: w,
                    height: h,
                },
            };
            let area_target = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: sw,
                    height: sh,
                },
            };
            let _ = h;
            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            };
            // Soft particles sample scene depth. When SSAO already extracted
            // linear depth into `ao_source`, sample that directly so we avoid
            // splitting the MSAA render pass for soft particles and the viewmodel.
            let soft_dist = st.soft_particle_distance;
            let has_soft = fte.as_ref().is_some_and(|(_, _, b)| b.iter().any(|(_, _, b, _)| b.soft && !b.lines))
                || fte_quads.as_ref().is_some_and(|(_, b)| b.iter().any(|(_, _, b, _)| b.soft));
            let soft_active = self.isolated_portal.is_none() && soft_dist > 0.0 && view.is_some() && has_soft;
            let use_ao_depth = ao_on && t.ao_source.is_some();
            let split = soft_active && !use_ao_depth;
            // load: keep earlier contents; last: the final scene pass (MSAA
            // resolves only there); ro: depth attachment read-only.
            let begin = |load: bool, last: bool, ro: bool| {
                let lop = if load { vk::AttachmentLoadOp::LOAD } else { vk::AttachmentLoadOp::CLEAR };
                let att = |ms: Option<&Image>, single: &Image| match ms {
                    Some(ms) if last => vk::RenderingAttachmentInfo::default()
                        .image_view(ms.view)
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .resolve_mode(vk::ResolveModeFlags::AVERAGE)
                        .resolve_image_view(single.view)
                        .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(lop)
                        .store_op(vk::AttachmentStoreOp::DONT_CARE)
                        .clear_value(clear),
                    Some(ms) => vk::RenderingAttachmentInfo::default()
                        .image_view(ms.view)
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(lop)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(clear),
                    None => vk::RenderingAttachmentInfo::default()
                        .image_view(single.view)
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(lop)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(clear),
                };
                let catt = [att(t.color_ms.as_ref(), &t.color)];
                let datt = vk::RenderingAttachmentInfo::default()
                    .image_view(t.depth.view)
                    .image_layout(if ro {
                        vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                    } else {
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                    })
                    .load_op(lop)
                    .store_op(if last && !keep_depth { vk::AttachmentStoreOp::DONT_CARE } else { vk::AttachmentStoreOp::STORE })
                    .clear_value(vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: 1.0,
                            stencil: 0,
                        },
                    });
                let ri = vk::RenderingInfo::default()
                    .render_area(area_target)
                    .layer_count(1)
                    .color_attachments(&catt)
                    .depth_attachment(&datt);
                dev.cmd_begin_rendering(cmd, &ri);
            };
            let depth_layout = |old: vk::ImageLayout, new: vk::ImageLayout| {
                gpu::barrier(&dev, cmd, t.depth.image, drng, old, new);
            };
            ts(3);
            self.draw_portals(
                cmd,
                portal_draws,
                particles,
                fte.as_ref().map(|f| (f.0, f.1)),
                fte_quads.as_ref().map(|q| q.0),
                world_draws,
                alias_draws,
                fi,
                [scene_w, scene_h],
                pi,
                view,
                st.portal_quality.min(2),
            );
            if rt_half_on && view.is_some() {
                self.record_rt_refl(cmd, fi, ubo_ofs, [scene_w, scene_h], [sw, sh], world_draws);
            }
            ts(4);
            if self.isolated_portal.is_some() && !multi {
                // Destination draws above retain their scenery and entities. Only
                // the source sheet survives in the main diagnostic view below.
                world_draws.truncate(1);
                alias_draws.clear();
                sprite_draws.clear();
                trans.clear();
                vel_draws.clear();
                particles = Range::default();
                fte = None;
                fte_quads = None;
            }
            begin(false, !split && !ao_on, false);
            let vp_full = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let vp_scene = vk::Viewport {
                width: scene_w as f32,
                height: scene_h as f32,
                ..vp_full
            };
            let area_scene = vk::Rect2D {
                extent: vk::Extent2D {
                    width: scene_w,
                    height: scene_h,
                },
                ..area
            };
            dev.cmd_set_viewport(cmd, 0, &[vp_scene]);
            dev.cmd_set_scissor(cmd, 0, &[area_scene]);
            let gfx = vk::PipelineBindPoint::GRAPHICS;
            let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let push = |data: &[u8]| dev.cmd_push_constants(cmd, p.layout, all, 0, data);
            let samples = t.color_ms.as_ref().map_or(1, |_| self.msaa);
            // fullscreen pass into a single-sampled effect target
            let fx_pass = |img: &Image, area_rt: vk::Rect2D, vp: vk::Viewport, scissor: vk::Rect2D, clear: [f32; 4], body: &dyn Fn()| {
                gpu::barrier(&dev, cmd, img.image, crng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                let att = [vk::RenderingAttachmentInfo::default()
                    .image_view(img.view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: clear } })];
                let ri = vk::RenderingInfo::default().render_area(area_rt).layer_count(1).color_attachments(&att);
                dev.cmd_begin_rendering(cmd, &ri);
                dev.cmd_set_viewport(cmd, 0, &[vp]);
                dev.cmd_set_scissor(cmd, 0, &[scissor]);
                dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[ubo_ofs]);
                body();
                dev.cmd_end_rendering(cmd);
                gpu::barrier(
                    &dev,
                    cmd,
                    img.image,
                    crng,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            };
            if view.is_some() {
                dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[ubo_ofs]);
                dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                let draw_world = |wd: &WorldDraw, cats: &[(Range, vk::Pipeline)]| {
                    dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                    push(bytes_of(&[WorldPush {
                        model: wd.model,
                        params: [wd.alpha, 0.0, 0.0, 0.0],
                    }]));
                    // vkQuake gl_zfix values for a D32F depth buffer
                    if wd.zfix {
                        dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                    }
                    for (r, pipe) in cats {
                        if r.count > 0 {
                            dev.cmd_bind_pipeline(cmd, gfx, *pipe);
                            dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                        }
                    }
                    if wd.zfix {
                        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                    }
                };
                if let Some(wg) = &self.world {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                    for wd in world_draws.iter().filter(|wd| wd.main_visible && wd.alpha >= 1.0) {
                        draw_world(
                            wd,
                            &[
                                (wd.opaque, p.world),
                                (wd.sky, p.sky),
                                (wd.water[0], water_pipe),
                                (wd.water[1], water_pipe),
                                (wd.water[2], water_pipe),
                                (wd.water[3], water_pipe),
                            ],
                        );
                    }
                }
                ts(5);
                let draw_alias = |a: &AliasDraw, pipe: vk::Pipeline| {
                    let Some(Model::Alias(am)) = self.models.get(a.model).filter(|_| !a.portal_only && a.main_visible) else {
                        return;
                    };
                    dev.cmd_bind_pipeline(cmd, gfx, pipe);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                    push(bytes_of(&[a.push]));
                    dev.cmd_draw_indexed(cmd, a.num_indices, 1, a.first_index, 0, 0);
                };
                for a in alias_draws
                    .iter()
                    .filter(|a| a.push.light[3] >= 1.0 && !a.viewmodel)
                {
                    draw_alias(a, p.alias);
                }
                ts(6);
                if ao_on && let Some(vs) = &view && let Some(ao_src) = &t.ao_source {
                    // SSAO between the opaque and translucent draws, so water,
                    // particles and the viewmodel are not darkened
                    dev.cmd_end_rendering(cmd);
                    let scene_img = t.color_ms.as_ref().unwrap_or(&t.color);
                    gpu::barrier(&dev, cmd, scene_img.image, crng, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    depth_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
                    let ao_vw = ((scene_w as f32 * t.ao_w as f32 / sw.max(1) as f32).round() as u32).clamp(1, t.ao_w);
                    let ao_vh = ((scene_h as f32 * t.ao_h as f32 / sh.max(1) as f32).round() as u32).clamp(1, t.ao_h);
                    let area_ao_rt = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: t.ao_w, height: t.ao_h } };
                    let vp_ao = vk::Viewport { width: ao_vw as f32, height: ao_vh as f32, ..vp_full };
                    let area_ao = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: ao_vw, height: ao_vh } };
                    let scene_per_ao = [scene_w as f32 / ao_vw as f32, scene_h as f32 / ao_vh as f32];
                    let scene_max = [scene_w.max(1) as i32 - 1, scene_h.max(1) as i32 - 1];
                    let quality = st.ssao_quality.min(2);
                    // Build SAO linear-depth mip chain: mip 0 from scene depth, then rotated-grid downsamples.
                    let mips = ao_src.mip_views.len() as u32;
                    for m in 0..mips {
                        let mw = (t.ao_w >> m).max(1);
                        let mh = (t.ao_h >> m).max(1);
                        let vw = (ao_vw >> m).max(1);
                        let vh = (ao_vh >> m).max(1);
                        let mrng = gpu::range_mip(vk::ImageAspectFlags::COLOR, m, 1, 1);
                        gpu::barrier(&dev, cmd, ao_src.img.image, mrng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                        let att = [vk::RenderingAttachmentInfo::default()
                            .image_view(ao_src.mip_views[m as usize])
                            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                            .load_op(vk::AttachmentLoadOp::DONT_CARE)
                            .store_op(vk::AttachmentStoreOp::STORE)];
                        let ri = vk::RenderingInfo::default()
                            .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: mw, height: mh } })
                            .layer_count(1)
                            .color_attachments(&att);
                        dev.cmd_begin_rendering(cmd, &ri);
                        dev.cmd_set_viewport(cmd, 0, &[vk::Viewport { width: vw as f32, height: vh as f32, ..vp_full }]);
                        dev.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: vw, height: vh } }]);
                        dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[ubo_ofs]);
                        if m == 0 {
                            dev.cmd_bind_pipeline(cmd, gfx, p.ao_source);
                            push(bytes_of(&[AoSourcePush { scene_per_ao, scene_max, msaa: samples }]));
                        } else {
                            let pmw = (ao_vw >> (m - 1)).max(1) as i32 - 1;
                            let pmh = (ao_vh >> (m - 1)).max(1) as i32 - 1;
                            dev.cmd_bind_pipeline(cmd, gfx, p.ao_down);
                            push(bytes_of(&[AoDownPush { src: ao_src.slot, src_mip: (m - 1) as i32, src_max: [pmw, pmh] }]));
                        }
                        dev.cmd_draw(cmd, 3, 1, 0, 0);
                        dev.cmd_end_rendering(cmd);
                        gpu::barrier(&dev, cmd, ao_src.img.image, mrng, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    }
                    fx_pass(&t.ao, area_ao_rt, vp_ao, area_ao, [1.0, 0.0, 0.0, 0.0], &|| {
                        ts(7); // transitions, linear-depth mip chain and AO target setup complete
                        dev.cmd_bind_pipeline(cmd, gfx, p.ao);
                        push(bytes_of(&[AoPush {
                            proj: [vs.tan[0], vs.tan[1], samples as f32, st.ssao_radius.clamp(4.0, 256.0)],
                            size: [ao_vw as f32, ao_vh as f32, st.ssao.min(4.0), ao_src.slot as f32],
                            scene_per_ao,
                            scene_max,
                            quality,
                            color_slot: t.color_slot,
                            max_mip: (mips - 1) as i32,
                            _pad: 0,
                        }]));
                        dev.cmd_draw(cmd, 3, 1, 0, 0);
                        ts(8); // AO generation, before its target becomes shader-readable
                    });
                    gpu::barrier(&dev, cmd, scene_img.image, crng, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                    ts(9); // AO/scene transitions complete
                    // multiply the blurred AO into the scene colour (all samples)
                    let att = [vk::RenderingAttachmentInfo::default()
                        .image_view(scene_img.view)
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::LOAD)
                        .store_op(vk::AttachmentStoreOp::STORE)];
                    let ri = vk::RenderingInfo::default().render_area(area_target).layer_count(1).color_attachments(&att);
                    dev.cmd_begin_rendering(cmd, &ri);
                    dev.cmd_set_viewport(cmd, 0, &[vp_scene]);
                    dev.cmd_set_scissor(cmd, 0, &[area_scene]);
                    dev.cmd_bind_pipeline(cmd, gfx, p.ao_apply);
                    push(bytes_of(&[AoApplyPush {
                        ao_tex: t.ao_slot,
                        msaa: samples,
                        quality,
                        _pad: 0,
                        ao_per_scene: [ao_vw as f32 / scene_w.max(1) as f32, ao_vh as f32 / scene_h.max(1) as f32],
                        ao_max: [ao_vw as i32 - 1, ao_vh as i32 - 1],
                    }]));
                    dev.cmd_draw(cmd, 3, 1, 0, 0);
                    dev.cmd_end_rendering(cmd);
                    ts(10); // AO application complete
                    depth_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL);
                    begin(true, !split, false);
                    dev.cmd_set_viewport(cmd, 0, &[vp_scene]);
                    dev.cmd_set_scissor(cmd, 0, &[area_scene]);
                    dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[ubo_ofs]);
                    dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                    ts(11); // depth transition and scene rendering resumed
                } else {
                    // Every query must be written, including when AO is disabled.
                    for k in 7..=11 { ts(k); }
                }
                // translucent: world liquids first, then sorted entities
                if let Some(wg) = &self.world {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                    for wd in world_draws.iter().filter(|wd| wd.main_visible && wd.alpha >= 1.0) {
                        let wa = wd.water_alpha.map(|r| (r, water_alpha_pipe));
                        draw_world(wd, &wa);
                    }
                }
                for (_, tr) in trans.iter() {
                    match *tr {
                        Trans::World(i) => {
                            if let Some(wg) = &self.world {
                                let wd = &world_draws[i];
                                dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                                dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                                draw_world(
                                    wd,
                                    &[
                                        (wd.opaque, p.world_alpha),
                                        (wd.sky, p.sky),
                                        (wd.water[0], water_alpha_pipe),
                                        (wd.water_alpha[0], water_alpha_pipe),
                                    ],
                                );
                            }
                        }
                        Trans::Alias(i) => draw_alias(&alias_draws[i], p.alias_alpha),
                        Trans::Sprite(i) => {
                            let s = &sprite_draws[i];
                            dev.cmd_bind_pipeline(cmd, gfx, p.poly);
                            dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[s.vofs]);
                            push(bytes_of(&[PolyPush {
                                color: [1.0, 1.0, 1.0, s.alpha],
                                tex: s.tex,
                            }]));
                            dev.cmd_draw(cmd, 6, 1, 0, 0);
                        }
                    }
                }
                ts(12);
                let wire = st.mapper.wire;
                if wire != 0 && self.gpu.wireframe {
                    let mode = (wire.min(2) - 1) as usize;
                    if let Some(wg) = &self.world {
                        // SAFETY: buffers and pipelines belong to the active render pass.
                        mapper::draw_world_wire(&dev, cmd, p, wg, dynbuf, world_draws, mode);
                    }
                    for a in alias_draws.iter().filter(|a| !a.viewmodel) {
                        let mut a = AliasDraw { push: a.push, ..*a };
                        a.push.texs[3] |= 2048 | if mode == 1 { 4096 } else { 0 };
                        dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                        draw_alias(&a, p.wire_alias[mode]);
                    }
                    dev.cmd_bind_pipeline(cmd, gfx, p.wire_sprite[mode]);
                    let color = if mode == 0 { [0.75, 0.95, 1.0, 0.9] } else { [1.0, 0.32, 0.12, 0.35] };
                    push(bytes_of(&[PolyPush { color, tex: NO_TEX }]));
                    for sprite in sprite_draws.iter() {
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[sprite.vofs]);
                        dev.cmd_draw(cmd, 6, 1, 0, 0);
                    }
                    dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                }
                if particles.count > 0 {
                    dev.cmd_bind_pipeline(cmd, gfx, p.particle);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[particles.ofs]);
                    dev.cmd_draw(cmd, 6, particles.count, 0, 0);
                }
                if st.goggles.mode == 2 {
                    // thermal goggles: warm bodies seen through walls
                    for a in alias_draws.iter().filter(|a| !a.viewmodel && (a.push.texs[3] >> 8) & 3 == 2) {
                        let mut x = AliasDraw { push: a.push, ..*a };
                        x.push.texs[3] |= 1 << 10;
                        draw_alias(&x, p.alias_xray);
                    }
                }
                let vm: Vec<&AliasDraw> = alias_draws.iter().filter(|a| a.viewmodel).collect();
                if fte.is_some() || fte_quads.is_some() {
                    let restore = |vp| {
                        dev.cmd_set_viewport(cmd, 0, &[vp]);
                        dev.cmd_set_scissor(cmd, 0, &[area_scene]);
                        dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[ubo_ofs]);
                        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                    };
                    if split {
                        dev.cmd_end_rendering(cmd);
                        depth_layout(
                            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                        );
                        begin(true, vm.is_empty(), true);
                        restore(vp_scene);
                    }
                    let samples = t.color_ms.as_ref().map_or(1, |_| self.msaa);
                    let (depth_mode, depth_tex, depth_uv) = if use_ao_depth
                        && let Some(ao_src) = &t.ao_source
                    {
                        let ao_vw = ((scene_w as f32 * t.ao_w as f32 / sw.max(1) as f32).round() as u32).clamp(1, t.ao_w);
                        let ao_vh = ((scene_h as f32 * t.ao_h as f32 / sh.max(1) as f32).round() as u32).clamp(1, t.ao_h);
                        (
                            -1.0f32,
                            ao_src.slot,
                            [
                                ao_vw as f32 / (scene_w.max(1) as f32 * t.ao_w.max(1) as f32),
                                ao_vh as f32 / (scene_h.max(1) as f32 * t.ao_h.max(1) as f32),
                            ],
                        )
                    } else {
                        let ds = if samples <= 1 {
                            0
                        } else if st.soft_particle_quality > 0 {
                            samples
                        } else {
                            1
                        };
                        (ds as f32, 0, [0.0; 2])
                    };
                    if let Some((vofs, iofs, batches)) = &fte {
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[*vofs]);
                        dev.cmd_bind_index_buffer(cmd, dynbuf, *iofs, vk::IndexType::UINT32);
                        let mut last_fte_pipe = usize::MAX;
                        for (pipe, slot, b, mode) in batches {
                            if *pipe != last_fte_pipe {
                                dev.cmd_bind_pipeline(cmd, gfx, p.fte[*pipe]);
                                last_fte_pipe = *pipe;
                            }
                            let fogmix = if matches!(b.blend, crate::FteBlend::Blend | crate::FteBlend::BlendColour) { 1.0 } else { 0.0 };
                            let soft = if soft_active && b.soft && !b.lines { 1.0 / soft_dist } else { 0.0 };
                            push(bytes_of(&[FtePush {
                                fogmix: [fogmix, soft, depth_mode, *mode],
                                tex: *slot,
                                depth_tex,
                                depth_uv,
                            }]));
                            dev.cmd_draw_indexed(cmd, b.num_indices, 1, b.first_index, 0, 0);
                        }
                    }
                    if let Some((qofs, batches)) = &fte_quads {
                        dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[*qofs]);
                        let mut last_q_pipe = usize::MAX;
                        for (pipe, slot, b, mode) in batches {
                            if *pipe != last_q_pipe {
                                dev.cmd_bind_pipeline(cmd, gfx, p.fte_quad[*pipe]);
                                last_q_pipe = *pipe;
                            }
                            let fogmix = if matches!(b.blend, crate::FteBlend::Blend | crate::FteBlend::BlendColour) { 1.0 } else { 0.0 };
                            let soft = if soft_active && b.soft { 1.0 / soft_dist } else { 0.0 };
                            push(bytes_of(&[FtePush {
                                fogmix: [fogmix, soft, depth_mode, *mode],
                                tex: *slot,
                                depth_tex,
                                depth_uv,
                            }]));
                            dev.cmd_draw(cmd, 6, b.num_indices, 0, b.first_index);
                        }
                    }
                    if split && !vm.is_empty() {
                        dev.cmd_end_rendering(cmd);
                        depth_layout(
                            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
                            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                        );
                        begin(true, true, false);
                        restore(vp_scene);
                    }
                }
                if !vm.is_empty() {
                    dev.cmd_set_viewport(
                        cmd,
                        0,
                        &[vk::Viewport {
                            max_depth: 0.3,
                            ..vp_scene
                        }],
                    );
                    for a in vm {
                        draw_alias(
                            a,
                            if a.push.light[3] < 1.0 {
                                p.alias_alpha
                            } else {
                                p.alias
                            },
                        );
                        if wire != 0 && self.gpu.wireframe {
                            let mode = (wire.min(2) - 1) as usize;
                            let mut a = AliasDraw { push: a.push, ..*a };
                            a.push.texs[3] |= 2048 | if mode == 1 { 4096 } else { 0 };
                            dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                            draw_alias(&a, p.wire_alias[mode]);
                            dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                        }
                    }
                }
            } else {
                // Menu/no-view frames have no world, models, AO or translucency.
                for k in 5..=12 { ts(k); }
            }
            dev.cmd_end_rendering(cmd);
            ts(13);
            if keep_depth && view.is_some() {
                // the last scene pass leaves depth read-only only when it was
                // the soft-particle pass (no viewmodel after it)
                let vm_any = alias_draws.iter().any(|a| a.viewmodel);
                let end_layout = if split && !vm_any {
                    vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
                } else {
                    vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL
                };
                depth_layout(end_layout, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
                if !vel_draws.is_empty() {
                    let [vel_vw, vel_vh] = vel_viewport(t, [scene_w, scene_h]);
                    let area_vel_rt = vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: vk::Extent2D { width: t.vel_w, height: t.vel_h },
                    };
                    let vp_vel = vk::Viewport { width: vel_vw as f32, height: vel_vh as f32, ..vp_full };
                    let area_vel = vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: vk::Extent2D { width: vel_vw, height: vel_vh },
                    };
                    fx_pass(&t.vel, area_vel_rt, vp_vel, area_vel, [0.0; 4], &|| {
                        for vd in vel_draws.iter() {
                            match *vd {
                                VelDraw::Alias(i, prev) => {
                                    let a = &alias_draws[i];
                                    let Some(Model::Alias(am)) = self.models.get(a.model) else {
                                        continue;
                                    };
                                    dev.cmd_bind_pipeline(cmd, gfx, p.vel_alias);
                                    dev.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                                    dev.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                                    let pp = a.push.poses;
                                    push(bytes_of(&[VelPush {
                                        model: a.push.model,
                                        prev,
                                        misc: [
                                            pp[0],
                                            pp[1],
                                            a.push.shadevec[3].to_bits(),
                                            samples | (u32::from(am.md3) << 16) | (u32::from(am.iqm) << 17),
                                        ],
                                    }]));
                                    dev.cmd_draw_indexed(cmd, a.num_indices, 1, a.first_index, 0, 0);
                                }
                                VelDraw::World(i, prev) => {
                                    let Some(wg) = &self.world else { continue };
                                    let wd = &world_draws[i];
                                    dev.cmd_bind_pipeline(cmd, gfx, p.vel_world);
                                    dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                                    dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                                    push(bytes_of(&[VelPush { model: wd.model, prev, misc: [0, 0, 0, samples] }]));
                                    for r in [wd.opaque, wd.water[0], wd.water[1], wd.water[2], wd.water[3]] {
                                        if r.count > 0 {
                                            dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
            }
            gpu::barrier(
                &dev,
                cmd,
                t.color.image,
                crng,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            let bloom = match (&t.bloom, bloom_on) {
                (Some(b), true) => {
                    let gs = bloom::GlowScene {
                        world: self.world.as_ref(),
                        models: &self.models,
                        dynbuf,
                        world_draws,
                        alias_draws,
                        sprite_draws,
                        trans,
                        particles,
                        fte: fte.as_ref(),
                        fte_quads: fte_quads.as_ref(),
                        soft_enabled: soft_active,
                        samples,
                    };
                    let r = bloom::record(&dev, cmd, p, set[0], ubo_ofs, b, [scene_w, scene_h], [sw, sh], st, &gs);
                    dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[0]);
                    Some(r)
                }
                _ => None,
            };
            let water_packed = pass.water.packed(st.water_quality > 0);
            let drops_age = pass.water.drops_age();
            let mut post_water = water_packed;
            if drops_age >= 0.0 && ((water_packed >> 2) & 3) != 0 && let Some((dimg, dslot)) = &t.droplets {
                let uvs = [rect[2] as f32 / w.max(1) as f32, rect[3] as f32 / h.max(1) as f32];
                let view_h = (h.max(1) as f32 * uvs[1]).max(1.0);
                let aspect = (w.max(1) as f32 * uvs[0]) / view_h;
                let area_drop = vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D { width: dimg.width, height: dimg.height },
                };
                let vp_drop = vk::Viewport { width: dimg.width as f32, height: dimg.height as f32, ..vp_full };
                fx_pass(dimg, area_drop, vp_drop, area_drop, [0.0; 4], &|| {
                    dev.cmd_bind_pipeline(cmd, gfx, p.droplets);
                    push(bytes_of(&[DropletsPush {
                        age: drops_age,
                        seed: water_packed >> 8,
                        high: u32::from(st.water_quality > 0),
                        aspect,
                        px: 1.0 / view_h,
                    }]));
                    dev.cmd_draw(cmd, 3, 1, 0, 0);
                });
                // Pass droplet field texture slot in bits 8..31 (seed is no longer needed in post.frag).
                post_water = (water_packed & 0xff) | (*dslot << 8);
            }
            let (fx, fy) = (pw as f32 / w.max(1) as f32, ph as f32 / h.max(1) as f32);
            let vp_cell = vk::Viewport {
                x: rect[0] as f32 * fx,
                y: rect[1] as f32 * fy,
                width: rect[2] as f32 * fx,
                height: rect[3] as f32 * fy,
                ..vp_full
            };
            let cell_area = vk::Rect2D {
                offset: vk::Offset2D { x: vp_cell.x as i32, y: vp_cell.y as i32 },
                extent: vk::Extent2D { width: vp_cell.width as u32, height: vp_cell.height as u32 },
            };
            gpu::barrier(
                &dev,
                cmd,
                post_img,
                crng,
                if pi == 0 { vk::ImageLayout::UNDEFINED } else { vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL },
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            // split cells may leave parts of the output uncovered: clear it
            let post_load = match (pi, multi) {
                (0, false) => vk::AttachmentLoadOp::DONT_CARE,
                (0, true) => vk::AttachmentLoadOp::CLEAR,
                _ => vk::AttachmentLoadOp::LOAD,
            };
            let oatt = [vk::RenderingAttachmentInfo::default()
                .image_view(post_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(post_load)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear)];
            let ri = vk::RenderingInfo::default()
                .render_area(post_area)
                .layer_count(1)
                .color_attachments(&oatt);
            dev.cmd_begin_rendering(cmd, &ri);
            dev.cmd_set_viewport(cmd, 0, &[vp_cell]);
            dev.cmd_set_scissor(cmd, 0, &[cell_area]);
            dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[0]);
            dev.cmd_bind_pipeline(cmd, gfx, p.post);
            let mb = if vel_draws.is_empty() { 0.0 } else { st.motion_blur.min(4.0) / (60.0 * mb_dt as f32) };
            push(bytes_of(&[PostPush {
                blend,
                gamma: st.gamma.clamp(0.1, 4.0),
                contrast: st.contrast.clamp(0.1, 4.0),
                scene: t.color_slot,
                dither: crate::softemu::post_dither(st),
                mode: crate::softemu::post_mode(st),
                scale: if let Some((_, _, pw, _)) = slang_src {
                    sw.max(1) as f32 / pw.max(1) as f32
                } else {
                    self.render_scale
                },
                colorblind: st.colorblind.min(3),
                vel_tex: t.vel_slot,
                // blur length: r_motionblur × the motion over 1/60 s
                mbx: mb * scene_w as f32 / sw as f32,
                mby: mb * scene_h as f32 / sh as f32,
                uvs: [rect[2] as f32 / w.max(1) as f32, rect[3] as f32 / h.max(1) as f32],
                goggles: st.goggles.mode.min(2),
                g_gain: st.goggles.intensity.clamp(0.25, 3.0),
                g_noise: st.goggles.noise.clamp(0.0, 2.0),
                g_time: (fr.time % 1000.0) as f32,
                speed_blur: pass.speed_blur.max(st.edge_blur.clamp(0.0, 1.0)),
                mb_quality: st.motion_blur_quality.min(1),
                mb_w: w.max(1) as f32,
                mb_h: h.max(1) as f32,
                g_quality: st.goggles.quality.min(2),
                warp: pass.warp,
                bloom_tex: bloom.as_ref().map_or(0, |b| b.slot),
                bloom: bloom.as_ref().map_or(0.0, |b| b.amount),
                bloom_uv: bloom.as_ref().map_or([0.0; 2], |b| b.uv),
                water: post_water,
                drops: drops_age,
            }]));
            dev.cmd_draw(cmd, 3, 1, 0, 0);
            if pi + 1 < npasses {
                dev.cmd_end_rendering(cmd);
            }
            ts(14);
            }
            let post_area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: pw, height: ph } };
            let vp_post = vk::Viewport { width: pw as f32, height: ph as f32, ..vp_full };
            dev.cmd_set_scissor(cmd, 0, &[post_area]);
            let draw_ui = |vp: vk::Viewport, full_area: vk::Rect2D| {
                if let Some(ofs) = ui_ofs {
                    dev.cmd_set_viewport(cmd, 0, &[vp]);
                    dev.cmd_bind_pipeline(cmd, gfx, p.ui);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[ofs]);
                    push(bytes_of(&[[1.0 / w as f32, 1.0 / h as f32]]));
                    let sx = vp.width / w.max(1) as f32;
                    let sy = vp.height / h.max(1) as f32;
                    let max_w = full_area.extent.width as i32;
                    let max_h = full_area.extent.height as i32;
                    for &(clip, first, count) in &ui_ranges {
                        let sc = match clip {
                            Some([cx, cy, cw, ch]) => {
                                let x0 = (cx * sx).floor() as i32;
                                let y0 = (cy * sy).floor() as i32;
                                let x1 = ((cx + cw) * sx).ceil() as i32;
                                let y1 = ((cy + ch) * sy).ceil() as i32;
                                let x0 = x0.clamp(0, max_w);
                                let y0 = y0.clamp(0, max_h);
                                let x1 = x1.clamp(x0, max_w);
                                let y1 = y1.clamp(y0, max_h);
                                vk::Rect2D {
                                    offset: vk::Offset2D { x: x0, y: y0 },
                                    extent: vk::Extent2D { width: (x1 - x0) as u32, height: (y1 - y0) as u32 },
                                }
                            }
                            None => full_area,
                        };
                        dev.cmd_set_scissor(cmd, 0, &[sc]);
                        dev.cmd_draw(cmd, count, 1, first, 0);
                    }
                    dev.cmd_set_scissor(cmd, 0, &[full_area]);
                }
            };
            // 2D on top at output resolution, unaffected by render scale
            if slang_src.is_none() || ui_in_src {
                draw_ui(vp_post, post_area);
            }
            if let Some((src_img, ..)) = slang_src {
                dev.cmd_end_rendering(cmd);
                gpu::barrier(
                    &dev,
                    cmd,
                    src_img,
                    crng,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
                gpu::barrier(
                    &dev,
                    cmd,
                    out_img,
                    crng,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                );
                let out_fmt = p.post_format;
                if let Some(sl) = self.slang.as_mut()
                    && let Err(e) = sl.record(cmd, out_img, out_fmt, (w, h), self.frame_count)
                {
                    eprintln!("{e}");
                }
                // the chain binds its own state; restore ours for the UI
                let oatt = [vk::RenderingAttachmentInfo::default()
                    .image_view(out_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::LOAD)
                    .store_op(vk::AttachmentStoreOp::STORE)];
                let ri = vk::RenderingInfo::default()
                    .render_area(area)
                    .layer_count(1)
                    .color_attachments(&oatt);
                dev.cmd_begin_rendering(cmd, &ri);
                dev.cmd_set_scissor(cmd, 0, &[area]);
                dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[0]);
                if !ui_in_src {
                    draw_ui(vp_full, area);
                }
            }
            dev.cmd_end_rendering(cmd);
            if let Some(sw) = self.swap.as_ref().filter(|_| present) {
                let mut last = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                if std::mem::take(&mut self.capture_requested) && sw.readable {
                    if let Some((b, _, _)) = self.capture.take() {
                        self.gpu.free_buffer(b);
                    }
                    let buf = self.gpu.buffer(
                        (w * h * 4) as u64,
                        vk::BufferUsageFlags::TRANSFER_DST,
                        true,
                    )?;
                    gpu::barrier(
                        &dev,
                        cmd,
                        out_img,
                        crng,
                        last,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    );
                    last = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                    let region = [vk::BufferImageCopy {
                        image_subresource: vk::ImageSubresourceLayers {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            mip_level: 0,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        image_extent: vk::Extent3D {
                            width: w,
                            height: h,
                            depth: 1,
                        },
                        ..Default::default()
                    }];
                    dev.cmd_copy_image_to_buffer(cmd, out_img, last, buf.buf, &region);
                    self.capture = Some((buf, w, h));
                }
                gpu::barrier(
                    &dev,
                    cmd,
                    out_img,
                    crng,
                    last,
                    vk::ImageLayout::PRESENT_SRC_KHR,
                );
            } else if let Some((_, buf)) = &t.out {
                gpu::barrier(
                    &dev,
                    cmd,
                    out_img,
                    crng,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                );
                let region = [vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_extent: vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    },
                    ..Default::default()
                }];
                // The readback buffer is shared by frames in flight: order
                // this copy after the previous frame's copy and host read.
                let rb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &rb,
                    &[],
                    &[],
                );
                dev.cmd_copy_image_to_buffer(
                    cmd,
                    out_img,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    buf.buf,
                    &region,
                );
            }
            timestamp(npasses as u32 * timing::VIEW_TS as u32 + 1);
            dev.end_command_buffer(cmd)?;

            let cmds = [cmd];
            let wait_stage = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let wait = [acquire];
            if present && let (Some(s), Some(scf)) = (&self.swap, &self.gpu.swapchain_fn) {
                let sig = [s.render_sems[img_idx as usize]];
                let si = [vk::SubmitInfo::default()
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&wait_stage)
                    .command_buffers(&cmds)
                    .signal_semaphores(&sig)];
                dev.queue_submit(self.gpu.queue, &si, fence)?;
                let scs = [s.sc];
                let idx = [img_idx];
                let pi = vk::PresentInfoKHR::default()
                    .wait_semaphores(&sig)
                    .swapchains(&scs)
                    .image_indices(&idx);
                match scf.queue_present(self.gpu.queue, &pi) {
                    Ok(false) => {}
                    Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.needs_resize = true,
                    Err(e) => return Err(e.into()),
                }
            } else {
                let si = [vk::SubmitInfo::default().command_buffers(&cmds)];
                dev.queue_submit(self.gpu.queue, &si, fence)?;
            }
        }
        self.ui_verts = ui;
        self.ui_ranges = ui_ranges;
        for up in pic_uploads {
            if let Some(b) = up.staging {
                self.frames[fi].staging.push(b);
            }
        }
        self.cur = (self.cur + 1) % FRAMES;
        self.frame_count += 1;
        Ok(())
    }
}
