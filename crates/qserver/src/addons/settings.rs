//! What a customized game is made of: saved with it (`setting` lines of the
//! `.qsa` header), never in the config. Preferences that don't change play
//! (weapon slots, colours, quality, controls) are archived instead. Only the
//! listed cvars may be restored from a save header.
use qcore::cvar::Cvars;

use super::Toy;

/// Besides the toys (`toy` lines) and the modifiers (`modifier` lines).
const NAMES: &[&str] = &[
    "addon_dodge", "addon_goggles", "addon_projectile_shotguns", "addon_shotgun_speed", "addon_bullet_time",
    "addon_bullet_time_toggle", "addon_bullet_time_meter",
    "addon_bullet_time_duration", "addon_bullet_time_regen",
    "addon_hitstop", "addon_hitstop_weapons", "addon_hitstop_targets", "addon_hitstop_duration",
    "addon_hitstop_scale_damage", "addon_hitstop_kill_bonus", "addon_hitstop_cooldown",
    "addon_hitstop_max", "addon_hitstop_min_damage", "addon_hitstop_tough",
    "addon_sticky_damage", "addon_boomerang_stun", "addon_boomerang_speed", "addon_rail_damage",
    "addon_hook_range", "addon_hook_wood", "addon_hook_metal", "addon_hook_stone", "addon_hook_enemies",
    "addon_washer_push", "addon_cable_length", "addon_cable_metal", "addon_cable_damage",
    "addon_akimbo_damage", "addon_akimbo_spread", "addon_akimbo_mag", "addon_akimbo_reload",
    "addon_rifle_damage", "addon_rifle_mag", "addon_rifle_reload", "addon_rifle_recoil",
];

/// Every cvar that belongs to a game and its saves.
pub fn per_game() -> impl Iterator<Item = &'static str> {
    Toy::ALL.into_iter().map(Toy::cvar).chain(NAMES.iter().copied()).chain(crate::modifiers::CVARS.iter().map(|c| c.0))
}

pub(super) fn save(cv: &Cvars) -> String {
    let known = NAMES.iter().filter(|n| cv.by_name(n).is_some());
    known.map(|name| format!("setting {name} {}\n", cv.value_of(name))).collect()
}

/// Saves from before a setting existed get its default.
pub(crate) fn load(cv: &mut Cvars, text: &str) {
    for name in NAMES {
        if let Some(d) = cv.by_name(name).map(|c| c.default.clone()) {
            cv.set(name, &d);
        }
    }
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("setting") { continue; }
        let (Some(name), Some(value)) = (words.next(), words.next()) else { continue; };
        if NAMES.contains(&name) && value.parse::<f32>().is_ok_and(f32::is_finite) {
            cv.set(name, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut cv = Cvars::new();
        for &(n, d, f) in crate::addons::CVARS {
            cv.register(n, d, f);
        }
        cv.set("addon_rail_damage", "250");
        cv.set("addon_portalgun_slot", "3");
        let text = save(&cv);
        cv.set("addon_rail_damage", "1");
        cv.set("addon_sticky_damage", "1");
        load(&mut cv, &text);
        assert_eq!((cv.value_of("addon_rail_damage"), cv.value_of("addon_sticky_damage")), (250.0, 120.0));
        // an old or hostile header
        load(&mut cv, "setting addon_portalgun_slot 9\nsetting addon_rail_damage nan\nsetting skill 3\n");
        assert_eq!((cv.value_of("addon_rail_damage"), cv.value_of("addon_portalgun_slot")), (100.0, 3.0));
    }
}
