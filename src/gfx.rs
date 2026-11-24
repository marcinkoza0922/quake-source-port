//! The host's renderer: `qrender`'s, or nothing. A dedicated server draws
//! nothing and must not need a GPU or a graphics driver (the references' don't),
//! so [`Renderer::none`] answers the same calls: loads fail, queries are empty.

use std::path::Path;

use qformats::{Mdl, Spr};
use qrender::{
    Error, GpuViewTiming, LevelBsp, MapperStats, Md3SkinImage, Md3SurfaceSkins, ModelId, PicId, RenderFrame, Result,
    ShaderParam,
};

#[cfg(not(target_arch = "wasm32"))]
type Palette = [[u8; 3]; 256];

pub struct Renderer(Option<qrender::Renderer>);

fn none<T>() -> Result<T> {
    Err(Error::new("no renderer (dedicated server)"))
}

/// Methods forwarded to `qrender::Renderer`, with the answer without one.
macro_rules! forward {
    ($($(#[$m:meta])* fn $n:ident(& $($mt:ident)? $(, $a:ident : $t:ty)*) $(-> $r:ty)? = $none:expr;)*) => {
        $($(#[$m])* pub fn $n(& $($mt)? self $(, $a: $t)*) $(-> $r)? {
            match &$($mt)? self.0 {
                Some(r) => r.$n($($a),*),
                None => $none,
            }
        })*
    };
}

impl Renderer {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn none() -> Self {
        Self(None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless(width: u32, height: u32, palette: &Palette) -> Result<Self> {
        qrender::Renderer::headless(width, height, palette).map(Self::from)
    }

    /// The backend renderer (Android surface changes).
    #[cfg(target_os = "android")]
    pub fn windowed(&mut self) -> Option<&mut qrender::Renderer> {
        self.0.as_mut()
    }

    /// CPU time spent waiting on the frame-in-flight fence last frame (ns).
    pub fn fence_wait_ns(&self) -> u64 {
        self.0.as_ref().map_or(0, |r| r.fence_wait_ns)
    }

    /// Keeps GPU timestamps on without QSP_GPUTIMES.
    pub fn want_gpu_times(&mut self, on: bool) {
        if let Some(r) = &mut self.0 {
            r.gpu_times_wanted = on;
        }
    }

    forward! {
        fn device_name(&) -> &str = "none";
        fn vram(&) -> Option<(u64, u64)> = None;
        fn size(&) -> (u32, u32) = (640, 480);
        fn resize(&mut, width: u32, height: u32) = ();
        fn render(&mut, fr: &RenderFrame) -> Result<()> = Ok(());
        fn request_capture(&mut) = ();
        fn set_offscreen(&mut, on: bool) -> Result<()> = none();
        fn present_next(&mut, present: bool) = ();
        fn take_capture(&mut) -> Result<Option<(u32, u32, Vec<u8>)>> = Ok(None);
        #[cfg(test)]
        fn read_pixels(&mut) -> Result<Vec<u8>> = none();
        fn light_point(&, p: [f32; 3], lightstyles: &[i32]) -> [f32; 3] = [0.0; 3];
        fn has_world(&) -> bool = false;
        fn model_bounds(&, id: ModelId) -> Option<([f32; 3], [f32; 3])> = None;
        fn world_box_leaves(&, mins: [f32; 3], maxs: [f32; 3], out: &mut Vec<u32>) = ();
        fn world_pvs_bits(&mut, eye: [f32; 3]) -> Option<&[u32]> = None;
        fn load_level(&mut, level: LevelBsp, brushes: &[LevelBsp]) -> Result<(Vec<ModelId>, Vec<ModelId>)> = none();
        fn load_alias(&mut, mdl: &Mdl) -> Result<ModelId> = none();
        fn load_alias_glow(&mut, mdl: &Mdl, glow: &[Vec<u8>]) -> Result<ModelId> = none();
        fn load_md3(&mut, md3: &qformats::md3::Md3, skins: &[Md3SurfaceSkins]) -> Result<ModelId> = none();
        fn load_iqm(&mut, iqm: &qformats::Iqm, skins: &[Md3SurfaceSkins]) -> Result<ModelId> = none();
        fn load_sprite(&mut, spr: &Spr) -> Result<ModelId> = none();
        fn set_skybox(&mut, faces: Option<&[qformats::image::Rgba; 6]>) -> Result<()> = none();
        fn set_conchars(&mut, pixels: &[u8]) -> Result<PicId> = none();
        fn register_pic(&mut, pic: &qformats::image::Pic) -> Result<PicId> = none();
        fn register_pic_rgba(&mut, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> = none();
        fn register_pic_rgba_linear(&mut, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> = none();
        fn resize_pic(&mut, pic: PicId, width: u32, height: u32) -> Result<()> = none();
        fn update_pic(&mut, pic: PicId, rgba: &[u8]) -> Result<()> = none();
        fn update_pic_rows(&mut, pic: PicId, rgba: &[u8], rows: u32) -> Result<()> = none();
        fn max_msaa(&) -> u32 = 1;
        fn supports_wireframe(&) -> bool = false;
        fn mapper_stats(&) -> MapperStats = MapperStats::default();
        fn gpu_view_times(&) -> Vec<GpuViewTiming> = Vec::new();
        fn gpu_destination_times(&) -> Vec<GpuViewTiming> = Vec::new();
        fn take_gpu_times(&mut) -> Option<(u64, Vec<(&'static str, f64)>)> = None;
        fn has_camera_override(&) -> bool = false;
        fn debug_visibility(&mut, args: &[String]) -> String = "No level loaded.\n".into();
        fn set_shader_preset(&mut, path: Option<&Path>) -> std::result::Result<Vec<ShaderParam>, String> = Err("no renderer".into());
        fn shader_preset(&) -> Option<(&Path, &[ShaderParam])> = None;
        fn set_shader_param(&mut, id: &str, value: f32) -> bool = false;
    }

    pub fn md3_indexed_skin(&self, width: u32, height: u32, pixels: &[u8], holey: bool) -> Md3SkinImage {
        match &self.0 {
            Some(r) => r.md3_indexed_skin(width, height, pixels, holey),
            None => Md3SkinImage { width, height, base: Vec::new(), fb: None, cmap: None },
        }
    }
}

impl From<qrender::Renderer> for Renderer {
    fn from(r: qrender::Renderer) -> Self {
        Self(Some(r))
    }
}
