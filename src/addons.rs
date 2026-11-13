//! Client side of the add-on toys (see `qserver::addons`): each local
//! player's toy slot, toggled with `+toy`, and the usercmd buttons it sends.
//! With add-ons off this is a single `Option` check per player per frame.

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

/// One local player's toy slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToySlot {
    pub held: Option<Toy>,
    last_slot: Option<u8>,
}

impl ToySlot {
    /// Apply this frame's `+toy` press and weapon impulse. `enabled` lists
    /// the usable toys (empty when add-ons are not allowed). Returns the
    /// toy just picked up, for a centerprint.
    pub fn update(&mut self, enabled: &[Toy], toy_pressed: bool, impulse: u8) -> Option<Toy> {
        if self.held.is_some_and(|t| !enabled.contains(&t)) {
            self.held = None;
        }
        // any QC weapon pick (1-8, next/prev) puts the toy away
        if impulse != 0 && (impulse <= 8 || impulse == 10 || impulse == 12) {
            self.held = None;
        }
        if !toy_pressed || enabled.is_empty() {
            return None;
        }
        // cycle through the toys, then back to the QC weapon
        self.held = match self.held {
            None => Some(enabled[0]),
            Some(t) => enabled.iter().position(|&e| e == t).and_then(|i| enabled.get(i + 1)).copied(),
        };
        self.held
    }

    /// Select a weapon slot independently of its key binding. Slots 1..8
    /// include the QC weapon first; slots 9/0 contain add-ons only. Only
    /// real QC selections reach the server, so slot 9 never grants weapons.
    pub fn update_slots(
        &mut self, cv: &qcore::cvar::Cvars, enabled: &[Toy], toy_pressed: bool,
        selection: (u8, &mut u8), active_weapon: i32,
    ) -> Option<Toy> {
        let (requested, impulse) = selection;
        let explicit_slot = (1..=10).contains(&requested);
        let slot = if explicit_slot { requested } else { *impulse };
        if !(1..=8).contains(&slot) && !explicit_slot {
            if *impulse != 0 || toy_pressed { self.last_slot = None; }
            return self.update(enabled, toy_pressed, *impulse);
        }
        if self.held.is_some_and(|t| !enabled.contains(&t)) { self.held = None; }
        let assigned: Vec<_> = enabled.iter().copied()
            .filter(|t| cv.value_of(t.slot_cvar()).clamp(1.0, 10.0) as u8 == slot).collect();
        let same_slot = self.last_slot == Some(slot);
        let holding_here = self.held.filter(|t| assigned.contains(t));
        let base_selected = self.held.is_none() && slot <= 8
            && (same_slot || active_weapon == if slot == 1 { 4096 } else { 1 << (slot - 2) });
        self.held = if let Some(t) = holding_here {
            let next = assigned.iter().position(|&a| a == t).and_then(|i| assigned.get(i + 1)).copied();
            // The two extra slots have no empty/base weapon entry.
            next.or_else(|| if slot > 8 { assigned.first().copied() } else { None })
        } else if slot > 8 || base_selected {
            assigned.first().copied()
        } else { None };
        self.last_slot = Some(slot);
        *impulse = if slot <= 8 && self.held.is_none() { slot } else { 0 };
        self.held
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

    #[test]
    fn shared_slot_cycles_base_then_assigned_addons() {
        let cv = slot_cvars();
        let enabled = [Toy::Boomerang, Toy::Hookshot];
        let mut s = ToySlot::default();
        let mut impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, false, (0, &mut impulse), 4096), Some(Toy::Boomerang));
        assert_eq!(impulse, 0);
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, false, (0, &mut impulse), 4096), Some(Toy::Hookshot));
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &enabled, false, (0, &mut impulse), 4096), None);
        assert_eq!(impulse, 1);
    }

    #[test]
    fn another_slot_selects_its_base_before_addons() {
        let cv = slot_cvars();
        let mut s = ToySlot::default();
        let mut impulse = 1;
        assert_eq!(s.update_slots(&cv, &[Toy::Boomerang], false, (0, &mut impulse), 1), None);
        assert_eq!(impulse, 1);
        impulse = 1;
        assert_eq!(s.update_slots(&cv, &[Toy::Boomerang], false, (0, &mut impulse), 1), Some(Toy::Boomerang));
        assert_eq!(impulse, 0);
    }

    #[test]
    fn extra_slots_are_consumed_and_never_send_cheats_or_cycle_impulses() {
        let mut cv = slot_cvars();
        let mut s = ToySlot::default();
        for slot in [9, 10] {
            cv.set(Toy::PortalGun.slot_cvar(), &slot.to_string());
            for _ in 0..2 {
                let mut impulse = 0;
                assert_eq!(s.update_slots(&cv, &[Toy::PortalGun], false, (slot, &mut impulse), 4096), Some(Toy::PortalGun));
                assert_eq!(impulse, 0);
            }
            let mut impulse = 0;
            s.update_slots(&cv, &[], false, (slot, &mut impulse), 4096);
            assert_eq!((s.held, impulse), (None, 0));
        }
        let mut impulse = 9;
        s.update_slots(&cv, &[], false, (0, &mut impulse), 4096);
        assert_eq!(impulse, 9); // Explicit console cheat remains QC-owned.
        for value in 240..=249 {
            let mut impulse = value;
            s.update_slots(&cv, &[], false, (0, &mut impulse), 4096);
            assert_eq!(impulse, value); // Mods retain their entire impulse space.
        }
    }

    #[test]
    fn toy_slot_cycles_and_weapons_put_it_away() {
        let mut s = ToySlot::default();
        let on = [Toy::PortalGun];
        assert_eq!(s.update(&on, true, 0), Some(Toy::PortalGun));
        assert_eq!(s.buttons(true, false, false), Toy::PortalGun.id() << TOY_SHIFT | BUTTON_TOY_FIRE);
        // pressing again cycles past the last toy back to the weapon
        assert_eq!(s.update(&on, true, 0), None);
        assert_eq!(s.buttons(true, true, true), 0b11);
        s.update(&on, true, 0);
        s.update(&on, false, 3);
        assert_eq!(s.held, None);
        // add-ons switched off mid-game drop the toy
        s.update(&on, true, 0);
        s.update(&[], false, 0);
        assert_eq!(s.held, None);
    }
}
