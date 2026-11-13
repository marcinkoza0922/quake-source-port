//! Equipment wheel: hold `+equipwheel`, point with a stick or the mouse
//! (shared with the weapon wheel's aim), release to switch the selected
//! equipment on or off. Drawn in the weapon wheel's style; the slot count
//! follows [`Equip::ALL`], so new equipment only needs a new variant.

use qinput::keys::*;

use super::goggles::{Equip, Equipment};
use crate::draw::Canvas;
use crate::glyphs::Glyphs;

#[derive(Default)]
pub struct EquipWheel {
    pub open: bool,
    pub sel: Option<usize>,
}

/// Slot for a direction (+y down) on a wheel of `n` slots, 0 at the top,
/// clockwise.
pub fn sector(aim: [f32; 2], n: usize) -> Option<usize> {
    if aim == [0.0; 2] || n == 0 {
        return None;
    }
    let step = 360.0 / n as f32;
    let a = aim[0].atan2(-aim[1]).to_degrees().rem_euclid(360.0);
    Some(((a + step / 2.0) / step) as usize % n)
}

/// Angle (radians, clockwise from the top) of slot `i` of `n`.
fn slot_angle(i: usize, n: usize) -> f32 {
    (i as f32 * 360.0 / n as f32).to_radians()
}

impl EquipWheel {
    /// Advance one frame; returns the equipment to toggle when the wheel
    /// closes on a slot.
    pub fn update(&mut self, held: bool, allowed: bool, aim: [f32; 2]) -> Option<Equip> {
        if !allowed {
            *self = Self::default();
            return None;
        }
        if held {
            self.open = true;
            if let Some(i) = sector(aim, Equip::ALL.len()) {
                self.sel = Some(i);
            }
            return None;
        }
        if !self.open {
            return None;
        }
        self.open = false;
        self.sel.take().map(|i| Equip::ALL[i])
    }

    pub fn draw(&self, c: &mut Canvas, g: &Glyphs, eq: &Equipment, pad: bool, key: i32) {
        if !self.open {
            return;
        }
        let n = Equip::ALL.len();
        let (cx, cy) = ((c.vw() / 2.0).floor(), (c.vh() / 2.0).floor());
        let (r, d) = (56.0, 42.0);
        g.disc(c, cx - r - d / 2.0 - 6.0, cy - r - d / 2.0 - 6.0, 2.0 * r + d + 12.0, [0.0, 0.0, 0.0, 0.45]);
        for (i, e) in Equip::ALL.iter().enumerate() {
            let a = slot_angle(i, n);
            let (x, y) = ((cx + r * a.sin()).floor(), (cy - r * a.cos()).floor());
            let sel = self.sel == Some(i);
            let on = eq.is_on(*e);
            let bg = if sel {
                [0.75, 0.48, 0.15, 0.95]
            } else if on {
                [0.18, 0.35, 0.12, 0.9]
            } else {
                [0.08, 0.07, 0.06, 0.85]
            };
            g.disc(c, x - d / 2.0, y - d / 2.0, d, bg);
            let s = e.short();
            for (k, b) in s.bytes().enumerate() {
                c.ch(x - s.len() as f32 * 4.0 + k as f32 * 8.0, y - 9.0, b);
            }
            let state = if on { "ON" } else { "OFF" };
            let tint = if on { [0.5, 1.0, 0.4, 1.0] } else { [0.6, 0.6, 0.6, 1.0] };
            for (k, b) in state.bytes().enumerate() {
                c.ch_color(x - state.len() as f32 * 4.0 + k as f32 * 8.0, y + 2.0, b, tint);
            }
        }
        if let Some(i) = self.sel {
            let e = Equip::ALL[i];
            let s = format!("{} ({})", e.name(), if eq.is_on(e) { "turn off" } else { "turn on" });
            let y = cy - r - d / 2.0 - 16.0;
            c.fill(cx - s.len() as f32 * 4.0 - 4.0, y - 2.0, s.len() as f32 * 8.0 + 8.0, 12.0, [0.0, 0.0, 0.0, 0.6]);
            crate::menu::gold(c, cx - s.len() as f32 * 4.0, y, &s);
        }
        let hint_y = cy + r + d / 2.0 + 8.0;
        let back = |c: &mut Canvas, w: f32| c.fill(cx - w / 2.0 - 6.0, hint_y - 3.0, w + 12.0, 16.0, [0.0, 0.0, 0.0, 0.7]);
        if pad {
            let items = [(K_LTHUMB, "/"), (K_RTHUMB, "Select"), (key, "Release to toggle")];
            let w = g.hints(c, 0.0, 0.0, &items, false);
            back(c, w);
            g.hints_centered(c, cx, hint_y, &items);
        } else {
            let s = "Move mouse to select, release to toggle";
            back(c, s.len() as f32 * 8.0);
            c.string(cx - s.len() as f32 * 4.0, hint_y + 1.0, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sectors_follow_slot_count() {
        assert_eq!(sector([0.0, -1.0], 2), Some(0));
        assert_eq!(sector([0.0, 1.0], 2), Some(1));
        assert_eq!(sector([1.0, 0.0], 4), Some(1));
        assert_eq!(sector([0.0, 0.0], 2), None);
    }

    #[test]
    fn release_toggles_selection() {
        let mut w = EquipWheel::default();
        assert_eq!(w.update(true, true, [0.0; 2]), None);
        assert!(w.open);
        // released without pointing anywhere: nothing
        assert_eq!(w.update(false, true, [0.0; 2]), None);
        w.update(true, true, [0.0, 1.0]);
        assert_eq!(w.update(false, true, [0.0; 2]), Some(Equip::Thermal));
        // not allowed (dead, menu): closes without a toggle
        w.update(true, true, [0.0, -1.0]);
        assert_eq!(w.update(true, false, [0.0; 2]), None);
        assert!(!w.open);
    }
}
