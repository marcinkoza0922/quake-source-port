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
    /// Pixel rows reserved at the bottom of the screen (classic status bar);
    /// the 3D scene is drawn above them.
    pub reserve_bottom: u32,
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
    /// Stable identity across frames for per-object motion blur (the client
    /// entity number); 0 = none (statics, temp entities, the view entity).
    pub key: u32,
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
            key: 0,
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

/// FTE particle blend modes (source, destination factors).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FteBlend {
    /// SRC_ALPHA, ONE_MINUS_SRC_ALPHA
    #[default]
    Blend,
    /// SRC_COLOR, ONE_MINUS_SRC_COLOR
    BlendColour,
    /// SRC_ALPHA, ONE
    AddA,
    /// SRC_COLOR, ONE
    AddC,
    /// SRC_ALPHA, ONE_MINUS_SRC_COLOR
    Subtract,
    /// ZERO, ONE_MINUS_SRC_ALPHA
    InvModA,
    /// ZERO, ONE_MINUS_SRC_COLOR
    InvModC,
    /// ONE, ONE_MINUS_SRC_ALPHA
    Premul,
}

impl FteBlend {
    pub const ALL: [FteBlend; 8] = [
        FteBlend::Blend,
        FteBlend::BlendColour,
        FteBlend::AddA,
        FteBlend::AddC,
        FteBlend::Subtract,
        FteBlend::InvModA,
        FteBlend::InvModC,
        FteBlend::Premul,
    ];
}

/// World-space particle vertex (FTE scripted particles).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FteVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// Indexed draw of FTE particle geometry; drawn in slice order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FteBatch {
    pub pic: PicId,
    pub blend: FteBlend,
    /// Line list instead of triangles.
    pub lines: bool,
    pub first_index: u32,
    pub num_indices: u32,
    /// Fades out near scene depth when soft particles are on.
    pub soft: bool,
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
    /// Scene MSAA samples (1 = off); clamped to the device maximum.
    pub msaa: u32,
    /// Scene resolution relative to the output (0.25..=2).
    pub render_scale: f32,
    /// Bilinear/trilinear world and model textures (false = nearest).
    pub texture_linear: bool,
    /// Max anisotropy for world/model textures (1 = off).
    pub anisotropy: f32,
    /// gl_texturemode index into [`TEXTURE_MODES`]; `None` derives it from
    /// `texture_linear` (nearest/linear with linear mips).
    pub texture_mode: Option<usize>,
    /// gl_lodbias; `None` = auto (-log2(samples)/2 when supersampling).
    pub lod_bias: Option<f32>,
    /// Shade every MSAA sample (vid_fsaamode 1).
    pub supersample: bool,
    /// r_dither strength.
    pub dither: f32,
    /// r_softemu: 0 off, 1 fine (screen dither), 2 coarse (world dither), 3 banded.
    pub softemu: u32,
    /// r_softemu_metric: <0 auto, 0 naive RGB, 1 Riemersma, 2 OKLab.
    pub softemu_metric: i32,
    /// r_softemu_lightmap_banding: <0 auto (banded mode), 0 off, >0 on.
    pub lightmap_banding: f32,
    /// r_softemu_mdl_warp: <0 auto (banded mode), 0 off, >0 on.
    pub mdl_warp: f32,
    /// r_softemu_dither_screen / r_softemu_dither_texture scales.
    pub dither_screen: f32,
    pub dither_texture: f32,
    /// r_colorblind: 0 off, 1 protanopia, 2 deuteranopia, 3 tritanopia filter.
    pub colorblind: u32,
    /// r_softparticles ? r_softparticledistance : 0. Soft FTE particles fade
    /// over this many units in front of scene depth; 0 disables.
    pub soft_particle_distance: f32,
    /// r_ssao: screen-space ambient occlusion strength (0 = off).
    pub ssao: f32,
    /// r_ssao_radius: occlusion radius in world units.
    pub ssao_radius: f32,
    /// r_motionblur: per-object motion blur; the blur covers this fraction of
    /// the motion over 1/60 s (0 = off). Camera motion is not blurred.
    pub motion_blur: f32,
    /// r_waterripples: ripple rings and gentle swell on liquid surfaces
    /// (strength, 0 = classic flat warp).
    pub water_ripples: f32,
    /// r_watersplash: droplet splashes when things enter or leave liquids.
    pub water_splashes: bool,
    /// r_watercaustics: animated caustics on submerged world faces (strength).
    pub water_caustics: f32,
}

/// gl_texturemode names (ironwail order): (name, linear mag/min, mipmapped, linear mips).
pub const TEXTURE_MODES: [(&str, bool, bool, bool); 6] = [
    ("GL_NEAREST", false, false, false),
    ("GL_NEAREST_MIPMAP_NEAREST", false, true, false),
    ("GL_NEAREST_MIPMAP_LINEAR", false, true, true),
    ("GL_LINEAR", true, false, false),
    ("GL_LINEAR_MIPMAP_NEAREST", true, true, false),
    ("GL_LINEAR_MIPMAP_LINEAR", true, true, true),
];

/// Parses a gl_texturemode string (name, any case, or 1-based index).
pub fn parse_texture_mode(s: &str) -> Option<usize> {
    let s = s.trim();
    TEXTURE_MODES
        .iter()
        .position(|m| m.0.eq_ignore_ascii_case(s))
        .or_else(|| s.parse::<usize>().ok().filter(|i| (1..=6).contains(i)).map(|i| i - 1))
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
            msaa: 1,
            render_scale: 1.0,
            texture_linear: false,
            anisotropy: 1.0,
            texture_mode: None,
            lod_bias: None,
            supersample: false,
            dither: 1.0,
            softemu: 0,
            softemu_metric: -1,
            lightmap_banding: -1.0,
            mdl_warp: -1.0,
            dither_screen: 1.0,
            dither_texture: 1.0,
            colorblind: 0,
            soft_particle_distance: 4.0,
            ssao: 0.0,
            ssao_radius: 32.0,
            motion_blur: 0.0,
            water_ripples: 0.0,
            water_splashes: false,
            water_caustics: 0.0,
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
    /// FTE particle geometry (indices are absolute into `fte_vertices`).
    pub fte_vertices: &'a [FteVertex],
    pub fte_indices: &'a [u32],
    pub fte_batches: &'a [FteBatch],
    pub dlights: &'a [DLight],
    /// d_lightstylevalue-style values (256 = normal, 'm' = 264); missing = 256.
    pub lightstyles: &'a [i32],
    pub fog: Fog,
    /// v_blend polyblend (rgba, 0..1).
    pub blend: [f32; 4],
    pub draw2d: &'a [Draw2D],
    pub settings: Settings,
    /// The player's (view entity's) origin, for splashes and wakes; the view
    /// entity is usually not in `entities`.
    pub player: Option<Vec3>,
}

impl Default for RenderFrame<'_> {
    fn default() -> Self {
        Self {
            time: 0.0,
            view: None,
            entities: &[],
            particles: &[],
            fte_vertices: &[],
            fte_indices: &[],
            fte_batches: &[],
            dlights: &[],
            lightstyles: &[],
            fog: Fog::default(),
            blend: [0.0; 4],
            draw2d: &[],
            settings: Settings::default(),
            player: None,
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
