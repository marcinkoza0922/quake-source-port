use super::{Error, Palette, Renderer, Result};

pub(super) struct Presentation {
    instance: wgpu::Instance,
    window_id: usize,
    pub(super) lost: bool,
    /// The surface's configuration while there is none.
    cfg: Option<wgpu::SurfaceConfiguration>,
}

impl Renderer {
    /// Present to an owning reference to an Android native window.
    pub fn new_android(
        window: impl wgpu::DisplayAndWindowHandle + Clone + 'static,
        window_id: usize,
        (width, height): (u32, u32),
        palette: &Palette,
    ) -> Result<Self> {
        let mut errors = Vec::new();
        let available = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::GL);
        for backend in [wgpu::Backends::VULKAN, wgpu::Backends::GL] {
            if !available.contains(backend) {
                continue;
            }
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            desc.backends = backend;
            let instance = wgpu::Instance::new(desc);
            let result = create_surface(&instance, window.clone()).and_then(|surface| {
                pollster::block_on(Self::create(instance.clone(), Some(surface), width, height, palette))
            });
            match result {
                Ok(mut renderer) => {
                    renderer.android_surface = Some(Presentation { instance, window_id, lost: false, cfg: None });
                    return Ok(renderer);
                }
                Err(e) => errors.push(format!("{backend:?}: {e}")),
            }
        }
        if errors.is_empty() {
            return Err(Error::new("Android graphics needs Vulkan or OpenGL ES"));
        }
        Err(Error::new(errors.join("\n")))
    }

    /// SDL can replace the native window when Android recreates the SurfaceView.
    pub fn sync_android_window(
        &mut self,
        window: impl wgpu::DisplayAndWindowHandle + 'static,
        window_id: usize,
        (width, height): (u32, u32),
    ) -> Result<()> {
        let Some(state) = &mut self.android_surface else { return Ok(()) };
        if state.window_id == window_id && !state.lost {
            return Ok(());
        }
        // Drop the old EGL or Vulkan surface before creating a new one for the same window.
        // Its configuration outlives a creation that fails, to try again.
        if let Some((_, cfg)) = self.surface.take() {
            state.cfg = Some(cfg);
        }
        let Some(cfg) = state.cfg.clone() else { return Ok(()) };
        state.lost = true;
        let surface = create_surface(&state.instance, window)?;
        self.surface = Some((surface, cfg));
        state.window_id = window_id;
        state.lost = false;
        self.resize(width, height);
        self.needs_resize = true;
        Ok(())
    }
}

fn create_surface(
    instance: &wgpu::Instance,
    window: impl wgpu::DisplayAndWindowHandle + 'static,
) -> Result<wgpu::Surface<'static>> {
    instance.create_surface(wgpu::SurfaceTarget::DisplayAndWindow(Box::new(window)))
        .map_err(|e| Error::new(format!("Android graphics surface: {e}")))
}
