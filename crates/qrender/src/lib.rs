//! qrender — Vulkan renderer (ash, dynamic loader). See docs/DESIGN.md "Renderer".
//!
//! Inputs are `qformats` data plus a per-frame [`RenderFrame`]; no SDL or
//! qclient dependency. Presentation goes through [`SurfaceProvider`]; without
//! one the renderer draws to an offscreen image readable with
//! [`Renderer::read_pixels`].

mod convert;
mod frame;
mod gpu;
mod pipelines;
pub mod types;
mod water;
mod world;

pub use types::*;

use ash::vk;
use convert::Palette;
use gpu::{Buffer, Gpu, Image};
use pipelines::{COLOR_FORMAT, DEPTH_FORMAT, LM_FORMAT, MAX_TEXTURES, Pipelines};
use qformats::bsp::{Bsp, TextureKind};
use qformats::image::Pic;
use qformats::{Mdl, Spr};

#[derive(Debug, Clone)]
pub struct Error(pub String);

impl Error {
    pub fn new(s: impl Into<String>) -> Self {
        Error(s.into())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

const FRAMES: usize = 2;
const DYN_SIZE: u64 = 48 << 20;
const OFS_LMDYN: u64 = 1024;
const OFS_VIS: u64 = 8192;
const MAX_VIS: u64 = 1 << 17;
const OFS_BUMP: u64 = OFS_VIS + MAX_VIS * 4;
const NO_TEX: u32 = u32::MAX;

struct FrameRes {
    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    acquire: vk::Semaphore,
    dynbuf: Buffer,
    set: vk::DescriptorSet,
    comp_set: vk::DescriptorSet,
    cull_set: vk::DescriptorSet,
}

struct Swapchain {
    sc: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    render_sems: Vec<vk::Semaphore>,
    vsync: bool,
    format: vk::Format,
    /// TRANSFER_SRC usage is supported (screenshots).
    readable: bool,
}

struct Targets {
    /// Single-sampled scene colour (MSAA resolve target), sampled by post.
    color: Image,
    /// Multisampled scene colour when MSAA is on.
    color_ms: Option<Image>,
    depth: Image,
    /// Scene size in pixels (output size × render scale).
    scene_w: u32,
    scene_h: u32,
    color_slot: u32,
    /// SSAO target (visibility, view depth) and per-object velocity target,
    /// both single-sampled at scene size, sampled by post.
    ao: Image,
    ao_slot: u32,
    vel: Image,
    vel_slot: u32,
    /// Headless output image + readback buffer.
    out: Option<(Image, Buffer)>,
}

struct Tex {
    /// None: slot reserved for a render target owned elsewhere.
    image: Option<Image>,
    level: bool,
    sampling: Sampling,
}

/// How a texture is sampled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sampling {
    /// World/model surfaces: follows the texture filter + anisotropy setting.
    World,
    /// Always nearest, repeat (UI pictures, colormap index textures).
    Nearest,
    /// Nearest, clamp to edge (sprites, skybox faces).
    Clamp,
    /// Bilinear, clamp to edge (smooth UI shapes).
    Linear,
}

struct WorldGpu {
    cpu: world::World,
    vbuf: Buffer,
    samples: Buffer,
    surfs: Buffer,
    materials: Buffer,
    /// Per BSP texture: kind and material flags.
    tex_info: Vec<(TextureKind, u32)>,
    submodel_ids: Vec<ModelId>,
    cull: WorldCull,
    /// Per-frame scratch reused by `gpu_world_draw`.
    scratch_bits: Vec<u32>,
    scratch_cmds: Vec<vk::DrawIndexedIndirectCommand>,
}

/// GPU world culling (cull.comp) resources.
struct WorldCull {
    marks: Buffer,
    surfs: Buffer,
    src: Buffer,
    surf_frame: Buffer,
    /// Output index buffer per frame in flight.
    out: Vec<Buffer>,
    /// Draw commands + lightmap dispatch args per frame in flight
    /// (device-local: cull.comp updates them with atomics).
    cmdbuf: Vec<Buffer>,
    nmarks: u32,
    /// Per draw command: batch texture and first index of its region.
    cmd_tex: Vec<u32>,
    cmd_first: Vec<u32>,
    /// Command index ranges: opaque, sky, then liquids of type 1..=4.
    cats: [std::ops::Range<u32>; 6],
    framecount: u32,
}

struct AliasModel {
    vbuf: Buffer,
    ibuf: Buffer,
    num_indices: u32,
    verts_base: u32,
    num_verts: u32,
    /// (first pose, pose count, intervals)
    frames: Vec<convert::FrameInfo>,
    /// Per skin group: images as [base, fb, cmap] slots + intervals.
    skins: Vec<(Vec<[u32; 3]>, Vec<f32>)>,
    scale: [f32; 3],
    scale_origin: [f32; 3],
    radius: f32,
    maxs_z: f32,
    holey: bool,
}

struct SpriteFrameGpu {
    images: Vec<(u32, [f32; 4])>, // slot, (left, right, down, up)
    intervals: Vec<f32>,
}

struct SpriteModel {
    kind: i32,
    frames: Vec<SpriteFrameGpu>,
    radius: f32,
}

enum Model {
    Brush(u32),
    Alias(Box<AliasModel>),
    Sprite(SpriteModel),
}

struct PicInfo {
    slot: u32,
    width: u32,
    height: u32,
}

/// The renderer. Not `Send`: keep it on the render thread.
pub struct Renderer {
    gpu: Gpu,
    pipes: Pipelines,
    desc_pool: vk::DescriptorPool,
    cmd_pool: vk::CommandPool,
    frames: Vec<FrameRes>,
    cur: usize,
    /// GPU pass timing (QSP_GPUTIMES=1): printed as averages on drop.
    gpu_times: Option<GpuTimes>,
    swap: Option<Swapchain>,
    surface_size: Box<dyn Fn() -> (u32, u32)>,
    width: u32,
    height: u32,
    targets: Option<Targets>,
    sampler_repeat: vk::Sampler,
    sampler_clamp: vk::Sampler,
    sampler_linear: vk::Sampler,
    /// Sampler for [`Sampling::World`] textures, rebuilt when filtering changes.
    sampler_world: vk::Sampler,
    /// (linear, anisotropy) of `sampler_world`.
    world_filter: (usize, f32, f32),
    /// Colour metric the palette LUT was built with (-1 = not built).
    lut_metric: i32,
    /// Scene MSAA sample count and render scale in effect.
    msaa: u32,
    render_scale: f32,
    /// Last frame's entity transforms by `RenderEntity::key` (motion blur).
    prev_xforms: std::collections::HashMap<u32, ([f32; 3], [f32; 3])>,
    prev_xform_time: f64,
    /// Ripples, splashes and wakes on liquids (r_waterripples/r_watersplash).
    water_fx: water::WaterFx,
    textures: Vec<Option<Tex>>,
    palette: Palette,
    palette_buf: Buffer,
    lightmap: Option<Image>,
    dummy: Buffer,
    world: Option<WorldGpu>,
    models: Vec<Model>,
    pics: Vec<PicInfo>,
    conchars: Option<PicId>,
    alias_cpu: Vec<u32>,
    alias_buf: Buffer,
    sky_layers: [u32; 2],
    skybox: Option<[u32; 6]>,
    buckets: Vec<Vec<u32>>,
    needs_resize: bool,
    frame_count: u64,
    /// Windowed screenshot: copy the next presented image to a buffer.
    capture_requested: bool,
    /// (buffer, width, height) of the last captured swapchain image.
    capture: Option<(Buffer, u32, u32)>,
}

/// Raw byte view of plain-old-data slices.
pub(crate) fn bytes_of<T: Copy>(s: &[T]) -> &[u8] {
    // SAFETY: T is Copy (repr(C) POD in this crate, no padding read as uninit for our types).
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

impl Renderer {
    /// Windowed renderer presenting to the provider's surface.
    pub fn new(surface: &dyn SurfaceProvider, palette: &Palette) -> Result<Renderer> {
        let mk = |inst: u64| surface.create_surface(inst);
        let gpu = Gpu::new(surface.instance_extensions(), Some(&mk))?;
        let (w, h) = surface.drawable_size();
        // The provider is borrowed; resize sizes come via `resize`.
        let mut r = Self::init(gpu, palette, w, h)?;
        r.create_swapchain(true)?;
        Ok(r)
    }

    /// Offscreen renderer (no surface); read results with [`Renderer::read_pixels`].
    pub fn headless(width: u32, height: u32, palette: &Palette) -> Result<Renderer> {
        let gpu = Gpu::new(Vec::new(), None)?;
        let mut r = Self::init(gpu, palette, width, height)?;
        r.create_targets()?;
        Ok(r)
    }

    pub fn device_name(&self) -> &str {
        &self.gpu.name
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn init(mut gpu: Gpu, palette: &Palette, width: u32, height: u32) -> Result<Renderer> {
        let pipes = Pipelines::new(&gpu, COLOR_FORMAT, 1)?;
        let dev = gpu.device.clone();
        let sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 8,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: (MAX_TEXTURES + 4) * FRAMES as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 64,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 4,
            },
        ];
        let pci = vk::DescriptorPoolCreateInfo::default()
            .max_sets(4 * FRAMES as u32)
            .pool_sizes(&sizes);
        // SAFETY: valid device.
        let desc_pool = unsafe { dev.create_descriptor_pool(&pci, None)? };
        let cpci = vk::CommandPoolCreateInfo::default()
            .queue_family_index(gpu.qfam)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: valid device.
        let cmd_pool = unsafe { dev.create_command_pool(&cpci, None)? };
        let mut frames = Vec::new();
        for _ in 0..FRAMES {
            let ai = vk::CommandBufferAllocateInfo::default()
                .command_pool(cmd_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let layouts = [pipes.set_layout, pipes.comp_set_layout, pipes.cull_set_layout];
            let dai = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(desc_pool)
                .set_layouts(&layouts);
            // SAFETY: valid device/pools.
            let (cmd, fence, acquire, sets) = unsafe {
                (
                    dev.allocate_command_buffers(&ai)?[0],
                    dev.create_fence(
                        &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )?,
                    dev.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                    dev.allocate_descriptor_sets(&dai)?,
                )
            };
            let dynbuf = gpu.buffer(
                DYN_SIZE,
                vk::BufferUsageFlags::UNIFORM_BUFFER
                    | vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::VERTEX_BUFFER
                    | vk::BufferUsageFlags::INDEX_BUFFER
                    | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_SRC,
                true,
            )?;
            frames.push(FrameRes {
                cmd,
                fence,
                acquire,
                dynbuf,
                set: sets[0],
                comp_set: sets[1],
                cull_set: sets[2],
            });
        }
        let sampler =
            |filter: vk::Filter, mode: vk::SamplerAddressMode, mip: vk::SamplerMipmapMode| {
                let ci = vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .mipmap_mode(mip)
                    .address_mode_u(mode)
                    .address_mode_v(mode)
                    .address_mode_w(mode)
                    .max_lod(vk::LOD_CLAMP_NONE);
                // SAFETY: valid device.
                unsafe { dev.create_sampler(&ci, None) }
            };
        let sampler_repeat = sampler(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::REPEAT,
            vk::SamplerMipmapMode::LINEAR,
        )?;
        let sampler_clamp = sampler(
            vk::Filter::NEAREST,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerMipmapMode::NEAREST,
        )?;
        let sampler_linear = sampler(
            vk::Filter::LINEAR,
            vk::SamplerAddressMode::CLAMP_TO_EDGE,
            vk::SamplerMipmapMode::NEAREST,
        )?;
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let mut palette_buf = gpu.buffer(1024 + convert::LUT_BYTES as u64, vk::BufferUsageFlags::STORAGE_BUFFER, true)?;
        let pal: Vec<u32> = palette
            .iter()
            .map(|c| c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | 0xff << 24)
            .collect();
        palette_buf.bytes_mut()[..1024].copy_from_slice(bytes_of(&pal));
        let dummy = gpu.buffer(256, storage, false)?;
        let alias_buf = gpu.buffer(4 << 20, storage, false)?;
        let lightmap = Some(Self::make_lightmap(&mut gpu, 16, 1)?);
        let mut r = Renderer {
            gpu,
            pipes,
            desc_pool,
            cmd_pool,
            frames,
            cur: 0,
            gpu_times: None,
            swap: None,
            surface_size: Box::new(move || (width, height)),
            width,
            height,
            targets: None,
            sampler_repeat,
            sampler_clamp,
            sampler_linear,
            sampler_world: sampler_repeat,
            world_filter: (usize::MAX, 0.0, 0.0),
            lut_metric: -1,
            msaa: 1,
            render_scale: 1.0,
            prev_xforms: Default::default(),
            prev_xform_time: 0.0,
            water_fx: {
                let mut w = water::WaterFx::default();
                w.set_palette(palette);
                w
            },
            textures: Vec::new(),
            palette: *palette,
            palette_buf,
            lightmap,
            dummy,
            world: None,
            models: Vec::new(),
            pics: Vec::new(),
            conchars: None,
            alias_cpu: Vec::new(),
            alias_buf,
            sky_layers: [NO_TEX; 2],
            skybox: None,
            buckets: Vec::new(),
            needs_resize: false,
            frame_count: 0,
            capture_requested: false,
            capture: None,
        };
        r.write_static_descriptors();
        Ok(r)
    }

    fn make_lightmap(gpu: &mut Gpu, size: u32, layers: u32) -> Result<Image> {
        let img = gpu.image(
            size,
            size,
            layers,
            1,
            LM_FORMAT,
            vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageAspectFlags::COLOR,
        )?;
        let up = gpu.begin_upload()?;
        let rng = gpu::range(vk::ImageAspectFlags::COLOR, 1, layers);
        gpu::barrier(
            &gpu.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
        let c = vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        };
        // SAFETY: image in GENERAL layout, recording command buffer.
        unsafe {
            gpu.device.cmd_clear_color_image(
                up.cmd,
                img.image,
                vk::ImageLayout::GENERAL,
                &c,
                &[rng],
            )
        };
        gpu::barrier(
            &gpu.device,
            up.cmd,
            img.image,
            rng,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::GENERAL,
        );
        gpu.end_upload(up)?;
        Ok(img)
    }

    fn idle(&self) {
        // SAFETY: valid device.
        let _ = unsafe { self.gpu.device.device_wait_idle() };
    }

    /// Rewrites every non-texture descriptor (after resource replacement; device idle).
    fn write_static_descriptors(&mut self) {
        let dev = &self.gpu.device;
        let Some(lightmap) = &self.lightmap else {
            return;
        };
        let (samples, surfs, materials) = match &self.world {
            Some(w) => (w.samples.buf, w.surfs.buf, w.materials.buf),
            None => (self.dummy.buf, self.dummy.buf, self.dummy.buf),
        };
        let whole = |b: vk::Buffer| {
            [vk::DescriptorBufferInfo {
                buffer: b,
                offset: 0,
                range: vk::WHOLE_SIZE,
            }]
        };
        let lm_img = [vk::DescriptorImageInfo {
            sampler: self.sampler_linear,
            image_view: lightmap.view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        let lm_storage = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: lightmap.view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        for (fi, f) in self.frames.iter().enumerate() {
            let d = self.dummy.buf;
            let cull_bufs = match &self.world {
                Some(w) => [
                    w.cull.marks.buf,
                    w.cull.surfs.buf,
                    w.cull.src.buf,
                    w.cull.out[fi].buf,
                    f.dynbuf.buf,
                    w.cull.surf_frame.buf,
                    w.cull.cmdbuf[fi].buf,
                ],
                None => [d, d, d, d, f.dynbuf.buf, d, d],
            };
            let cull_info: Vec<_> = cull_bufs.iter().map(|&b| whole(b)).collect();
            let cull_writes: Vec<_> = cull_info
                .iter()
                .enumerate()
                .map(|(i, info)| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(f.cull_set)
                        .dst_binding(i as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(info)
                })
                .collect();
            // SAFETY: device idle or sets not in use; all handles valid.
            unsafe { dev.update_descriptor_sets(&cull_writes, &[]) };
            let ubo = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: 0,
                range: 1024,
            }];
            let lmdyn = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: OFS_LMDYN,
                range: 4096,
            }];
            let vis = [vk::DescriptorBufferInfo {
                buffer: f.dynbuf.buf,
                offset: OFS_VIS,
                range: MAX_VIS * 4,
            }];
            let alias = whole(self.alias_buf.buf);
            let mats = whole(materials);
            let pal = whole(self.palette_buf.buf);
            let smp = whole(samples);
            let srf = whole(surfs);
            let w = |set, b, ty| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(b)
                    .descriptor_type(ty)
            };
            let sb = vk::DescriptorType::STORAGE_BUFFER;
            let writes = [
                w(f.set, 0, vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&ubo),
                w(f.set, 2, sb).buffer_info(&alias),
                w(f.set, 3, vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&lm_img),
                w(f.set, 4, sb).buffer_info(&mats),
                w(f.set, 5, sb).buffer_info(&pal),
                w(f.comp_set, 0, sb).buffer_info(&smp),
                w(f.comp_set, 1, sb).buffer_info(&srf),
                w(f.comp_set, 2, sb).buffer_info(&lmdyn),
                w(f.comp_set, 3, sb).buffer_info(&vis),
                w(f.comp_set, 4, vk::DescriptorType::STORAGE_IMAGE).image_info(&lm_storage),
            ];
            // SAFETY: device idle or sets not in use; all handles valid.
            unsafe { dev.update_descriptor_sets(&writes, &[]) };
        }
    }

    fn write_texture_descriptor(&self, slot: u32, view: vk::ImageView, sampler: vk::Sampler) {
        let info = [vk::DescriptorImageInfo {
            sampler,
            image_view: view,
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        }];
        for f in &self.frames {
            let w = [vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(1)
                .dst_array_element(slot)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&info)];
            // SAFETY: callers wait for device idle before registering textures.
            unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
        }
    }

    fn sampler_for(&self, s: Sampling) -> vk::Sampler {
        match s {
            Sampling::World => self.sampler_world,
            Sampling::Nearest => self.sampler_repeat,
            Sampling::Clamp => self.sampler_clamp,
            Sampling::Linear => self.sampler_linear,
        }
    }

    /// Highest MSAA sample count the device supports for scene targets.
    pub fn max_msaa(&self) -> u32 {
        self.gpu.max_samples
    }

    /// Highest anisotropy the device supports (1 = none).
    pub fn max_anisotropy(&self) -> f32 {
        self.gpu.max_anisotropy
    }

    /// Applies MSAA / render scale / texture filtering from `st`, rebuilding
    /// targets, pipelines or samplers when they changed.
    fn apply_settings(&mut self, st: &Settings) -> Result<()> {
        let msaa = match st.msaa {
            0 | 1 => 1,
            n => n.next_power_of_two().min(self.gpu.max_samples).max(1),
        };
        let scale = st.render_scale.clamp(0.25, 2.0);
        let supersample = st.supersample && self.gpu.sample_shading;
        if msaa != self.msaa || supersample != self.pipes.supersample {
            self.idle();
            self.msaa = msaa;
            self.pipes.supersample = supersample;
            let fmt = self.pipes.post_format;
            self.pipes.build(&self.gpu, fmt, msaa)?;
            self.destroy_targets();
        }
        if scale != self.render_scale {
            self.render_scale = scale;
            self.idle();
            self.destroy_targets();
        }
        // TexMgr_ApplySettings: gl_texturemode (r_texfilter picks the default),
        // anisotropy, lod bias; softemu 2/3 or lightmap banding force nearest
        // with linear mips and at least 8x anisotropy.
        let mut mode = st.texture_mode.unwrap_or(if st.texture_linear { 5 } else { 2 });
        let mut aniso = st.anisotropy;
        if mode != 0 && (st.softemu >= 2 || st.lightmap_banding > 0.0) {
            mode = 2;
            aniso = aniso.max(8.0);
        }
        let aniso = aniso.clamp(1.0, self.gpu.max_anisotropy);
        let bias = st.lod_bias.unwrap_or(if supersample && msaa > 1 { -(msaa as f32).log2() / 2.0 } else { 0.0 });
        let filter = (mode, aniso, bias);
        if filter != self.world_filter {
            self.idle();
            let (_, lin, mipped, lin_mips) = TEXTURE_MODES[mode];
            let f = if lin { vk::Filter::LINEAR } else { vk::Filter::NEAREST };
            let ci = vk::SamplerCreateInfo::default()
                .mag_filter(f)
                .min_filter(f)
                .mipmap_mode(if lin_mips { vk::SamplerMipmapMode::LINEAR } else { vk::SamplerMipmapMode::NEAREST })
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .mip_lod_bias(bias)
                .anisotropy_enable(aniso > 1.0 && mipped)
                .max_anisotropy(aniso)
                .max_lod(if mipped { vk::LOD_CLAMP_NONE } else { 0.0 });
            // SAFETY: valid device.
            let new = unsafe { self.gpu.device.create_sampler(&ci, None)? };
            let old = std::mem::replace(&mut self.sampler_world, new);
            if old != self.sampler_repeat {
                // SAFETY: device idle; descriptors are rewritten below.
                unsafe { self.gpu.device.destroy_sampler(old, None) };
            }
            self.world_filter = filter;
            for i in 0..self.textures.len() {
                if let Some(Tex { image: Some(img), sampling: Sampling::World, .. }) = &self.textures[i] {
                    self.write_texture_descriptor(i as u32, img.view, new);
                }
            }
        }
        // GLPalette_UpdateLookupTable: auto metric is OKLab except in banded
        // mode on maps without coloured light.
        if st.softemu > 0 {
            let lit = self.world.as_ref().is_some_and(|w| w.cpu.lit);
            let metric = if st.softemu_metric < 0 {
                if st.softemu != 3 || lit { 2 } else { 0 }
            } else {
                st.softemu_metric.min(2)
            };
            if metric != self.lut_metric {
                self.idle();
                let lut = convert::palette_lut(&self.palette, metric as u32);
                self.palette_buf.bytes_mut()[1024..1024 + lut.len()].copy_from_slice(&lut);
                self.lut_metric = metric;
            }
        }
        Ok(())
    }

    fn alloc_slot(&mut self) -> Result<u32> {
        if let Some(i) = self.textures.iter().position(|t| t.is_none()) {
            return Ok(i as u32);
        }
        if self.textures.len() as u32 >= MAX_TEXTURES {
            return Err(Error::new("out of texture slots"));
        }
        self.textures.push(None);
        Ok(self.textures.len() as u32 - 1)
    }

    /// Creates an RGBA8 texture from a mip chain (device must be idle).
    fn add_texture(
        &mut self,
        up: &mut gpu::Upload,
        w: u32,
        h: u32,
        mips: &[&[u8]],
        level: bool,
        sampling: Sampling,
    ) -> Result<u32> {
        let slot = self.alloc_slot()?;
        let img = self.gpu.image(
            w.max(1),
            h.max(1),
            1,
            mips.len().max(1) as u32,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageAspectFlags::COLOR,
        )?;
        self.gpu.upload_image(up, &img, mips)?;
        self.write_texture_descriptor(slot, img.view, self.sampler_for(sampling));
        self.textures[slot as usize] = Some(Tex {
            image: Some(img),
            level,
            sampling,
        });
        Ok(slot)
    }

    fn free_level(&mut self) {
        self.idle();
        for i in 0..self.textures.len() {
            if self.textures[i].as_ref().is_some_and(|t| t.level)
                && let Some(t) = self.textures[i].take()
                && let Some(img) = t.image
            {
                self.gpu.free_image(img);
            }
        }
        for m in self.models.drain(..) {
            if let Model::Alias(a) = m {
                self.gpu.free_buffer(a.vbuf);
                self.gpu.free_buffer(a.ibuf);
            }
        }
        if let Some(w) = self.world.take() {
            self.gpu.free_buffer(w.vbuf);
            self.gpu.free_buffer(w.samples);
            self.gpu.free_buffer(w.surfs);
            self.gpu.free_buffer(w.materials);
            let c = w.cull;
            for b in [c.marks, c.surfs, c.src, c.surf_frame].into_iter().chain(c.out).chain(c.cmdbuf) {
                self.gpu.free_buffer(b);
            }
        }
        self.alias_cpu.clear();
        self.sky_layers = [NO_TEX; 2];
        self.skybox = None;
    }

    /// Uploads a BSP's textures, appending to `materials`/`tex_info`.
    fn upload_bsp_textures(
        &mut self,
        up: &mut gpu::Upload,
        bsp: &Bsp,
        materials: &mut Vec<[u32; 4]>,
        tex_info: &mut Vec<(TextureKind, u32)>,
    ) -> Result<()> {
        let pal = self.palette;
        for t in &bsp.textures {
            let checker: Vec<u8> = (0..16 * 16)
                .map(|i| {
                    if ((i % 16) / 8 + (i / 128)) % 2 == 0 {
                        0
                    } else {
                        15
                    }
                })
                .collect();
            let fallback = qformats::bsp::MipTex {
                name: "notexture".into(),
                width: 16,
                height: 16,
                kind: TextureKind::Normal,
                mips: vec![checker],
                shift: 0,
            };
            let mut t = t.clone().unwrap_or_else(|| fallback.clone());
            if t.mips.is_empty() {
                t = qformats::bsp::MipTex {
                    name: t.name.clone(),
                    kind: t.kind,
                    ..fallback
                };
            }
            let flags = world::material_flags(&t.name, t.kind);
            if t.kind == TextureKind::Sky
                && self.sky_layers[0] == NO_TEX
                && let Some((w, h, solid, alpha)) = convert::sky_layers(&pal, &t, bsp.version == Some(qformats::bsp::BspVersion::Quake64))
            {
                let s = self.add_texture(up, w, h, &[&solid], true, Sampling::World)?;
                let a = self.add_texture(up, w, h, &[&alpha], true, Sampling::World)?;
                self.sky_layers = [s, a];
            }
            let (base, fb) = convert::world_texture(&pal, &t);
            let bm: Vec<&[u8]> = base.iter().map(|m| m.as_slice()).collect();
            let b = self.add_texture(up, t.width, t.height, &bm, true, Sampling::World)?;
            let f = match &fb {
                Some(fb) => {
                    let fm: Vec<&[u8]> = fb.iter().map(|m| m.as_slice()).collect();
                    self.add_texture(up, t.width, t.height, &fm, true, Sampling::World)?
                }
                None => NO_TEX,
            };
            materials.push([b, f, flags, 0]);
            tex_info.push((t.kind, flags));
        }
        if bsp.textures.is_empty() {
            materials.push([NO_TEX, NO_TEX, 0, 0]);
            tex_info.push((TextureKind::Normal, 0));
        }
        Ok(())
    }

    /// Loads a new map (frees all previous level models/textures). `lit` is a
    /// parsed `.lit` (RGB per mono sample). Returns ids for submodels `*1..`.
    pub fn load_world(&mut self, bsp: &Bsp, lit: Option<&[u8]>) -> Result<Vec<ModelId>> {
        Ok(self.load_level(bsp, lit, &[])?.0)
    }

    /// `load_world` plus external brush models (`maps/b_*.bsp`, each with
    /// its own textures and lightmaps, drawn like submodels). Returns
    /// (world submodel ids `*1..`, one id per entry of `brushes`).
    pub fn load_level(
        &mut self,
        bsp: &Bsp,
        lit: Option<&[u8]>,
        brushes: &[(&Bsp, Option<&[u8]>)],
    ) -> Result<(Vec<ModelId>, Vec<ModelId>)> {
        self.free_level();
        self.water_fx.reset();
        let mut cpu = world::World::build(bsp, lit);
        let world_subs = cpu.submodels.len();
        let mut up = self.gpu.begin_upload()?;
        let mut materials: Vec<[u32; 4]> = Vec::new();
        let mut tex_info = Vec::new();
        self.upload_bsp_textures(&mut up, bsp, &mut materials, &mut tex_info)?;
        let mut brush_ids = Vec::new();
        for (b, blit) in brushes {
            let first = cpu.add_brush(b, *blit, materials.len() as u32);
            self.upload_bsp_textures(&mut up, b, &mut materials, &mut tex_info)?;
            self.models.push(Model::Brush(first));
            brush_ids.push(ModelId(self.models.len() as u32 - 1));
        }
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let vbytes = bytes_of(&cpu.vertices);
        let vbuf = self.gpu.buffer(
            vbytes.len() as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, vbuf.buf, 0, vbytes)?;
        let samples = self
            .gpu
            .buffer((cpu.samples.len() * 4) as u64, usage, false)?;
        self.gpu
            .upload_buffer(&mut up, samples.buf, 0, bytes_of(&cpu.samples))?;
        let surfs = self.gpu.buffer(
            std::mem::size_of_val(cpu.lm_surfs.as_slice()) as u64,
            usage,
            false,
        )?;
        self.gpu
            .upload_buffer(&mut up, surfs.buf, 0, bytes_of(&cpu.lm_surfs))?;
        let mats = self
            .gpu
            .buffer((materials.len() * 16) as u64, usage, false)?;
        self.gpu
            .upload_buffer(&mut up, mats.buf, 0, bytes_of(&materials))?;
        let cull = self.build_world_cull(&mut up, &cpu, &tex_info)?;
        self.gpu.end_upload(up)?;
        if let Some(old) = self.lightmap.take() {
            self.gpu.free_image(old);
        }
        self.lightmap = Some(Self::make_lightmap(
            &mut self.gpu,
            world::ATLAS,
            cpu.atlas_layers,
        )?);
        let mut ids = Vec::new();
        for i in 1..world_subs {
            self.models.push(Model::Brush(i as u32));
            ids.push(ModelId(self.models.len() as u32 - 1));
        }
        self.buckets = vec![Vec::new(); tex_info.len()];
        self.world = Some(WorldGpu {
            cpu,
            vbuf,
            samples,
            surfs,
            materials: mats,
            tex_info,
            submodel_ids: ids.clone(),
            cull,
            scratch_bits: Vec::new(),
            scratch_cmds: Vec::new(),
        });
        self.write_static_descriptors();
        Ok((ids, brush_ids))
    }

    /// Uploads cull.comp's static inputs. Draw commands are one per batch
    /// texture used by world leaf surfaces, ordered opaque, sky, liquids by
    /// type, each owning a region of the output index buffer sized for all
    /// its surfaces.
    fn build_world_cull(
        &mut self,
        up: &mut gpu::Upload,
        cpu: &world::World,
        tex_info: &[(TextureKind, u32)],
    ) -> Result<WorldCull> {
        let (marks, mut surfs) = cpu.gpu_cull(|_| 0);
        let mut seen = vec![false; cpu.surfaces.len()];
        let mut cap: std::collections::BTreeMap<u32, u32> = Default::default();
        for &[_, si] in &marks {
            if !std::mem::replace(&mut seen[si as usize], true) {
                *cap.entry(cpu.slot_texture(si)).or_default() += cpu.surfaces[si as usize].num_indices;
            }
        }
        let cat = |t: u32| match tex_info.get(t as usize).map_or(TextureKind::Normal, |i| i.0) {
            TextureKind::Sky => 1,
            TextureKind::Liquid => 1 + (tex_info[t as usize].1 & 0xf).clamp(1, 4) as usize,
            _ => 0,
        };
        let mut order: Vec<u32> = cap.keys().copied().collect();
        order.sort_by_key(|&t| (cat(t), t));
        let cats: [std::ops::Range<u32>; 6] = std::array::from_fn(|c| {
            let a = order.iter().position(|&t| cat(t) >= c).unwrap_or(order.len()) as u32;
            let b = order.iter().position(|&t| cat(t) > c).unwrap_or(order.len()) as u32;
            a..b
        });
        let mut cmd_of = std::collections::HashMap::new();
        let mut cmd_first = Vec::with_capacity(order.len());
        let mut total = 0u32;
        for (i, &t) in order.iter().enumerate() {
            cmd_of.insert(t, i as u32);
            cmd_first.push(total);
            total += cap[&t];
        }
        for (si, s) in surfs.iter_mut().enumerate() {
            if seen[si] {
                s.center[3] = f32::from_bits(cmd_of[&cpu.slot_texture(si as u32)]);
            }
        }
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let mut make = |data: &[u8]| -> Result<Buffer> {
            let b = self.gpu.buffer(data.len() as u64, usage, false)?;
            self.gpu.upload_buffer(up, b.buf, 0, data)?;
            Ok(b)
        };
        let marks_b = make(bytes_of(&marks))?;
        let surfs_b = make(bytes_of(&surfs))?;
        let src_b = make(bytes_of(&cpu.indices))?;
        let frame_b = make(&vec![0u8; cpu.surfaces.len().max(1) * 4])?;
        let mut out = Vec::new();
        let mut cmdbuf = Vec::new();
        for _ in 0..FRAMES {
            cmdbuf.push(self.gpu.buffer(
                order.len() as u64 * 20 + 16 + cpu.leafs_len().div_ceil(32) as u64 * 4,
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST,
                false,
            )?);
            out.push(self.gpu.buffer(
                total.max(1) as u64 * 4,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                false,
            )?);
        }
        Ok(WorldCull {
            marks: marks_b,
            surfs: surfs_b,
            src: src_b,
            surf_frame: frame_b,
            out,
            cmdbuf,
            nmarks: marks.len() as u32,
            cmd_tex: order,
            cmd_first,
            cats,
            framecount: 0,
        })
    }

    /// Id of world submodel `*n` (n >= 1).
    pub fn submodel(&self, n: usize) -> Option<ModelId> {
        self.world
            .as_ref()?
            .submodel_ids
            .get(n.checked_sub(1)?)
            .copied()
    }

    /// Two-layer sky is taken from the map; this sets/clears a skybox
    /// (faces rt, bk, lf, ft, up, dn).
    pub fn set_skybox(&mut self, faces: Option<&[qformats::image::Rgba; 6]>) -> Result<()> {
        self.idle();
        if let Some(old) = self.skybox.take() {
            for s in old {
                if let Some(img) = self.textures[s as usize].take().and_then(|t| t.image) {
                    self.gpu.free_image(img);
                }
            }
        }
        let Some(faces) = faces else { return Ok(()) };
        let mut up = self.gpu.begin_upload()?;
        let mut slots = [NO_TEX; 6];
        for (i, f) in faces.iter().enumerate() {
            slots[i] = self.add_texture(&mut up, f.width, f.height, &[&f.pixels], true, Sampling::Clamp)?;
        }
        self.gpu.end_upload(up)?;
        self.skybox = Some(slots);
        Ok(())
    }

    /// Loads an alias model.
    pub fn load_alias(&mut self, mdl: &Mdl) -> Result<ModelId> {
        self.idle();
        let (verts, idx) = convert::alias_mesh(mdl);
        let (poses, frames) = convert::alias_poses(mdl);
        let holey = mdl.flags & qformats::mdl::MF_HOLEY != 0;
        let mut up = self.gpu.begin_upload()?;
        let mut skins = Vec::new();
        let pal = self.palette;
        for s in &mdl.skins {
            let mut imgs = Vec::new();
            for img in &s.images {
                let (b, fb, cm) = convert::alias_skin(&pal, img, holey);
                let (w, h) = (mdl.skin_width, mdl.skin_height);
                let bs = self.add_texture(&mut up, w, h, &[&b], true, Sampling::World)?;
                let fs = match fb {
                    Some(fb) => self.add_texture(&mut up, w, h, &[&fb], true, Sampling::World)?,
                    None => NO_TEX,
                };
                let cs = match cm {
                    Some(cm) => self.add_texture(&mut up, w, h, &[&cm], true, Sampling::Nearest)?,
                    None => NO_TEX,
                };
                imgs.push([bs, fs, cs]);
            }
            skins.push((imgs, s.intervals.clone()));
        }
        let usage = vk::BufferUsageFlags::TRANSFER_DST;
        let vb = bytes_of(&verts);
        let vbuf = self.gpu.buffer(
            vb.len() as u64,
            usage | vk::BufferUsageFlags::VERTEX_BUFFER,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, vbuf.buf, 0, vb)?;
        let ib = bytes_of(&idx);
        let ibuf = self.gpu.buffer(
            ib.len() as u64,
            usage | vk::BufferUsageFlags::INDEX_BUFFER,
            false,
        )?;
        self.gpu.upload_buffer(&mut up, ibuf.buf, 0, ib)?;
        let base = self.alias_cpu.len() as u32;
        self.alias_cpu.extend_from_slice(&poses);
        let need = (self.alias_cpu.len() * 4) as u64;
        let mut grow = false;
        if need > self.alias_buf.size {
            let nb = self.gpu.buffer(
                (need * 2).max(4 << 20),
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                false,
            )?;
            self.gpu
                .upload_buffer(&mut up, nb.buf, 0, bytes_of(&self.alias_cpu))?;
            let old = std::mem::replace(&mut self.alias_buf, nb);
            self.gpu.end_upload(up)?;
            self.gpu.free_buffer(old);
            grow = true;
        } else {
            let dst = self.alias_buf.buf;
            self.gpu
                .upload_buffer(&mut up, dst, base as u64 * 4, bytes_of(&poses))?;
            self.gpu.end_upload(up)?;
        }
        if grow {
            self.write_static_descriptors();
        }
        let mut maxs_z: f32 = 0.0;
        let mut radius: f32 = mdl.bounding_radius;
        for f in &mdl.frames {
            let a = mdl.decode(f.bboxmin);
            let b = mdl.decode(f.bboxmax);
            maxs_z = maxs_z.max(b[2]);
            radius = radius
                .max(world::dot(a, a).sqrt())
                .max(world::dot(b, b).sqrt());
        }
        self.models.push(Model::Alias(Box::new(AliasModel {
            vbuf,
            ibuf,
            num_indices: idx.len() as u32,
            verts_base: base,
            num_verts: mdl.num_verts as u32,
            frames,
            skins,
            scale: mdl.scale,
            scale_origin: mdl.scale_origin,
            radius,
            maxs_z,
            holey,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Loads a sprite (SPR v1 palette or SPR32).
    pub fn load_sprite(&mut self, spr: &Spr) -> Result<ModelId> {
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let pal = self.palette;
        let mut frames = Vec::new();
        for f in &spr.frames {
            let mut images = Vec::new();
            for im in &f.images {
                let rgba = if spr.is_rgba() {
                    im.pixels.clone()
                } else {
                    convert::to_rgba(&pal, &im.pixels, Some(255))
                };
                let slot = self.add_texture(&mut up, im.width, im.height, &[&rgba], true, Sampling::Clamp)?;
                let left = im.origin[0] as f32;
                let up_ = im.origin[1] as f32;
                images.push((
                    slot,
                    [left, left + im.width as f32, up_ - im.height as f32, up_],
                ));
            }
            frames.push(SpriteFrameGpu {
                images,
                intervals: f.intervals.clone(),
            });
        }
        self.gpu.end_upload(up)?;
        self.models.push(Model::Sprite(SpriteModel {
            kind: spr.kind,
            frames,
            radius: spr.bounding_radius,
        }));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Registers a 2D picture (index 255 transparent). Persists across maps.
    pub fn register_pic(&mut self, pic: &Pic) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, &pic.pixels, Some(255));
        self.register_pic_rgba(pic.width, pic.height, &rgba)
    }

    pub fn register_pic_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Nearest)
    }

    /// Like [`register_pic_rgba`](Self::register_pic_rgba) but bilinear
    /// filtered (smooth shapes drawn larger than their texels).
    pub fn register_pic_rgba_linear(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Linear)
    }

    fn register_pic_sampled(&mut self, width: u32, height: u32, rgba: &[u8], sampling: Sampling) -> Result<PicId> {
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let slot = self.add_texture(&mut up, width, height, &[rgba], false, sampling)?;
        self.gpu.end_upload(up)?;
        self.pics.push(PicInfo {
            slot,
            width,
            height,
        });
        Ok(PicId(self.pics.len() as u32 - 1))
    }

    /// Sets conchars (128×128 indices, 0 transparent) used by [`Draw2D::Char`].
    pub fn set_conchars(&mut self, pixels: &[u8]) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, pixels, Some(0));
        let id = self.register_pic_rgba(128, 128, &rgba)?;
        self.conchars = Some(id);
        Ok(id)
    }

    /// Replaces a picture's texels (same size it was registered with), for
    /// CPU-built atlases such as the UI text cache. Waits for the GPU.
    pub fn update_pic(&mut self, pic: PicId, rgba: &[u8]) -> Result<()> {
        let Some(&PicInfo { slot, width, height }) = self.pics.get(pic.0 as usize) else {
            return Err(Error::new("no such picture"));
        };
        self.idle();
        let mut up = self.gpu.begin_upload()?;
        let img = self.gpu.image(
            width,
            height,
            1,
            1,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            vk::ImageAspectFlags::COLOR,
        )?;
        self.gpu.upload_image(&mut up, &img, &[rgba])?;
        self.gpu.end_upload(up)?;
        let sampling = self.textures[slot as usize].as_ref().map_or(Sampling::Nearest, |t| t.sampling);
        self.write_texture_descriptor(slot, img.view, self.sampler_for(sampling));
        let old = self.textures[slot as usize].replace(Tex { image: Some(img), level: false, sampling });
        if let Some(img) = old.and_then(|t| t.image) {
            self.gpu.free_image(img);
        }
        Ok(())
    }

    pub fn pic_size(&self, pic: PicId) -> Option<(u32, u32)> {
        self.pics.get(pic.0 as usize).map(|p| (p.width, p.height))
    }

    /// Call when the window drawable size changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        if (width, height) != (self.width, self.height) {
            self.surface_size = Box::new(move || (width, height));
            self.needs_resize = true;
        }
    }

    fn destroy_targets(&mut self) {
        if let Some(t) = self.targets.take() {
            self.textures[t.color_slot as usize] = None;
            self.textures[t.ao_slot as usize] = None;
            self.textures[t.vel_slot as usize] = None;
            self.gpu.free_image(t.ao);
            self.gpu.free_image(t.vel);
            self.gpu.free_image(t.color);
            self.gpu.free_image(t.depth);
            if let Some(m) = t.color_ms {
                self.gpu.free_image(m);
            }
            if let Some((i, b)) = t.out {
                self.gpu.free_image(i);
                self.gpu.free_buffer(b);
            }
        }
    }

    fn create_targets(&mut self) -> Result<()> {
        self.idle();
        self.destroy_targets();
        let (w, h) = (self.width.max(1), self.height.max(1));
        let scale = self.render_scale;
        let sw = ((w as f32 * scale).round() as u32).clamp(1, 16384);
        let shh = ((h as f32 * scale).round() as u32).clamp(1, 16384);
        let color = self.gpu.image(
            sw,
            shh,
            1,
            1,
            COLOR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let color_ms = if self.msaa > 1 {
            Some(self.gpu.image_ms(
                sw,
                shh,
                1,
                1,
                COLOR_FORMAT,
                // sampled by the SSAO pass (scene alpha masks the sky)
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
                self.msaa,
            )?)
        } else {
            None
        };
        let depth = self.gpu.image_ms(
            sw,
            shh,
            1,
            1,
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
            self.msaa,
        )?;
        // Soft particles read scene depth with texelFetch while the depth
        // attachment is read-only.
        let dinfo = [vk::DescriptorImageInfo {
            sampler: self.sampler_clamp,
            image_view: depth.view,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        }];
        for f in &self.frames {
            let w = [vk::WriteDescriptorSet::default()
                .dst_set(f.set)
                .dst_binding(if self.msaa > 1 { 7 } else { 6 })
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&dinfo)];
            // SAFETY: device idle (create_targets); handles valid.
            unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
            if let Some(ms) = &color_ms {
                let cinfo = [vk::DescriptorImageInfo {
                    sampler: self.sampler_clamp,
                    image_view: ms.view,
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                }];
                let w = [vk::WriteDescriptorSet::default()
                    .dst_set(f.set)
                    .dst_binding(8)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&cinfo)];
                // SAFETY: as above.
                unsafe { self.gpu.device.update_descriptor_sets(&w, &[]) };
            }
        }
        let slot = self.alloc_slot()?;
        let post_sampler = if (scale - 1.0).abs() < 1e-3 {
            self.sampler_clamp
        } else {
            self.sampler_linear
        };
        self.write_texture_descriptor(slot, color.view, post_sampler);
        let out = if self.swap.is_none() {
            let img = self.gpu.image(
                w,
                h,
                1,
                1,
                COLOR_FORMAT,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                vk::ImageAspectFlags::COLOR,
            )?;
            let buf =
                self.gpu
                    .buffer((w * h * 4) as u64, vk::BufferUsageFlags::TRANSFER_DST, true)?;
            Some((img, buf))
        } else {
            None
        };
        self.textures[slot as usize] = Some(Tex {
            image: None,
            level: false,
            sampling: Sampling::Clamp,
        });
        let fx_target = |r: &mut Self, format| -> Result<(Image, u32)> {
            let img = r.gpu.image(
                sw,
                shh,
                1,
                1,
                format,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::ImageAspectFlags::COLOR,
            )?;
            let slot = r.alloc_slot()?;
            r.write_texture_descriptor(slot, img.view, r.sampler_clamp);
            r.textures[slot as usize] = Some(Tex { image: None, level: false, sampling: Sampling::Clamp });
            Ok((img, slot))
        };
        let (ao, ao_slot) = fx_target(self, pipelines::AO_FORMAT)?;
        let (vel, vel_slot) = fx_target(self, pipelines::VEL_FORMAT)?;
        self.targets = Some(Targets {
            color,
            color_ms,
            depth,
            scene_w: sw,
            scene_h: shh,
            color_slot: slot,
            ao,
            ao_slot,
            vel,
            vel_slot,
            out,
        });
        Ok(())
    }

    fn create_swapchain(&mut self, vsync: bool) -> Result<()> {
        self.idle();
        let (Some(sf), Some(scf)) = (&self.gpu.surface_fn, &self.gpu.swapchain_fn) else {
            return Ok(());
        };
        let pd = self.gpu.pdev;
        let surface = self.gpu.surface;
        // SAFETY: valid pdev/surface.
        let caps = unsafe { sf.get_physical_device_surface_capabilities(pd, surface)? };
        // SAFETY: valid pdev/surface.
        let formats = unsafe { sf.get_physical_device_surface_formats(pd, surface)? };
        // SAFETY: valid pdev/surface.
        let modes = unsafe { sf.get_physical_device_surface_present_modes(pd, surface)? };
        let fmt = formats
            .iter()
            .find(|f| {
                matches!(
                    f.format,
                    vk::Format::B8G8R8A8_UNORM | vk::Format::R8G8B8A8_UNORM
                )
            })
            .copied()
            .unwrap_or(formats[0]);
        let mode = if vsync {
            vk::PresentModeKHR::FIFO
        } else if modes.contains(&vk::PresentModeKHR::MAILBOX) {
            vk::PresentModeKHR::MAILBOX
        } else if modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
            vk::PresentModeKHR::IMMEDIATE
        } else {
            vk::PresentModeKHR::FIFO
        };
        let (w, h) = (self.surface_size)();
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: w.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: h.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        let mut count = caps.min_image_count + 1;
        if caps.max_image_count > 0 {
            count = count.min(caps.max_image_count);
        }
        let old = self
            .swap
            .as_ref()
            .map_or(vk::SwapchainKHR::null(), |s| s.sc);
        let ci = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(count)
            .image_format(fmt.format)
            .image_color_space(fmt.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | (caps.supported_usage_flags & vk::ImageUsageFlags::TRANSFER_SRC),
            )
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(mode)
            .clipped(true)
            .old_swapchain(old);
        // SAFETY: valid surface and create info.
        let sc = unsafe { scf.create_swapchain(&ci, None)? };
        if let Some(s) = self.swap.take() {
            self.destroy_swapchain(s);
        }
        let scf = self
            .gpu
            .swapchain_fn
            .as_ref()
            .ok_or_else(|| Error::new("no swapchain"))?;
        // SAFETY: valid swapchain.
        let images = unsafe { scf.get_swapchain_images(sc)? };
        let mut views = Vec::new();
        let mut render_sems = Vec::new();
        for &img in &images {
            let vci = vk::ImageViewCreateInfo::default()
                .image(img)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(fmt.format)
                .subresource_range(gpu::range(vk::ImageAspectFlags::COLOR, 1, 1));
            // SAFETY: valid swapchain image.
            unsafe {
                views.push(self.gpu.device.create_image_view(&vci, None)?);
                render_sems.push(
                    self.gpu
                        .device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                );
            }
        }
        self.swap = Some(Swapchain {
            sc,
            images,
            views,
            render_sems,
            vsync,
            format: fmt.format,
            readable: caps
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC),
        });
        if self.pipes.post_format != fmt.format {
            self.pipes.build(&self.gpu, fmt.format, self.msaa)?;
        }
        self.width = extent.width;
        self.height = extent.height;
        self.create_targets()?;
        self.needs_resize = false;
        Ok(())
    }

    fn destroy_swapchain(&self, s: Swapchain) {
        // SAFETY: device idle; resources owned by `s`.
        unsafe {
            for v in s.views {
                self.gpu.device.destroy_image_view(v, None);
            }
            for sem in s.render_sems {
                self.gpu.device.destroy_semaphore(sem, None);
            }
            if let Some(f) = &self.gpu.swapchain_fn {
                f.destroy_swapchain(s.sc, None);
            }
        }
        let _ = s.images;
    }

    /// Windowed mode: the next `render` copies the image it presents so
    /// `read_pixels` can return it. No-op when headless.
    pub fn request_capture(&mut self) {
        self.capture_requested = self.swap.is_some();
    }

    /// Waits for the last frame and returns its RGBA8 pixels: the offscreen
    /// image when headless, else the swapchain image captured by the last
    /// `render` after `request_capture`.
    pub fn read_pixels(&mut self) -> Result<Vec<u8>> {
        self.idle();
        if let Some(s) = &self.swap {
            let bgra = matches!(
                s.format,
                vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
            );
            let (mut buf, w, h) = self
                .capture
                .take()
                .ok_or_else(|| Error::new("no captured frame (call request_capture before render)"))?;
            let mut px = buf.bytes_mut()[..(w * h * 4) as usize].to_vec();
            self.gpu.free_buffer(buf);
            if bgra {
                px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
            }
            px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
            return Ok(px);
        }
        let (w, h) = (self.width, self.height);
        let t = self
            .targets
            .as_mut()
            .ok_or_else(|| Error::new("no targets"))?;
        let (_, buf) = t.out.as_mut().ok_or_else(|| Error::new("not headless"))?;
        Ok(buf.bytes_mut()[..(w * h * 4) as usize].to_vec())
    }

    /// Light level at a point (R_LightPoint), RGB 0..255+.
    pub fn light_point(&self, p: Vec3, lightstyles: &[i32]) -> Vec3 {
        self.world
            .as_ref()
            .and_then(|w| w.cpu.light_point(p, lightstyles))
            .unwrap_or([0.0; 3])
    }
}

/// Timestamp queries around each render stage, per frame in flight.
struct GpuTimes {
    pool: vk::QueryPool,
    written: [bool; FRAMES],
    sum_ns: [f64; GPU_STAGES.len()],
    frames: u64,
}

/// Stage names; timestamp k+1 ends stage k (timestamp 0 is the start).
const GPU_STAGES: [&str; 9] =
    ["cull", "lightmap", "targets", "world", "models", "translucent", "particles+vm", "post", "ui+present"];
const GPU_TS: u32 = GPU_STAGES.len() as u32 + 1;

impl Renderer {
    fn init_gpu_times(&mut self) {
        if std::env::var_os("QSP_GPUTIMES").is_none() || self.gpu_times.is_some() {
            return;
        }
        let ci = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(GPU_TS * FRAMES as u32);
        // SAFETY: valid device.
        let Ok(pool) = (unsafe { self.gpu.device.create_query_pool(&ci, None) }) else { return };
        self.gpu_times = Some(GpuTimes { pool, written: [false; FRAMES], sum_ns: [0.0; GPU_STAGES.len()], frames: 0 });
    }

    /// Accumulates frame `fi`'s timestamps (its fence has been waited on).
    fn collect_gpu_times(&mut self, fi: usize) {
        let period = self.gpu.timestamp_period as f64;
        let Some(t) = &mut self.gpu_times else { return };
        if !std::mem::take(&mut t.written[fi]) {
            return;
        }
        let mut v = [0u64; GPU_TS as usize];
        // SAFETY: queries written by a completed submission.
        let ok = unsafe {
            self.gpu.device.get_query_pool_results(
                t.pool,
                fi as u32 * GPU_TS,
                &mut v,
                vk::QueryResultFlags::TYPE_64,
            )
        };
        if ok.is_ok() {
            for k in 0..GPU_STAGES.len() {
                t.sum_ns[k] += v[k + 1].saturating_sub(v[k]) as f64 * period;
            }
            t.frames += 1;
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Some(t) = self.gpu_times.take() {
            let n = t.frames.max(1) as f64;
            let total: f64 = t.sum_ns.iter().sum();
            eprintln!("GPU times over {} frames (avg us/frame):", t.frames);
            for (k, name) in GPU_STAGES.iter().enumerate() {
                eprintln!("  {name:<14} {:8.1}", t.sum_ns[k] / n / 1000.0);
            }
            eprintln!("  {:<14} {:8.1}", "total", total / n / 1000.0);
            self.idle();
            // SAFETY: device idle; the pool is no longer used.
            unsafe { self.gpu.device.destroy_query_pool(t.pool, None) };
        }
        self.drop_inner();
    }
}

impl Renderer {
    fn drop_inner(&mut self) {
        self.idle();
        self.free_level();
        self.destroy_targets();
        if let Some((b, _, _)) = self.capture.take() {
            self.gpu.free_buffer(b);
        }
        if let Some(s) = self.swap.take() {
            self.destroy_swapchain(s);
        }
        let dev = self.gpu.device.clone();
        for i in 0..self.textures.len() {
            if let Some(img) = self.textures[i].take().and_then(|t| t.image) {
                self.gpu.free_image(img);
            }
        }
        let frames: Vec<FrameRes> = std::mem::take(&mut self.frames);
        for f in frames {
            // SAFETY: device idle.
            unsafe {
                dev.destroy_fence(f.fence, None);
                dev.destroy_semaphore(f.acquire, None);
            }
            self.gpu.free_buffer(f.dynbuf);
        }
        for b in [&self.palette_buf, &self.dummy, &self.alias_buf] {
            self.gpu.free_buffer_ref(b);
        }
        if let Some(lm) = self.lightmap.take() {
            self.gpu.free_image(lm);
        }
        // SAFETY: device idle; handles owned by self.
        unsafe {
            dev.destroy_sampler(self.sampler_repeat, None);
            dev.destroy_sampler(self.sampler_clamp, None);
            dev.destroy_sampler(self.sampler_linear, None);
            if self.sampler_world != self.sampler_repeat {
                dev.destroy_sampler(self.sampler_world, None);
            }
            dev.destroy_descriptor_pool(self.desc_pool, None);
            dev.destroy_command_pool(self.cmd_pool, None);
        }
        self.pipes.destroy(&dev);
        self.gpu.destroy();
    }
}

/// Loads `gfx/env/<name>{rt,bk,lf,ft,up,dn}.tga` (or .pcx) for [`Renderer::set_skybox`].
pub fn load_skybox(fs: &qcore::fs::Fs, name: &str) -> Option<[qformats::image::Rgba; 6]> {
    let mut out = Vec::with_capacity(6);
    for suf in ["rt", "bk", "lf", "ft", "up", "dn"] {
        let base = format!("gfx/env/{name}{suf}");
        let img = if let Some(d) = fs.open(&format!("{base}.tga")) {
            qformats::image::parse_tga(&d).ok()?
        } else {
            let d = fs.open(&format!("{base}.pcx"))?;
            let (pic, pal) = qformats::image::parse_pcx(&d).ok()?;
            qformats::image::Rgba {
                width: pic.width,
                height: pic.height,
                pixels: convert::to_rgba(&pal, &pic.pixels, None),
            }
        };
        out.push(img);
    }
    out.try_into().ok()
}
