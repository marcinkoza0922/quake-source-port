//! Add-ons: optional engine-side toys (Main menu → Add-ons). They live in
//! the engine, not QuakeC, so they work with any progs. Off by default and
//! only allowed in single player and local split-screen: never in
//! deathmatch or with a remote client connected.
//!
//! While a player holds a toy, the client keeps the fire buttons away from
//! QC (`button0` stays 0) and sends them in usercmd bits QC never sees:
//! [`BUTTON_TOY_FIRE`], [`BUTTON_TOY_ALT`] and the toy id in the top bits.

use qcore::cvar::{CVAR_ARCHIVE, Cvars};

use crate::ents::Ents;
use crate::server::{Core, Level};

pub mod portal;
pub mod portal_move;

/// (name, default, flags) of the add-on cvars.
pub const CVARS: &[(&str, &str, u32)] = &[
    ("addons_enable", "0", CVAR_ARCHIVE),
    ("addon_portalgun", "1", CVAR_ARCHIVE),
    // see-through portals: on/off, live views per frame, live distance
    ("addon_portal_view", "1", CVAR_ARCHIVE),
    ("addon_portal_max", "4", CVAR_ARCHIVE),
    ("addon_portal_dist", "4096", CVAR_ARCHIVE),
    ("addon_portal_depth", "3", CVAR_ARCHIVE),
];

/// Usercmd button bits (bit 0 attack and bit 1 jump are QC's).
pub const BUTTON_TOY_FIRE: u8 = 1 << 2;
pub const BUTTON_TOY_ALT: u8 = 1 << 3;
/// The held toy's id sits in bits 4..7 (0 = none).
pub const TOY_SHIFT: u8 = 4;

/// The toys, in toy-slot cycling order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toy {
    PortalGun,
}

impl Toy {
    pub const ALL: [Toy; 1] = [Toy::PortalGun];

    /// Wire id (1..=15).
    pub fn id(self) -> u8 {
        self as u8 + 1
    }
    pub fn from_id(id: u8) -> Option<Toy> {
        Self::ALL.get((id as usize).checked_sub(1)?).copied()
    }
    pub fn name(self) -> &'static str {
        match self {
            Toy::PortalGun => "Portal Gun",
        }
    }
    /// The cvar that enables this toy (on top of `addons_enable`).
    pub fn cvar(self) -> &'static str {
        match self {
            Toy::PortalGun => "addon_portalgun",
        }
    }
    /// The toys switched on in the menu.
    pub fn enabled(cv: &Cvars) -> impl Iterator<Item = Toy> + '_ {
        Self::ALL.into_iter().filter(|t| cv.value_of(t.cvar()) != 0.0)
    }
    /// The toy (if any) a usercmd's buttons say is held.
    pub fn from_buttons(buttons: u8) -> Option<Toy> {
        Self::from_id(buttons >> TOY_SHIFT)
    }
}

/// Add-ons may run: switched on, not deathmatch, and every connected
/// client plays on this machine.
pub fn allowed(cv: &Cvars, deathmatch: bool, all_local: bool) -> bool {
    cv.value_of("addons_enable") != 0.0 && !deathmatch && all_local
}

/// Add-ons may run on this server this frame (cheap when off).
pub(crate) fn active(c: &Core) -> bool {
    c.cvars.value_of("addons_enable") != 0.0 && c.cvars.value_of("deathmatch") == 0.0 && all_local(&c.clients)
}

/// After physics: portal crossings, then new shots, then client updates.
pub(crate) fn frame(l: &mut Level, c: &mut Core) {
    let st = &mut l.portals;
    let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
    portal_move::cross(st, &mut l.world, &mut ents);
    let gun = c.cvars.value_of(Toy::PortalGun.cvar()) != 0.0;
    for (i, slot) in st.presses(&c.clients) {
        if !gun {
            continue;
        }
        let ents = Ents { vm: &mut l.vm, c: &mut *c };
        let others: Vec<portal::Portal> =
            st.pairs.iter().enumerate().flat_map(|(o, p)| p.iter().enumerate().filter(move |&(s, _)| (o, s) != (i, slot)).filter_map(|(_, p)| *p)).collect();
        if let Some(p) = portal::place(&l.world, &ents, i as u32 + 1, &others) {
            st.set(i, slot, Some(p));
        }
    }
    st.send(&mut c.clients);
}

impl crate::Server {
    /// Add-on state for a savegame sidecar (`<save>.qsp`), so the `.sav`
    /// itself stays loadable by other engines. None when there is none.
    pub fn addon_save_text(&self) -> Option<String> {
        self.level.as_ref()?.portals.save_text()
    }
    /// Restore a sidecar after `load_game`.
    pub fn addon_load_text(&mut self, text: &str) {
        if let Some(l) = &mut self.level {
            l.portals.load_text(text);
        }
    }
}

/// Add-ons were switched off (or a remote client joined): remove every
/// portal from the level and the clients.
pub(crate) fn shut_down(l: &mut Level, c: &mut Core) {
    let st = &mut l.portals;
    for i in 0..st.pairs.len() {
        for slot in 0..2 {
            if st.pairs[i][slot].is_some() {
                st.set(i, slot, None);
            }
        }
    }
    st.send(&mut c.clients);
    *st = Default::default();
}

/// No remote client is connected.
pub fn all_local(clients: &[crate::client::Client]) -> bool {
    clients.iter().filter(|c| c.active).all(|c| c.local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toy_ids_round_trip_through_buttons() {
        for t in Toy::ALL {
            let b = t.id() << TOY_SHIFT | BUTTON_TOY_FIRE | 1;
            assert_eq!(Toy::from_buttons(b), Some(t));
        }
        assert_eq!(Toy::from_buttons(0b11), None);
        assert_eq!(Toy::from_id(15), None);
    }
}
