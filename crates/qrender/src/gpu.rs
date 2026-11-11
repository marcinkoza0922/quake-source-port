//! Vulkan device, block suballocator, buffers/images and batched uploads.

use crate::{Error, Result};
use ash::vk;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};

const BLOCK_SIZE: u64 = 64 << 20;

/// Sub-allocation from a [`Block`].
#[derive(Debug)]
pub struct Mem {
    pub memory: vk::DeviceMemory,
    pub offset: u64,
    pub size: u64,
    /// Host pointer at `offset` (null if not host visible).
    pub ptr: *mut u8,
    key: (u32, bool),
    block: usize,
}

struct Block {
    memory: vk::DeviceMemory,
    size: u64,
    ptr: *mut u8,
    /// Sorted free ranges (offset, size).
    free: Vec<(u64, u64)>,
}

/// Big-block allocator: one pool per (memory type, linear) so buffer/image
/// granularity never matters; first-fit with neighbour merging on free.
#[derive(Default)]
pub struct Allocator {
    pools: HashMap<(u32, bool), Vec<Block>>,
}

impl Allocator {
    fn alloc(
        &mut self,
        dev: &ash::Device,
        props: &vk::PhysicalDeviceMemoryProperties,
        req: vk::MemoryRequirements,
        flags: vk::MemoryPropertyFlags,
        linear: bool,
    ) -> Result<Mem> {
        let ty = (0..props.memory_type_count)
            .find(|&i| {
                req.memory_type_bits & (1 << i) != 0
                    && props.memory_types[i as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or_else(|| Error::new("no suitable memory type"))?;
        let host = flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE);
        let key = (ty, linear);
        let pool = self.pools.entry(key).or_default();
        let align = req.alignment.max(16);
        for (bi, b) in pool.iter_mut().enumerate() {
            if let Some(off) = take(&mut b.free, req.size, align) {
                return Ok(Mem {
                    memory: b.memory,
                    offset: off,
                    size: req.size,
                    ptr: if b.ptr.is_null() {
                        b.ptr
                    } else {
                        // SAFETY: the block is mapped in full; off + size <= block size.
                        unsafe { b.ptr.add(off as usize) }
                    },
                    key,
                    block: bi,
                });
            }
        }
        let size = req.size.max(BLOCK_SIZE).next_multiple_of(BLOCK_SIZE);
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(size)
            .memory_type_index(ty);
        // SAFETY: valid device and allocate info.
        let memory = unsafe { dev.allocate_memory(&info, None)? };
        let ptr = if host {
            // SAFETY: memory is host visible and not mapped yet.
            let p =
                unsafe { dev.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())? };
            p as *mut u8
        } else {
            std::ptr::null_mut()
        };
        let mut b = Block {
            memory,
            size,
            ptr,
            free: vec![(0, size)],
        };
        let off = take(&mut b.free, req.size, align).ok_or_else(|| Error::new("alloc"))?;
        pool.push(b);
        Ok(Mem {
            memory,
            offset: off,
            size: req.size,
            ptr: if ptr.is_null() {
                ptr
            } else {
                // SAFETY: inside the freshly mapped block.
                unsafe { ptr.add(off as usize) }
            },
            key,
            block: pool.len() - 1,
        })
    }

    fn free(&mut self, m: &Mem) {
        let Some(b) = self.pools.get_mut(&m.key).and_then(|p| p.get_mut(m.block)) else {
            return;
        };
        let i = b.free.partition_point(|r| r.0 < m.offset);
        b.free.insert(i, (m.offset, m.size));
        // merge with next, then previous
        if i + 1 < b.free.len() && b.free[i].0 + b.free[i].1 == b.free[i + 1].0 {
            b.free[i].1 += b.free[i + 1].1;
            b.free.remove(i + 1);
        }
        if i > 0 && b.free[i - 1].0 + b.free[i - 1].1 == b.free[i].0 {
            b.free[i - 1].1 += b.free[i].1;
            b.free.remove(i);
        }
    }

    fn destroy(&mut self, dev: &ash::Device) {
        for (_, p) in self.pools.drain() {
            for b in p {
                let _ = b.size;
                // SAFETY: device idle at teardown; memory owned by the block.
                unsafe { dev.free_memory(b.memory, None) };
            }
        }
    }
}

fn take(free: &mut Vec<(u64, u64)>, size: u64, align: u64) -> Option<u64> {
    for i in 0..free.len() {
        let (o, s) = free[i];
        let a = o.next_multiple_of(align);
        if a + size <= o + s {
            let tail = (o + s) - (a + size);
            let head = a - o;
            match (head > 0, tail > 0) {
                (false, false) => {
                    free.remove(i);
                }
                (true, false) => free[i] = (o, head),
                (false, true) => free[i] = (a + size, tail),
                (true, true) => {
                    free[i] = (o, head);
                    free.insert(i + 1, (a + size, tail));
                }
            }
            return Some(a);
        }
    }
    None
}

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

    pub fn begin_upload(&mut self) -> Result<Upload> {
        let ai = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.upload_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: valid pool.
        let cmd = unsafe { self.device.allocate_command_buffers(&ai)? }[0];
        let bi = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        // SAFETY: fresh command buffer.
        unsafe { self.device.begin_command_buffer(cmd, &bi)? };
        Ok(Upload {
            cmd,
            staging: Vec::new(),
        })
    }

    /// Submits the upload and waits for completion.
    pub fn end_upload(&mut self, up: Upload) -> Result<()> {
        // SAFETY: recording command buffer; queue use is single threaded.
        unsafe {
            self.device.end_command_buffer(up.cmd)?;
            let cmds = [up.cmd];
            let si = [vk::SubmitInfo::default().command_buffers(&cmds)];
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)?;
            self.device.queue_submit(self.queue, &si, fence)?;
            self.device.wait_for_fences(&[fence], true, u64::MAX)?;
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.upload_pool, &cmds);
        }
        for b in up.staging {
            self.free_buffer(b);
        }
        Ok(())
    }

    /// Copies `data` into a device-local buffer via staging.
    pub fn upload_buffer(
        &mut self,
        up: &mut Upload,
        dst: vk::Buffer,
        offset: u64,
        data: &[u8],
    ) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut st = self.buffer(data.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        st.bytes_mut()[..data.len()].copy_from_slice(data);
        let r = [vk::BufferCopy {
            src_offset: 0,
            dst_offset: offset,
            size: data.len() as u64,
        }];
        // SAFETY: recording; both buffers live until end_upload.
        unsafe { self.device.cmd_copy_buffer(up.cmd, st.buf, dst, &r) };
        up.staging.push(st);
        Ok(())
    }

    /// Uploads RGBA8 mip chain into `img` (layer 0) and transitions it to shader-read.
    pub fn upload_image(&mut self, up: &mut Upload, img: &Image, mips: &[&[u8]]) -> Result<()> {
        let total: usize = mips.iter().map(|m| m.len()).sum();
        let mut st = self.buffer(total as u64, vk::BufferUsageFlags::TRANSFER_SRC, true)?;
        let mut regions = Vec::new();
        let mut off = 0usize;
        for (lvl, m) in mips.iter().enumerate() {
            st.bytes_mut()[off..off + m.len()].copy_from_slice(m);
            regions.push(vk::BufferImageCopy {
                buffer_offset: off as u64,
                image_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: lvl as u32,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                image_extent: vk::Extent3D {
                    width: (img.width >> lvl).max(1),
                    height: (img.height >> lvl).max(1),
                    depth: 1,
                },
                ..Default::default()
            });
            off += m.len();
        }
        let rng = range(vk::ImageAspectFlags::COLOR, img.mips, img.layers);
        barrier(
            &self.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        // SAFETY: image in TRANSFER_DST layout; staging lives until end_upload.
        unsafe {
            self.device.cmd_copy_buffer_to_image(
                up.cmd,
                st.buf,
                img.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            )
        };
        barrier(
            &self.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        up.staging.push(st);
        Ok(())
    }

    pub fn destroy(&mut self) {
        // SAFETY: device idle; all resources freed by the caller.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.upload_pool, None);
            self.alloc.destroy(&self.device);
            self.device.destroy_device(None);
            if let Some(s) = &self.surface_fn {
                s.destroy_surface(self.surface, None);
            }
            self.instance.destroy_instance(None);
        }
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
