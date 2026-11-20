//! Cvar-like renderer settings (`Settings`) and gl_texturemode parsing.

use super::Goggles;

#[derive(Clone, Copy, Debug, Default)]
pub struct MapperSettings {
    /// 1 draws visible edges, 2 includes overdraw behind depth.
    pub wire: u32,
    pub lightmap: bool,
    pub lock_pvs: bool,
    pub speeds: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MapperStats {
    pub world_polys: u32,
    pub entity_polys: u32,
    pub triangles: u32,
    pub leaves: u32,
    pub draws: u32,
}

impl MapperSettings {
    pub(crate) fn lightmap_flag(self) -> u32 {
        u32::from(self.lightmap) << 5
    }
}

/// Cvar-like settings.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub gamma: f32,
    pub contrast: f32,
    pub wateralpha: f32,
    /// `wateralpha` is the vanilla `r_wateralpha`: it applies only where the
    /// map's VIS sees through water (or there is no VIS); elsewhere water
    /// stays opaque, as in ironwail/vkQuake (`map_wateralpha` at load).
    pub wateralpha_vised_only: bool,
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
    /// r_fullbright: world lightmaps read as neutral light and alias models
    /// get ironwail's 0.5 light colour (single-player cheat).
    pub fullbright: bool,
    pub mapper: MapperSettings,
    /// Sky_SetupFrame: skybox wind vector (world space, xyz) and loop phase
    /// (w); zero = still.
    pub sky_wind: [f32; 4],
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
    /// Soft particle MSAA depth quality (0 = low / sample 0, 1 = high / all samples).
    pub soft_particle_quality: u32,
    /// r_particles: classic particle shape (1 GLQuake round, 2 software square).
    pub particle_style: u32,
    /// r_ssao / r_ssao_strength: screen-space ambient occlusion strength (0 = off).
    pub ssao: f32,
    /// SSAO resolution and sample quality (0 = low, 1 = medium, 2 = high).
    pub ssao_quality: u32,
    /// Portal & teleporter destination view quality (0 = low, 1 = medium, 2 = high).
    pub portal_quality: u32,
    /// r_ssao_radius: occlusion radius in world units.
    pub ssao_radius: f32,
    /// r_motionblur: per-object motion blur; the blur covers this fraction of
    /// the motion over 1/60 s (0 = off). Camera motion is not blurred.
    pub motion_blur: f32,
    /// r_motionblur_quality: per-object motion blur quality (0 = low, 1 = high).
    pub motion_blur_quality: u32,
    /// r_bloom ? r_bloom_intensity : 0. Light sources (fullbright texels,
    /// lava, teleporters, bright effects) glow; 0 disables.
    pub bloom: f32,
    /// r_bloom_size: weight of the wider blur levels (0.5..=2, 1 = default).
    pub bloom_size: f32,
    /// r_bloom_bright: bright scene colours also glow, as in DarkPlaces (0..=1).
    pub bloom_bright: f32,
    /// r_bloom_quality: 0 = low (chain from quarter resolution), 1 = high (half).
    pub bloom_quality: u32,
    /// r_shadow_realtime_dlight: dynamic lights are lit per pixel on the
    /// world and models instead of being added to the lightmaps.
    pub rt_dlights: bool,
    /// r_shadow_realtime_dlight_shadows: the most important ones cast shadows.
    pub dlight_shadows: bool,
    /// r_shadow_quality: 0 low (2 shadowed lights, 256² faces), 1 medium
    /// (4, 512²), 2 high (8, 512², softer filtering).
    pub shadow_quality: u32,
    /// Peripheral radial blur independent of camera velocity.
    pub edge_blur: f32,
    /// r_waterripples: ripple rings and gentle swell on liquid surfaces
    /// (strength, 0 = classic flat warp).
    pub water_ripples: f32,
    /// r_watersplash: droplet splashes when things enter or leave liquids.
    pub water_splashes: bool,
    /// r_watercaustics: animated caustics on submerged world faces (strength).
    pub water_caustics: f32,
    /// r_water_quality: enhanced water caustics, ripples & splashes quality (0 = low, 1 = high).
    pub water_quality: u32,
    /// r_water_reflections: 0 off, 1 planar (the scene mirrored across each
    /// nearby horizontal water/slime plane, drawn as a portal view), 2 ray
    /// traced (ray queries from every water/slime pixel; planar on devices
    /// without them).
    pub water_reflections: u32,
    /// r_water_reflections_amount: reflection strength (0..=1.5).
    pub water_reflection_amount: f32,
    /// addon_gel_quality: gel coat & droplet shading quality (0 = low / single fetch, 1 = high).
    pub gel_quality: u32,
    /// r_shader_scope: 0 = slang preset on the 3D view only (UI drawn after
    /// it), 1 = on the whole frame including the UI.
    pub shader_scope: u32,
    /// r_shader_scale: 0 = cap slang preset input at output resolution,
    /// 1 = match full 3D scene resolution.
    pub shader_scale: u32,
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
            wateralpha_vised_only: false,
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
            fullbright: false,
            mapper: MapperSettings::default(),
            sky_wind: [0.0; 4],
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
            soft_particle_quality: 0,
            particle_style: 1,
            ssao: 0.0,
            ssao_quality: 1,
            portal_quality: 1,
            ssao_radius: 32.0,
            motion_blur: 0.0,
            motion_blur_quality: 1,
            water_reflections: 0,
            water_reflection_amount: 1.0,
            bloom: 0.0,
            bloom_size: 1.0,
            bloom_bright: 0.0,
            bloom_quality: 1,
            rt_dlights: false,
            dlight_shadows: true,
            shadow_quality: 1,
            edge_blur: 0.0,
            water_ripples: 0.0,
            water_splashes: false,
            water_caustics: 0.0,
            water_quality: 1,
            gel_quality: 1,
            shader_scope: 0,
            shader_scale: 0,
            goggles: Goggles::default(),
        }
    }
}
