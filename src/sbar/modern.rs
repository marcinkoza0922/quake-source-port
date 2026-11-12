use super::*;

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
                HudElem::Weapons => (weapons.len() as f32 * 24.0, 16.0),
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
    pub(super) fn draw_elem(&mut self, cl: &ClientState, e: HudElem, x: f32, y: f32, w: f32, right: bool, face_anim: bool, face: bool, weapons: &[String], items: &[(String, f32, f32)]) {
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
                for (i, p) in weapons.iter().enumerate() {
                    self.draw_pic(x + i as f32 * 24.0, y, p);
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
    pub(super) fn weapon_row(&self, cl: &ClientState) -> Vec<String> {
        let st = &cl.stats;
        let items = cl.items;
        let mut row: Vec<String> = Vec::new();
        let hip = if self.pack == Pack::Hipnotic { hip_weapons(cl) } else { Vec::new() };
        for (i, w) in WEAPONS.iter().enumerate() {
            if items & (IT_SHOTGUN << i) == 0 && !(i == 4 && hip.iter().any(|h| h.0 == 2)) {
                continue;
            }
            if self.pack == Pack::Hipnotic && i == 4 {
                // grenade launcher slot shows the prox variants
                if let Some(h) = hip.iter().find(|h| h.0 == 2) {
                    row.push(h.1.clone());
                    continue;
                }
            }
            let active = st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i;
            if self.pack == Pack::Rogue && i >= 2 && st[STAT_ACTIVEWEAPON] == RIT_LAVA_NAILGUN << (i - 2) {
                row.push(ROGUE_WEAPONS[i - 2].into());
                continue;
            }
            row.push(format!("{}{w}", if active { "inv2_" } else { "inv_" }));
        }
        for h in hip.iter().filter(|h| h.0 < 2) {
            row.push(h.1.clone());
        }
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
