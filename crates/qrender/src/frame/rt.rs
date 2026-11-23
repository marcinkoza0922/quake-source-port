//! Ray-traced reflections, per frame: fills the slot's texture animation
//! table, alias draws and TLAS instances (`prepare_rt`), records the alias
//! posing and BLAS/TLAS builds (`record_rt`), and traces reflections into the
//! half-resolution target (`record_rt_refl`).

use super::*;
use crate::rt::{
    ALIAS_BUILD_FLAGS, ALIAS_REFITS, ANIM_ALT, ANIM_BYTES, Instance, MAX_ALIAS_TRIS, MAX_DRAWS,
    MAX_HOLEY_TRIS, MAX_INSTANCES, RtDraw, alias_geometries, instance_geometry,
};

/// Instance custom index bits (rt_types.glsl).
const RT_ALIAS: u32 = 0x80_0000;
const RT_ALT: u32 = 0x40_0000;
/// gl_GeometryInstanceFlags: no back-face culling (Quake brushes are one-sided).
const CULL_DISABLE: u32 = 0x1;
/// Max distance from visible water bounds for alias models to enter the RT BLAS.
const RT_ALIAS_MAX_DIST: f32 = 3072.0;

/// What `record_rt` builds this frame.
#[derive(Clone, Copy)]
pub(super) struct RtBuild {
    draws: u32,
    pub(super) opaque_tris: u32,
    holey_tris: u32,
    instances: u32,
    /// Refit the alias BLAS (same model list as its last build).
    refit: bool,
}

fn rows(m: &Mat4) -> [[f32; 4]; 3] {
    std::array::from_fn(|r| [m[r], m[4 + r], m[8 + r], m[12 + r]])
}

/// True when an alias model at `(origin, radius)` can contribute to reflections
/// on visible water within `water_bounds` from an eye at height `eye_z`.
#[inline]
pub(super) fn rt_alias_relevant(
    origin: [f32; 3],
    radius: f32,
    water_bounds: Option<([f32; 3], [f32; 3])>,
    eye_z: f32,
) -> bool {
    let Some((mn, mx)) = water_bounds else { return true };
    // Eye above all visible water: reflected rays head upward, so anything wholly below water cannot be hit.
    if eye_z > mx[2] + 1.0 && origin[2] + radius < mn[2] {
        return false;
    }
    let dx = origin[0] - origin[0].clamp(mn[0], mx[0]);
    let dy = origin[1] - origin[1].clamp(mn[1], mx[1]);
    let dz = origin[2] - origin[2].clamp(mn[2], mx[2]);
    let lim = RT_ALIAS_MAX_DIST + radius;
    dx * dx + dy * dy + dz * dz <= lim * lim
}

/// Viewport inside `RtReflTargets` for this view's scene region.
pub(super) fn rt_refl_viewport(rt: &RtReflTargets, [scene_w, scene_h]: [u32; 2], [sw, sh]: [u32; 2]) -> [u32; 2] {
    [
        ((scene_w as f32 * rt.width as f32 / sw.max(1) as f32).round() as u32).clamp(1, rt.width),
        ((scene_h as f32 * rt.height as f32 / sh.max(1) as f32).round() as u32).clamp(1, rt.height),
    ]
}

/// Packs two normalized `[0, 1]` floats into GLSL `unpackUnorm2x16` layout.
pub(super) fn pack_unorm2x16(x: f32, y: f32) -> u32 {
    let a = (x.clamp(0.0, 1.0) * 65535.0).round() as u32;
    let b = (y.clamp(0.0, 1.0) * 65535.0).round() as u32;
    a | (b << 16)
}

impl Renderer {
    /// Writes slot `fi`'s tables for this frame's alias models (`draws`,
    /// partitioned opaque-first then holey) and brush entities.
    pub(super) fn prepare_rt(
        &mut self,
        fi: usize,
        fr: &RenderFrame,
        draws: &[AliasDraw],
        time: f64,
        water_bounds: Option<([f32; 3], [f32; 3])>,
        eye_z: f32,
    ) -> Option<RtBuild> {
        let rt = self.rt.as_mut()?;
        let rw = rt.world.as_ref()?;
        let w = self.world.as_ref()?;
        let f = &mut rt.frames[fi];
        // texture animation: resolved texture per BSP texture, then alternates
        let ntex = w.tex_info.len().min(ANIM_ALT);
        let data = f.data.bytes_mut();
        let (anim, rest) = data.split_at_mut(ANIM_BYTES as usize);
        for t in 0..ntex {
            for (half, alt) in [(0, false), (ANIM_ALT, true)] {
                let r = w.cpu.resolve_texture(t as u32, alt, time, 0);
                anim[(half + t) * 4..(half + t) * 4 + 4].copy_from_slice(&r.to_le_bytes());
            }
        }
        // Alias surfaces partitioned into geometry 0 (opaque, hardware OPAQUE flag)
        // followed by geometry 1 (holey/alpha-tested, any-hit traversal only for these).
        let out = &mut f.out_scratch;
        let sig = &mut f.sig_scratch;
        out.clear();
        sig.clear();
        let mut opaque_tris = 0u32;
        let mut holey_tris = 0u32;
        for holey_pass in [false, true] {
            for a in draws.iter().filter(|a| !a.viewmodel) {
                let is_holey = (a.push.texs[3] & 1) != 0;
                if is_holey != holey_pass {
                    continue;
                }
                if !rt_alias_relevant(a.origin, a.radius, water_bounds, eye_z) {
                    continue;
                }
                let Some(Model::Alias(am)) = self.models.get(a.model) else { continue };
                let n = a.num_indices / 3;
                if n == 0 {
                    continue;
                }
                let total = opaque_tris + holey_tris;
                if total + n > MAX_ALIAS_TRIS || (holey_pass && holey_tris + n > MAX_HOLEY_TRIS) || out.len() >= MAX_DRAWS {
                    if !rt.warned {
                        rt.warned = true;
                        eprintln!("ray-traced reflections: more than {MAX_ALIAS_TRIS} model triangles, some models left out");
                    }
                    break;
                }
                let va = self.gpu.device_address(am.vbuf.buf);
                let ia = self.gpu.device_address(am.ibuf.buf);
                out.push(RtDraw {
                    m: rows(&a.push.model),
                    light: a.push.light,
                    shadevec: a.push.shadevec,
                    poses: a.push.poses,
                    texs: a.push.texs,
                    addr: [va as u32, (va >> 32) as u32, ia as u32, (ia >> 32) as u32],
                    range: [a.first_index, n, total, 0],
                });
                sig.push([a.model as u32, a.first_index, n]);
                if holey_pass {
                    holey_tris += n;
                } else {
                    opaque_tris += n;
                }
            }
        }
        let tris = opaque_tris + holey_tris;
        let refit = tris > 0
            && opaque_tris == f.alias_opaque_count
            && *sig == f.alias_sig
            && f.alias_age < ALIAS_REFITS;
        if refit {
            f.alias_age += 1;
        } else {
            std::mem::swap(&mut f.alias_sig, sig);
            f.alias_opaque_count = opaque_tris;
            f.alias_age = 0;
        }
        let db = bytes_of(out);
        rest[..db.len()].copy_from_slice(db);
        // instances: the world, brush entities, then all alias models
        let inst = &mut f.inst_scratch;
        inst.clear();
        let ident_rows = rows(&ident());
        let flat = |r: [[f32; 4]; 3]| -> [f32; 12] { std::array::from_fn(|i| r[i / 4][i % 4]) };
        let add = |inst: &mut Vec<Instance>, transform: [f32; 12], custom: u32, blas: u64| {
            if inst.len() < MAX_INSTANCES {
                inst.push(Instance { transform, custom_mask: custom | 0xff << 24, sbt_flags: CULL_DISABLE << 24, blas });
            }
        };
        if let Some(Some(b)) = rw.blas.first() {
            add(inst, flat(ident_rows), 0, b.addr);
        }
        for e in fr.entities {
            if self.hidden_entities.contains(&e.key) {
                continue;
            }
            let Some(Model::Brush(sm)) = self.models.get(e.model.0 as usize) else { continue };
            let Some(Some(b)) = rw.blas.get(*sm as usize) else { continue };
            let m = affine(entity_rotation(e.angles), e.origin);
            add(inst, flat(rows(&m)), *sm | if e.frame != 0 { RT_ALT } else { 0 }, b.addr);
        }
        if tris > 0 {
            add(inst, flat(ident_rows), RT_ALIAS, f.alias_blas.addr);
        }
        f.instances.bytes_mut()[..inst.len() * 64].copy_from_slice(bytes_of(inst));
        Some(RtBuild {
            draws: out.len() as u32,
            opaque_tris,
            holey_tris,
            instances: inst.len() as u32,
            refit,
        })
    }

    /// Records the alias posing and the BLAS/TLAS builds of slot `fi`, and
    /// binds graphics set 1 for the ray-query water pipelines.
    pub(super) fn record_rt(&self, cmd: vk::CommandBuffer, fi: usize, b: RtBuild) {
        let (Some(rt), Some(rp), Some(fns)) = (&self.rt, &self.pipes.rt, &self.gpu.rt) else { return };
        let f = &rt.frames[fi];
        let dev = &self.gpu.device;
        let barrier = |src_stage, src, dst_stage, dst| {
            let mb = [vk::MemoryBarrier::default().src_access_mask(src).dst_access_mask(dst)];
            // SAFETY: recording `cmd`.
            unsafe { dev.cmd_pipeline_barrier(cmd, src_stage, dst_stage, vk::DependencyFlags::empty(), &mb, &[], &[]) };
        };
        let build_stage = vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR;
        let as_write = vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR;
        let as_read = vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR;
        let tris = b.opaque_tris + b.holey_tris;
        // SAFETY: recording `cmd`; every buffer and structure belongs to slot
        // `fi`, whose previous use finished (fence waited).
        unsafe {
            if tris > 0 {
                let cs = vk::PipelineBindPoint::COMPUTE;
                dev.cmd_bind_pipeline(cmd, cs, rp.pose);
                dev.cmd_bind_descriptor_sets(cmd, cs, rp.comp_layout, 0, &[f.comp_set], &[]);
                dev.cmd_push_constants(cmd, rp.comp_layout, vk::ShaderStageFlags::COMPUTE, 0, bytes_of(&[b.draws, tris]));
                dev.cmd_dispatch(cmd, tris.div_ceil(64), 1, 1);
                barrier(
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::AccessFlags::SHADER_WRITE,
                    build_stage | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::AccessFlags::SHADER_READ | as_read,
                );
                let geo = alias_geometries(self.gpu.device_address(f.pos.buf), b.opaque_tris, b.holey_tris);
                let info = [vk::AccelerationStructureBuildGeometryInfoKHR::default()
                    .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                    .flags(ALIAS_BUILD_FLAGS)
                    .mode(if b.refit {
                        vk::BuildAccelerationStructureModeKHR::UPDATE
                    } else {
                        vk::BuildAccelerationStructureModeKHR::BUILD
                    })
                    .src_acceleration_structure(if b.refit { f.alias_blas.handle } else { vk::AccelerationStructureKHR::null() })
                    .dst_acceleration_structure(f.alias_blas.handle)
                    .scratch_data(vk::DeviceOrHostAddressKHR { device_address: f.alias_scratch.1 })
                    .geometries(&geo)];
                let range = [
                    vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(b.opaque_tris),
                    vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(b.holey_tris),
                ];
                fns.accel.cmd_build_acceleration_structures(cmd, &info, &[&range]);
                barrier(build_stage, as_write, build_stage, as_read);
            }
            let geo = [instance_geometry(self.gpu.device_address(f.instances.buf))];
            let info = [vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
                .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
                .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                .dst_acceleration_structure(f.tlas.handle)
                .scratch_data(vk::DeviceOrHostAddressKHR { device_address: f.tlas_scratch.1 })
                .geometries(&geo)];
            let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(b.instances)];
            fns.accel.cmd_build_acceleration_structures(cmd, &info, &[&range]);
            barrier(build_stage, as_write, vk::PipelineStageFlags::FRAGMENT_SHADER, as_read);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipes.layout, 1, &[f.set], &[]);
        }
    }

    /// Records the half-resolution single-sampled depth prepass + RT water
    /// reflection trace pass into `Targets::rt_refl`. Leaves `rt_refl.color`
    /// shader-readable for `water.frag` in the main scene pass.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_rt_refl(
        &self,
        cmd: vk::CommandBuffer,
        fi: usize,
        ubo_ofs: u32,
        region: [u32; 2],
        scene: [u32; 2],
        world_draws: &[WorldDraw],
    ) {
        let (Some(t), Some(wg), Some(rt)) = (&self.targets, &self.world, &self.rt) else { return };
        let (Some(rt_t), Some(half_depth)) = (&t.rt_refl, &t.half_depth) else { return };
        let dev = &self.gpu.device;
        let p = &self.pipes;
        let f = &self.frames[fi];
        let dynbuf = f.dynbuf.buf;
        let gfx = vk::PipelineBindPoint::GRAPHICS;
        let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
        let drng = gpu::range(vk::ImageAspectFlags::DEPTH, 1, 1);
        let [rw, rh] = rt_refl_viewport(rt_t, region, scene);
        gpu::barrier(dev, cmd, rt_t.color.image, crng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        gpu::barrier(dev, cmd, half_depth.image, drng, vk::ImageLayout::UNDEFINED, vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL);
        let catt = [vk::RenderingAttachmentInfo::default()
            .image_view(rt_t.color.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } })];
        let datt = vk::RenderingAttachmentInfo::default()
            .image_view(half_depth.view)
            .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } });
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D { width: rt_t.width, height: rt_t.height },
        };
        let ri = vk::RenderingInfo::default()
            .render_area(area)
            .layer_count(1)
            .color_attachments(&catt)
            .depth_attachment(&datt);
        let vp = vk::Viewport { x: 0.0, y: 0.0, width: rw as f32, height: rh as f32, min_depth: 0.0, max_depth: 1.0 };
        let scissor = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: rw, height: rh } };
        // SAFETY: recording `cmd`; all targets, buffers, pipelines and descriptor sets belong to this frame.
        unsafe {
            dev.cmd_begin_rendering(cmd, &ri);
            dev.cmd_set_viewport(cmd, 0, &[vp]);
            dev.cmd_set_scissor(cmd, 0, &[scissor]);
            dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &[f.set], &[ubo_ofs]);
            dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 1, &[rt.frames[fi].set], &[]);
            dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);

            // 1. Opaque world & brush depth prepass so Early-Z rejects occluded water before tracing rays.
            dev.cmd_bind_pipeline(cmd, gfx, p.rt_depth_world);
            for wd in world_draws.iter().filter(|wd| wd.main_visible && wd.alpha >= 1.0) {
                let ranges = if wd.cbuf.is_some() {
                    [wd.opaque, wd.sky, wd.water[1], wd.water[3]]
                } else {
                    [wd.opaque, wd.sky, Range::default(), Range::default()]
                };
                if ranges.iter().all(|r| r.count == 0) {
                    continue;
                }
                dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[WorldPush { model: wd.model, params: [wd.alpha, 0.0, 0.0, 0.0] }]));
                if wd.zfix {
                    dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                } else {
                    dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                }
                for r in ranges {
                    if r.count > 0 {
                        dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                    }
                }
            }

            // 2. Half-res RT reflection trace for visible water & slime sheets.
            dev.cmd_bind_pipeline(cmd, gfx, p.water_rt_half);
            for wd in world_draws.iter().filter(|wd| wd.main_visible) {
                let ranges = if wd.cbuf.is_some() {
                    [wd.water[0], wd.water[2], wd.water_alpha[0], wd.water_alpha[2]]
                } else {
                    [wd.water[0], wd.water_alpha[0], Range::default(), Range::default()]
                };
                if ranges.iter().all(|r| r.count == 0) {
                    continue;
                }
                dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                dev.cmd_push_constants(cmd, p.layout, all, 0, bytes_of(&[WorldPush { model: wd.model, params: [wd.alpha, 0.0, 0.0, 0.0] }]));
                if wd.zfix {
                    dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                } else {
                    dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                }
                for r in ranges {
                    if r.count > 0 {
                        dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                    }
                }
            }
            dev.cmd_end_rendering(cmd);
        }
        gpu::barrier(dev, cmd, rt_t.color.image, crng, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }
}
