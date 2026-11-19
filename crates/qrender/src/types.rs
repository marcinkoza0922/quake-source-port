//! Per-frame input types. These mirror the parts of `qclient::ClientState`
//! the renderer needs; the binary converts.

use std::ffi::CString;

pub type Vec3 = [f32; 3];

mod settings;
pub use settings::*;

/// Window-system glue provided by the platform layer (no SDL dependency here).
pub trait SurfaceProvider {
    /// Instance extensions needed for presentation.
    fn instance_extensions(&self) -> Vec<CString>;
    /// Creates a `VkSurfaceKHR` for the given `VkInstance` (raw handles).
    fn create_surface(&self, instance: u64) -> u64;
    /// Current drawable size in pixels.
    fn drawable_size(&self) -> (u32, u32);
}

/// One skin frame for an MD3 surface (`base` RGBA8 plus optional `fb`/`cmap`).
#[derive(Clone, Debug)]
pub struct Md3SkinImage {
    pub width: u32,
    pub height: u32,
    pub base: Vec<u8>,
    pub fb: Option<(u32, u32, Vec<u8>)>,
    pub cmap: Option<(u32, u32, Vec<u8>)>,
}

/// Skin set for one MD3 surface (`skins[skin_idx] = (frames, intervals)`).
#[derive(Clone, Debug, Default)]
pub struct Md3SurfaceSkins {
    pub skins: Vec<(Vec<Md3SkinImage>, Vec<f32>)>,
    pub holey: bool,
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
    /// Horizontal field of view in degrees at 4:3; widened for the scene aspect.
    pub fov_x: f32,
    /// Pixel rows reserved at the bottom of the screen (classic status bar);
    /// the 3D scene is drawn above them.
    pub reserve_bottom: u32,
    /// Peripheral radial blur strength, 0..=1; centre and UI stay sharp.
    pub speed_blur: f32,
    /// Underwater view effect (r_waterwarp).
    pub waterwarp: WaterWarp,
}

/// Underwater view distortion; the value is the warp clock in seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WaterWarp {
    #[default]
    None,
    /// Software Quake: sine ripple over the whole image (post pass).
    Screen(f32),
    /// GLQuake: the horizontal and vertical FOV pulse in opposite directions.
    Fov(f32),
}

/// Key bit of an entity attached to the entity `key & !ATTACHED_KEY` (a
/// player's visible weapon): its own motion-blur identity, but not a separate
/// mover for water effects.
pub const ATTACHED_KEY: u32 = 1 << 24;

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
    /// Alias model drawn from a CPU-built pose in [`RenderFrame::dyn_verts`]
    /// instead of its own frames (ragdolls).
    pub dyn_pose: Option<DynPose>,
    /// Split-screen views this entity is drawn in (bit k = view k); 0 = all.
    pub views: u8,
    /// Drawn only in portal views (the player's own body).
    pub portal_only: bool,
    /// Moves with the camera (the player's own body in third person): no
    /// per-object motion blur, since it stays put on screen.
    pub follows_camera: bool,
    /// Thermal goggles: [`HEAT_COLD`], [`HEAT_WARM`], [`HEAT_BODY`] (also
    /// seen through walls) or [`HEAT_FIRE`].
    pub heat: u8,
}

pub const HEAT_COLD: u8 = 0;
pub const HEAT_WARM: u8 = 1;
pub const HEAT_BODY: u8 = 2;
pub const HEAT_FIRE: u8 = 3;

/// Add-on goggles post effect (see `post.frag`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goggles {
    /// 0 off, 1 night vision, 2 thermal.
    pub mode: u32,
    /// Image gain (night vision) or heat contrast (thermal), 0.25..=3.
    pub intensity: f32,
    /// Sensor noise and scanlines, 0..=2.
    pub noise: f32,
    /// Glow quality: 0 low (no halo taps), 1 medium (4 diagonal taps), 2 high (8 ring taps).
    pub quality: u32,
}

impl Default for Goggles {
    fn default() -> Self {
        Self { mode: 0, intensity: 0.0, noise: 0.0, quality: 1 }
    }
}

/// An add-on (portal gun) portal: an oval on a wall, `half` its half width
/// and height along `right = up × normal` and `up`. With `exit` set and
/// `live`, the view out of the exit is drawn into it; otherwise it is a
/// flat disc in `color`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunPortal {
    pub origin: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
    pub half: [f32; 2],
    pub color: [u8; 3],
    /// Linked portal's (origin, normal, up).
    pub exit: Option<(Vec3, Vec3, Vec3)>,
    pub live: bool,
}

/// One split-screen view: its camera, its rectangle of the output (x, y,
/// w, h in output pixels) and its polyblend.
#[derive(Clone, Copy, Debug)]
pub struct SplitView {
    pub view: View,
    pub rect: [u32; 4],
    pub blend: [f32; 4],
}

/// A per-frame alias pose: `num_verts` packed vertices (x, y, z bytes and an
/// anorm index) at `ofs` in [`RenderFrame::dyn_verts`], placed in world space
/// as `origin + v * scale` (no entity rotation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynPose {
    pub ofs: u32,
    pub origin: Vec3,
    pub scale: Vec3,
    /// Culling radius around the entity origin.
    pub radius: f32,
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
            views: 0,
            dyn_pose: None,
            portal_only: false,
            follows_camera: false,
            heat: HEAT_COLD,
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
    /// Alpha-blended line list drawn without depth testing (debug overlays
    /// such as `r_showbboxes`); `lines` is implied.
    Overlay,
}

impl FteBlend {
    /// fte.frag blend mode (`fogmix.w`); Overlay shades like Blend, since
    /// mode 8 is the gun portal.
    pub fn shader_mode(self) -> f32 {
        if self == FteBlend::Overlay { 0.0 } else { self as u32 as f32 }
    }
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

pub use qcore::vertex::ParticleVertex as FteVertex;

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
    /// Gel shading: the texture alpha is a height field lit as a thick,
    /// glossy liquid (fte.frag mode 9).
    pub gel: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DLight {
    pub origin: Vec3,
    pub radius: f32,
    pub minlight: f32,
    /// 1,1,1 for classic white lights.
    pub color: Vec3,
    /// Entity that emits the light (the client's dlight key: a rocket, a
    /// player's muzzle flash), 0 = none. With real-time dynamic lights that
    /// entity casts no shadow from its own light.
    pub key: u32,
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
    /// Shot impact points this frame (bullets, nails, explosions; never
    /// beams): a splash where the line from the view to the impact crosses
    /// a liquid surface. The flag marks explosions (big splash underwater).
    pub shot_impacts: &'a [(Vec3, bool)],
    /// Packed alias vertices referenced by [`RenderEntity::dyn_pose`].
    pub dyn_verts: &'a [u32],
    /// Split-screen: when non-empty, these views are drawn instead of
    /// `view` and `blend`.
    pub splits: &'a [SplitView],
    /// Add-on portals (empty unless the portal gun is in use).
    pub gun_portals: &'a [GunPortal],
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
            shot_impacts: &[],
            dyn_verts: &[],
            splits: &[],
            gun_portals: &[],
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
