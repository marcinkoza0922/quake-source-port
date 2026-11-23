//! Graphics presets: `graphics_preset classic|enhanced|lowend` sets every
//! visual option at once. The first-run pop-up (`menu/preset.rs`) runs it
//! when a game starts with no user config of its own or inherited.
use super::*;

/// Cvar values shared by every preset unless it overrides them: the
/// enhancement masters and their sub-options, so switching presets never
/// leaves a stray effect from the previous one.
const BASE: &[(&str, &str)] = &[
    ("r_enhancedmodels", "1"),
    ("r_msaa", "4"),
    ("vid_fsaamode", "0"),
    ("r_renderscale", "1"),
    ("r_anisotropy", "16"),
    ("r_softemu", "0"),
    ("r_dynamic", "1"),
    ("r_coloredlight", "1"),
    ("r_fog", "1"),
    ("r_lerpmodels", "1"),
    ("r_lerpmove", "1"),
    ("r_ssao", "0"),
    ("r_motionblur", "0"),
    ("r_motionblur_quality", "1"),
    ("r_speedblur", "0"),
    ("r_bloom", "0"),
    ("r_bloom_quality", "1"),
    ("r_shadow_realtime_dlight", "0"),
    ("r_shadow_realtime_dlight_shadows", "1"),
    ("r_shadow_quality", "1"),
    ("r_shader_preset", ""),
    ("r_enhancedfx", "0"),
    ("r_enhancedparticles", "0"),
    ("r_particle_quality", "1"),
    ("r_impact_fx", "1"),
    ("r_impact_bullets", "0"),
    ("r_impact_nails", "0"),
    ("r_impact_scorches", "0"),
    ("r_blood", "0"),
    ("r_blood_quality", "1"),
    ("r_enhancedwater", "0"),
    ("r_water_quality", "1"),
    ("r_watertranslucency", "0"),
    // Vanilla opacity (ironwail/vkQuake default); translucent presets lower it.
    ("r_wateralpha", "1"),
    ("r_waterripples", "0"),
    ("r_watersplash", "0"),
    ("r_watercaustics", "0"),
    ("r_water_refraction", "0"),
    ("r_water_droplets", "0"),
    ("r_water_reflections", "0"),
    ("r_teleportwindows", "0"),
    ("r_portal_quality", "1"),
    ("r_debris", "0"),
    ("r_debris_quality", "1"),
];

/// Like the 2021 re-release: enhanced models, nothing else added.
const CLASSIC: &[(&str, &str)] = &[];

/// Every enhancement on at its highest quality.
const ENHANCED: &[(&str, &str)] = &[
    ("r_msaa", "8"),
    ("r_ssao", "2"),
    ("r_motionblur", "1"),
    ("r_speedblur", "1"),
    ("r_bloom", "1"),
    ("r_shadow_realtime_dlight", "1"),
    ("r_shadow_quality", "2"),
    ("r_enhancedfx", "1"),
    ("r_enhancedparticles", "1"),
    ("r_particle_quality", "2"),
    ("r_impact_fx", "4"),
    ("r_impact_bullets", "1"),
    ("r_impact_nails", "1"),
    ("r_impact_scorches", "1"),
    ("r_blood", "1"),
    ("r_blood_quality", "2"),
    ("r_enhancedwater", "1"),
    ("r_watertranslucency", "1"),
    ("r_wateralpha", "0.5"),
    ("r_waterripples", "1"),
    ("r_watersplash", "1"),
    ("r_watercaustics", "1"),
    ("r_water_refraction", "1"),
    ("r_water_droplets", "1"),
    ("r_water_reflections", "1"),
    ("r_teleportwindows", "1"),
    ("r_portal_quality", "2"),
    ("r_debris", "1"),
    ("r_debris_quality", "2"),
];

/// Light on old or integrated GPUs: no anti-aliasing, classic models,
/// lowest quality tiers.
const LOW_END: &[(&str, &str)] = &[
    ("r_enhancedmodels", "0"),
    ("r_msaa", "1"),
    ("r_anisotropy", "4"),
    ("r_motionblur_quality", "0"),
    ("r_bloom_quality", "0"),
    ("r_shadow_quality", "0"),
    ("r_particle_quality", "0"),
    ("r_blood_quality", "0"),
    ("r_water_quality", "0"),
    ("r_portal_quality", "0"),
    ("r_debris_quality", "0"),
];

/// Preset names as typed in the console, in pop-up order.
pub(crate) const NAMES: [&str; 3] = ["classic", "enhanced", "lowend"];

/// The full cvar list a preset sets, or None for an unknown name.
pub(crate) fn values(name: &str) -> Option<Vec<(&'static str, &'static str)>> {
    let over = match name.to_ascii_lowercase().as_str() {
        "classic" => CLASSIC,
        "enhanced" => ENHANCED,
        "lowend" | "low" => LOW_END,
        _ => return None,
    };
    Some(BASE.iter().map(|&(n, v)| over.iter().find(|o| o.0 == n).copied().unwrap_or((n, v))).collect())
}

pub(super) fn register_preset_commands(c: &mut Commands<Host>) {
    c.add("graphics_preset", |h, a| {
        let Some(vals) = values(a.argv(1)) else {
            h.print(&format!("graphics_preset <{}>\n", NAMES.join("|")));
            return;
        };
        for (n, v) in vals {
            h.server.core.cvars.set(n, v);
        }
        h.print(&qcore::ctr!("graphics preset: {}\n", a.argv(1).to_ascii_lowercase()));
    });
    c.add("menu_preset", |h, _| {
        h.menu.open(Screen::Main);
        h.menu.open_preset();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_only_set_registered_archived_cvars() {
        for &(n, _) in BASE {
            let c = HOST_CVARS.iter().chain(COMPAT_CVARS).find(|c| c.0 == n).unwrap_or_else(|| panic!("{n} not a host cvar"));
            assert!(c.2 & CVAR_ARCHIVE != 0, "{n} not archived");
        }
        for over in [CLASSIC, ENHANCED, LOW_END] {
            for (n, _) in over {
                assert!(BASE.iter().any(|b| b.0 == *n), "{n} missing from BASE");
            }
        }
    }

    #[test]
    fn classic_matches_the_defaults_with_enhancements_off() {
        let v = values("classic").unwrap();
        let get = |n| v.iter().find(|c| c.0 == n).unwrap().1;
        assert_eq!(get("r_enhancedmodels"), "1");
        assert_eq!(get("r_enhancedfx"), "0");
        assert_eq!(get("r_bloom"), "0");
        assert_eq!(get("r_shadow_realtime_dlight"), "0");
        assert_eq!(get("r_water_reflections"), "0");
        let e = values("enhanced").unwrap();
        let eget = |n| e.iter().find(|c| c.0 == n).unwrap().1;
        assert_eq!(eget("r_impact_fx"), "4");
        assert_eq!(eget("r_bloom"), "1");
        assert_eq!(eget("r_bloom_quality"), "1");
        assert_eq!(eget("r_shadow_realtime_dlight"), "1");
        assert_eq!(eget("r_shadow_realtime_dlight_shadows"), "1");
        assert_eq!(eget("r_shadow_quality"), "2");
        assert_eq!(eget("r_water_reflections"), "1");
        assert!(values("nope").is_none());
    }
}
