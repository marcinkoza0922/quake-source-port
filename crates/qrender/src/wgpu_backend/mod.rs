//! wgpu backend of [`Renderer`] (web via WebGPU, natively with the `wgpu`
//! feature). Same public API as the Vulkan backend; the scene is drawn with
//! CPU-built draw lists (world from the PVS, alias poses lerped on the CPU,
//! lightmaps built on the CPU) and WGSL shaders in `shaders/`. Advanced
//! effects of the Vulkan backend (MSAA, SSAO, motion blur, portals, slang
//! presets, softemu, goggles, water ripples) are not drawn.

mod capture;
mod ents;
mod frame;
mod level;
mod lightmap;
mod math;
mod models;
mod pipes;
mod scene;
mod textures;

use crate::convert::Palette;
use crate::{Error, GpuViewTiming, PicId, Result, ShaderParam, SurfaceProvider, Vec3};
use std::collections::HashMap;
use std::path::Path;

pub(super) const NO_TEX: u32 = u32::MAX;
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Scene targets at render-scaled size, plus the headless output and its readback.
struct Targets {
    /// Scene colour/depth size (output size × render scale).
    scene: (u32, u32),
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    /// Headless: (output texture, readback buffer, padded bytes per row).
    out: Option<(wgpu::Texture, wgpu::Buffer, u32)>,
}

/// The renderer. Not `Send`: keep it on the render thread.
pub struct Renderer {
    /// Force GPU timestamps on (no-op in this backend).
    pub gpu_times_wanted: bool,
    /// CPU time spent waiting for the GPU last frame (ns); always 0 here.
    pub fence_wait_ns: u64,
    dev: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
    surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    width: u32,
    height: u32,
    pipes: pipes::Pipes,
    samplers: textures::Samplers,
    textures: Vec<Option<textures::Tex>>,
    /// 1x1 white (missing base) and transparent black (missing extra layers).
    white: wgpu::TextureView,
    black: wgpu::TextureView,
    bind_groups: HashMap<[u32; 3], wgpu::BindGroup>,
    sky_group: Option<wgpu::BindGroup>,
    palette: Palette,
    palette_view: wgpu::TextureView,
    lightmap: lightmap::Lightmaps,
    world: Option<level::WorldRes>,
    models: Vec<models::Model>,
    pics: Vec<textures::PicInfo>,
    conchars: Option<PicId>,
    /// Round dot for classic particles.
    dot_tex: u32,
    sky_layers: [u32; 2],
    skybox: Option<[u32; 6]>,
    targets: Option<Targets>,
    bufs: frame::Buffers,
    /// Linear world/model texture filtering in effect.
    world_linear: bool,
    needs_resize: bool,
    /// The last headless frame was copied to the readback buffer.
    frame_ready: bool,
    /// Scene resolution relative to the output (r_scale), 0.25..=2.
    render_scale: f32,
    /// Copy the next presented frame for [`Renderer::take_capture`].
    capture_req: bool,
    /// A presented frame being copied and mapped for reading.
    capture: Option<capture::Pending>,
}

impl Renderer {
    /// Windowed rendering through a Vulkan surface provider is not supported
    /// by this backend (the web uses [`Renderer::new_web`]).
    pub fn new(_surface: &dyn SurfaceProvider, _palette: &Palette) -> Result<Renderer> {
        Err(Error::new("the wgpu renderer has no native window support; run with -headless"))
    }

    /// Offscreen renderer (no surface); read results with [`Renderer::read_pixels`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless(width: u32, height: u32, palette: &Palette) -> Result<Renderer> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        if std::env::var_os("WGPU_BACKEND").is_none() {
            desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
        }
        let instance = wgpu::Instance::new(desc);
        pollster::block_on(Self::create(instance, None, width, height, palette))
    }

    /// Offscreen rendering cannot block for readback in the browser.
    #[cfg(target_arch = "wasm32")]
    pub fn headless(_width: u32, _height: u32, _palette: &Palette) -> Result<Renderer> {
        Err(Error::new("headless rendering is not available on the web"))
    }

    /// Renderer presenting to a canvas through WebGPU.
    #[cfg(target_arch = "wasm32")]
    pub async fn new_web(canvas: web_sys::HtmlCanvasElement, palette: &Palette) -> Result<Renderer> {
        let (w, h) = (canvas.width().max(1), canvas.height().max(1));
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(desc);
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| Error::new(format!("WebGPU surface: {e}")))?;
        Self::create(instance, Some(surface), w, h, palette).await
    }

    async fn create(
        instance: wgpu::Instance,
        surface: Option<wgpu::Surface<'static>>,
        width: u32,
        height: u32,
        palette: &Palette,
    ) -> Result<Renderer> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface.as_ref(),
                ..Default::default()
            })
            .await
            .map_err(|e| Error::new(format!("no wgpu adapter: {e}")))?;
        let name = format!("{} (wgpu {:?})", adapter.get_info().name, adapter.get_info().backend);
        let (dev, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("qrender"),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| Error::new(format!("wgpu device: {e}")))?;
        let (width, height) = (width.max(1), height.max(1));
        let surface = match surface {
            Some(s) => {
                let caps = s.get_capabilities(&adapter);
                let format = caps
                    .formats
                    .iter()
                    .copied()
                    .find(|f| !f.is_srgb())
                    .or(caps.formats.first().copied())
                    .ok_or_else(|| Error::new("surface has no formats"))?;
                // COPY_SRC lets screenshots and save thumbnails read the frame back
                // (a WebGPU canvas accepts any usage, but wgpu's browser
                // backend reports only RENDER_ATTACHMENT, so ask regardless)
                let copy = if cfg!(target_arch = "wasm32") { wgpu::TextureUsages::COPY_SRC } else { caps.usages & wgpu::TextureUsages::COPY_SRC };
                let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | copy;
                let cfg = wgpu::SurfaceConfiguration {
                    usage,
                    format,
                    color_space: Default::default(),
                    width,
                    height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
                    view_formats: Vec::new(),
                };
                s.configure(&dev, &cfg);
                Some((s, cfg))
            }
            None => None,
        };
        let out_format = surface.as_ref().map_or(wgpu::TextureFormat::Rgba8Unorm, |s| s.1.format);
        let pipes = pipes::Pipes::new(&dev, out_format);
        let samplers = textures::Samplers::new(&dev, false);
        let white = textures::solid(&dev, &queue, [255; 4]);
        let black = textures::solid(&dev, &queue, [0; 4]);
        let palette_view = textures::palette_texture(&dev, &queue, palette);
        let lightmap = lightmap::Lightmaps::new(&dev, &queue, 1);
        let mut r = Renderer {
            gpu_times_wanted: false,
            fence_wait_ns: 0,
            dev,
            queue,
            name,
            surface,
            width,
            height,
            pipes,
            samplers,
            textures: Vec::new(),
            white,
            black,
            bind_groups: HashMap::new(),
            sky_group: None,
            palette: *palette,
            palette_view,
            lightmap,
            world: None,
            models: Vec::new(),
            pics: Vec::new(),
            conchars: None,
            dot_tex: NO_TEX,
            sky_layers: [NO_TEX; 2],
            skybox: None,
            targets: None,
            bufs: frame::Buffers::default(),
            world_linear: false,
            needs_resize: true,
            frame_ready: false,
            render_scale: 1.0,
            capture_req: false,
            capture: None,
        };
        let dot = textures::particle_dot();
        r.dot_tex = r.add_texture(8, 8, &[&dot], false, textures::Sampling::Linear)?;
        Ok(r)
    }

    pub fn device_name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Call when the window drawable size changes.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) != (self.width, self.height) {
            self.width = width;
            self.height = height;
            self.needs_resize = true;
        }
    }

    pub fn pic_size(&self, pic: PicId) -> Option<(u32, u32)> {
        self.pics.get(pic.0 as usize).map(|p| (p.width, p.height))
    }

    /// Copies the next presented frame for [`Renderer::take_capture`]
    /// (headless renderers copy every frame). Ignored while a capture is
    /// still being read back.
    pub fn request_capture(&mut self) {
        self.capture_req = self.surface.is_some() && self.capture.is_none();
    }

    /// Waits for the last frame and returns its RGBA8 pixels (headless only).
    pub fn read_pixels(&mut self) -> Result<Vec<u8>> {
        self.read_back()
    }

    /// Light level at a point (R_LightPoint), RGB 0..255+.
    pub fn light_point(&self, p: Vec3, lightstyles: &[i32]) -> Vec3 {
        self.world
            .as_ref()
            .and_then(|w| w.cpu.light_point(p, lightstyles))
            .unwrap_or([0.0; 3])
    }

    /// MSAA is not implemented in this backend.
    pub fn max_msaa(&self) -> u32 {
        1
    }

    pub fn max_anisotropy(&self) -> f32 {
        1.0
    }

    pub fn gpu_destination_times(&self) -> Vec<GpuViewTiming> {
        Vec::new()
    }

    pub fn gpu_view_times(&self) -> Vec<GpuViewTiming> {
        Vec::new()
    }

    /// GPU timestamps are not collected by this backend.
    pub fn take_gpu_times(&mut self) -> Option<(u64, Vec<(&'static str, f64)>)> {
        None
    }

    /// Debug isolation is a Vulkan-backend feature.
    pub fn has_camera_override(&self) -> bool {
        false
    }

    pub fn debug_visibility(&mut self, _args: &[String]) -> String {
        "r_visibility is not available in the wgpu renderer.\n".into()
    }

    /// Slang presets are not supported by this backend.
    pub fn set_shader_preset(&mut self, path: Option<&Path>) -> std::result::Result<Vec<ShaderParam>, String> {
        match path {
            None => Ok(Vec::new()),
            Some(_) => Err("shader presets are not supported by the wgpu renderer".into()),
        }
    }

    pub fn shader_src_size(&self) -> Option<(u32, u32)> {
        None
    }

    pub fn shader_preset(&self) -> Option<(&Path, &[ShaderParam])> {
        None
    }

    pub fn set_shader_param(&mut self, _id: &str, _value: f32) -> bool {
        false
    }
}
