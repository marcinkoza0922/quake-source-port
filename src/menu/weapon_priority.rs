//! Options → Gameplay → Weapon Priority: drag-and-drop and controller reorderable
//! priority list for `cl_weaponpriority`.

use qserver::weaponswitch::{DEFAULT_PRIORITY, format_priority, parse_priority, read_priority};

use crate::font::Shape;
use crate::wheel::SLOTS;

use super::*;

const WP_X: f32 = 152.0;
const WP_W: f32 = 336.0;
const WP_ROW0: f32 = 82.0;
const WP_ROW_H: f32 = 21.0;
pub(super) const WP_COUNT: usize = 8;
pub(super) const WP_BACK: (f32, f32) = (WP_X, WP_ROW0 + WP_COUNT as f32 * WP_ROW_H + 10.0);
const RESET_BTN: [f32; 4] = [WP_X + WP_W - 132.0, WP_BACK.1, 132.0, 16.0];

#[derive(Clone, Copy, Debug)]
struct MouseDrag {
    index: usize,
    grab_dy: f32,
    pointer_y: f32,
}

#[derive(Default)]
pub(super) struct WeaponPriorityUi {
    pub cursor: usize,
    /// When grabbed via controller A / Enter: (original index, original order).
    grabbed: Option<(usize, [u8; 8])>,
    /// Active mouse drag-and-drop state.
    drag: Option<MouseDrag>,
}

impl WeaponPriorityUi {
    pub fn reset_state(&mut self) {
        self.cursor = 0;
        self.grabbed = None;
        self.drag = None;
    }

    pub fn mouse_up(&mut self) {
        self.drag = None;
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed.is_some()
    }
}

pub(super) fn slot_for_impulse(impulse: u8) -> &'static crate::wheel::Slot {
    &SLOTS[(impulse.clamp(1, 8) - 1) as usize]
}

fn row_top(i: usize) -> f32 {
    WP_ROW0 + i as f32 * WP_ROW_H
}

fn row_at(pos: (f32, f32)) -> Option<usize> {
    if pos.0 < WP_X || pos.0 > WP_X + WP_W {
        return None;
    }
    let y = pos.1 - WP_ROW0;
    if y < 0.0 || y >= WP_COUNT as f32 * WP_ROW_H {
        return None;
    }
    Some((y / WP_ROW_H) as usize)
}

fn in_rect(pos: (f32, f32), r: [f32; 4]) -> bool {
    pos.0 >= r[0] && pos.0 <= r[0] + r[2] && pos.1 >= r[1] && pos.1 <= r[1] + r[3]
}

pub(super) fn save_order(cv: &mut Cvars, order: &[u8; 8]) {
    cv.set("cl_weaponpriority", &format_priority(order));
}

pub(super) fn move_entry(order: &mut [u8; 8], from: usize, to: usize) {
    if from == to || from >= WP_COUNT || to >= WP_COUNT {
        return;
    }
    let v = order[from];
    if from < to {
        order.copy_within(from + 1..=to, from);
    } else {
        order.copy_within(to..from, to + 1);
    }
    order[to] = v;
}

impl Menu {
    pub(super) fn wp_mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) -> Option<&'static str> {
        if let Some(mut d) = self.wp.drag {
            d.pointer_y = pos.1;
            let target = ((pos.1 - WP_ROW0) / WP_ROW_H).floor().clamp(0.0, (WP_COUNT - 1) as f32) as usize;
            let changed = target != d.index;
            if changed {
                let mut order = read_priority(cv);
                move_entry(&mut order, d.index, target);
                save_order(cv, &order);
                d.index = target;
                self.wp.cursor = target;
            }
            self.wp.drag = Some(d);
            return changed.then_some("misc/menu1.wav");
        }
        if self.wp.grabbed.is_some() {
            return None;
        }
        if let Some(i) = row_at(pos) {
            return (self.wp.cursor != i).then(|| {
                self.wp.cursor = i;
                "misc/menu1.wav"
            });
        }
        if in_rect(pos, RESET_BTN) {
            return (self.wp.cursor != WP_COUNT).then(|| {
                self.wp.cursor = WP_COUNT;
                "misc/menu1.wav"
            });
        }
        None
    }

    pub(super) fn wp_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) {
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_SPACE | K_ABUTTON);
        let back = matches!(key, K_ESCAPE | K_BBUTTON);

        if key == K_MOUSE1 {
            let Some(pos) = self.mouse else { return };
            if let Some((bx, by, bw, bh)) = self.back_button()
                && in_rect(pos, [bx, by, bw, bh])
            {
                self.wp.grabbed = None;
                self.wp.drag = None;
                self.leave_wp(out);
                return;
            }
            if in_rect(pos, RESET_BTN) {
                self.wp.grabbed = None;
                self.wp.drag = None;
                self.wp.cursor = WP_COUNT;
                cv.set("cl_weaponpriority", DEFAULT_PRIORITY);
                out.push(Action::Sound("misc/menu2.wav"));
                return;
            }
            if let Some(i) = row_at(pos) {
                self.wp.grabbed = None;
                self.wp.cursor = i;
                self.wp.drag = Some(MouseDrag { index: i, grab_dy: pos.1 - row_top(i), pointer_y: pos.1 });
                out.push(Action::Sound("misc/menu2.wav"));
            }
            return;
        }

        if let Some((orig_idx, orig_order)) = self.wp.grabbed {
            if up && self.wp.cursor > 0 {
                let mut order = read_priority(cv);
                order.swap(self.wp.cursor, self.wp.cursor - 1);
                self.wp.cursor -= 1;
                save_order(cv, &order);
                out.push(Action::Sound("misc/menu1.wav"));
            } else if down && self.wp.cursor + 1 < WP_COUNT {
                let mut order = read_priority(cv);
                order.swap(self.wp.cursor, self.wp.cursor + 1);
                self.wp.cursor += 1;
                save_order(cv, &order);
                out.push(Action::Sound("misc/menu1.wav"));
            } else if enter {
                self.wp.grabbed = None;
                out.push(Action::Sound("misc/menu2.wav"));
            } else if back {
                save_order(cv, &orig_order);
                self.wp.cursor = orig_idx;
                self.wp.grabbed = None;
                out.push(Action::Sound("misc/menu3.wav"));
            }
            return;
        }

        if back {
            self.leave_wp(out);
            return;
        }
        if up {
            self.wp.cursor = if self.wp.cursor == 0 { WP_COUNT } else { self.wp.cursor - 1 };
            out.push(Action::Sound("misc/menu1.wav"));
        } else if down {
            self.wp.cursor = (self.wp.cursor + 1) % (WP_COUNT + 1);
            out.push(Action::Sound("misc/menu1.wav"));
        } else if key == K_YBUTTON {
            cv.set("cl_weaponpriority", DEFAULT_PRIORITY);
            out.push(Action::Sound("misc/menu2.wav"));
        } else if enter {
            if self.wp.cursor < WP_COUNT {
                let order = read_priority(cv);
                self.wp.grabbed = Some((self.wp.cursor, order));
                out.push(Action::Sound("misc/menu2.wav"));
            } else {
                cv.set("cl_weaponpriority", DEFAULT_PRIORITY);
                out.push(Action::Sound("misc/menu2.wav"));
            }
        }
    }

    fn leave_wp(&mut self, out: &mut Vec<Action>) {
        self.wp.reset_state();
        let (parent, row) = self
            .sub_return
            .take()
            .filter(|&(_, child, _)| child == Page::WeaponPriority)
            .map(|(par, _, row)| (par, row))
            .or_else(|| parent_page(Page::WeaponPriority))
            .unwrap_or((Page::Modifiers, 0));
        self.screen = Screen::OptionsPage(parent);
        self.opt_cursor = row;
        out.push(Action::Sound("misc/menu3.wav"));
    }

    pub(super) fn draw_weapon_priority(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars) {
        let p = pic("gfx/qplaque.lmp");
        c.pic(PANEL_X - 40.0, 16.0, p);
        if let Some(t) = pic("gfx/p_option.lmp") {
            c.pic(((640 - t.1 as i32) / 2) as f32, 16.0, Some(t));
        }
        self.draw_page_title(c, Page::WeaponPriority);
        c.text_center(320.0, WP_ROW0 - 15.0, qcore::t!("Highest priority at top"), TXT, MUTED);

        // Panel background.
        let panel_h = WP_COUNT as f32 * WP_ROW_H + 8.0;
        c.round_box(WP_X - 6.0, WP_ROW0 - 4.0, WP_W + 12.0, panel_h, 4.0, [0.0, 0.0, 0.0, 0.55], Some(BORDER));

        let order = parse_priority(cv.by_name("cl_weaponpriority").map_or(DEFAULT_PRIORITY, |c| c.string.as_str()));
        let dragging = self.wp.drag;

        for (i, &imp) in order.iter().enumerate() {
            let y = row_top(i);
            if dragging.is_some_and(|d| d.index == i) {
                // Drop slot placeholder while dragged card floats on top.
                c.round_box(WP_X, y + 1.0, WP_W, WP_ROW_H - 2.0, 3.0, [0.85, 0.65, 0.30, 0.14], Some([0.95, 0.78, 0.35, 0.55]));
                continue;
            }
            let selected = self.wp.cursor == i;
            let grabbed = selected && self.wp.grabbed.is_some();
            draw_weapon_row(c, pic, i, imp, y, selected, grabbed, false);
        }

        // Draw dragged card floating smoothly under the pointer.
        if let Some(d) = dragging {
            let imp = order[d.index];
            let fy = (d.pointer_y - d.grab_dy).clamp(WP_ROW0 - 4.0, row_top(WP_COUNT - 1) + 4.0);
            c.round_box(WP_X + 2.0, fy + 3.0, WP_W, WP_ROW_H - 2.0, 3.0, [0.0, 0.0, 0.0, 0.55], None);
            draw_weapon_row(c, pic, d.index, imp, fy, true, true, true);
        }

        // Reset to Defaults button.
        let hover_reset = self.wp.cursor == WP_COUNT
            || self.mouse.is_some_and(|m| in_rect(m, RESET_BTN));
        let bg = if hover_reset { HILITE } else { [0.0, 0.0, 0.0, 0.7] };
        c.round_box(RESET_BTN[0], RESET_BTN[1], RESET_BTN[2], RESET_BTN[3], 4.0, bg, Some(BORDER));
        let fg = if hover_reset { GOLD_FG } else { FG };
        let pad_key = self.pad_ui.then_some(K_YBUTTON);
        self.glyphs.button_label(c, RESET_BTN, pad_key, qcore::t!("Reset to Defaults"), TXT, fg);

        // Help line below buttons.
        let help = if self.pad_ui {
            if self.wp.grabbed.is_some() {
                qcore::t!("Use D-Pad or stick to move, A to place, B to cancel")
            } else {
                qcore::t!("Press A to pick up a weapon, then move it up or down")
            }
        } else if self.wp.grabbed.is_some() {
            qcore::t!("Use Up and Down arrows to move, Enter to place, Esc to cancel")
        } else {
            qcore::t!("Drag weapons to reorder, or press Enter and use arrow keys")
        };
        let hw = c.text_w(help, TXT);
        let hy = WP_BACK.1 + 24.0;
        c.round_box(320.0 - hw * 0.5 - 8.0, hy - 3.0, hw + 16.0, 14.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        c.text_center(320.0, hy, help, TXT, MUTED);

        if self.pad_ui {
            let items: &[(i32, &str)] = if self.wp.grabbed.is_some() {
                &[(K_DPAD_UP, qcore::t!("Reorder")), (K_ABUTTON, qcore::t!("Place")), (K_BBUTTON, qcore::t!("Cancel"))]
            } else {
                &[(K_ABUTTON, qcore::t!("Move"))]
            };
            self.boxed_hints(c, 344.0, items);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_weapon_row(c: &mut Canvas, pic: &mut PicFn, rank: usize, impulse: u8, y: f32, selected: bool, grabbed: bool, floating: bool) {
    let bg = if grabbed || floating {
        [0.42, 0.28, 0.10, 0.96]
    } else if selected {
        HILITE
    } else {
        [0.06, 0.05, 0.04, 0.72]
    };
    let edge = if grabbed || floating { Some(GOLD_FG) } else if selected { Some(BORDER) } else { Some([1.0, 1.0, 1.0, 0.08]) };
    c.round_box(WP_X, y + 1.0, WP_W, WP_ROW_H - 2.0, 3.0, bg, edge);

    // Priority number badge.
    let num = format!("{}.", rank + 1);
    c.text_raw(WP_X + 8.0, y + 6.0, &num, TXT, if grabbed || selected { GOLD_FG } else { MUTED });

    // Weapon icon (from gfx.wad) or compact badge for Axe.
    let slot = slot_for_impulse(impulse);
    let ix = WP_X + 28.0;
    match slot.icon.and_then(&mut *pic) {
        Some((id, w, h)) => {
            let scale = (14.0 / h.max(1) as f32).min(26.0 / w.max(1) as f32).min(1.0);
            let dw = (w as f32 * scale).round();
            let dh = (h as f32 * scale).round();
            c.pic_color(ix + (26.0 - dw) * 0.5, y + (WP_ROW_H - dh) * 0.5, Some((id, dw as u32, dh as u32)), [1.0; 4]);
        }
        None => {
            c.round_box(ix + 4.0, y + 5.0, 18.0, 12.0, 2.0, [0.18, 0.15, 0.12, 0.9], Some(BORDER));
            c.text_raw(ix + 7.5, y + 6.5, "AX", 7.0, GOLD_FG);
        }
    }

    // Weapon name.
    c.text(WP_X + 62.0, y + 6.0, slot.name, TXT, if grabbed || floating { GOLD_FG } else { FG });

    // Right side: up/down arrows while grabbed, else subtle drag-grip bars.
    let rx = WP_X + WP_W - 22.0;
    if grabbed || floating {
        if rank > 0 {
            c.shape(rx, y + 3.0, 7.0, 7.0, Shape::ArrowUp, GOLD_FG);
        }
        if rank + 1 < WP_COUNT {
            c.shape(rx, y + 12.0, 7.0, 7.0, Shape::ArrowDown, GOLD_FG);
        }
    } else {
        let grip = if selected { GOLD_FG } else { [0.65, 0.60, 0.52, 0.55] };
        for k in 0..3 {
            c.fill(rx, y + 7.0 + k as f32 * 3.0, 10.0, 1.0, grip);
        }
    }
}
