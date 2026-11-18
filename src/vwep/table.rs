//! Which model each weapon shows in the player's hands, and which weapon a
//! remote player's fire sound gives away.

use super::rig::Spec;

/// What a player holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Held {
    /// The model's own axe.
    Axe,
    /// A model fitted to the hand.
    Gun(Spec),
}

/// Pickup (or view) model in hand per view model, with its fit tuning.
/// Rogue's lava and multi weapons reuse the base pickups, as rogue does;
/// hipnotic's Mjolnir has its own player model (playham.mdl).
const GUNS: &[(&str, &str, f32, [f32; 3])] = &[
    ("v_shot", "v_shot", 1.0, [2.0, 0.0, 0.0]),
    ("v_shot2", "g_shot", 0.7, [4.0, 0.0, 0.0]),
    ("v_nail", "g_nail", 0.8, [3.0, 0.0, 0.0]),
    ("v_nail2", "g_nail2", 0.7, [4.0, 0.0, 0.0]),
    ("v_rock", "g_rock", 0.8, [3.0, 0.0, 0.0]),
    ("v_rock2", "g_rock2", 0.65, [5.0, 0.0, 0.0]),
    ("v_light", "g_light", 0.75, [4.0, 0.0, 0.0]),
    ("v_laserg", "g_laserg", 0.6, [4.0, 0.0, 0.0]),
    ("v_prox", "g_prox", 0.8, [3.0, 0.0, 0.0]),
    ("v_lava", "g_nail", 0.8, [3.0, 0.0, 0.0]),
    ("v_lava2", "g_nail2", 0.7, [4.0, 0.0, 0.0]),
    ("v_multi", "g_rock", 0.8, [3.0, 0.0, 0.0]),
    ("v_multi2", "g_rock2", 0.65, [5.0, 0.0, 0.0]),
    ("v_plasma", "v_plasma", 1.0, [2.0, 0.0, 0.0]),
];

/// View models that never get a fitted model (the player model or the
/// add-on shows its own).
const NONE: &[&str] = &["v_hammer", "v_grpple", "v_star"];

/// What `weaponmodel` (a `progs/v_*.mdl` path) puts in the hands. Unknown
/// `v_<name>` models fall back to `g_<name>` when `exists` finds it.
pub fn lookup(weaponmodel: &str, exists: impl Fn(&str) -> bool) -> Option<Held> {
    let name = weaponmodel.to_ascii_lowercase();
    let stem = name.strip_prefix("progs/")?.strip_suffix(".mdl")?;
    if stem == "v_axe" {
        return Some(Held::Axe);
    }
    if NONE.contains(&stem) {
        return None;
    }
    let spec = |source: &str, scale, offset| Spec {
        source: format!("progs/{source}.mdl"),
        scale,
        offset,
        front_only: source.starts_with("v_"),
    };
    if let Some(&(_, source, scale, offset)) = GUNS.iter().find(|g| g.0 == stem) {
        return Some(Held::Gun(spec(source, scale, offset)));
    }
    let pickup = format!("g_{}", stem.strip_prefix("v_")?);
    exists(&format!("progs/{pickup}.mdl")).then(|| Held::Gun(spec(&pickup, 0.75, [3.0, 0.0, 0.0])))
}

/// The weapon whose fire sound this is (vanilla, hipnotic and rogue).
pub fn from_sound(sound: &str) -> Option<&'static str> {
    Some(match sound.to_ascii_lowercase().as_str() {
        "weapons/ax1.wav" => "progs/v_axe.mdl",
        "weapons/guncock.wav" => "progs/v_shot.mdl",
        "weapons/shotgn2.wav" => "progs/v_shot2.mdl",
        "weapons/rocket1i.wav" => "progs/v_nail.mdl",
        "weapons/spike2.wav" => "progs/v_nail2.mdl",
        "weapons/grenade.wav" => "progs/v_rock.mdl",
        "weapons/sgun1.wav" => "progs/v_rock2.mdl",
        "weapons/lstart.wav" | "weapons/lhit.wav" => "progs/v_light.mdl",
        "hipweap/laserg.wav" => "progs/v_laserg.mdl",
        "hipweap/proxbomb.wav" => "progs/v_prox.mdl",
        "lavagun/snail.wav" => "progs/v_lava.mdl",
        "plasma/fire.wav" => "progs/v_plasma.mdl",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_weapons_map_to_pickups() {
        let none = |_: &str| false;
        assert_eq!(lookup("progs/v_axe.mdl", none), Some(Held::Axe));
        let Some(Held::Gun(s)) = lookup("progs/v_rock2.mdl", none) else { panic!() };
        assert_eq!(s.source, "progs/g_rock2.mdl");
        assert!(!s.front_only);
        let Some(Held::Gun(s)) = lookup("PROGS/V_SHOT.MDL", none) else { panic!() };
        assert!(s.front_only && s.source == "progs/v_shot.mdl");
        assert_eq!(lookup("progs/v_hammer.mdl", none), None);
        assert_eq!(lookup("", none), None);
    }

    #[test]
    fn unknown_view_models_use_a_matching_pickup() {
        let Some(Held::Gun(s)) = lookup("progs/v_blaster.mdl", |p| p == "progs/g_blaster.mdl") else { panic!() };
        assert_eq!(s.source, "progs/g_blaster.mdl");
        assert_eq!(lookup("progs/v_blaster.mdl", |_| false), None);
    }

    #[test]
    fn fire_sounds_name_their_weapon() {
        assert_eq!(from_sound("weapons/sgun1.wav"), Some("progs/v_rock2.mdl"));
        assert_eq!(from_sound("weapons/r_exp3.wav"), None);
        for w in ["weapons/ax1.wav", "weapons/guncock.wav", "weapons/shotgn2.wav", "weapons/rocket1i.wav", "weapons/spike2.wav", "weapons/grenade.wav", "weapons/lhit.wav"] {
            assert!(lookup(from_sound(w).unwrap(), |_| false).is_some(), "{w}");
        }
    }
}
