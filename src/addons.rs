//! Local weapon inventory selection and add-on usercmd buttons. Enabled
//! add-on weapons are owned from the start and share stock weapon slots.

pub mod hook_fx;
mod hook_model;
pub mod boomerang_fx;
mod boomerang_model;
pub mod equip_wheel;
pub mod goggles;
pub mod portal_draw;
pub mod portal_fx;
pub mod portal_model;
mod portal_skin;
pub mod rail_fx;
mod rail_model;
pub mod sticky_draw;
pub mod sticky_model;

use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, TOY_SHIFT, Toy};
use crate::wheel::{Entry, Weapon};

/// One local player's weapon selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponSelection {
    pub held: Option<Toy>,
    last_slot: Option<u8>,
    pending_base: Option<(u8, i32)>,
}

impl WeaponSelection {
    /// Select a weapon slot independently of its key binding. Slots 1..8
    /// include the QC weapon first; slots 9/0 contain add-ons only. Only
    /// real QC selections reach the server, so slot 9 never grants weapons.
    pub fn update_slots(
        &mut self, cv: &qcore::cvar::Cvars, enabled: &[Toy],
        selection: (u8, &mut u8), cl: &qclient::ClientState,
    ) -> Option<Toy> {
        let (requested, impulse) = selection;
        let active_weapon = cl.stats[qnet::protocol::STAT_ACTIVEWEAPON];
        if self.pending_base.is_some_and(|(_, observed)| observed != active_weapon) {
            self.pending_base = None;
        }
        if self.held.is_some_and(|t| !enabled.contains(&t)) { self.held = None; }
        let explicit_slot = (1..=10).contains(&requested);
        let slot = if explicit_slot { requested } else { *impulse };
        if !explicit_slot && matches!(*impulse, 10 | 12) && !enabled.is_empty() {
            return self.cycle(cv, enabled, impulse, cl);
        }
        if !(1..=8).contains(&slot) && !explicit_slot {
            if *impulse != 0 { self.last_slot = None; }
            return None;
        }
        let assigned: Vec<_> = enabled.iter().copied()
            .filter(|t| cv.value_of(t.slot_cvar()).clamp(1.0, 10.0) as u8 == slot
                && (Entry { weapon: Weapon::Addon(*t), slot }).usable(cl)).collect();
        let same_slot = self.last_slot == Some(slot);
        let holding_here = self.held.filter(|t| assigned.contains(t));
        let base_weapon = if slot == 1 { 4096 } else { 1 << (slot - 2) };
        let base_usable = slot <= 8
            && (Entry { weapon: Weapon::Stock(slot), slot }).usable(cl);
        if !base_usable && assigned.is_empty() {
            // Leave stock selection to QC when no add-on is currently held.
            *impulse = if self.held.is_none() && slot <= 8 { slot } else { 0 };
            return self.held;
        }
        let base_selected = self.held.is_none() && slot <= 8
            && (same_slot || active_weapon == base_weapon);
        self.held = if let Some(t) = holding_here {
            let next = assigned.iter().position(|&a| a == t).and_then(|i| assigned.get(i + 1)).copied();
            // Skip the base entry when it is unowned or lacks ammunition.
            next.or_else(|| if !base_usable { assigned.first().copied() } else { None })
        } else if !base_usable || base_selected {
            assigned.first().copied()
        } else { None };
        self.last_slot = Some(slot);
        self.pending_base = (slot <= 8 && self.held.is_none()).then_some((slot, active_weapon));
        *impulse = if slot <= 8 && self.held.is_none() { slot } else { 0 };
        self.held
    }

    /// An exact wheel selection bypasses repeated-slot cycling.
    pub fn select(&mut self, weapon: Weapon, cl: &qclient::ClientState, impulse: &mut u8) {
        if !(Entry { weapon, slot: 0 }).usable(cl) { return; }
        match weapon {
            Weapon::Stock(slot) => {
                self.held = None;
                self.last_slot = Some(slot);
                self.pending_base = Some((slot, cl.stats[qnet::protocol::STAT_ACTIVEWEAPON]));
                *impulse = slot;
            }
            Weapon::Addon(toy) => {
                self.held = Some(toy);
                self.last_slot = None;
                self.pending_base = None;
                *impulse = 0;
            }
        }
    }

    /// Next/previous traverses the same inventory in slot order, stock first
    /// within each slot. Keep QC's mod-specific cycling when add-ons are off.
    fn cycle(&mut self, cv: &qcore::cvar::Cvars, enabled: &[Toy], impulse: &mut u8,
        cl: &qclient::ClientState) -> Option<Toy> {
        let weapons: Vec<_> = crate::wheel::inventory(cv, enabled).into_iter()
            .filter(|entry| entry.usable(cl)).map(|entry| match entry.weapon {
                Weapon::Stock(slot) => (slot, None),
                Weapon::Addon(toy) => (entry.slot, Some(toy)),
            }).collect();
        let active = cl.stats[qnet::protocol::STAT_ACTIVEWEAPON];
        let base_slot = self.pending_base.map(|(slot, _)| slot).or_else(||
            crate::wheel::SLOTS.iter().find(|s| s.item == active).map(|s| s.impulse));
        let current = weapons.iter().position(|&(slot, toy)| match self.held {
            Some(held) => toy == Some(held),
            None => toy.is_none() && Some(slot) == base_slot,
        });
        let n = weapons.len();
        if n == 0 {
            *impulse = 0;
            return self.held;
        }
        let next = if *impulse == 10 {
            current.map_or(0, |i| (i + 1) % n)
        } else {
            current.map_or(n - 1, |i| (i + n - 1) % n)
        };
        let (slot, toy) = weapons[next];
        self.held = toy;
        self.last_slot = Some(slot);
        self.pending_base = toy.is_none().then_some((slot, active));
        *impulse = if toy.is_none() { slot } else { 0 };
        toy
    }

    /// Usercmd buttons: QC's attack bit is withheld while a toy is held.
    pub fn buttons(&self, attack: bool, attack2: bool, jump: bool) -> u8 {
        let jump = (jump as u8) << 1;
        match self.held {
            None => attack as u8 | jump,
            Some(t) => {
                jump | t.id() << TOY_SHIFT
                    | if attack { BUTTON_TOY_FIRE } else { 0 }
                    | if attack2 { BUTTON_TOY_ALT } else { 0 }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    fn slot_cvars() -> qcore::cvar::Cvars {
        let mut cv = qcore::cvar::Cvars::new();
        for &(name, default, flags) in qserver::addons::CVARS { cv.register(name, default, flags); }
        cv
    }

    fn inventory(active: i32, items: i32) -> qclient::ClientState {
        let mut cl = qclient::ClientState::default();
        cl.items = items;
        cl.stats[qnet::protocol::STAT_ACTIVEWEAPON] = active;
        for a in [qnet::protocol::STAT_SHELLS, qnet::protocol::STAT_NAILS,
            qnet::protocol::STAT_ROCKETS, qnet::protocol::STAT_CELLS] { cl.stats[a] = 10; }
        cl
    }

    #[test]
    fn shared_slot_cycles_base_then_assigned_addons() {
        let cv = slot_cvars();
        let enabled = [Toy::Boomerang, Toy::Hookshot];
        let mut s = WeaponSelection::default();
        let mut impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, (0, &mut impulse), &inventory(4096, 4096)), Some(Toy::Boomerang));
        assert_eq!(impulse, 0);
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, (0, &mut impulse), &inventory(4096, 4096)), Some(Toy::Hookshot));
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, (0, &mut impulse), &inventory(4096, 4096)), None);
        assert_eq!(impulse, 1);
    }

    #[test]
    fn another_slot_selects_its_base_before_addons() {
        let cv = slot_cvars();
        let mut s = WeaponSelection::default();
        let mut impulse = 1;
        assert_eq!(s.update_slots(&cv, &[Toy::Boomerang], (0, &mut impulse), &inventory(1, 4097)), None);
        assert_eq!(impulse, 1);
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &[Toy::Boomerang], (0, &mut impulse), &inventory(1, 4097)), Some(Toy::Boomerang));
        assert_eq!(impulse, 0);
    }

    #[test]
    fn sticky_slot_without_grenade_launcher_selects_immediately() {
        let cv = slot_cvars();
        let mut s = WeaponSelection::default();
        for _ in 0..3 {
            let mut impulse = 6;
            assert_eq!(s.update_slots(&cv, &[Toy::Sticky],
                (6, &mut impulse), &inventory(1, 4097)), Some(Toy::Sticky));
            assert_eq!(impulse, 0);
        }
        // Picking up the grenade launcher adds it to the cycle.
        let mut impulse = 6;
        assert_eq!(s.update_slots(&cv, &[Toy::Sticky],
            (6, &mut impulse), &inventory(1, 4097 | 16)), None);
        assert_eq!(impulse, 6);
    }

    #[test]
    fn extra_slots_are_consumed_and_never_send_cheats_or_cycle_impulses() {
        let mut cv = slot_cvars();
        let mut s = WeaponSelection::default();
        for slot in [9, 10] {
            cv.set(Toy::PortalGun.slot_cvar(), &slot.to_string());
            for _ in 0..2 {
                let mut impulse = 0;
                assert_eq!(s.update_slots(&cv, &[Toy::PortalGun], (slot, &mut impulse), &inventory(4096, 4096)), Some(Toy::PortalGun));
                assert_eq!(impulse, 0);
            }
            let mut impulse = 0;
            s.update_slots(&cv, &[], (slot, &mut impulse), &inventory(4096, 4096));
            assert_eq!((s.held, impulse), (None, 0));
        }
        let mut impulse = 9;
        s.update_slots(&cv, &[], (0, &mut impulse), &inventory(4096, 4096));
        assert_eq!(impulse, 9); // Explicit console cheat remains QC-owned.
        for value in 240..=249 {
            let mut impulse = value;
            s.update_slots(&cv, &[], (0, &mut impulse), &inventory(4096, 4096));
            assert_eq!(impulse, value); // Mods retain their entire impulse space.
        }
    }

    #[test]
    fn every_stock_slot_cycles_all_assigned_weapons_stock_first() {
        for slot in 1..=8 {
            let mut cv = slot_cvars();
            for toy in Toy::ALL { cv.set(toy.slot_cvar(), &slot.to_string()); }
            let base = crate::wheel::SLOTS[slot as usize - 1].item;
            let cl = inventory(if slot == 2 { 4096 } else { 1 }, 4097 | base);
            let mut s = WeaponSelection::default();
            for expected in [None].into_iter().chain(Toy::ALL.map(Some)).chain([None]) {
                let mut impulse = 0;
                assert_eq!(s.update_slots(&cv, &Toy::ALL, (slot, &mut impulse), &cl), expected);
                assert_eq!(impulse, if expected.is_none() { slot } else { 0 });
            }
        }
    }

    #[test]
    fn next_and_previous_include_addons_in_assigned_slot_order() {
        let cv = slot_cvars();
        let cl = inventory(4096, 4097 | 16);
        // Starting at axe; each stock weapon is followed by its assigned add-ons.
        let forward = [(0, Some(Toy::Boomerang)), (0, Some(Toy::Hookshot)),
            (2, None), (6, None), (0, Some(Toy::Sticky)),
            (0, Some(Toy::Railgun)), (0, Some(Toy::PortalGun)), (1, None)];
        let mut s = WeaponSelection::default();
        for (direction, expected) in [(10, forward.to_vec()),
            (12, forward[..7].iter().rev().copied().chain([(1, None)]).collect())] {
            for (sent, held) in expected {
                let mut impulse = direction;
                assert_eq!(s.update_slots(&cv, &Toy::ALL, (0, &mut impulse), &cl), held);
                assert_eq!(impulse, sent);
            }
        }
    }

    #[test]
    fn scrolling_skips_weapons_without_ammunition() {
        let cv = slot_cvars();
        let mut cl = inventory(4096, 4097 | 16);
        cl.stats[qnet::protocol::STAT_SHELLS] = 0;
        cl.stats[qnet::protocol::STAT_ROCKETS] = 0;
        let mut s = WeaponSelection::default();
        let mut impulse = 10;
        assert_eq!(s.update_slots(&cv, &[Toy::Sticky], (0, &mut impulse), &cl), None);
        assert_eq!(impulse, 1);
        // Without add-ons, preserve the mod's own next/previous handling.
        for value in [10, 12] {
            let mut impulse = value;
            s.update_slots(&cv, &[], (0, &mut impulse), &cl);
            assert_eq!(impulse, value);
        }
    }

    #[test]
    fn addon_ammo_requirements_apply_to_slots_cycles_and_exact_selections() {
        use qnet::protocol::{STAT_CELLS, STAT_ROCKETS};
        for toy in Toy::ALL {
            let mut cv = slot_cvars();
            cv.set(toy.slot_cvar(), "9");
            for ammo in [0, 1, 2] {
                let mut cl = inventory(4096, 4096);
                cl.stats[STAT_CELLS] = ammo;
                cl.stats[STAT_ROCKETS] = ammo;
                let usable = match toy {
                    Toy::Sticky => ammo >= 1,
                    Toy::Railgun => ammo >= 2,
                    _ => true,
                };
                let expected = usable.then_some(toy);
                let mut s = WeaponSelection::default();
                let mut impulse = 0;
                s.update_slots(&cv, &[toy], (9, &mut impulse), &cl);
                assert_eq!((s.held, impulse), (expected, 0), "slot: {toy:?}, ammo {ammo}");
                for direction in [10, 12] {
                    let mut s = WeaponSelection::default();
                    let mut impulse = direction;
                    s.update_slots(&cv, &[toy], (0, &mut impulse), &cl);
                    assert_eq!((s.held, impulse), (expected, if usable { 0 } else { 1 }),
                        "cycle {direction}: {toy:?}, ammo {ammo}");
                }
                let mut s = WeaponSelection::default();
                let mut impulse = 0;
                s.select(crate::wheel::Weapon::Addon(toy), &cl, &mut impulse);
                assert_eq!((s.held, impulse), (expected, 0), "exact: {toy:?}, ammo {ammo}");
            }
        }
    }

    #[test]
    fn unavailable_slots_preserve_current_weapon_and_shared_slots_skip_empty_entries() {
        let mut cv = slot_cvars();
        let mut cl = inventory(4096, 4096 | 16);
        cl.stats[qnet::protocol::STAT_ROCKETS] = 1;
        let mut empty = inventory(4096, 4096 | 16);
        empty.stats[qnet::protocol::STAT_ROCKETS] = 0;
        let mut s = WeaponSelection { held: Some(Toy::Hookshot), ..Default::default() };
        let mut impulse = 6;
        s.update_slots(&cv, &Toy::ALL, (6, &mut impulse), &empty);
        assert_eq!((s.held, impulse), (Some(Toy::Hookshot), 0));
        cv.set(Toy::Hookshot.slot_cvar(), "6");
        for _ in 0..3 {
            impulse = 6;
            s.update_slots(&cv, &Toy::ALL, (6, &mut impulse), &empty);
            assert_eq!((s.held, impulse), (Some(Toy::Hookshot), 0));
        }
        // One rocket makes both the stock launcher and sticky selectable again.
        impulse = 6;
        s.update_slots(&cv, &Toy::ALL, (6, &mut impulse), &cl);
        assert_eq!((s.held, impulse), (None, 6));
        impulse = 6;
        s.update_slots(&cv, &Toy::ALL, (6, &mut impulse), &cl);
        assert_eq!((s.held, impulse), (Some(Toy::Sticky), 0));
        // A snapshot without any usable inventory must not underflow cycling.
        let mut impulse = 12;
        s.update_slots(&cv, &[Toy::Sticky], (0, &mut impulse), &qclient::ClientState::default());
        assert_eq!(impulse, 0);
    }

    #[test]
    fn disabled_weapons_are_put_away_and_buttons_follow_selection() {
        let mut s = WeaponSelection { held: Some(Toy::Sticky), ..Default::default() };
        assert_eq!(s.buttons(true, false, false), Toy::Sticky.id() << TOY_SHIFT | BUTTON_TOY_FIRE);
        let mut impulse = 0;
        s.update_slots(&slot_cvars(), &[], (0, &mut impulse), &inventory(1, 4097));
        assert_eq!(s.held, None);
        assert_eq!(s.buttons(true, true, true), 0b11);
    }
}
