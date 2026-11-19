//! Weapon and item rows shared by the HUD elements (`crate::hud`).

use super::*;

/// A weapon owned, for the weapon grid.
pub struct WeaponIcon {
    pub slot: u8,
    pub pic: Option<String>,
    pub label: &'static str,
    pub active: bool,
}

impl<'a, 'b> Sbar<'a, 'b> {
    /// Owned weapon icons, left to right.
    pub fn weapon_row(&self, cl: &ClientState) -> Vec<WeaponIcon> {
        let st = &cl.stats;
        let items = cl.items;
        let mut row = Vec::new();
        let stock_active = self.held_addon.is_none();
        let icon = |slot, pic: String, active| WeaponIcon { slot, pic: Some(pic), label: "??", active: stock_active && active };
        let hip = if self.pack == Pack::Hipnotic { hip_weapons(cl) } else { Vec::new() };
        for (i, w) in WEAPONS.iter().enumerate() {
            if items & (IT_SHOTGUN << i) == 0 && !(i == 4 && hip.iter().any(|h| h.0 == 2)) {
                continue;
            }
            if self.pack == Pack::Hipnotic && i == 4 {
                // grenade launcher slot shows the prox variants
                if let Some(h) = hip.iter().find(|h| h.0 == 2) {
                    row.push(icon(i as u8 + 2, h.1.clone(), h.2));
                    continue;
                }
            }
            let active = stock_active && st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i;
            if self.pack == Pack::Rogue && i >= 2 && st[STAT_ACTIVEWEAPON] == RIT_LAVA_NAILGUN << (i - 2) {
                row.push(icon(i as u8 + 2, ROGUE_WEAPONS[i - 2].into(), true));
                continue;
            }
            row.push(icon(i as u8 + 2, format!("{}{w}", if active { "inv2_" } else { "inv_" }), active));
        }
        for h in hip.iter().filter(|h| h.0 < 2) {
            row.push(icon(9, h.1.clone(), h.2));
        }
        if self.pack != Pack::Rogue && items & 4096 != 0 {
            row.push(WeaponIcon { slot: 1, pic: None, label: "AX", active: stock_active && st[STAT_ACTIVEWEAPON] == 4096 });
        }
        row.extend(self.addon_weapons.iter().map(|e| WeaponIcon {
            slot: e.slot, pic: None, label: e.label(), active: e.active(cl, self.held_addon),
        }));
        row.sort_by_key(|w| w.slot);
        row
    }

    /// Keys, powerups, pack items and sigils: (pic, advance, x offset).
    pub fn item_row(&self, cl: &ClientState) -> Vec<(String, f32, f32)> {
        let items = cl.items;
        let mut row = Vec::new();
        let sigils = if self.pack == Pack::Rogue { 0 } else { 4 };
        for i in 0..sigils {
            if items & (1 << (28 + i)) != 0 {
                row.push((format!("sb_sigil{}", i + 1), 10.0, 0.0));
            }
        }
        if !row.is_empty() {
            row.last_mut().unwrap().1 = 18.0;
        }
        for (i, n) in ITEMS.iter().enumerate() {
            if items & (1 << (17 + i)) != 0 {
                row.push(((*n).into(), 18.0, 0.0));
            }
        }
        for &(bit, n) in self.pack.extra_items() {
            if items & (1 << bit) != 0 {
                row.push((n.into(), 18.0, 0.0));
            }
        }
        if let Some(l) = row.last_mut() {
            l.1 = 16.0;
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_row_includes_addons_and_tracks_the_held_weapon() {
        use crate::wheel::{Entry, Weapon};
        use qserver::addons::Toy;
        let mut c = Canvas::new(640, 480);
        let mut pic = |_: &str| Some((PicId(1), 24, 16));
        let mut sb = Sbar::new(&mut c, &mut pic, Pack::Id);
        let mut cl = ClientState::default();
        cl.items = 4096 | 127;
        cl.stats[STAT_HEALTH] = 100;
        cl.stats[STAT_ACTIVEWEAPON] = 16;
        sb.addon_weapons = Toy::ALL[..5].iter().map(|&t| Entry { weapon: Weapon::Addon(t), slot: 6 }).collect();
        sb.held_addon = Some(Toy::Sticky);
        let icons = sb.weapon_row(&cl);
        assert_eq!(icons.len(), 13);
        assert_eq!(icons.iter().filter(|w| w.active).count(), 1);
        assert_eq!(icons.iter().find(|w| w.active).unwrap().label, "SB");
        assert!(icons.windows(2).all(|w| w[0].slot <= w[1].slot));
    }
}
