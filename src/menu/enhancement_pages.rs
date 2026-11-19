//! The Enhancements page and its sub-pages (FX, Water, Bloom, Real-time
//! Lights, Debris).

use super::*;

pub(super) const ENHANCEMENTS: &[Item] = &[
    Item::Choice { label: "Ambient Occlusion", cvar: "r_ssao", choices: SSAO },
    toggle("Motion Blur", "r_motionblur"),
    Item::Choice { label: "Motion Blur Quality", cvar: "r_motionblur_quality", choices: MOTIONBLUR_QUALITY },
    slider("Motion Blur Amount", "r_motionblur_amount", 0.1, 2.0, 0.1),
    toggle("Speed Blur", "r_speedblur"),
    toggle("Bloom", "r_bloom"),
    Item::Sub { label: "Bloom Settings", page: Page::Bloom },
    toggle("Real-time Dynamic Lights", "r_shadow_realtime_dlight"),
    Item::Sub { label: "Real-time Light Settings", page: Page::RtLights },
    Item::Sub { label: "Screen Shader", page: Page::ScreenShader },
    toggle("Enhanced FX", "r_enhancedfx"),
    Item::Sub { label: "FX Settings", page: Page::Impacts },
    toggle("Enhanced Water", "r_enhancedwater"),
    Item::Sub { label: "Water Settings", page: Page::Water },
    toggle("See-through Teleporters", "r_teleportwindows"),
    Item::Choice { label: "Portal View Quality", cvar: "r_portal_quality", choices: PORTAL_QUALITY },
    toggle("Debris", "r_debris"),
    Item::Sub { label: "Debris Settings", page: Page::Debris },
];

pub(super) const IMPACT_FX: &[(&str, f32)] =
    &[("off", 0.0), ("subtle", 1.0), ("normal", 2.0), ("intense", 3.0), ("spectacle", 4.0)];

pub(super) const IMPACTS: &[Item] = &[
    toggle("Enhanced Particles", "r_enhancedparticles"),
    Item::Choice { label: "Particle Quality", cvar: "r_particle_quality", choices: DEBRIS_QUALITY },
    Item::Choice { label: "Surface Hit Effects", cvar: "r_impact_fx", choices: IMPACT_FX },
    toggle("Shotgun Bullet Decals", "r_impact_bullets"),
    toggle("Physical Nail Impacts", "r_impact_nails"),
    toggle("Explosion Scorch Marks", "r_impact_scorches"),
    slider("Impact Lifetime", "r_impact_lifetime", 5.0, 600.0, 5.0),
    slider("Maximum Impacts", "r_impact_limit", 32.0, 2048.0, 32.0),
    toggle("Enhanced Blood Splatter", "r_blood"),
    Item::Sub { label: "Blood Splatter Settings", page: Page::Blood },
];

/// Reached from Enhancements; unavailable while `r_enhancedwater` is off.
pub(super) const WATER: &[Item] = &[
    Item::Choice { label: "Water Quality", cvar: "r_water_quality", choices: WATER_QUALITY },
    toggle("Translucent Water", "r_watertranslucency"),
    slider("Water Opacity", "r_wateralpha", 0.0, 1.0, 0.1),
    toggle("Ripples", "r_waterripples"),
    slider("Ripple Intensity", "r_waterripples_amount", 0.1, 2.0, 0.1),
    toggle("Splashes", "r_watersplash"),
    toggle("Caustics", "r_watercaustics"),
    slider("Caustic Intensity", "r_watercaustics_amount", 0.1, 2.0, 0.1),
    Item::Choice { label: "Reflections", cvar: "r_water_reflections", choices: WATER_REFLECTIONS },
    slider("Reflection Strength", "r_water_reflections_amount", 0.25, 1.5, 0.25),
];

pub(super) const WATER_REFLECTIONS: &[(&str, f32)] = &[("off", 0.0), ("planar", 1.0), ("ray traced", 2.0)];

pub(super) const BLOOM_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("high", 1.0)];

/// Reached from Enhancements; unavailable while `r_bloom` is off.
pub(super) const BLOOM: &[Item] = &[
    Item::Choice { label: "Bloom Quality", cvar: "r_bloom_quality", choices: BLOOM_QUALITY },
    slider("Bloom Intensity", "r_bloom_intensity", 0.25, 3.0, 0.25),
    slider("Bloom Size", "r_bloom_size", 0.5, 2.0, 0.1),
    slider("Bright Surfaces", "r_bloom_bright", 0.0, 1.0, 0.1),
];

/// Reached from Enhancements; unavailable while `r_shadow_realtime_dlight` is off.
pub(super) const RT_LIGHTS: &[Item] = &[
    toggle("Shadows", "r_shadow_realtime_dlight_shadows"),
    Item::Choice { label: "Shadow Quality", cvar: "r_shadow_quality", choices: PORTAL_QUALITY },
];

pub(super) const DEBRIS_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("medium", 1.0), ("high", 2.0)];
#[allow(dead_code)] // ragdoll options are hidden
pub(super) const RAGDOLL_START: &[(&str, f32)] = &[("after animation", 0.0), ("at death", 1.0), ("mixed", 2.0)];

/// Reached from Enhancements; unavailable while `r_debris` is off.
pub(super) const DEBRIS: &[Item] = &[
    Item::Choice { label: "Debris Quality", cvar: "r_debris_quality", choices: DEBRIS_QUALITY },
    toggle("Gibs", "r_debris_gibs"),
    toggle("Shrapnel", "r_debris_shrapnel"),
    toggle("Hit Doors & Lifts", "r_debris_movers"),
    slider("Push Strength", "r_debris_force", 0.0, 6.0, 0.25),
];
