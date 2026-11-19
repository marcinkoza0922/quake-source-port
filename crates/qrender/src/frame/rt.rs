//! Ray-traced reflections, per frame: fills the slot's texture animation
//! table, alias draws and TLAS instances (`prepare_rt`), then records the
//! alias posing, the alias BLAS and the TLAS builds (`record_rt`).

use super::*;
use crate::rt::{ALIAS_BUILD_FLAGS, ALIAS_REFITS, ANIM_ALT, ANIM_BYTES, Instance, MAX_ALIAS_TRIS, MAX_DRAWS, MAX_INSTANCES, RtDraw, alias_geometry, instance_geometry};

/// Instance custom index bits (rt_types.glsl).
const RT_ALIAS: u32 = 0x80_0000;
const RT_ALT: u32 = 0x40_0000;
/// gl_GeometryInstanceFlags: no back-face culling (Quake brushes are one-sided).
const CULL_DISABLE: u32 = 0x1;

/// What `record_rt` builds this frame.
#[derive(Clone, Copy)]
pub(super) struct RtBuild {
    draws: u32,
    tris: u32,
    instances: u32,
    /// Refit the alias BLAS (same model list as its last build).
    refit: bool,
}

fn rows(m: &Mat4) -> [[f32; 4]; 3] {
    std::array::from_fn(|r| [m[r], m[4 + r], m[8 + r], m[12 + r]])
}

impl Renderer {
    /// Writes slot `fi`'s tables for this frame's alias models (`draws`,
    /// the first view's, including off-screen ones) and brush entities.
    pub(super) fn prepare_rt(&mut self, fi: usize, fr: &RenderFrame, draws: &[AliasDraw], time: f64) -> Option<RtBuild> {
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
        // alias surfaces, in output triangle order
        let mut out: Vec<RtDraw> = Vec::with_capacity(draws.len());
        let mut tris = 0u32;
        for a in draws.iter().filter(|a| !a.viewmodel) {
            let Some(Model::Alias(am)) = self.models.get(a.model) else { continue };
            let n = a.num_indices / 3;
            if n == 0 {
                continue;
            }
            if tris + n > MAX_ALIAS_TRIS || out.len() >= MAX_DRAWS {
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
                range: [a.first_index, n, tris, 0],
            });
            tris += n;
        }
        let sig: Vec<[u32; 3]> = draws
            .iter()
            .filter(|a| !a.viewmodel)
            .take(out.len())
            .map(|a| [a.model as u32, a.first_index, a.num_indices / 3])
            .collect();
        let refit = tris > 0 && sig == f.alias_sig && f.alias_age < ALIAS_REFITS;
        if refit {
            f.alias_age += 1;
        } else {
            f.alias_sig = sig;
            f.alias_age = 0;
        }
        let db = bytes_of(&out);
        rest[..db.len()].copy_from_slice(db);
        // instances: the world, brush entities, then all alias models
        let mut inst: Vec<Instance> = Vec::new();
        let ident_rows = rows(&ident());
        let flat = |r: [[f32; 4]; 3]| -> [f32; 12] { std::array::from_fn(|i| r[i / 4][i % 4]) };
        let add = |inst: &mut Vec<Instance>, transform: [f32; 12], custom: u32, blas: u64| {
            if inst.len() < MAX_INSTANCES {
                inst.push(Instance { transform, custom_mask: custom | 0xff << 24, sbt_flags: CULL_DISABLE << 24, blas });
            }
        };
        if let Some(Some(b)) = rw.blas.first() {
            add(&mut inst, flat(ident_rows), 0, b.addr);
        }
        for e in fr.entities {
            if self.hidden_entities.contains(&e.key) {
                continue;
            }
            let Some(Model::Brush(sm)) = self.models.get(e.model.0 as usize) else { continue };
            let Some(Some(b)) = rw.blas.get(*sm as usize) else { continue };
            let m = affine(entity_rotation(e.angles), e.origin);
            add(&mut inst, flat(rows(&m)), *sm | if e.frame != 0 { RT_ALT } else { 0 }, b.addr);
        }
        if tris > 0 {
            add(&mut inst, flat(ident_rows), RT_ALIAS, f.alias_blas.addr);
        }
        f.instances.bytes_mut()[..inst.len() * 64].copy_from_slice(bytes_of(&inst));
        Some(RtBuild { draws: out.len() as u32, tris, instances: inst.len() as u32, refit })
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
        // SAFETY: recording `cmd`; every buffer and structure belongs to slot
        // `fi`, whose previous use finished (fence waited).
        unsafe {
            if b.tris > 0 {
                let cs = vk::PipelineBindPoint::COMPUTE;
                dev.cmd_bind_pipeline(cmd, cs, rp.pose);
                dev.cmd_bind_descriptor_sets(cmd, cs, rp.comp_layout, 0, &[f.comp_set], &[]);
                dev.cmd_push_constants(cmd, rp.comp_layout, vk::ShaderStageFlags::COMPUTE, 0, bytes_of(&[b.draws, b.tris]));
                dev.cmd_dispatch(cmd, b.tris.div_ceil(64), 1, 1);
                barrier(
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::AccessFlags::SHADER_WRITE,
                    build_stage | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::AccessFlags::SHADER_READ | as_read,
                );
                let geo = [alias_geometry(self.gpu.device_address(f.pos.buf), b.tris)];
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
                let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(b.tris)];
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
}
