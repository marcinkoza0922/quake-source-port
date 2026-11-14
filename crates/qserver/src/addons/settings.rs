//! Save gameplay addon configuration with each add-on game. Only explicitly
//! listed cvars may be restored from a save header.
use qcore::cvar::Cvars;
const NAMES: &[&str] = &[
    "addon_dodge", "addon_projectile_shotguns", "addon_shotgun_speed", "addon_bullet_time",
    "addon_bullet_time_toggle", "addon_bullet_time_meter",
    "addon_bullet_time_duration", "addon_bullet_time_regen",
];
pub(super) fn save(cv: &Cvars) -> String {
    NAMES.iter().map(|name| format!("setting {name} {}\n", cv.value_of(name))).collect()
}
pub(crate) fn load(cv: &mut Cvars, text: &str) {
    for &(name, default, _) in super::CVARS {
        if NAMES.contains(&name) { cv.set(name, default); }
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
