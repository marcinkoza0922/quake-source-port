//! Client side of the add-on toys (see `qserver::addons`): each local
//! player's toy slot, toggled with `+toy`, and the usercmd buttons it sends.
//! With add-ons off this is a single `Option` check per player per frame.

pub mod equip_wheel;
pub mod goggles;
pub mod portal_draw;
pub mod portal_fx;
pub mod portal_model;
mod portal_skin;

use qserver::addons::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, TOY_SHIFT, Toy};

/// One local player's toy slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToySlot {
    pub held: Option<Toy>,
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
