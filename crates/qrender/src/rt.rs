//! Ray-traced reflections (r_water_reflections 2) on devices with ray
//! queries: resources and their descriptors.
//!
//! The static world (every submodel, including external `b_*.bsp` brush
//! models) gets one BLAS per submodel at the first frame that needs it
//! (`world.rs`). Per frame slot there is a TLAS (world, brush entities and
//! one instance holding all alias models), the alias BLAS rebuilt every
//! frame from `rt_pose.comp`'s output, and a host buffer with the texture
//! animation table and the alias draws (`frame/rt.rs` fills and records it).
//! Everything is created only once ray-traced reflections are first used.

use crate::gpu::Accel;
use crate::pipelines::{RT_COMP_BUFFERS, RT_SET_BUFFERS};
use crate::*;
mod world;
pub(crate) use world::RtWorld;

/// Alias triangles posed per frame (excess models are left out).
pub(crate) const MAX_ALIAS_TRIS: u32 = 131_072;
/// Maximum holey (alpha-tested) alias triangles in BLAS geometry 1.
pub(crate) const MAX_HOLEY_TRIS: u32 = 32_768;
/// Alias model surfaces per frame.
pub(crate) const MAX_DRAWS: usize = 4096;
/// TLAS instances per frame (world, brush entities, alias models).
pub(crate) const MAX_INSTANCES: usize = 2048;
/// Texture animation table words (primary, then alternates from
/// `RT_ANIM_ALT` in rt_types.glsl).
pub(crate) const ANIM_ALT: usize = 4096;
pub(crate) const ANIM_BYTES: u64 = (ANIM_ALT * 2 * 4) as u64;
/// The alias BLAS is rebuilt per frame or refitted while the model list
/// stays the same.
pub(crate) const ALIAS_BUILD_FLAGS: vk::BuildAccelerationStructureFlagsKHR =
    vk::BuildAccelerationStructureFlagsKHR::from_raw(
        vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_BUILD.as_raw()
            | vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE.as_raw(),
    );
/// Refits before a full rebuild (refits loosen the tree as models animate).
pub(crate) const ALIAS_REFITS: u32 = 32;

/// One alias model surface of the frame (`RtDraw` in rt_types.glsl).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct RtDraw {
    pub m: [[f32; 4]; 3],
    pub light: [f32; 4],
    pub shadevec: [f32; 4],
    pub poses: [u32; 4],
    pub texs: [u32; 4],
    pub addr: [u32; 4],
    pub range: [u32; 4],
}

/// `VkAccelerationStructureInstanceKHR` as plain data.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Instance {
    pub transform: [f32; 12],
    pub custom_mask: u32,
    pub sbt_flags: u32,
    pub blas: u64,
}

/// Per frame slot: acceleration structures, posed alias geometry and the
/// host-written tables, plus their descriptor sets.
pub(crate) struct RtFrame {
    pub tlas: Accel,
    pub tlas_scratch: (Buffer, u64),
    /// `MAX_INSTANCES` instances (host visible).
    pub instances: Buffer,
    pub alias_blas: Accel,
    pub alias_scratch: (Buffer, u64),
    /// Alias draws (model, first index, triangles) the BLAS was last fully
    /// built for, and frames refitted since: the same list is refitted
    /// (positions only) instead of rebuilt, up to `ALIAS_REFITS` times.
    pub alias_sig: Vec<[u32; 3]>,
    pub alias_opaque_count: u32,
    pub alias_age: u32,
    /// Per-frame scratch buffers reused by `prepare_rt` (zero heap allocations).
    pub out_scratch: Vec<RtDraw>,
    pub sig_scratch: Vec<[u32; 3]>,
    pub inst_scratch: Vec<Instance>,
    /// Posed alias triangles: 3 positions each (BLAS input).
    pub pos: Buffer,
    /// Alias triangle attributes (3 vec4 each).
    pub attr: Buffer,
    /// Texture animation table, then the alias draws (host visible).
    pub data: Buffer,
    pub set: vk::DescriptorSet,
    pub comp_set: vk::DescriptorSet,
}

pub(crate) struct Rt {
    pool: vk::DescriptorPool,
    pub frames: Vec<RtFrame>,
    pub world: Option<RtWorld>,
    /// The alias triangle cap was hit (warned once).
    pub warned: bool,
}

/// Build geometries of the alias BLAS: geometry 0 holds `opaque` triangles
/// (`OPAQUE`, bypassing any-hit traversal), geometry 1 holds `holey` triangles
/// starting at `pos + opaque * 36` (`NO_DUPLICATE_ANY_HIT_INVOCATION`).
pub(crate) fn alias_geometries(
    pos: u64,
    opaque: u32,
    holey: u32,
) -> [vk::AccelerationStructureGeometryKHR<'static>; 2] {
    let make = |addr: u64, n: u32, flags: vk::GeometryFlagsKHR| {
        let tri = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
            .vertex_format(vk::Format::R32G32B32_SFLOAT)
            .vertex_data(vk::DeviceOrHostAddressConstKHR { device_address: addr })
            .vertex_stride(12)
            .max_vertex((n * 3).max(1) - 1)
            .index_type(vk::IndexType::NONE_KHR);
        vk::AccelerationStructureGeometryKHR::default()
            .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
            .geometry(vk::AccelerationStructureGeometryDataKHR { triangles: tri })
            .flags(flags)
    };
    let holey_addr = if holey > 0 { pos + u64::from(opaque) * 36 } else { pos };
    [
        make(pos, opaque, vk::GeometryFlagsKHR::OPAQUE),
        make(holey_addr, holey, vk::GeometryFlagsKHR::NO_DUPLICATE_ANY_HIT_INVOCATION),
    ]
}

/// Build geometry of a TLAS over the instances at `addr`.
pub(crate) fn instance_geometry(addr: u64) -> vk::AccelerationStructureGeometryKHR<'static> {
    let inst = vk::AccelerationStructureGeometryInstancesDataKHR::default()
        .array_of_pointers(false)
        .data(vk::DeviceOrHostAddressConstKHR { device_address: addr });
    vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::INSTANCES)
        .geometry(vk::AccelerationStructureGeometryDataKHR { instances: inst })
}

impl Renderer {
    /// Reflection mode in effect: 0 off, 1 planar, 2 ray traced (planar
    /// where the device has no ray queries).
    pub(crate) fn reflection_mode(&self, st: &Settings) -> u32 {
        match st.water_reflections {
            0 => 0,
            2 if self.gpu.rt.is_some() => 2,
            _ => 1,
        }
    }

    /// The device can ray trace reflections (`r_water_reflections 2`).
    pub fn ray_tracing_supported(&self) -> bool {
        self.gpu.rt.is_some()
    }

    /// Creates the per-frame resources and the world's BLASes when missing.
    pub(crate) fn ensure_rt(&mut self) -> Result<()> {
        let mut dirty = false;
        if self.rt.is_none() {
            self.idle();
            self.rt = Some(self.create_rt()?);
            dirty = true;
        }
        if self.world.is_some() && self.rt.as_ref().is_some_and(|r| r.world.is_none()) {
            self.idle();
            let w = self.build_rt_world()?;
            if let Some(rt) = &mut self.rt {
                rt.world = Some(w);
            }
            dirty = true;
        }
        if dirty {
            self.idle();
            self.write_rt_descriptors();
        }
        Ok(())
    }

    fn create_rt(&mut self) -> Result<Rt> {
        let Some(rp) = &self.pipes.rt else {
            return Err(Error::new("ray queries unsupported"));
        };
        let (set_layout, comp_layout) = (rp.set_layout, rp.comp_set_layout);
        let n = FRAMES as u32;
        let sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::ACCELERATION_STRUCTURE_KHR, descriptor_count: n },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: n * (RT_SET_BUFFERS + RT_COMP_BUFFERS),
            },
        ];
        let dev = self.gpu.device.clone();
        // SAFETY: valid device and create info.
        let pool = unsafe {
            dev.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2 * n).pool_sizes(&sizes), None)?
        };
        let build_input = vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR
            | vk::BufferUsageFlags::STORAGE_BUFFER;
        let mut frames = Vec::new();
        for _ in 0..FRAMES {
            let layouts = [set_layout, comp_layout];
            // SAFETY: valid pool with room for both sets.
            let sets = unsafe {
                dev.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts),
                )?
            };
            let instances = self.gpu.buffer(MAX_INSTANCES as u64 * 64, build_input, true)?;
            let pos = self.gpu.buffer(u64::from(MAX_ALIAS_TRIS) * 36, build_input, false)?;
            let attr = self.gpu.buffer(u64::from(MAX_ALIAS_TRIS) * 48, vk::BufferUsageFlags::STORAGE_BUFFER, false)?;
            let data = self.gpu.buffer(
                ANIM_BYTES + (MAX_DRAWS * std::mem::size_of::<RtDraw>()) as u64,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                true,
            )?;
            // alias BLAS sized for the cap (geometry 0 = opaque, geometry 1 = holey)
            let geo = alias_geometries(self.gpu.device_address(pos.buf), MAX_ALIAS_TRIS, MAX_HOLEY_TRIS);
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .flags(ALIAS_BUILD_FLAGS)
                .geometries(&geo);
            let (size, scratch, update) = self.gpu.accel_sizes(&info, &[MAX_ALIAS_TRIS, MAX_HOLEY_TRIS]);
            let alias_blas = self.gpu.create_accel(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL, size)?;
            let alias_scratch = self.gpu.scratch_buffer(scratch.max(update))?;
            let geo = [instance_geometry(self.gpu.device_address(instances.buf))];
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
                .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
                .geometries(&geo);
            let (size, scratch, _) = self.gpu.accel_sizes(&info, &[MAX_INSTANCES as u32]);
            let tlas = self.gpu.create_accel(vk::AccelerationStructureTypeKHR::TOP_LEVEL, size)?;
            let tlas_scratch = self.gpu.scratch_buffer(scratch)?;
            frames.push(RtFrame {
                tlas,
                tlas_scratch,
                instances,
                alias_blas,
                alias_scratch,
                alias_sig: Vec::new(),
                alias_opaque_count: 0,
                alias_age: 0,
                out_scratch: Vec::new(),
                sig_scratch: Vec::new(),
                inst_scratch: Vec::new(),
                pos,
                attr,
                data,
                set: sets[0],
                comp_set: sets[1],
            });
        }
        Ok(Rt { pool, frames, world: None, warned: false })
    }

    /// Points the RT descriptor sets at the current buffers (device idle).
    pub(crate) fn write_rt_descriptors(&self) {
        let Some(rt) = &self.rt else { return };
        let d = self.dummy.buf;
        let (vbuf, idx, tex, geom) = match (&self.world, &rt.world) {
            (Some(w), Some(r)) => (w.vbuf.buf, r.idx.buf, r.tex.buf, r.geom.buf),
            _ => (d, d, d, d),
        };
        let info = |b: vk::Buffer, offset: u64, range: u64| [vk::DescriptorBufferInfo { buffer: b, offset, range }];
        for (fi, f) in rt.frames.iter().enumerate() {
            let w = vk::WHOLE_SIZE;
            let bufs = [
                info(vbuf, 0, w),
                info(idx, 0, w),
                info(tex, 0, w),
                info(geom, 0, w),
                info(f.data.buf, 0, ANIM_BYTES),
                info(f.data.buf, ANIM_BYTES, w),
                info(f.attr.buf, 0, w),
            ];
            let comp = [
                info(self.alias_buf.buf, 0, w),
                info(self.frames[fi].dynbuf.buf, 0, w),
                info(f.data.buf, ANIM_BYTES, w),
                info(f.pos.buf, 0, w),
                info(f.attr.buf, 0, w),
            ];
            let tl = [f.tlas.handle];
            let mut asw = vk::WriteDescriptorSetAccelerationStructureKHR::default().acceleration_structures(&tl);
            let mut writes = vec![vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::ACCELERATION_STRUCTURE_KHR)
                .push_next(&mut asw)];
            let sb = vk::DescriptorType::STORAGE_BUFFER;
            for (i, b) in bufs.iter().enumerate() {
                writes.push(vk::WriteDescriptorSet::default().dst_set(f.set).dst_binding(i as u32 + 1).descriptor_type(sb).buffer_info(b));
            }
            for (i, b) in comp.iter().enumerate() {
                writes.push(vk::WriteDescriptorSet::default().dst_set(f.comp_set).dst_binding(i as u32).descriptor_type(sb).buffer_info(b));
            }
            // SAFETY: device idle; all handles valid.
            unsafe { self.gpu.device.update_descriptor_sets(&writes, &[]) };
        }
    }

    /// Frees the world's BLASes and tables (device idle; new level).
    pub(crate) fn free_rt_world(&mut self) {
        if let Some(w) = self.rt.as_mut().and_then(|r| r.world.take()) {
            w.free(&mut self.gpu);
        }
    }

    /// Frees everything ray tracing created (device idle; teardown).
    pub(crate) fn free_rt(&mut self) {
        self.free_rt_world();
        let Some(rt) = self.rt.take() else { return };
        for f in rt.frames {
            self.gpu.free_accel(f.tlas);
            self.gpu.free_accel(f.alias_blas);
            for b in [f.tlas_scratch.0, f.alias_scratch.0, f.instances, f.pos, f.attr, f.data] {
                self.gpu.free_buffer(b);
            }
        }
        // SAFETY: device idle; the sets die with the pool.
        unsafe { self.gpu.device.destroy_descriptor_pool(rt.pool, None) };
    }
}
