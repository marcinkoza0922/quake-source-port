//! Smart weapon auto-switch and custom weapon priority (`cl_autoswitch`, `cl_weaponpriority`).
//!
//! Vanilla Quake's `weapon_touch` unconditionally switches to picked-up weapons
//! in single player (even mid-fire, before `PlayerPostThink` calls `W_Attack`).
//! Intercepting pickup-driven weapon changes right after `physics_client_move`
//! (and after non-client entity physics for dropped backpacks) prevents accidental
//! self-damage from Rocket Launchers / Thunderbolts without interfering with
//! manual weapon switches (`ImpulseCommands`) or out-of-ammo fallbacks.

use qcore::cvar::{CVAR_ARCHIVE, Cvars};
use qvm::Vm;

use crate::fields::Fields;

/// Default priority from highest (index 0) to lowest (index 7):
/// Thunderbolt, Rocket Launcher, Super Nailgun, Grenade Launcher,
/// Double-Barrelled Shotgun, Nailgun, Shotgun, Axe.
pub const DEFAULT_ORDER: [u8; 8] = [8, 7, 5, 6, 3, 4, 2, 1];
pub const DEFAULT_PRIORITY: &str = "8 7 5 6 3 4 2 1";

/// Cvars registered on the shared server/host registry.
pub const CVARS: &[(&str, &str, u32)] = &[
    // 0 = Never, 1 = Better weapon (uses cl_weaponpriority)
    ("cl_autoswitch", "1", CVAR_ARCHIVE),
    ("cl_weaponpriority", DEFAULT_PRIORITY, CVAR_ARCHIVE),
];

/// Stock Quake weapon bit masks indexed by impulse 1..=8.
pub const IT_SHOTGUN: i32 = 1;
pub const IT_SUPER_SHOTGUN: i32 = 2;
pub const IT_NAILGUN: i32 = 4;
pub const IT_SUPER_NAILGUN: i32 = 8;
pub const IT_GRENADE_LAUNCHER: i32 = 16;
pub const IT_ROCKET_LAUNCHER: i32 = 32;
pub const IT_LIGHTNING: i32 = 64;
pub const IT_AXE: i32 = 4096;

/// HUD ammo-icon bits in `self.items` (IT_SHELLS | IT_NAILS | IT_ROCKETS | IT_CELLS).
const IT_SHELLS: i32 = 256;
const IT_NAILS: i32 = 512;
const IT_ROCKETS: i32 = 1024;
const IT_CELLS: i32 = 2048;
const AMMO_ICON_MASK: i32 = IT_SHELLS | IT_NAILS | IT_ROCKETS | IT_CELLS;

/// Maps a stock `IT_*` weapon bit to its impulse number (1..=8).
pub fn item_to_impulse(weapon_bit: i32) -> Option<u8> {
    Some(match weapon_bit {
        IT_AXE => 1,
        IT_SHOTGUN => 2,
        IT_SUPER_SHOTGUN => 3,
        IT_NAILGUN => 4,
        IT_SUPER_NAILGUN => 5,
        IT_GRENADE_LAUNCHER => 6,
        IT_ROCKET_LAUNCHER => 7,
        IT_LIGHTNING => 8,
        _ => return None,
    })
}

/// Maps a stock impulse number (1..=8) to its `IT_*` weapon bit.
pub fn impulse_to_item(impulse: u8) -> Option<i32> {
    Some(match impulse {
        1 => IT_AXE,
        2 => IT_SHOTGUN,
        3 => IT_SUPER_SHOTGUN,
        4 => IT_NAILGUN,
        5 => IT_SUPER_NAILGUN,
        6 => IT_GRENADE_LAUNCHER,
        7 => IT_ROCKET_LAUNCHER,
        8 => IT_LIGHTNING,
        _ => return None,
    })
}

/// Parses `cl_weaponpriority` into a complete, deduplicated permutation of 1..=8
/// (highest priority first). Missing entries are appended in [`DEFAULT_ORDER`].
pub fn parse_priority(s: &str) -> [u8; 8] {
    let mut out = [0u8; 8];
    let mut seen = [false; 9];
    let mut len = 0usize;
    for tok in s.split(|c: char| !c.is_ascii_digit()) {
        if let Ok(n) = tok.parse::<u8>()
            && (1..=8).contains(&n)
            && !seen[n as usize]
        {
            seen[n as usize] = true;
            out[len] = n;
            len += 1;
            if len == 8 {
                return out;
            }
        }
    }
    for n in DEFAULT_ORDER {
        if !seen[n as usize] {
            out[len] = n;
            len += 1;
        }
    }
    out
}

/// Serialises a priority permutation back to the `cl_weaponpriority` cvar format.
pub fn format_priority(order: &[u8; 8]) -> String {
    order.map(|n| n.to_string()).join(" ")
}

/// Reads the current priority order from cvars.
pub fn read_priority(cv: &Cvars) -> [u8; 8] {
    let s = cv.by_name("cl_weaponpriority").map_or(DEFAULT_PRIORITY, |c| c.string.as_str());
    parse_priority(s)
}

/// Priority rank (0 = highest, 7 = lowest) of a stock impulse in `order`.
pub fn rank(order: &[u8; 8], impulse: u8) -> Option<usize> {
    order.iter().position(|&n| n == impulse)
}

/// Whether picking up `new_weapon` while holding `old_weapon` should switch.
///
/// - `mode == 0` ("Never"): always `false`.
/// - `mode != 0` ("Better weapon"): switches only when `new_weapon` has strictly
///   higher priority than `old_weapon` in `order`. For unknown mod-only weapon
///   bits, switches only when the weapon is newly acquired (`old_items & new_weapon == 0`).
pub fn should_switch(mode: i32, order: &[u8; 8], old_weapon: i32, new_weapon: i32, is_new_pickup: bool, new_is_mod: bool, old_is_mod: bool) -> bool {
    if mode == 0 {
        return false;
    }
    let old_imp = (!old_is_mod).then(|| item_to_impulse(old_weapon)).flatten();
    let new_imp = (!new_is_mod).then(|| item_to_impulse(new_weapon)).flatten();
    match (old_imp, new_imp) {
        (Some(o), Some(n)) => rank(order, n) < rank(order, o),
        // Unknown mod weapon picked up: allow only when newly acquired.
        (_, None) => is_new_pickup,
        // Holding an unknown mod weapon and walked over a stock weapon: keep the mod weapon.
        (None, Some(_)) => false,
    }
}

/// Snapshot of a client edict's active weapon state before pickup touches run.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WeaponSnapshot {
    weapon: i32,
    weaponmodel: i32,
    weaponframe: f32,
    currentammo: f32,
    items: i32,
    moditems: i32,
    think: i32,
    frame: f32,
    nextthink: f32,
    health: f32,
    weaponismoditems: f32,
}

impl WeaponSnapshot {
    pub(crate) fn capture(vm: &Vm, f: &Fields, e: u32) -> Self {
        Self {
            weapon: vm.ent_f(e, f.weapon) as i32,
            weaponmodel: vm.ent_i(e, f.weaponmodel),
            weaponframe: vm.ent_f(e, f.weaponframe),
            currentammo: vm.ent_f(e, f.currentammo),
            items: vm.ent_f(e, f.items) as i32,
            moditems: f.moditems.map_or(0, |o| vm.ent_f(e, o) as i32),
            think: vm.ent_i(e, f.think),
            frame: vm.ent_f(e, f.frame),
            nextthink: vm.ent_f(e, f.nextthink),
            health: vm.ent_f(e, f.health),
            weaponismoditems: f.weaponismoditems.map_or(0.0, |o| vm.ent_f(e, o)),
        }
    }

    /// Reverts an unwanted pickup auto-switch while preserving newly gained
    /// inventory items and updating `currentammo` if matching ammo was gained.
    pub(crate) fn apply(self, vm: &mut Vm, f: &Fields, cv: &Cvars, e: u32) {
        if self.weapon == 0 || self.health <= 0.0 || vm.is_free(e) {
            return;
        }
        let new_health = vm.ent_f(e, f.health);
        if new_health <= 0.0 {
            return;
        }
        let new_weapon = vm.ent_f(e, f.weapon) as i32;
        let new_is_mod = f.weaponismoditems.is_some_and(|o| vm.ent_f(e, o) != 0.0);
        let old_is_mod = self.weaponismoditems != 0.0;
        if new_weapon == self.weapon && new_is_mod == old_is_mod {
            return;
        }
        // Only keep the old weapon if the player still owns it (a mod script
        // that strips weapons is always respected).
        let new_items = vm.ent_f(e, f.items) as i32;
        let owns_old = if old_is_mod {
            f.moditems.is_none_or(|o| vm.ent_f(e, o) as i32 & self.weapon != 0)
        } else {
            new_items & self.weapon != 0
        };
        if !owns_old {
            return;
        }
        let is_new_pickup = if new_is_mod {
            self.moditems & new_weapon == 0
        } else {
            self.items & new_weapon == 0
        };
        let mode = (cv.value_of("cl_autoswitch").round() as i32).clamp(0, 1);
        let order = read_priority(cv);
        if should_switch(mode, &order, self.weapon, new_weapon, is_new_pickup, new_is_mod, old_is_mod) {
            return;
        }
        // Restore weapon state without interrupting firing animation or losing picked-up items/ammo.
        vm.set_ent_f(e, f.weapon, self.weapon as f32);
        vm.set_ent_i(e, f.weaponmodel, self.weaponmodel);
        vm.set_ent_f(e, f.weaponframe, self.weaponframe);
        vm.set_ent_i(e, f.think, self.think);
        vm.set_ent_f(e, f.frame, self.frame);
        vm.set_ent_f(e, f.nextthink, self.nextthink);
        if let Some(o) = f.weaponismoditems {
            vm.set_ent_f(e, o, self.weaponismoditems);
        }
        let merged_items = (new_items & !AMMO_ICON_MASK) | (self.items & AMMO_ICON_MASK);
        vm.set_ent_f(e, f.items, merged_items as f32);
        let ammo = match self.items & AMMO_ICON_MASK {
            IT_SHELLS => vm.ent_f(e, f.ammo_shells),
            IT_NAILS => vm.ent_f(e, f.ammo_nails),
            IT_ROCKETS => vm.ent_f(e, f.ammo_rockets),
            IT_CELLS => vm.ent_f(e, f.ammo_cells),
            _ => self.currentammo,
        };
        vm.set_ent_f(e, f.currentammo, ammo);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_priority_deduplicates_and_fills_missing() {
        assert_eq!(parse_priority(DEFAULT_PRIORITY), DEFAULT_ORDER);
        assert_eq!(parse_priority(""), DEFAULT_ORDER);
        assert_eq!(parse_priority("3 5 3 99 0"), [3, 5, 8, 7, 6, 4, 2, 1]);
        assert_eq!(format_priority(&[3, 5, 8, 7, 6, 4, 2, 1]), "3 5 8 7 6 4 2 1");
    }

    #[test]
    fn never_mode_never_switches() {
        let order = DEFAULT_ORDER;
        assert!(!should_switch(0, &order, IT_AXE, IT_ROCKET_LAUNCHER, true, false, false));
        assert!(!should_switch(0, &order, IT_SHOTGUN, IT_LIGHTNING, true, false, false));
    }

    #[test]
    fn better_weapon_respects_custom_priority() {
        // Put SSG (#3) and SNG (#5) above RL (#7) and LG (#8) to prevent self-damage!
        let order = parse_priority("5 3 4 2 6 7 8 1");
        assert!(!should_switch(1, &order, IT_SUPER_SHOTGUN, IT_ROCKET_LAUNCHER, true, false, false));
        assert!(!should_switch(1, &order, IT_SUPER_SHOTGUN, IT_LIGHTNING, true, false, false));
        assert!(should_switch(1, &order, IT_SUPER_SHOTGUN, IT_SUPER_NAILGUN, true, false, false));
        assert!(should_switch(1, &order, IT_AXE, IT_ROCKET_LAUNCHER, true, false, false));
    }
}
