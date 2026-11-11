//! Per-frame input types. These mirror the parts of `qclient::ClientState`
//! the renderer needs; the binary converts.

use std::ffi::CString;

pub type Vec3 = [f32; 3];

/// Window-system glue provided by the platform layer (no SDL dependency here).
pub trait SurfaceProvider {
    /// Instance extensions needed for presentation.
    fn instance_extensions(&self) -> Vec<CString>;
    /// Creates a `VkSurfaceKHR` for the given `VkInstance` (raw handles).
    fn create_surface(&self, instance: u64) -> u64;
    /// Current drawable size in pixels.
    fn drawable_size(&self) -> (u32, u32);
}

/// Handle to a loaded model (world submodel, alias or sprite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModelId(pub u32);

/// Handle to a 2D picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PicId(pub u32);

/// Camera.
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    pub origin: Vec3,
    /// Pitch, yaw, roll in degrees (Quake convention).
    pub angles: Vec3,
    /// Horizontal field of view in degrees; vertical is derived from the aspect.
    pub fov_x: f32,
}

/// One entity to draw.
#[derive(Clone, Copy, Debug)]
pub struct RenderEntity {
    pub model: ModelId,
    pub origin: Vec3,
    pub angles: Vec3,
    /// Current frame (alias frame/group, sprite frame, or brush texture alt-frame when != 0).
    pub frame: u32,
    /// Previous frame for alias vertex lerp.
    pub prev_frame: u32,
    /// Blend from `prev_frame` (0) to `frame` (1).
    pub lerp: f32,
    pub skin: u32,
    /// Player shirt/pants colours (0..=13) for colormapped skins.
    pub colormap: Option<[u8; 2]>,
    /// 1.0 = opaque.
    pub alpha: f32,
    /// Time offset for group animations (syncbase).
    pub syncbase: f32,
    /// First-person weapon: drawn last with a compressed depth range.
    pub viewmodel: bool,
    /// Ignore lighting (e.g. flames).
    pub fullbright: bool,
    /// Entity is a player (minimum light 8).
    pub player: bool,
}

impl Default for RenderEntity {
    fn default() -> Self {
        Self {
            model: ModelId(0),
            origin: [0.0; 3],
            angles: [0.0; 3],
            frame: 0,
            prev_frame: 0,
            lerp: 1.0,
            skin: 0,
            colormap: None,
            alpha: 1.0,
            syncbase: 0.0,
            viewmodel: false,
            fullbright: false,
            player: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub origin: Vec3,
    /// Palette index.
    pub color: u8,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DLight {
    pub origin: Vec3,
    pub radius: f32,
    pub minlight: f32,
    /// 1,1,1 for classic white lights.
    pub color: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fog {
    /// Quake density (as in the `fog` command; 0 = off).
    pub density: f32,
    pub color: Vec3,
}

/// 2D draw command; coordinates are framebuffer pixels, origin top-left.
#[derive(Clone, Copy, Debug)]
pub enum Draw2D {
    /// Picture stretched to `w`×`h`, optionally a sub-rectangle `uv` (u0, v0, u1, v1).
    Pic {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        pic: PicId,
        uv: [f32; 4],
        color: [f32; 4],
    },
    /// Conchars glyph (0..=255) of `size` pixels.
    Char {
        x: f32,
        y: f32,
        size: f32,
        ch: u8,
        color: [f32; 4],
    },
    /// Solid rectangle.
    Fill {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    /// Tiled picture (backtile), `scale` texels per pixel.
    Tile {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        pic: PicId,
        scale: f32,
    },
}

/// Cvar-like settings.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub gamma: f32,
    pub contrast: f32,
    pub wateralpha: f32,
    /// Per-type alphas; `None` uses `wateralpha`.
    pub lavaalpha: Option<f32>,
    pub slimealpha: Option<f32>,
    pub telealpha: Option<f32>,
    /// Sky blend toward fog colour (r_skyfog).
    pub skyfog: f32,
    /// Alpha of the front sky layer (r_skyalpha).
    pub skyalpha: f32,
    pub dynamic: bool,
    pub vsync: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gamma: 1.0,
            contrast: 1.0,
            wateralpha: 1.0,
            lavaalpha: None,
            slimealpha: None,
            telealpha: None,
            skyfog: 0.5,
            skyalpha: 1.0,
            dynamic: true,
            vsync: true,
        }
    }
}

/// Everything needed to draw one frame.
#[derive(Clone, Copy, Debug)]
pub struct RenderFrame<'a> {
    /// Client time in seconds (animations, warps, sky scroll).
    pub time: f64,
    /// `None` draws only 2D (console / menus without a map).
    pub view: Option<View>,
    pub entities: &'a [RenderEntity],
    pub particles: &'a [Particle],
    pub dlights: &'a [DLight],
    /// d_lightstylevalue-style values (256 = normal, 'm' = 264); missing = 256.
    pub lightstyles: &'a [i32],
    pub fog: Fog,
    /// v_blend polyblend (rgba, 0..1).
    pub blend: [f32; 4],
    pub draw2d: &'a [Draw2D],
    pub settings: Settings,
}

impl Default for RenderFrame<'_> {
    fn default() -> Self {
        Self {
            time: 0.0,
            view: None,
            entities: &[],
            particles: &[],
            dlights: &[],
            lightstyles: &[],
            fog: Fog::default(),
            blend: [0.0; 4],
            draw2d: &[],
            settings: Settings::default(),
        }
    }
}

/// d_lightstylevalue for a lightstyle string at `time` (R_AnimateLight).
pub fn lightstyle_value(pattern: &str, time: f64) -> i32 {
    let b = pattern.as_bytes();
    if b.is_empty() {
        return 256;
    }
    let i = ((time * 10.0) as i64).rem_euclid(b.len() as i64) as usize;
    (b[i] as i32 - b'a' as i32) * 22
}

/// Parses a worldspawn `fog` key / `fog` command args: "density [r g b]".
pub fn parse_fog(s: &str) -> Fog {
    let v: Vec<f32> = s
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    match v.as_slice() {
        [d] => Fog {
            density: *d,
            color: [0.3; 3],
        },
        [d, r, g, b, ..] => Fog {
            density: *d,
            color: [*r, *g, *b],
        },
        _ => Fog::default(),
    }
}
