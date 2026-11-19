//! Client-side smart weapon auto-switch fallback for remote multiplayer servers
//! (local single-player / co-op / listen servers handle this directly in
//! `qserver::weaponswitch` before `PlayerPostThink` can fire).

use qclient::ClientState;
use qcore::cvar::Cvars;
use qnet::protocol::{STAT_ACTIVEWEAPON, STAT_HEALTH};
use qserver::weaponswitch::{item_to_impulse, read_priority, should_switch};

/// Weapon item bits for the 8 stock Quake weapons.
const STOCK_WEAPON_MASK: i32 = 1 | 2 | 4 | 8 | 16 | 32 | 64 | 4096;

#[derive(Default)]
pub(super) struct RemoteSwitch {
    last_weapon: i32,
    last_items: i32,
    manual_until: f64,
}

impl RemoteSwitch {
    pub fn reset(&mut self) {
        self.last_weapon = 0;
        self.last_items = 0;
        self.manual_until = 0.0;
    }

    /// Note a manual weapon selection impulse so it isn't undone.
    pub fn note_impulse(&mut self, impulse: u8, realtime: f64) {
        if matches!(impulse, 1..=8 | 10 | 12) {
            self.manual_until = realtime + 0.35;
        }
    }

    /// Returns a corrective impulse (1..=8) if a remote server auto-switched
    /// on pickup against `cl_autoswitch` / `cl_weaponpriority`.
    pub fn check(&mut self, cl: &ClientState, cv: &Cvars, realtime: f64) -> Option<u8> {
        if cl.signon != 4 || cl.intermission != 0 || cl.stats[STAT_HEALTH] <= 0 {
            self.reset();
            return None;
        }
        let weapon = cl.stats[STAT_ACTIVEWEAPON];
        let items = cl.items;
        let (prev_w, prev_i) = (self.last_weapon, self.last_items);
        self.last_weapon = weapon;
        self.last_items = items;

        if prev_w == 0 || weapon == prev_w || realtime < self.manual_until {
            return None;
        }
        // Only intervene when a pickup happened (items gained a weapon or ammo bit)
        // and the previous weapon is still owned.
        let gained_weapon = (items & STOCK_WEAPON_MASK) & !(prev_i & STOCK_WEAPON_MASK) != 0;
        if !gained_weapon || (items & prev_w) == 0 {
            return None;
        }
        let mode = (cv.value_of("cl_autoswitch").round() as i32).clamp(0, 1);
        let order = read_priority(cv);
        let is_new = (prev_i & weapon) == 0;
        if should_switch(mode, &order, prev_w, weapon, is_new, false, false) {
            return None;
        }
        let imp = item_to_impulse(prev_w)?;
        self.manual_until = realtime + 0.2;
        Some(imp)
    }
}
