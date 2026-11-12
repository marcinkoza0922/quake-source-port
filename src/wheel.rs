//! Weapon wheel (in the style of the 2021 re-release): hold `+weaponwheel`,
//! point with the look stick (or the mouse), release to switch. Eight slots
//! clockwise from the top: axe, shotgun ... lightning gun (impulses 1..8).
//! While it is open, single-player time runs at `cl_wheel_timescale`.

use qclient::ClientState;
use qnet::protocol::{STAT_ACTIVEWEAPON, STAT_CELLS, STAT_NAILS, STAT_ROCKETS, STAT_SHELLS};
use qinput::keys::*;

use crate::draw::Canvas;
use crate::glyphs::Glyphs;
use crate::sbar::PicFn;

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

#[derive(Default)]
pub struct Wheel {
    pub open: bool,
    pub sel: Option<usize>,
}

/// Slot index for a direction (+y down), 0 at the top, clockwise.
pub fn sector(aim: [f32; 2]) -> Option<usize> {
    if aim == [0.0; 2] {
        return None;
    }
    let a = aim[0].atan2(-aim[1]).to_degrees().rem_euclid(360.0);
    Some(((a + 22.5) / 45.0) as usize % 8)
}

fn owned(cl: &ClientState, i: usize) -> bool {
    cl.items & SLOTS[i].item != 0
}

fn usable(cl: &ClientState, i: usize) -> bool {
    owned(cl, i) && SLOTS[i].ammo.is_none_or(|a| cl.stats[a] > 0)
}

impl Wheel {
    /// Advance one frame. `held` is the wheel button, `allowed` whether the
    /// player can switch now (in game, alive). Returns the impulse to send
    /// when the wheel closes on a usable weapon other than the current one.
    pub fn update(&mut self, held: bool, allowed: bool, aim: [f32; 2], cl: &ClientState) -> Option<u8> {
        if !allowed {
            *self = Self::default();
            return None;
        }
        if held {
            if !self.open {
                self.open = true;
                self.sel = SLOTS.iter().position(|s| s.item == cl.stats[STAT_ACTIVEWEAPON]);
            }
            if let Some(i) = sector(aim) {
                self.sel = Some(i);
            }
            return None;
        }
        if !self.open {
            return None;
        }
        self.open = false;
        let i = self.sel.take()?;
        (usable(cl, i) && SLOTS[i].item != cl.stats[STAT_ACTIVEWEAPON]).then_some(SLOTS[i].impulse)
    }

    pub fn draw(&self, c: &mut Canvas, pic: &mut PicFn, g: &Glyphs, cl: &ClientState, pad: bool, wheel_key: i32) {
        if !self.open {
            return;
        }
        let (cx, cy) = ((c.vw() / 2.0).floor(), (c.vh() / 2.0).floor());
        let (r, d) = (66.0, 38.0);
        g.disc(c, cx - r - d / 2.0 - 6.0, cy - r - d / 2.0 - 6.0, 2.0 * r + d + 12.0, [0.0, 0.0, 0.0, 0.45]);
        for (i, s) in SLOTS.iter().enumerate() {
            let a = (i as f32 * 45.0).to_radians();
            let (x, y) = ((cx + r * a.sin()).floor(), (cy - r * a.cos()).floor());
            let sel = self.sel == Some(i);
            let current = s.item == cl.stats[STAT_ACTIVEWEAPON];
            let bg = if sel {
                [0.75, 0.48, 0.15, 0.95]
            } else if current {
                [0.35, 0.25, 0.12, 0.9]
            } else {
                [0.08, 0.07, 0.06, 0.85]
            };
            g.disc(c, x - d / 2.0, y - d / 2.0, d, bg);
            let has = owned(cl, i);
            let tint = if !has {
                [0.25, 0.25, 0.25, 0.5]
            } else if usable(cl, i) {
                [1.0; 4]
            } else {
                [0.6, 0.3, 0.3, 1.0]
            };
            match s.icon.and_then(&mut *pic) {
                Some(p) => c.pic_color(x - (p.1 / 2) as f32, y - p.2 as f32 + 2.0, Some(p), tint),
                None => {
                    for (k, b) in "AXE".bytes().enumerate() {
                        c.ch_color(x - 12.0 + k as f32 * 8.0, y - 7.0, b, tint);
                    }
                }
            }
            if let (Some(a), true) = (s.ammo, has) {
                let t = cl.stats[a].to_string();
                for (k, b) in t.bytes().enumerate() {
                    c.ch_color(x - t.len() as f32 * 4.0 + k as f32 * 8.0, y + 4.0, b, tint);
                }
            }
            // slot number at the outer edge
            let (nx, ny) = ((cx + (r + d / 2.0 + 2.0) * a.sin()).floor(), (cy - (r + d / 2.0 + 2.0) * a.cos()).floor());
            c.ch(nx - 4.0, ny - 4.0, b'1' + i as u8 + if sel { 128 } else { 0 });
        }
        if let Some(i) = self.sel {
            let n = SLOTS[i].name;
            let n = if owned(cl, i) { n.to_string() } else { "--".into() };
            let y = cy - r - d / 2.0 - 16.0;
            c.fill(cx - n.len() as f32 * 4.0 - 4.0, y - 2.0, n.len() as f32 * 8.0 + 8.0, 12.0, [0.0, 0.0, 0.0, 0.6]);
            crate::menu::gold(c, cx - n.len() as f32 * 4.0, y, &n);
        }
        let hint_y = cy + r + d / 2.0 + 8.0;
        let back = |c: &mut Canvas, w: f32| c.fill(cx - w / 2.0 - 6.0, hint_y - 3.0, w + 12.0, 16.0, [0.0, 0.0, 0.0, 0.7]);
        if pad {
            let items = [(K_LTHUMB, "/"), (K_RTHUMB, "Select"), (wheel_key, "Release to equip")];
            let w = g.hints(c, 0.0, 0.0, &items, false);
            back(c, w);
            g.hints_centered(c, cx, hint_y, &items);
        } else {
            let s = "Move mouse to select, release to equip";
            back(c, s.len() as f32 * 8.0);
            c.string(cx - s.len() as f32 * 4.0, hint_y + 1.0, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sectors_clockwise_from_top() {
        assert_eq!(sector([0.0, -1.0]), Some(0));
        assert_eq!(sector([1.0, 0.0]), Some(2));
        assert_eq!(sector([0.0, 1.0]), Some(4));
        assert_eq!(sector([-1.0, 0.0]), Some(6));
        assert_eq!(sector([-0.7, -0.7]), Some(7));
        assert_eq!(sector([0.0, 0.0]), None);
    }
    #[test]
    fn release_switches_to_owned_weapon() {
        let mut cl = ClientState::default();
        cl.items = 4096 | 1 | 32;
        cl.stats[STAT_ACTIVEWEAPON] = 1;
        cl.stats[STAT_ROCKETS] = 5;
        let mut w = Wheel::default();
        assert_eq!(w.update(true, true, [0.0; 2], &cl), None);
        assert_eq!(w.sel, Some(1));
        w.update(true, true, [-1.0, 0.0], &cl); // rocket launcher (left)
        assert_eq!(w.update(false, true, [0.0; 2], &cl), Some(7));
        // not owned: nothing
        w.update(true, true, [0.0, 1.0], &cl);
        assert_eq!(w.update(false, true, [0.0; 2], &cl), None);
        // current weapon: nothing
        w.update(true, true, [0.0; 2], &cl);
        assert_eq!(w.update(false, true, [0.0; 2], &cl), None);
    }
}
