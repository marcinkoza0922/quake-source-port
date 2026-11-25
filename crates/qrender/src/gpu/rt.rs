//! Ray-query support (r_water_reflections 2): device capability check,
//! acceleration structure buffers and builds.

use super::*;

/// Device extensions needed for ray queries.
pub(super) const RT_EXTENSIONS: [&CStr; 3] = [
    ash::khr::acceleration_structure::NAME,
    ash::khr::ray_query::NAME,
    ash::khr::deferred_host_operations::NAME,
];

/// Acceleration structure entry points and limits (ray queries enabled).
pub struct RtFns {
    pub accel: ash::khr::acceleration_structure::Device,
    pub scratch_align: u64,
}

/// Whether `pdev` can run ray queries (extensions + features). Disabled
/// with `QRENDER_NO_RT=1`.
pub(super) fn rt_supported(instance: &ash::Instance, pdev: vk::PhysicalDevice) -> bool {
    if std::env::var_os("QRENDER_NO_RT").is_some() {
        return false;
    }
    // SAFETY: valid physical device.
    let Ok(exts) = (unsafe { instance.enumerate_device_extension_properties(pdev) }) else {
        return false;
    };
    let has = |n: &CStr| exts.iter().any(|e| e.extension_name_as_c_str() == Ok(n));
    if !RT_EXTENSIONS.iter().all(|n| has(n)) {
        return false;
    }
    let mut asf = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut rqf = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
    let mut f12 = vk::PhysicalDeviceVulkan12Features::default();
    let mut f2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut asf)
        .push_next(&mut rqf)
        .push_next(&mut f12);
    // SAFETY: valid physical device; the chain lives for the call.
    unsafe { instance.get_physical_device_features2(pdev, &mut f2) };
    asf.acceleration_structure == vk::TRUE && rqf.ray_query == vk::TRUE && f12.buffer_device_address == vk::TRUE
}

pub(super) fn scratch_alignment(instance: &ash::Instance, pdev: vk::PhysicalDevice) -> u64 {
    let mut asp = vk::PhysicalDeviceAccelerationStructurePropertiesKHR::default();
    let mut p2 = vk::PhysicalDeviceProperties2::default().push_next(&mut asp);
    // SAFETY: valid physical device; the chain lives for the call.
    unsafe { instance.get_physical_device_properties2(pdev, &mut p2) };
    u64::from(asp.min_acceleration_structure_scratch_offset_alignment).max(16)
}

/// An acceleration structure and the buffer holding it.
pub struct Accel {
    pub handle: vk::AccelerationStructureKHR,
    pub buf: Buffer,
    /// Device address of the structure (TLAS instances refer to BLASes by it).
    pub addr: u64,
}

impl Gpu {
    pub fn device_address(&self, buf: vk::Buffer) -> u64 {
        // SAFETY: buffers carry SHADER_DEVICE_ADDRESS while ray queries are on.
        unsafe { self.device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(buf)) }
    }

    /// Build sizes (structure, build scratch, update scratch) for a geometry list.
    pub fn accel_sizes(
        &self,
        info: &vk::AccelerationStructureBuildGeometryInfoKHR,
        max_prims: &[u32],
    ) -> (u64, u64, u64) {
        let rt = self.rt.as_ref().expect("ray queries enabled");
        let mut sz = vk::AccelerationStructureBuildSizesInfoKHR::default();
        // SAFETY: valid device and build info; counts match the geometries.
        unsafe {
            rt.accel.get_acceleration_structure_build_sizes(
                vk::AccelerationStructureBuildTypeKHR::DEVICE,
                info,
                max_prims,
                &mut sz,
            )
        };
        (sz.acceleration_structure_size, sz.build_scratch_size, sz.update_scratch_size)
    }

    pub fn create_accel(&mut self, ty: vk::AccelerationStructureTypeKHR, size: u64) -> Result<Accel> {
        let buf = self.buffer(
            size,
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            false,
        )?;
        let rt = self.rt.as_ref().expect("ray queries enabled");
        let ci = vk::AccelerationStructureCreateInfoKHR::default()
            .buffer(buf.buf)
            .size(size)
            .ty(ty);
        // SAFETY: buffer bound to memory and big enough.
        let handle = match unsafe { rt.accel.create_acceleration_structure(&ci, None) } {
            Ok(h) => h,
            Err(e) => {
                self.free_buffer(buf);
                return Err(e.into());
            }
        };
        self.journal.add(Made::Accel(handle));
        // SAFETY: handle just created.
        let addr = unsafe {
            rt.accel.get_acceleration_structure_device_address(
                &vk::AccelerationStructureDeviceAddressInfoKHR::default().acceleration_structure(handle),
            )
        };
        Ok(Accel { handle, buf, addr })
    }

    /// Frees a structure the GPU no longer uses.
    pub fn free_accel(&mut self, a: Accel) {
        self.journal.forget_accel(a.handle);
        if let Some(rt) = &self.rt {
            // SAFETY: caller guarantees the GPU is done with it.
            unsafe { rt.accel.destroy_acceleration_structure(a.handle, None) };
        }
        self.free_buffer(a.buf);
    }

    /// A device-local scratch buffer for builds, sized with alignment slack.
    pub fn scratch_buffer(&mut self, size: u64) -> Result<(Buffer, u64)> {
        let align = self.rt.as_ref().map_or(256, |r| r.scratch_align);
        let buf = self.buffer(
            size + align,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            false,
        )?;
        let addr = self.device_address(buf.buf).next_multiple_of(align);
        Ok((buf, addr))
    }
}
