//! Weapon identities and slot ordering shared by wheel and HUD.
use super::*;

pub struct Slot {
    pub impulse: u8,
    pub item: i32,
    pub name: &'static str,
    pub icon: Option<&'static str>,
    pub ammo: Option<usize>,
}

pub const SLOTS: [Slot; 8] = [
    Slot { impulse: 1, item: 4096, name: "Axe", icon: None, ammo: None },
    Slot { impulse: 2, item: 1, name: "Shotgun", icon: Some("inv_shotgun"), ammo: Some(STAT_SHELLS) },
    Slot { impulse: 3, item: 2, name: "Double-Barrelled Shotgun", icon: Some("inv_sshotgun"), ammo: Some(STAT_SHELLS) },
    Slot { impulse: 4, item: 4, name: "Nailgun", icon: Some("inv_nailgun"), ammo: Some(STAT_NAILS) },
    Slot { impulse: 5, item: 8, name: "Super Nailgun", icon: Some("inv_snailgun"), ammo: Some(STAT_NAILS) },
    Slot { impulse: 6, item: 16, name: "Grenade Launcher", icon: Some("inv_rlaunch"), ammo: Some(STAT_ROCKETS) },
    Slot { impulse: 7, item: 32, name: "Rocket Launcher", icon: Some("inv_srlaunch"), ammo: Some(STAT_ROCKETS) },
    Slot { impulse: 8, item: 64, name: "Thunderbolt", icon: Some("inv_lightng"), ammo: Some(STAT_CELLS) },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weapon { Stock(u8), Addon(qserver::addons::Toy) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry { pub weapon: Weapon, pub slot: u8 }

pub fn inventory(cv: &qcore::cvar::Cvars, enabled: &[qserver::addons::Toy]) -> Vec<Entry> {
    let mut entries: Vec<_> = SLOTS.iter().map(|s| Entry { weapon: Weapon::Stock(s.impulse), slot: s.impulse }).collect();
    entries.extend(enabled.iter().map(|&t| Entry { weapon: Weapon::Addon(t), slot: cv.value_of(t.slot_cvar()).clamp(1.0, 10.0) as u8 }));
    entries.sort_by_key(|e| e.slot);
    entries
}

impl Entry {
    fn stock(self) -> Option<&'static Slot> {
        match self.weapon { Weapon::Stock(slot) => SLOTS.get(slot as usize - 1), _ => None }
    }
    pub fn name(self) -> &'static str {
        match self.weapon { Weapon::Addon(t) => t.name(), _ => self.stock().unwrap().name }
    }
    pub fn label(self) -> &'static str {
        use qserver::addons::Toy;
        match self.weapon {
            Weapon::Addon(Toy::PortalGun) => "PG", Weapon::Addon(Toy::Boomerang) => "BM",
            Weapon::Addon(Toy::Sticky) => "SB", Weapon::Addon(Toy::Hookshot) => "HK",
            Weapon::Addon(Toy::Railgun) => "RG", _ => "AX",
        }
    }
    pub fn icon(self) -> Option<&'static str> { self.stock().and_then(|s| s.icon) }
    pub fn ammo(self) -> Option<usize> {
        use qserver::addons::Toy;
        match self.weapon {
            Weapon::Addon(Toy::Sticky) => Some(STAT_ROCKETS),
            Weapon::Addon(Toy::Railgun) => Some(STAT_CELLS),
            Weapon::Addon(_) => None, _ => self.stock().and_then(|s| s.ammo),
        }
    }
    pub fn owned(self, cl: &ClientState) -> bool { self.stock().is_none_or(|s| cl.items & s.item != 0) }
    pub fn usable(self, cl: &ClientState) -> bool {
        let needed = if self.weapon == Weapon::Addon(qserver::addons::Toy::Railgun) {
            qserver::addons::railgun::AMMO as i32
        } else { 1 };
        self.owned(cl) && self.ammo().is_none_or(|a| cl.stats[a] >= needed)
    }
    pub fn active(self, cl: &ClientState, held: Option<qserver::addons::Toy>) -> bool {
        match self.weapon {
            Weapon::Addon(t) => held == Some(t),
            _ => held.is_none() && self.stock().is_some_and(|s| s.item == cl.stats[STAT_ACTIVEWEAPON]),
        }
    }
}
