const FRAMES: usize = 2;
const DYN_SIZE: u64 = 48 << 20;
const OFS_LMDYN: u64 = 2048;
const OFS_VIS: u64 = 8192;
const MAX_VIS: u64 = 1 << 17;
/// Real-time dynamic light list (common.glsl `Lights`, frame/dlights.rs).
const OFS_LIGHTS: u64 = OFS_VIS + MAX_VIS * 4;
const LIGHTS_BYTES: u64 = 8192;
const OFS_BUMP: u64 = OFS_LIGHTS + LIGHTS_BYTES;
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
    /// Force GPU timestamps on without QSP_GPUTIMES (profiling, `prof_start`).
    pub gpu_times_wanted: bool,
    /// CPU time spent waiting on the frame-in-flight fence last frame (ns).
    pub fence_wait_ns: u64,
    swap: Option<Swapchain>,
    surface_size: Box<dyn Fn() -> (u32, u32)>,
    width: u32,
    height: u32,
    targets: Option<Targets>,
    sampler_repeat: vk::Sampler,
    sampler_clamp: vk::Sampler,
    sampler_linear: vk::Sampler,
    sampler_linear_repeat: vk::Sampler,
    /// Sampler for [`Sampling::World`] textures, rebuilt when filtering changes.
    sampler_world: vk::Sampler,
    /// (linear, anisotropy) of `sampler_world`.
    world_filter: (usize, f32, f32),
    /// Colour metric the palette LUT was built with (-1 = not built).
    lut_metric: i32,
    /// Scene MSAA sample count, render scale, SSAO quality, portal quality, and motion-blur quality in effect.
    msaa: u32,
    render_scale: f32,
    ssao_quality: u32,
    portal_quality: u32,
    motion_blur_quality: u32,
    /// Bloom quality in effect (`None` = bloom off: no glow attachment or chain).
    bloom: Option<u32>,
    /// Cube shadow maps of real-time dynamic lights (always present; a
    /// placeholder while off) and their depth-compare sampler.
    shadow_maps: Option<ShadowMaps>,
    sampler_shadow: vk::Sampler,
    /// Last frame's entity transforms by `RenderEntity::key` (motion blur), plus reused scratch map.
    prev_xforms: std::collections::HashMap<u32, ([f32; 3], [f32; 3])>,
    cur_xforms: std::collections::HashMap<u32, ([f32; 3], [f32; 3])>,
    prev_xform_time: f64,
    /// Ripples, splashes and wakes on liquids (r_waterripples/r_watersplash).
    water_fx: water::WaterFx,
    textures: Vec<Option<Tex>>,
    /// Player's limit on a level texture's larger side (0 = the GPU's).
    max_texture_size: u32,
    palette: Palette,
    /// Prepacked opaque RGBA8 palette (`r | g<<8 | b<<16 | 0xff<<24`) for classic particle uploads.
    palette_rgba: [u32; 256],
    palette_buf: Buffer,
    lightmap: Option<Image>,
    /// Reused CPU lightmap union across all scene/destination views.
    lightmap_requests: frame::LightmapRequests,
    dummy: Buffer,
    world: Option<WorldGpu>,
    isolated_portal: Option<usize>,
    mapper_stats: MapperStats,
    /// Add-on portals have been drawn: keep the portal view targets.
    gun_targets: bool,
    isolated_distance: Option<f32>,
    hidden_entities: std::collections::HashSet<u32>,
    models: Vec<Model>,
    pics: Vec<PicInfo>,
    /// Queued same-size picture uploads recorded on the next frame without idling.
    pending_pic_uploads: Vec<PendingPicUpload>,
    /// Soft round blob for water spray puffs (big underwater-blast splashes).
    splash_pic: Option<PicId>,
    /// Precomputed tiling Worley caustic texture slot (`NO_TEX` if upload failed).
    caustic_tex: u32,
    conchars: Option<PicId>,
    alias_cpu: Vec<u32>,
    alias_buf: Buffer,
    sky_layers: [u32; 2],
    skybox: Option<[u32; 6]>,
    buckets: Vec<Vec<u32>>,
    /// Reused per-draw indirect command scratch for `build_world_draw`.
    world_draw_scratch: frame::WorldDrawScratch,
    /// Reused per-frame draw lists for view 0 (zero steady-state heap reallocs).
    frame_scratch: frame::FrameScratch,
    /// Reused per-frame 2D UI buffers (zero heap allocations in steady state).
    ui_verts: Vec<frame::UiVert>,
    ui_ranges: Vec<(Option<[f32; 4]>, u32, u32)>,
    needs_resize: bool,
    /// The window has no area to present to; `needs_resize` stays set.
    minimised: bool,
    /// Image acquired and fence reset, not submitted yet (`render`).
    frame_open: bool,
    frame_count: u64,
    /// Windowed screenshot: copy the next presented image to a buffer.
    capture_requested: bool,
    /// (buffer, width, height) of the last captured swapchain image.
    capture: Option<(Buffer, u32, u32)>,
    /// Windowed: keep an offscreen output (`targets.out`) for frames that
    /// aren't presented (video export), and whether the next `render`
    /// draws there instead of the swapchain.
    keep_out: bool,
    skip_present: bool,
    /// Loaded slang preset (r_shader_preset).
    slang: Option<slang::Slang>,
    /// Ray-traced reflections (created on first use).
    rt: Option<rt::Rt>,
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

    /// Makes a new window surface after [`Lost::Surface`]; the next `render`
    /// makes its swapchain.
    pub fn recreate_surface(&mut self, surface: &dyn SurfaceProvider) -> Result<()> {
        self.idle();
        if let Some(s) = self.swap.take() {
            self.destroy_swapchain(s);
        }
        self.needs_resize = true;
        self.gpu.recreate_surface(&|inst| surface.create_surface(inst))
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

    /// Video memory in use and the budget, in bytes, when the driver reports it.
    pub fn vram(&self) -> Option<(u64, u64)> {
        self.gpu.vram()
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

    /// Creates an RGBA8 texture from a mip chain (device must be idle),
    /// shrunk when it's over the size limit.
    fn add_texture(
        &mut self,
        up: &mut gpu::Upload,
        w: u32,
        h: u32,
        mips: &[&[u8]],
        usage: TexUse,
        sampling: Sampling,
    ) -> Result<u32> {
        let (w, h, mips) = if convert::complete(w, h, mips) { (w, h, mips) } else { convert::EMPTY };
        let fitted = convert::fit(w, h, mips, usage.limit(self.max_texture_size, self.gpu.max_texture), usage);
        let refs: Vec<&[u8]>;
        let (w, h, mips) = match &fitted {
            Some((w, h, m)) => {
                refs = m.iter().map(|m| &**m).collect();
                (*w, *h, &refs[..])
            }
            None => (w, h, mips),
        };
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
            level: usage.level(),
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
        self.capture_requested = self.swap.is_some() && !self.skip_present;
    }

    /// Windowed: allocate (or free) an offscreen output so frames can be
    /// drawn without being shown. Headless always has one.
    pub fn set_offscreen(&mut self, on: bool) -> Result<()> {
        if self.swap.is_none() || self.keep_out == on {
            return Ok(());
        }
        self.keep_out = on;
        self.skip_present = false;
        if self.targets.is_some() {
            self.create_targets()?;
        }
        Ok(())
    }

    /// With [`Self::set_offscreen`] on: the next `render` draws into the
    /// offscreen output (read with `read_pixels`) and the window keeps
    /// showing the last presented image.
    pub fn present_next(&mut self, present: bool) {
        self.skip_present = !present && self.keep_out;
    }

    /// Whether `render` presents this frame.
    pub(crate) fn presenting(&self) -> bool {
        self.swap.is_some() && !self.skip_present
    }

    /// The captured frame as (width, height, RGBA8). Only pending (`Ok(None)`)
    /// while the window is minimised (the wgpu backend's are until the
    /// read-back completes).
    pub fn take_capture(&mut self) -> Result<Option<(u32, u32, Vec<u8>)>> {
        if self.minimised && !self.skip_present && self.capture.is_none() {
            return Ok(None);
        }
        let (w, h) = match &self.swap {
            Some(_) if !self.skip_present => self.capture.as_ref().map_or((self.width, self.height), |c| (c.1, c.2)),
            _ => (self.width, self.height),
        };
        self.read_pixels().map(|px| Some((w, h, px)))
    }

    /// Waits for the last frame and returns its RGBA8 pixels: the offscreen
    /// image when headless, else the swapchain image captured by the last
    /// `render` after `request_capture`.
    pub fn read_pixels(&mut self) -> Result<Vec<u8>> {
        self.idle();
        if let Some(s) = &self.swap
            && !self.skip_present
        {
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
        let mut px = buf.bytes_mut()[..(w * h * 4) as usize].to_vec();
        // the windowed offscreen output has the swapchain's format
        if matches!(self.pipes.post_format, vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB) {
            px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
        }
        if self.swap.is_some() {
            px.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
        }
        Ok(px)
    }

    /// Light level at a point (R_LightPoint), RGB 0..255+.
    pub fn light_point(&self, p: Vec3, lightstyles: &[i32]) -> Vec3 {
        self.world
            .as_ref()
            .and_then(|w| w.cpu.light_point(p, lightstyles))
            .unwrap_or([0.0; 3])
    }

    /// True once a world BSP is loaded.
    pub fn has_world(&self) -> bool {
        self.world.is_some()
    }

    /// Conservative local-space `(mins, maxs)` for a loaded model (rotation-safe
    /// sphere box for alias/sprite models, submodel AABB for brush models).
    pub fn model_bounds(&self, id: ModelId) -> Option<([f32; 3], [f32; 3])> {
        match self.models.get(id.0 as usize)? {
            Model::Brush(sm) => {
                let sub = self.world.as_ref()?.cpu.submodels.get(*sm as usize)?;
                Some((sub.mins, sub.maxs))
            }
            Model::Alias(am) => {
                let r = am.radius.max(16.0);
                Some(([-r; 3], [r; 3]))
            }
            Model::Sprite(sm) => {
                let r = sm.radius.max(16.0);
                Some(([-r; 3], [r; 3]))
            }
        }
    }

    /// Appends the 1-based non-solid BSP leaves touched by `[mins, maxs]`.
    pub fn world_box_leaves(&self, mins: [f32; 3], maxs: [f32; 3], out: &mut Vec<u32>) {
        if let Some(w) = &self.world {
            w.cpu.box_leaves(mins, maxs, out);
        }
    }

    /// Flat PVS leaf bitset for `eye`'s view leaf, or `None` when PVS is off.
    pub fn world_pvs_bits(&mut self, eye: Vec3) -> Option<&[u32]> {
        self.world.as_mut()?.cpu.pvs_bits(eye)
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
        self.free_rt();
        self.destroy_targets();
        self.destroy_shadow_maps();
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
            for b in f.staging {
                self.gpu.free_buffer(b);
            }
        }
        for up in std::mem::take(&mut self.pending_pic_uploads) {
            if let Some(b) = up.staging {
                self.gpu.free_buffer(b);
            }
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
            dev.destroy_sampler(self.sampler_linear_repeat, None);
            dev.destroy_sampler(self.sampler_shadow, None);
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

