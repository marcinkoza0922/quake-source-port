//! Cvar-like renderer settings (`Settings`) and gl_texturemode parsing.

use super::Goggles;

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
    /// Opt-in destination views on linked teleporter surfaces.
    pub teleport_windows: bool,
    /// Most live portal-gun views per frame (each costs a scene draw) and
    /// the distance beyond which a portal is drawn flat.
    pub gun_portal_max: u32,
    pub gun_portal_dist: f32,
    /// Portals seen through portals: nesting levels drawn live (0 = none;
    /// deeper ones are flat). All views share [`crate::MAX_PORTAL_VIEWS`].
    pub gun_portal_depth: u32,
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
    /// Peripheral radial blur independent of camera velocity.
    pub edge_blur: f32,
    /// r_waterripples: ripple rings and gentle swell on liquid surfaces
    /// (strength, 0 = classic flat warp).
    pub water_ripples: f32,
    /// r_watersplash: droplet splashes when things enter or leave liquids.
    pub water_splashes: bool,
    /// r_watercaustics: animated caustics on submerged world faces (strength).
    pub water_caustics: f32,
    /// r_shader_scope: 0 = slang preset on the 3D view only (UI drawn after
    /// it), 1 = on the whole frame including the UI.
    pub shader_scope: u32,
    /// Add-on night vision / thermal goggles.
    pub goggles: Goggles,
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
            teleport_windows: false,
            gun_portal_max: 4,
            gun_portal_dist: 4096.0,
            gun_portal_depth: 3,
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
            edge_blur: 0.0,
            water_ripples: 0.0,
            water_splashes: false,
            water_caustics: 0.0,
            shader_scope: 0,
            goggles: Goggles::default(),
        }
    }
}
