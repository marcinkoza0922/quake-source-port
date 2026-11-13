//! qrender — Vulkan renderer (ash, dynamic loader). See docs/DESIGN.md "Renderer".
//!
//! Inputs are `qformats` data plus a per-frame [`RenderFrame`]; no SDL or
//! qclient dependency. Presentation goes through [`SurfaceProvider`]; without
//! one the renderer draws to an offscreen image readable with
//! [`Renderer::read_pixels`].

mod debug_visibility;
mod convert;
mod frame;
mod gpu;
mod pipelines;
mod slang;
pub use slang::ShaderParam;
pub mod types;
pub(crate) mod water;
mod world;
pub mod portals;

pub use types::*;

use ash::vk;
use convert::Palette;
use gpu::{Buffer, Gpu, Image};
use pipelines::{COLOR_FORMAT, DEPTH_FORMAT, LM_FORMAT, MAX_TEXTURES, Pipelines};
use qformats::bsp::{Bsp, TextureKind};
use qformats::image::Pic;
use qformats::{Mdl, Spr};
#[path = "lib/targets.rs"]
mod targets;
#[path = "lib/assets.rs"]
mod assets;
#[path = "lib/world_load.rs"]
mod world_load;
#[path = "lib/settings.rs"]
mod settings;
#[path = "lib/descriptors.rs"]
mod descriptors;
#[path = "lib/init.rs"]
mod init;
#[path = "lib/resources.rs"]
mod resources;
use resources::*;

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
const OFS_LMDYN: u64 = 2048;
const OFS_VIS: u64 = 8192;
const MAX_VIS: u64 = 1 << 17;
const OFS_BUMP: u64 = OFS_VIS + MAX_VIS * 4;
const NO_TEX: u32 = u32::MAX;

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
    isolated_portal: Option<usize>,
    isolated_distance: Option<f32>,
    hidden_entities: std::collections::HashSet<u32>,
    models: Vec<Model>,
    pics: Vec<PicInfo>,
    /// Soft round blob for water spray puffs (big underwater-blast splashes).
    splash_pic: Option<PicId>,
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
    /// Loaded slang preset (r_shader_preset).
    slang: Option<slang::Slang>,
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

    fn idle(&self) {
        // SAFETY: valid device.
        let _ = unsafe { self.gpu.device.device_wait_idle() };
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
        self.drop_shader();
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
