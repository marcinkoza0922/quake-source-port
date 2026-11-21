//! Slider values with their units: percentages, multipliers, degrees,
//! seconds and decibels. Symbols only, so nothing needs translating.

use qcore::cvar::format_float;

enum Unit {
    /// 0–1 fractions shown as 0–100%.
    Percent,
    /// Strength or scale relative to normal.
    Times,
    Degrees,
    DegreesPerSecond,
    Seconds,
    Decibels,
    Plain,
}

fn unit(cvar: &str) -> Unit {
    match cvar {
        "volume" | "bgmvolume" | "s_vol_ambient" | "s_vol_weapons" | "s_vol_monsters" | "s_vol_world" | "s_footsteps_volume"
        | "s_hit_volume" | "s_kill_volume" | "s_hit_drive" | "s_kill_drive" | "s_hit_echo" | "s_kill_echo" | "s_hit_reverb"
        | "s_kill_reverb" | "assist_grenadearc_alpha" | "cl_automap_alpha" | "touch_opacity" | "r_wateralpha" | "v_flashscale"
        | "r_skyfog" | "joy_rumble" | "addon_hitstop_shake" | "touch_size" | "r_deluxemapping_specular" | "r_bloom_bright"
        | "aim_rotation" | "aim_slowdown" => Unit::Percent,
        "r_motionblur_amount" | "r_bloom_intensity" | "r_bloom_size" | "r_watercaustics_amount" | "r_waterripples_amount"
        | "r_water_reflections_amount" | "s_reverb_amount" | "addon_goggles_intensity" | "r_blood_size" | "cl_automap_zoom"
        | "s_hit_pitch" | "s_kill_pitch" | "s_hit_speed" | "s_kill_speed" | "s_hit_length" | "s_kill_length"
        | "addon_washer_push" | "r_debris_force" => Unit::Times,
        "fov" | "assist_autoaim_cone" => Unit::Degrees,
        "joy_sensitivity_pitch" | "joy_sensitivity_yaw" => Unit::DegreesPerSecond,
        "s_music_crossfade" | "r_impact_lifetime" | "joy_flick_time" | "addon_boomerang_stun" => Unit::Seconds,
        "s_hit_bass" | "s_kill_bass" | "s_hit_treble" | "s_kill_treble" => Unit::Decibels,
        _ => Unit::Plain,
    }
}

/// The value column text of a slider.
pub(super) fn slider_text(cvar: &str, v: f32) -> String {
    let num = |x: f32| format_float((x * 1000.0).round() / 1000.0);
    match unit(cvar) {
        Unit::Percent => format!("{}%", (v * 100.0).round()),
        Unit::Times => format!("{}×", num(v)),
        Unit::Degrees => format!("{}°", num(v)),
        Unit::DegreesPerSecond => format!("{}°/s", num(v)),
        Unit::Seconds => format!("{} s", num(v)),
        Unit::Decibels if v > 0.0 => format!("+{} dB", num(v)),
        Unit::Decibels => format!("{} dB", num(v)),
        Unit::Plain => num(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_values_carry_units() {
        assert_eq!(slider_text("volume", 0.7), "70%");
        assert_eq!(slider_text("fov", 90.0), "90°");
        assert_eq!(slider_text("r_bloom_intensity", 1.25), "1.25×");
        assert_eq!(slider_text("s_music_crossfade", 0.5), "0.5 s");
        assert_eq!(slider_text("s_hit_bass", 3.0), "+3 dB");
        assert_eq!(slider_text("s_hit_bass", -3.0), "-3 dB");
        assert_eq!(slider_text("cl_bob", 0.02), "0.02");
    }
}
