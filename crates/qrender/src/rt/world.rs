//! Static world BLASes: one per submodel (the world, its brush entities and
//! external brush models), with opaque and alpha-tested (fence texture)
//! triangles as separate geometries so only fences run candidate tests.

use super::*;

/// Sky triangles in `RtWorld::tex` (rt.glsl `RT_SKY`).
const RT_SKY: u32 = 0x8000_0000;

pub(crate) struct RtWorld {
    /// Vertex indices into the world vertex buffer, 3 per triangle.
    pub idx: Buffer,
    /// BSP texture per triangle (| `RT_SKY`).
    pub tex: Buffer,
    /// Per submodel: first triangle of geometry 0 and geometry 1.
    pub geom: Buffer,
    /// Per submodel (None: no triangles).
    pub blas: Vec<Option<Accel>>,
}

impl RtWorld {
    pub(super) fn free(self, gpu: &mut Gpu) {
        for a in self.blas.into_iter().flatten() {
            gpu.free_accel(a);
        }
        for b in [self.idx, self.tex, self.geom] {
            gpu.free_buffer(b);
        }
    }
}

/// Triangle lists of one submodel: (opaque, alpha-tested), each a list of
/// (three vertex indices, texture).
type SubTris = [Vec<([u32; 3], u32)>; 2];

fn submodel_tris(w: &WorldGpu, sub: &crate::world::SubModel) -> SubTris {
    let cpu = &w.cpu;
    let mut out: SubTris = Default::default();
    for si in sub.first_surface..sub.first_surface + sub.num_surfaces {
        let Some(s) = cpu.surfaces.get(si as usize) else { break };
        if s.num_indices == 0 {
            continue;
        }
        let (kind, flags) = w.tex_info.get(s.texture as usize).copied().unwrap_or((TextureKind::Normal, 0));
        let tex = s.texture | if kind == TextureKind::Sky { RT_SKY } else { 0 };
        let list = &mut out[usize::from(flags & 0x10 != 0)];
        let idx = &cpu.indices[s.first_index as usize..(s.first_index + s.num_indices) as usize];
        list.extend(idx.as_chunks::<3>().0.iter().map(|t| (*t, tex)));
    }
    out
}

impl Renderer {
    /// Builds the BLAS of every submodel (waits for the GPU).
    pub(super) fn build_rt_world(&mut self) -> Result<RtWorld> {
        let Some(w) = self.world.as_ref() else {
            return Err(Error::new("no world"));
        };
        let nverts = w.cpu.vertices.len() as u32;
        let vaddr = self.gpu.device_address(w.vbuf.buf);
        let mut idx: Vec<u32> = Vec::new();
        let mut tex: Vec<u32> = Vec::new();
        let mut geom: Vec<u32> = Vec::new();
        // per submodel: geometries as (first triangle, count, opaque)
        let mut subs: Vec<Vec<(u32, u32, bool)>> = Vec::new();
        for sub in &w.cpu.submodels {
            let lists = submodel_tris(w, sub);
            let mut gs = Vec::new();
            for (li, list) in lists.iter().enumerate() {
                if list.is_empty() {
                    continue;
                }
                gs.push((tex.len() as u32, list.len() as u32, li == 0));
                for (t, x) in list {
                    idx.extend_from_slice(t);
                    tex.push(*x);
                }
            }
            geom.extend([gs.first().map_or(0, |g| g.0), gs.get(1).map_or(0, |g| g.0)]);
            subs.push(gs);
        }
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_DST
            | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
        let mut up = self.gpu.begin_upload()?;
        let upload = |gpu: &mut Gpu, up: &mut gpu::Upload, data: &[u32]| -> Result<Buffer> {
            let b = gpu.buffer((data.len().max(4) * 4) as u64, usage, false)?;
            if !data.is_empty() {
                gpu.upload_buffer(up, b.buf, 0, bytes_of(data))?;
            }
            Ok(b)
        };
        let idx_buf = upload(&mut self.gpu, &mut up, &idx)?;
        let tex_buf = upload(&mut self.gpu, &mut up, &tex)?;
        let geom_buf = upload(&mut self.gpu, &mut up, &geom)?;
        // the index data must be in place before the builds read it
        let mb = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR)];
        // SAFETY: recording the upload command buffer.
        unsafe {
            self.gpu.device.cmd_pipeline_barrier(
                up.cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                vk::DependencyFlags::empty(),
                &mb,
                &[],
                &[],
            )
        };
        let iaddr = self.gpu.device_address(idx_buf.buf);
        let geometry = |&(first, _, opaque): &(u32, u32, bool)| {
            let tri = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
                .vertex_format(vk::Format::R32G32B32_SFLOAT)
                .vertex_data(vk::DeviceOrHostAddressConstKHR { device_address: vaddr })
                .vertex_stride(std::mem::size_of::<crate::world::WorldVertex>() as u64)
                .max_vertex(nverts.max(1) - 1)
                .index_type(vk::IndexType::UINT32)
                .index_data(vk::DeviceOrHostAddressConstKHR { device_address: iaddr + u64::from(first) * 12 });
            vk::AccelerationStructureGeometryKHR::default()
                .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                .geometry(vk::AccelerationStructureGeometryDataKHR { triangles: tri })
                .flags(if opaque {
                    vk::GeometryFlagsKHR::OPAQUE
                } else {
                    vk::GeometryFlagsKHR::NO_DUPLICATE_ANY_HIT_INVOCATION
                })
        };
        // sizes first, then one scratch buffer shared by sequential builds
        let mut blas = Vec::new();
        let mut max_scratch = 0;
        for gs in &subs {
            if gs.is_empty() {
                blas.push(None);
                continue;
            }
            let geo: Vec<_> = gs.iter().map(geometry).collect();
            let counts: Vec<u32> = gs.iter().map(|g| g.1).collect();
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
                .geometries(&geo);
            let (size, scratch, _) = self.gpu.accel_sizes(&info, &counts);
            max_scratch = max_scratch.max(scratch);
            blas.push(Some(self.gpu.create_accel(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL, size)?));
        }
        let (scratch, scratch_addr) = self.gpu.scratch_buffer(max_scratch.max(256))?;
        let accel_fn = &self.gpu.rt.as_ref().expect("ray queries enabled").accel;
        for (gs, a) in subs.iter().zip(&blas) {
            let Some(a) = a else { continue };
            let geo: Vec<_> = gs.iter().map(geometry).collect();
            let ranges: Vec<_> = gs
                .iter()
                .map(|g| vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(g.1))
                .collect();
            let info = [vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
                .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                .dst_acceleration_structure(a.handle)
                .scratch_data(vk::DeviceOrHostAddressKHR { device_address: scratch_addr })
                .geometries(&geo)];
            let mb = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR)
                .dst_access_mask(
                    vk::AccessFlags::ACCELERATION_STRUCTURE_READ_KHR | vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR,
                )];
            // SAFETY: recording; buffers and structures alive until the upload completes.
            unsafe {
                accel_fn.cmd_build_acceleration_structures(up.cmd, &info, &[&ranges]);
                // the next build reuses the scratch buffer
                self.gpu.device.cmd_pipeline_barrier(
                    up.cmd,
                    vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::DependencyFlags::empty(),
                    &mb,
                    &[],
                    &[],
                );
            }
        }
        self.gpu.end_upload(up)?;
        self.gpu.free_buffer(scratch);
        Ok(RtWorld { idx: idx_buf, tex: tex_buf, geom: geom_buf, blas })
    }
}
