//! An interactive preview using the gameplay overlay's hit tests and safe area.

use super::*;
use crate::hud::place::{self, Rect, SnapResult};
use qinput::touch::{BUTTONS, LAYOUT_CVAR, TouchButtons, TouchLayout};

const STAGE: [f32; 4] = [16.0, 49.0, 608.0, 250.0];
const RESET: [f32; 4] = [16.0, 313.0, 117.0, 37.0];
const SELECT: [f32; 4] = [306.0, 313.0, 68.0, 37.0];
const SMALLER: [f32; 4] = [260.0, 313.0, 42.0, 37.0];
const LARGER: [f32; 4] = [378.0, 313.0, 42.0, 37.0];
const DONE: [f32; 4] = [524.0, 313.0, 100.0, 37.0];

#[derive(Default)]
pub(super) struct Editor {
    selected: usize,
    grab: Option<[f32; 2]>,
    guides: SnapResult,
}

impl Editor {
    pub(super) fn release(&mut self) {
        self.grab = None;
        self.guides = Default::default();
    }
}

fn stage(aspect: f32) -> [f32; 4] {
    let [x, y, w, h] = STAGE;
    let height = h.min(w / aspect);
    let width = height * aspect;
    [x + (w - width) * 0.5, y + (h - height) * 0.5, width, height]
}

fn rect([x, y]: [f32; 2], radius: f32) -> Rect {
    Rect { x: x - radius, y: y - radius, w: radius * 2.0, h: radius * 2.0 }
}

impl Menu {
    fn edit_touch_layout(&self, cv: &Cvars) -> TouchLayout {
        let mut layout = TouchLayout::from_cvars(cv, self.touch_aspect, self.touch_insets);
        layout.buttons = TouchButtons::PREVIEW;
        layout.move_fire = true;
        layout
    }

    pub(super) fn move_touch_button(&mut self, (px, py): (f32, f32), cv: &mut Cvars) {
        let Some([ox, oy]) = self.touch_editor.grab else { return };
        let index = self.touch_editor.selected;
        let mut layout = self.edit_touch_layout(cv);
        let [x, y, w, h] = stage(layout.aspect);
        let [l, t, r, b] = layout.insets;
        let (left, top) = (l * w, t * h);
        let width = w * (1.0 - l - r);
        let height = h * (1.0 - t - b);
        let radius = layout.button(index).1 * h;
        let desired = rect([px - x - left - ox, py - y - top - oy], radius);
        let others: Vec<_> = (0..BUTTONS.len()).filter(|&i| i != index).map(|i| {
            let ([cx, cy], radius) = layout.button(i);
            rect([cx * w - left, cy * h - top], radius * h)
        }).collect();
        let guides = place::snap_rect(desired, &others, width, height, 3.0);
        let center = [
            (guides.rect.x + radius + left) / w,
            (guides.rect.y + radius + top) / h,
        ];
        layout.placements[index] = Some(layout.placement_at(center, layout.placement(index).size));
        // Store the clamped centre, so reducing the size later doesn't shift the button.
        layout.placements[index] = Some(layout.placement_at(layout.button(index).0, layout.placement(index).size));
        layout.save(cv);
        self.touch_editor.guides = guides;
    }

    fn resize_touch_button(&mut self, delta: f32, cv: &mut Cvars) {
        let mut layout = self.edit_touch_layout(cv);
        let index = self.touch_editor.selected;
        let (center, _) = layout.button(index);
        let size = layout.placement(index).size + delta;
        layout.placements[index] = Some(layout.placement_at(center, size));
        layout.save(cv);
    }

    pub(super) fn touch_editor_key(&mut self, key: i32, cv: &mut Cvars) {
        match key {
            K_ESCAPE | K_BBUTTON => {
                self.touch_editor.release();
                self.screen = Screen::OptionsPage(Page::Touch);
            }
            K_TAB | K_DOWNARROW | K_DPAD_DOWN => {
                self.touch_editor.selected = (self.touch_editor.selected + 1) % BUTTONS.len();
            }
            K_UPARROW | K_DPAD_UP => {
                self.touch_editor.selected = (self.touch_editor.selected + BUTTONS.len() - 1) % BUTTONS.len();
            }
            K_LEFTARROW | K_DPAD_LEFT => self.resize_touch_button(-0.1, cv),
            K_RIGHTARROW | K_DPAD_RIGHT => self.resize_touch_button(0.1, cv),
            K_MOUSE1 => {
                let Some((px, py)) = self.mouse else { return };
                let pos = (px, py);
                if touch::hit(DONE, pos) {
                    self.touch_editor_key(K_ESCAPE, cv);
                } else if touch::hit(RESET, pos) {
                    cv.set(LAYOUT_CVAR, "");
                } else if touch::hit(SELECT, pos) {
                    self.touch_editor_key(K_TAB, cv);
                } else if touch::hit(SMALLER, pos) {
                    self.resize_touch_button(-0.1, cv);
                } else if touch::hit(LARGER, pos) {
                    self.resize_touch_button(0.1, cv);
                } else {
                    let layout = self.edit_touch_layout(cv);
                    let [x, y, w, h] = stage(layout.aspect);
                    if let Some(index) = layout.button_at([(px - x) / w, (py - y) / h]) {
                        let ([cx, cy], _) = layout.button(index);
                        self.touch_editor.selected = index;
                        self.touch_editor.grab = Some([px - x - cx * w, py - y - cy * h]);
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn draw_touch_editor(&self, c: &mut Canvas, cv: &Cvars) {
        let layout = self.edit_touch_layout(cv);
        let area = stage(layout.aspect);
        let [x, y, w, h] = area;
        let [l, t, r, b] = layout.insets;
        let safe = [x + l * w, y + t * h, (1.0 - l - r) * w, (1.0 - t - b) * h];
        c.round_box(8.0, 6.0, 624.0, 351.0, 8.0, [0.045, 0.037, 0.031, 0.98], Some(BORDER));
        c.text(16.0, 14.0, "Edit Layout", 19.0, GOLD_FG);
        c.text_right(624.0, 20.0, "Drag to move", 12.0, FG);
        c.fill(x, y, w, h, [0.07, 0.055, 0.045, 1.0]);
        c.fill(safe[0], safe[1], safe[2], safe[3], [0.16, 0.12, 0.09, 1.0]);
        for &gx in &self.touch_editor.guides.guides_x {
            c.fill(safe[0] + gx, safe[1], 1.0, safe[3], MUTED);
        }
        for &gy in &self.touch_editor.guides.guides_y {
            c.fill(safe[0], safe[1] + gy, safe[2], 1.0, MUTED);
        }
        let selected = self.touch_editor.selected;
        crate::touch_controls::draw(c, &layout, None, area, 1.0, false);
        let ([cx, cy], radius) = layout.button(selected);
        let radius = radius * h + 2.0;
        c.round_box(x + cx * w - radius, y + cy * h - radius, radius * 2.0, radius * 2.0, radius, [0.0; 4], Some(GOLD_FG));
        touch::touch_button(c, RESET, "Reset", false);
        touch::touch_button(c, SMALLER, "−", false);
        touch::touch_button(c, LARGER, "+", false);
        touch::touch_button(c, DONE, "Done", false);
        let [x, y, w, h] = SELECT;
        c.round_box(x, y, w, h, 5.0, [0.13, 0.10, 0.077, 1.0], Some(BORDER));
        c.text_center(340.0, 317.0, BUTTONS[selected].label, 10.0, FG);
        c.text_center(340.0, 332.0, &format!("{}%", (layout.placement(selected).size * 100.0).round()), 12.0, GOLD_FG);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_drag_and_resize_persist_independent_button_geometry() {
        let mut menu = Menu::new();
        let mut cv = Cvars::new();
        qinput::Input::new(&mut cv);
        menu.open(Screen::Main);
        menu.open_touch_from_main();
        menu.run(Run::EditTouch, &mut cv, &mut Vec::new());
        let layout = menu.edit_touch_layout(&cv);
        let ([fx, fy], _) = layout.button(0);
        let [x, y, w, h] = stage(layout.aspect);
        menu.mouse_move((x + fx * w, y + fy * h), &mut cv);
        menu.key(K_MOUSE1, &mut cv, true);
        menu.mouse_move((x + w * 0.3, y + h * 0.4), &mut cv);
        menu.mouse_up();
        menu.key(K_RIGHTARROW, &mut cv, true);
        menu.key(K_ESCAPE, &mut cv, true);
        assert_eq!(menu.screen, Screen::OptionsPage(Page::Touch));
        let edited = menu.edit_touch_layout(&cv);
        assert_ne!(edited.button(0).0, layout.button(0).0);
        assert!(edited.button(0).1 > layout.button(0).1);
        for i in 1..BUTTONS.len() {
            assert_eq!(edited.button(i), layout.button(i));
        }
        menu.key(K_ESCAPE, &mut cv, true);
        assert_eq!(menu.screen, Screen::Main);
    }
}
