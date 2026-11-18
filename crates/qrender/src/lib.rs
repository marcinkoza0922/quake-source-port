//! qrender — renderer (Vulkan via ash, or wgpu on the web / with the `wgpu`
//! feature). See docs/DESIGN.md "Renderer" and docs/WEB_PORT.md.
//!
//! Inputs are `qformats` data plus a per-frame [`RenderFrame`]; no SDL or
//! qclient dependency. Presentation goes through [`SurfaceProvider`]; without
//! one the renderer draws to an offscreen image readable with
//! [`Renderer::read_pixels`]. Both backends expose the same `Renderer` API.

#![cfg_attr(not(qvk), allow(dead_code, unused_imports))]

mod convert;
pub use convert::{ExternalTexture, LevelBsp};
mod projection;
pub use projection::{ViewFrustum, view_fov_tangents};
mod shared;
mod softemu;
pub use shared::{GpuViewTiming, ShaderParam};
pub mod types;
pub(crate) mod water;
mod world;
pub mod portals;
/// Portal view render targets (teleporter windows, gun portals and their
/// nested views).
pub const MAX_PORTAL_VIEWS: usize = 8;

pub use types::*;

use convert::Palette;
use qformats::bsp::{Bsp, TextureKind};

mod debug_visibility;
#[cfg(qvk)]
mod frame;
#[cfg(qvk)]
mod gpu;
#[cfg(qvk)]
mod pipelines;
#[cfg(qvk)]
mod slang;
#[cfg(qvk)]
#[path = "lib/targets.rs"]
mod targets;
#[cfg(qvk)]
#[path = "lib/assets.rs"]
mod assets;
#[cfg(qvk)]
#[path = "lib/world_load.rs"]
mod world_load;
#[cfg(qvk)]
#[path = "lib/settings.rs"]
mod settings;
#[cfg(qvk)]
#[path = "lib/descriptors.rs"]
mod descriptors;
#[cfg(qvk)]
#[path = "lib/init.rs"]
mod init;
#[cfg(qvk)]
#[path = "lib/timing.rs"]
mod timing;
#[cfg(qvk)]
use timing::{GpuTimes, GPU_STAGES};
#[cfg(qvk)]
#[path = "lib/resources.rs"]
mod resources;
#[cfg(qvk)]
use resources::*;
#[cfg(qvk)]
use ash::vk;
#[cfg(qvk)]
use gpu::{Buffer, Gpu, Image};
#[cfg(qvk)]
use pipelines::{COLOR_FORMAT, DEPTH_FORMAT, LM_FORMAT, MAX_TEXTURES, Pipelines};
#[cfg(qvk)]
use qformats::image::Pic;
#[cfg(qvk)]
use qformats::{Md3, Mdl, Spr};

// The Vulkan `Renderer` (struct and core impls) lives at the crate root so
// its private fields stay visible to the backend's modules.
#[cfg(qvk)]
include!("lib/vk.rs");

#[cfg(not(qvk))]
mod wgpu_backend;
#[cfg(not(qvk))]
pub use wgpu_backend::Renderer;

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

/// Raw byte view of plain-old-data slices.
pub(crate) fn bytes_of<T: Copy>(s: &[T]) -> &[u8] {
    // SAFETY: T is Copy (repr(C) POD in this crate, no padding read as uninit for our types).
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
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
