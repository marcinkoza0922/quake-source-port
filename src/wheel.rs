//! Weapon wheel (in the style of the 2021 re-release): hold `+weaponwheel`,
//! point with the look stick (or the mouse), release to switch. Stock and
//! enabled add-on weapons run clockwise in weapon-slot order.
//! While it is open, single-player time runs at `cl_wheel_timescale`.

use qclient::ClientState;
use qnet::protocol::{STAT_ACTIVEWEAPON, STAT_CELLS, STAT_NAILS, STAT_ROCKETS, STAT_SHELLS};
use qinput::keys::*;

use crate::draw::Canvas;
use crate::glyphs::Glyphs;
use crate::sbar::PicFn;

mod inventory;
pub use inventory::{Entry, Weapon, SLOTS, inventory};

#[derive(Default)]
pub struct Wheel {
    pub open: bool,
    pub sel: Option<usize>,
    entries: Vec<Entry>,
    held_addon: Option<qserver::addons::Toy>,
}

/// Slot index for a direction (+y down), 0 at the top, clockwise.
pub fn sector(aim: [f32; 2], count: usize) -> Option<usize> {
    if aim == [0.0; 2] || count == 0 {
        return None;
    }
    let a = aim[0].atan2(-aim[1]).to_degrees().rem_euclid(360.0);
    let step = 360.0 / count as f32;
    Some(((a + step / 2.0) / step) as usize % count)
}

impl Wheel {
    /// Advance one frame. `held` is the wheel button, `allowed` whether the
    /// player can switch now (in game, alive). Returns the exact weapon
    /// when the wheel closes on a usable weapon other than the current one.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, held: bool, allowed: bool, aim: [f32; 2], cl: &ClientState,
        entries: &[Entry], held_addon: Option<qserver::addons::Toy>) -> Option<Weapon> {
        if !allowed {
            *self = Self::default();
            return None;
        }
        if self.entries != entries { self.sel = None; }
        self.entries = entries.to_vec();
        self.held_addon = held_addon;
        if held {
            if !self.open {
                self.open = true;
                self.sel = entries.iter().position(|s| s.active(cl, held_addon));
            }
            if let Some(i) = sector(aim, entries.len()) {
                self.sel = Some(i);
            }
            return None;
        }
        if !self.open {
            return None;
        }
        self.open = false;
        let i = self.sel.take()?;
        let entry = entries.get(i)?;
        (entry.usable(cl) && !entry.active(cl, held_addon)).then_some(entry.weapon)
    }

    pub fn draw(&self, c: &mut Canvas, pic: &mut PicFn, g: &Glyphs, cl: &ClientState, pad: bool, wheel_key: i32) {
        if !self.open {
            return;
        }
        let (cx, cy) = ((c.vw() / 2.0).floor(), (c.vh() / 2.0).floor());
        let r = 66.0;
        let count = self.entries.len();
        if count == 0 { return; }
        let d = (2.0 * r * (std::f32::consts::PI / count as f32).sin() - 4.0).clamp(24.0, 38.0);
        g.disc(c, cx - r - d / 2.0 - 6.0, cy - r - d / 2.0 - 6.0, 2.0 * r + d + 12.0, [0.0, 0.0, 0.0, 0.45]);
        for (i, s) in self.entries.iter().enumerate() {
            let a = (i as f32 * 360.0 / count as f32).to_radians();
            let (x, y) = ((cx + r * a.sin()).floor(), (cy - r * a.cos()).floor());
            let sel = self.sel == Some(i);
            let current = s.active(cl, self.held_addon);
            let bg = if sel {
                [0.75, 0.48, 0.15, 0.95]
            } else if current {
                [0.35, 0.25, 0.12, 0.9]
            } else {
                [0.08, 0.07, 0.06, 0.85]
            };
            g.disc(c, x - d / 2.0, y - d / 2.0, d, bg);
            let has = s.owned(cl);
            let tint = if !has {
                [0.25, 0.25, 0.25, 0.5]
            } else if s.usable(cl) {
                [1.0; 4]
            } else {
                [0.6, 0.3, 0.3, 1.0]
            };
            match s.icon().and_then(&mut *pic) {
                Some((id, w, h)) => {
                    let scale = ((d - 4.0) / w as f32).min(1.0);
                    let (w, h) = ((w as f32 * scale) as u32, (h as f32 * scale) as u32);
                    c.pic_color(x - w as f32 / 2.0, y - h as f32 + 2.0, Some((id, w, h)), tint);
                }
                None => {
                    for (k, b) in s.label().bytes().enumerate() {
                        c.ch_color(x - 8.0 + k as f32 * 8.0, y - 7.0, b, tint);
                    }
                }
            }
            if let (Some(a), true) = (s.ammo(), has) {
                let t = cl.stats[a].to_string();
                for (k, b) in t.bytes().enumerate() {
                    c.ch_color(x - t.len() as f32 * 4.0 + k as f32 * 8.0, y + 4.0, b, tint);
                }
            }
            // slot number at the outer edge
            let (nx, ny) = ((cx + (r + d / 2.0 + 10.0) * a.sin()).floor(), (cy - (r + d / 2.0 + 10.0) * a.cos()).floor());
            c.ch(nx - 4.0, ny - 4.0, b'0' + s.slot % 10 + if sel { 128 } else { 0 });
        }
        if let Some(entry) = self.sel.and_then(|i| self.entries.get(i)) {
            let n = if entry.owned(cl) { entry.name() } else { "--" };
            let y = cy - r - d / 2.0 - 16.0;
            c.fill(cx - n.len() as f32 * 4.0 - 4.0, y - 2.0, n.len() as f32 * 8.0 + 8.0, 12.0, [0.0, 0.0, 0.0, 0.6]);
            crate::menu::gold(c, cx - n.len() as f32 * 4.0, y, n);
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
        assert_eq!(sector([0.0, -1.0], 8), Some(0));
        assert_eq!(sector([1.0, 0.0], 8), Some(2));
        assert_eq!(sector([0.0, 1.0], 8), Some(4));
        assert_eq!(sector([-1.0, 0.0], 8), Some(6));
        assert_eq!(sector([-0.7, -0.7], 8), Some(7));
        assert_eq!(sector([0.0, 0.0], 8), None);
    }
    #[test]
    fn addon_segments_select_exact_weapon_in_shared_and_extra_slots() {
        use qserver::addons::Toy;
        let mut cv = qcore::cvar::Cvars::new();
        for &(name, default, flags) in qserver::addons::CVARS { cv.register(name, default, flags); }
        for slot in [1, 6, 9, 10] {
            for toy in Toy::ALL { cv.set(toy.slot_cvar(), &slot.to_string()); }
            let entries = inventory(&cv, &Toy::ALL);
            assert_eq!(entries.len(), 13);
            assert!(entries.windows(2).all(|w| w[0].slot <= w[1].slot));
            let mut cl = ClientState::default();
            cl.items = 4096 | 1 | 16;
            cl.stats[STAT_ACTIVEWEAPON] = 1;
            cl.stats[STAT_ROCKETS] = 10;
            cl.stats[STAT_CELLS] = 10;
            for toy in Toy::ALL {
                let i = entries.iter().position(|e| e.weapon == Weapon::Addon(toy)).unwrap();
                let angle = i as f32 * std::f32::consts::TAU / entries.len() as f32;
                let aim = [angle.sin(), -angle.cos()];
                assert_eq!(sector(aim, entries.len()), Some(i));
                let mut wheel = Wheel::default();
                wheel.update(true, true, aim, &cl, &entries, None);
                let choice = wheel.update(false, true, [0.0; 2], &cl, &entries, None).unwrap();
                assert_eq!(choice, Weapon::Addon(toy));
                let mut selection = crate::addons::WeaponSelection::default();
                let mut impulse = 6;
                selection.select(choice, &cl, &mut impulse);
                assert_eq!((selection.held, impulse), (Some(toy), 0));
                // Selecting the stock entry while holding an addon must equip
                // that stock weapon directly, rather than cycle its slot.
                selection.select(Weapon::Stock(6), &cl, &mut impulse);
                assert_eq!((selection.held, impulse), (None, 6));
            }
        }
    }

    #[test]
    fn release_switches_to_owned_weapon() {
        let mut cl = ClientState::default();
        cl.items = 4096 | 1 | 32;
        cl.stats[STAT_ACTIVEWEAPON] = 1;
        cl.stats[STAT_ROCKETS] = 5;
        let entries = inventory(&qcore::cvar::Cvars::new(), &[]);
        let mut w = Wheel::default();
        assert_eq!(w.update(true, true, [0.0; 2], &cl, &entries, None), None);
        assert_eq!(w.sel, Some(1));
        w.update(true, true, [-1.0, 0.0], &cl, &entries, None); // rocket launcher (left)
        assert_eq!(w.update(false, true, [0.0; 2], &cl, &entries, None), Some(Weapon::Stock(7)));
        // not owned: nothing
        w.update(true, true, [0.0, 1.0], &cl, &entries, None);
        assert_eq!(w.update(false, true, [0.0; 2], &cl, &entries, None), None);
        // current weapon: nothing
        w.update(true, true, [0.0; 2], &cl, &entries, None);
        assert_eq!(w.update(false, true, [0.0; 2], &cl, &entries, None), None);
    }
}
