use super::*;

pub(super) struct WeaponIcon {
    slot: u8,
    pic: Option<String>,
    label: &'static str,
    active: bool,
}

fn weapon_grid_size(count: usize) -> (f32, f32) {
    (count.min(4) as f32 * 20.0, count.div_ceil(4) as f32 * 14.0)
}


impl<'a, 'b> Sbar<'a, 'b> {
    /// Modern HUD: each element (`HudElem`) is drawn in a box at its
    /// configured anchor; elements sharing an anchor stack away from the
    /// screen edge in `HudElem::ALL` order. Immersive elements show only for
    /// a few seconds after their value changes.
    pub fn draw_modern(&mut self, cl: &ClientState, show_scores: bool, face_anim: bool, cfg: &HudConfig, fade: &mut HudFade, now: f64) {
        let st = &cl.stats;
        if show_scores || st[STAT_HEALTH] <= 0 {
            self.draw_pic(0.0, 0.0, "scorebar");
            self.solo_scoreboard(cl);
            return;
        }
        let (vw, vh) = (self.c.vw(), self.c.vh());
        // screen-corner coordinates
        self.x0 = 0.0;
        self.y0 = 0.0;
        let weapons = self.weapon_row(cl);
        let items = self.item_row(cl);
        fade.update(cl, now);
        let addons = (self.addon_weapons.clone(), self.held_addon);
        if fade.last_addons.as_ref() != Some(&addons) {
            fade.last_addons = Some(addons);
            fade.changed[HudElem::Weapons as usize] = now;
        }
        let m = 8.0;
        let gap = 4.0;
        // per-anchor offset from the edge consumed so far
        let mut used = [0.0f32; HudAnchor::COUNT];
        // middle anchors stack downward from a centered start: precompute heights
        let mut mid_total = [0.0f32; HudAnchor::COUNT];
        let mut boxes = Vec::new();
        for e in HudElem::ALL {
            let mode = cfg.mode[e as usize];
            let mut alpha = match mode {
                HudMode::Off => continue,
                HudMode::On => 1.0,
                HudMode::Immersive => fade.alpha(e, now),
            };
            // low health / armor always visible
            if matches!(e, HudElem::Health) && st[STAT_HEALTH] <= 25 {
                alpha = 1.0;
            }
            let (w, h) = match e {
                HudElem::Health => (if cfg.face { 100.0 } else { 72.0 }, 24.0),
                HudElem::Armor => {
                    if self.pack.armor_pic(cl.items).is_none() {
                        continue;
                    }
                    (100.0, 24.0)
                }
                HudElem::Ammo => {
                    if self.pack.ammo_pic(cl.items).is_none() {
                        continue;
                    }
                    (100.0, 24.0)
                }
                HudElem::AmmoList => (88.0, 36.0),
                HudElem::Weapons => weapon_grid_size(weapons.len()),
                HudElem::Items => (items.iter().map(|i| i.1).sum::<f32>(), 16.0),
            };
            if w <= 0.0 || alpha <= 0.0 {
                continue;
            }
            let a = cfg.anchor[e as usize];
            mid_total[a as usize] += h + gap;
            boxes.push((e, a, w, h, alpha));
        }
        for (e, a, w, h, alpha) in boxes {
            let ai = a as usize;
            let x = match a.col() {
                0 => m,
                1 => ((vw - w) / 2.0).floor(),
                _ => vw - m - w,
            };
            let y = match a.row() {
                0 => m + used[ai],
                1 => ((vh - mid_total[ai] + gap) / 2.0).floor() + used[ai],
                _ => vh - m - h - used[ai],
            };
            used[ai] += h + gap;
            let right = a.col() == 2;
            self.alpha = alpha;
            self.draw_elem(cl, e, x, y, w, right, face_anim, cfg.face, &weapons, &items);
        }
        self.alpha = 1.0;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_elem(&mut self, cl: &ClientState, e: HudElem, x: f32, y: f32, w: f32, right: bool, face_anim: bool, face: bool, weapons: &[WeaponIcon], items: &[(String, f32, f32)]) {
        let st = &cl.stats;
        match e {
            HudElem::Health => {
                let mut nx = x;
                if face {
                    let f = Self::face_name(cl, face_anim);
                    self.draw_pic(x, y, &f);
                    nx += 28.0;
                }
                self.draw_num(nx, y, st[STAT_HEALTH], 3, st[STAT_HEALTH] <= 25);
            }
            HudElem::Armor => {
                if let Some(p) = self.pack.armor_pic(cl.items) {
                    self.draw_pic(x, y, p);
                }
                if cl.items & IT_INVULNERABILITY != 0 {
                    self.draw_num(x + 28.0, y, 666, 3, true);
                } else {
                    self.draw_num(x + 28.0, y, st[STAT_ARMOR], 3, st[STAT_ARMOR] <= 25);
                }
            }
            HudElem::Ammo => {
                let p = self.pack.ammo_pic(cl.items).unwrap_or("sb_shells");
                if right {
                    self.draw_num(x, y, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
                    self.draw_pic(x + 76.0, y, p);
                } else {
                    self.draw_pic(x, y, p);
                    self.draw_num(x + 28.0, y, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
                }
            }
            HudElem::AmmoList => {
                let bits = if self.pack == Pack::Rogue {
                    [RIT_SHELLS, RIT_NAILS, RIT_ROCKETS, RIT_CELLS]
                } else {
                    [IT_SHELLS, IT_NAILS, IT_ROCKETS, IT_CELLS]
                };
                let names = ["shells", "nails", "rockets", "cells"];
                for i in 0..4 {
                    let s = if right {
                        format!("{:>7} {:>3}", names[i], st[STAT_SHELLS + i])
                    } else {
                        format!("{:<7} {:>3}", names[i], st[STAT_SHELLS + i])
                    };
                    let sx = if right { x + w - s.len() as f32 * 8.0 } else { x };
                    let gold = cl.items & bits[i] != 0;
                    for (k, b) in s.bytes().enumerate() {
                        // conchars: +128 is the gold/brown alternate set
                        let ch = if gold && b != b' ' { b | 128 } else { b };
                        self.c.ch_color(sx + k as f32 * 8.0, y + i as f32 * 9.0, ch, [1.0, 1.0, 1.0, self.alpha]);
                    }
                }
            }
            HudElem::Weapons => {
                for (i, weapon) in weapons.iter().enumerate() {
                    let (wx, wy) = (x + (i % 4) as f32 * 20.0, y + (i / 4) as f32 * 14.0);
                    let color = if weapon.active { [1.0, 0.7, 0.3, self.alpha] } else { [1.0, 1.0, 1.0, self.alpha * 0.65] };
                    if weapon.active { self.c.fill(wx, wy, 18.0, 12.0, [0.35, 0.22, 0.07, self.alpha * 0.7]); }
                    if let Some((id, pw, ph)) = weapon.pic.as_ref().and_then(|p| (self.pic)(p)) {
                        let scale = (18.0 / pw as f32).min(12.0 / ph as f32).min(0.75);
                        let (pw, ph) = ((pw as f32 * scale) as u32, (ph as f32 * scale) as u32);
                        self.c.pic_color(wx, wy + (12.0 - ph as f32) / 2.0, Some((id, pw, ph)), color);
                    } else {
                        for (j, b) in weapon.label.bytes().enumerate() {
                            self.c.ch_color(wx + 1.0 + j as f32 * 8.0, wy + 2.0, b, color);
                        }
                    }
                }
            }
            HudElem::Items => {
                let mut ix = x;
                for (p, adv, dx) in items {
                    self.draw_pic(ix + dx, y, p);
                    ix += adv;
                }
            }
        }
    }

    /// Owned weapon icons, left to right.
    pub(super) fn weapon_row(&self, cl: &ClientState) -> Vec<WeaponIcon> {
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
    pub(super) fn item_row(&self, cl: &ClientState) -> Vec<(String, f32, f32)> {
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
    fn weapon_grid_wraps_addons_and_tracks_the_held_weapon() {
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
        assert_eq!(weapon_grid_size(icons.len()), (80.0, 56.0));
        assert_eq!(icons.iter().filter(|w| w.active).count(), 1);
        assert_eq!(icons.iter().find(|w| w.active).unwrap().label, "SB");
        assert!(icons.windows(2).all(|w| w[0].slot <= w[1].slot));
        let cfg = HudConfig { mode: [HudMode::Immersive; 6], anchor: [HudAnchor::BottomRight; 6], face: false };
        let mut fade = HudFade::default();
        sb.draw_modern(&cl, false, false, &cfg, &mut fade, 0.0);
        assert_eq!(fade.alpha(HudElem::Weapons, 4.0), 0.0);
        sb.held_addon = Some(Toy::Boomerang);
        sb.draw_modern(&cl, false, false, &cfg, &mut fade, 4.0);
        assert_eq!(fade.alpha(HudElem::Weapons, 4.0), 1.0);
    }
}
