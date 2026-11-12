//! Vulkan device, block suballocator, buffers/images and batched uploads.

use crate::{Error, Result};
use ash::vk;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
mod alloc;
pub use alloc::*;
mod upload;

const BLOCK_SIZE: u64 = 64 << 20;

pub struct Buffer {
    pub buf: vk::Buffer,
    pub mem: Mem,
    pub size: u64,
}

impl Buffer {
    /// Host slice of a mapped buffer.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        assert!(!self.mem.ptr.is_null());
        // SAFETY: mapped coherent memory of at least `size` bytes, owned by self.
        unsafe { std::slice::from_raw_parts_mut(self.mem.ptr, self.size as usize) }
    }
}

pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub mem: Mem,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mips: u32,
}

pub struct Gpu {
    /// Keeps the Vulkan loader loaded.
    _entry: ash::Entry,
    pub instance: ash::Instance,
    pub pdev: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub qfam: u32,
    pub mem_props: vk::PhysicalDeviceMemoryProperties,
    pub name: String,
    pub surface_fn: Option<ash::khr::surface::Instance>,
    pub swapchain_fn: Option<ash::khr::swapchain::Device>,
    pub surface: vk::SurfaceKHR,
    pub alloc: Allocator,
    pub upload_pool: vk::CommandPool,
    /// Highest MSAA sample count usable for colour+depth targets.
    pub max_samples: u32,
    /// Max sampler anisotropy (1 = unsupported).
    pub max_anisotropy: f32,
    /// sampleRateShading is enabled (supersampling).
    pub sample_shading: bool,
    /// Nanoseconds per timestamp tick.
    pub timestamp_period: f32,
}

impl Gpu {
    /// Creates instance + device. `make_surface` receives the raw VkInstance.
    pub fn new(
        mut extensions: Vec<CString>,
        make_surface: Option<&dyn Fn(u64) -> u64>,
    ) -> Result<Gpu> {
        // SAFETY: loads the system Vulkan loader.
        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| Error::new(format!("vulkan loader: {e}")))?;
        if make_surface.is_some()
            && !extensions
                .iter()
                .any(|e| e.as_c_str() == ash::khr::surface::NAME)
        {
            extensions.push(ash::khr::surface::NAME.to_owned());
        }
        let app = vk::ApplicationInfo::default()
            .application_name(c"quake-source-port")
            .engine_name(c"qrender")
            .api_version(vk::API_VERSION_1_3);
        let ext_ptrs: Vec<*const c_char> = extensions.iter().map(|e| e.as_ptr()).collect();
        let mut layers: Vec<*const c_char> = Vec::new();
        let validation = c"VK_LAYER_KHRONOS_validation";
        if std::env::var_os("QRENDER_VALIDATION").is_some() {
            layers.push(validation.as_ptr());
        }
        let ci = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(&ext_ptrs)
            .enabled_layer_names(&layers);
        // SAFETY: valid create info; pointers outlive the call.
        let instance = unsafe { entry.create_instance(&ci, None)? };
        let surface_fn = make_surface.map(|_| ash::khr::surface::Instance::new(&entry, &instance));
        let surface = match make_surface {
            Some(f) => {
                <vk::SurfaceKHR as vk::Handle>::from_raw(f(vk::Handle::as_raw(instance.handle())))
            }
            None => vk::SurfaceKHR::null(),
        };
        if make_surface.is_some() && surface == vk::SurfaceKHR::null() {
            return Err(Error::new("surface creation failed"));
        }

        // SAFETY: valid instance.
        let pdevs = unsafe { instance.enumerate_physical_devices()? };
        let want = std::env::var("QRENDER_DEVICE")
            .ok()
            .and_then(|s| s.parse::<usize>().ok());
        let mut best: Option<(i32, vk::PhysicalDevice, u32)> = None;
        for (i, &pd) in pdevs.iter().enumerate() {
            // SAFETY: pd from enumerate.
            let props = unsafe { instance.get_physical_device_properties(pd) };
            if props.api_version < vk::API_VERSION_1_3 {
                continue;
            }
            // SAFETY: pd valid.
            let qf = unsafe { instance.get_physical_device_queue_family_properties(pd) };
            let fam = qf.iter().enumerate().position(|(fi, q)| {
                q.queue_flags
                    .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                    && surface_fn.as_ref().is_none_or(|s| {
                        // SAFETY: valid pd/surface.
                        unsafe { s.get_physical_device_surface_support(pd, fi as u32, surface) }
                            .unwrap_or(false)
                    })
            });
            let Some(fam) = fam else { continue };
            let mut score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                vk::PhysicalDeviceType::CPU => 0,
                _ => 1,
            };
            if want == Some(i) {
                score = 100;
            }
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, pd, fam as u32));
            }
        }
        let Some((_, pdev, qfam)) = best else {
            return Err(Error::new("no Vulkan 1.3 device"));
        };
        // SAFETY: valid pdev.
        let props = unsafe { instance.get_physical_device_properties(pdev) };
        // SAFETY: valid pdev.
        let mem_props = unsafe { instance.get_physical_device_memory_properties(pdev) };
        let name = props
            .device_name_as_c_str()
            .map(|s: &CStr| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        let prio = [1.0f32];
        let qci = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(qfam)
            .queue_priorities(&prio)];
        let mut dev_exts: Vec<*const c_char> = Vec::new();
        if surface_fn.is_some() {
            dev_exts.push(ash::khr::swapchain::NAME.as_ptr());
        }
        let lim = props.limits;
        let counts = lim.framebuffer_color_sample_counts & lim.framebuffer_depth_sample_counts;
        let max_samples = [8u32, 4, 2]
            .into_iter()
            .find(|&n| counts.contains(vk::SampleCountFlags::from_raw(n)))
            .unwrap_or(1);
        // SAFETY: valid pdev.
        let supported = unsafe { instance.get_physical_device_features(pdev) };
        let aniso = supported.sampler_anisotropy == vk::TRUE;
        let max_anisotropy = if aniso { lim.max_sampler_anisotropy.max(1.0) } else { 1.0 };
        let sample_shading = supported.sample_rate_shading == vk::TRUE;
        let features = vk::PhysicalDeviceFeatures::default()
            .sampler_anisotropy(aniso)
            .sample_rate_shading(sample_shading)
            .multi_draw_indirect(true)
            .draw_indirect_first_instance(true);
        let mut f12 = vk::PhysicalDeviceVulkan12Features::default()
            .descriptor_indexing(true)
            .runtime_descriptor_array(true)
            .descriptor_binding_partially_bound(true)
            .shader_sampled_image_array_non_uniform_indexing(true);
        // glslc maps `discard` to OpDemoteToHelperInvocation for Vulkan 1.3
        // (core feature, required to be supported by 1.3 devices).
        let mut f13 = vk::PhysicalDeviceVulkan13Features::default()
            .dynamic_rendering(true)
            .shader_demote_to_helper_invocation(true);
        let dci = vk::DeviceCreateInfo::default()
            .queue_create_infos(&qci)
            .enabled_extension_names(&dev_exts)
            .enabled_features(&features)
            .push_next(&mut f12)
            .push_next(&mut f13);
        // SAFETY: valid pdev and create info.
        let device = unsafe { instance.create_device(pdev, &dci, None)? };
        // SAFETY: queue family/index created above.
        let queue = unsafe { device.get_device_queue(qfam, 0) };
        let swapchain_fn = surface_fn
            .as_ref()
            .map(|_| ash::khr::swapchain::Device::new(&instance, &device));
        let pci = vk::CommandPoolCreateInfo::default()
            .queue_family_index(qfam)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: valid device.
        let upload_pool = unsafe { device.create_command_pool(&pci, None)? };
        Ok(Gpu {
            _entry: entry,
            instance,
            pdev,
            device,
            queue,
            qfam,
            mem_props,
            name,
            surface_fn,
            swapchain_fn,
            surface,
            alloc: Allocator::default(),
            upload_pool,
            max_samples,
            max_anisotropy,
            timestamp_period: lim.timestamp_period,
            sample_shading,
        })
    }

    pub fn buffer(&mut self, size: u64, usage: vk::BufferUsageFlags, host: bool) -> Result<Buffer> {
        let size = size.max(16);
        let ci = vk::BufferCreateInfo::default().size(size).usage(usage);
        // SAFETY: valid device/create info.
        let buf = unsafe { self.device.create_buffer(&ci, None)? };
        // SAFETY: buf just created.
        let req = unsafe { self.device.get_buffer_memory_requirements(buf) };
        let flags = if host {
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT
        } else {
            vk::MemoryPropertyFlags::DEVICE_LOCAL
        };
        let mem = self
            .alloc
            .alloc(&self.device, &self.mem_props, req, flags, true)?;
        // SAFETY: memory satisfies requirements.
        unsafe {
            self.device
                .bind_buffer_memory(buf, mem.memory, mem.offset)?
        };
        Ok(Buffer { buf, mem, size })
    }

    /// Frees a buffer that lives as long as the renderer (teardown only;
    /// `b` must not be used afterwards).
    pub fn free_buffer_ref(&mut self, b: &Buffer) {
        // SAFETY: caller guarantees the GPU no longer uses it.
        unsafe { self.device.destroy_buffer(b.buf, None) };
        self.alloc.free(&b.mem);
    }

    pub fn free_buffer(&mut self, b: Buffer) {
        // SAFETY: caller guarantees the GPU no longer uses it.
        unsafe { self.device.destroy_buffer(b.buf, None) };
        self.alloc.free(&b.mem);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn image(
        &mut self,
        w: u32,
        h: u32,
        layers: u32,
        mips: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
    ) -> Result<Image> {
        self.image_ms(w, h, layers, mips, format, usage, aspect, 1)
    }

    /// [`Gpu::image`] with a sample count (1 = single-sampled).
    #[allow(clippy::too_many_arguments)]
    pub fn image_ms(
        &mut self,
        w: u32,
        h: u32,
        layers: u32,
        mips: u32,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        samples: u32,
    ) -> Result<Image> {
        let ci = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: w,
                height: h,
                depth: 1,
            })
            .mip_levels(mips)
            .array_layers(layers)
            .samples(vk::SampleCountFlags::from_raw(samples.max(1)))
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        // SAFETY: valid device/create info.
        let image = unsafe { self.device.create_image(&ci, None)? };
        // SAFETY: image just created.
        let req = unsafe { self.device.get_image_memory_requirements(image) };
        let mem = self.alloc.alloc(
            &self.device,
            &self.mem_props,
            req,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            false,
        )?;
        // SAFETY: memory satisfies requirements.
        unsafe {
            self.device
                .bind_image_memory(image, mem.memory, mem.offset)?
        };
        let vci = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(
                if layers > 1 || usage.contains(vk::ImageUsageFlags::STORAGE) {
                    vk::ImageViewType::TYPE_2D_ARRAY
                } else {
                    vk::ImageViewType::TYPE_2D
                },
            )
            .format(format)
            .subresource_range(range(aspect, mips, layers));
        // SAFETY: image bound to memory.
        let view = unsafe { self.device.create_image_view(&vci, None)? };
        Ok(Image {
            image,
            view,
            mem,
            width: w,
            height: h,
            layers,
            mips,
        })
    }

    pub fn free_image(&mut self, i: Image) {
        // SAFETY: caller guarantees the GPU no longer uses it.
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
        }
        self.alloc.free(&i.mem);
    }

    pub fn shader(&self, spv: &[u8]) -> Result<vk::ShaderModule> {
        let code = ash::util::read_spv(&mut std::io::Cursor::new(spv))
            .map_err(|e| Error::new(format!("spv: {e}")))?;
        let ci = vk::ShaderModuleCreateInfo::default().code(&code);
        // SAFETY: valid SPIR-V from glslc.
        Ok(unsafe { self.device.create_shader_module(&ci, None)? })
    }

}

pub struct Upload {
    pub cmd: vk::CommandBuffer,
    staging: Vec<Buffer>,
}

pub fn range(aspect: vk::ImageAspectFlags, mips: u32, layers: u32) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: aspect,
        base_mip_level: 0,
        level_count: mips,
        base_array_layer: 0,
        layer_count: layers,
    }
}

/// Coarse full-pipeline layout transition (only used at frame boundaries/uploads).
pub fn barrier(
    dev: &ash::Device,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    rng: vk::ImageSubresourceRange,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
) {
    let b = [vk::ImageMemoryBarrier::default()
        .image(image)
        .subresource_range(rng)
        .old_layout(old)
        .new_layout(new)
        .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)];
    // SAFETY: recording command buffer, valid image.
    unsafe {
        dev.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &b,
        )
    };
}

impl From<vk::Result> for Error {
    fn from(r: vk::Result) -> Self {
        Error::new(format!("vulkan: {r}"))
    }
}
