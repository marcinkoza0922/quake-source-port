//! Touch settings use large hit areas and inline choices. They share the
//! option values and keyboard/controller navigation of the other pages.

use super::*;
use qinput::touch::TouchLayout;

pub(super) const ITEMS: &[Item] = &[
    slider("Yaw Sensitivity", "touch_yaw_sensitivity", 7.0, 84.0, 1.0),
    slider("Pitch Sensitivity", "touch_pitch_sensitivity", 7.0, 84.0, 1.0),
    toggle("Look Acceleration", "touch_acceleration"),
    Item::Choice { label: "Phone Gyro", cvar: "gyro_phone_mode", choices: PHONE_GYRO_MODE },
    Item::Choice { label: "Aim Assist", cvar: "touch_aim_assist", choices: &[("off", 0.0), ("low", 1.0), ("high", 2.0)] },
    Item::Choice { label: "Layout", cvar: "touch_left_handed", choices: &[("Right", 0.0), ("Left", 1.0)] },
    slider("Button Size", "touch_size", 0.5, 2.0, 0.25),
    slider("Opacity", "touch_opacity", 0.1, 1.0, 0.1),
    toggle("Invert Look", "touch_invert"),
    Item::Choice { label: "Gameplay Overlay", cvar: "touch_enable", choices: &[("off", 0.0), ("auto", 1.0), ("always", 2.0)] },
    Item::Run { label: "Edit Layout", run: Run::EditTouch },
    Item::Sub { label: "Gyro & Flick Stick", page: Page::Gyro },
    Item::Run { label: "Reset", run: Run::ResetTouch },
];

const ROW_X: f32 = 24.0;
const ROW_Y: f32 = 50.0;
const ROW_WIDTH: f32 = 350.0;
const ROW_HEIGHT: f32 = 27.0;
const ROWS: usize = ITEMS.len() - 3;
const EDIT: [f32; 4] = [390.0, 231.0, 226.0, 36.0];
const GYRO: [f32; 4] = [390.0, 272.0, 226.0, 36.0];
const CHOICE_X: f32 = 169.0;
const CHOICE_WIDTH: f32 = 195.0;
const PREVIEW: [f32; 4] = [390.0, 58.0, 226.0, 113.0];
const RESET: [f32; 4] = [248.0, 323.0, 126.0, 34.0];
pub(super) const BACK: [f32; 4] = [ROW_X, 323.0, 106.0, 34.0];
pub(super) const RESUME: [f32; 4] = [480.0, 112.0, 144.0, 48.0];
pub(super) const SHORTCUT: [f32; 4] = [480.0, 168.0, 144.0, 48.0];
pub(super) const SLIDER: (f32, f32) = (175.0, 318.0);

pub(super) fn hit([x, y, w, h]: [f32; 4], (px, py): (f32, f32)) -> bool {
    (x..x + w).contains(&px) && (y..y + h).contains(&py)
}

pub(super) fn item_at(pos: (f32, f32)) -> Option<usize> {
    for (i, bounds) in [EDIT, GYRO, RESET].into_iter().enumerate() {
        if hit(bounds, pos) {
            return Some(ROWS + i);
        }
    }
    let rows = [ROW_X, ROW_Y, ROW_WIDTH, ROWS as f32 * ROW_HEIGHT];
    hit(rows, pos).then(|| ((pos.1 - ROW_Y) / ROW_HEIGHT) as usize)
}

pub(super) fn reset(cv: &mut Cvars) {
    cv.set(qinput::touch::LAYOUT_CVAR, "");
    for &item in ITEMS {
        let name = match item {
            Item::Choice { cvar, .. } | Item::Toggle { cvar, .. } | Item::Slider { cvar, .. } => cvar,
            _ => continue,
        };
        if let Some(default) = cv.by_name(name).map(|c| c.default.clone()) {
            cv.set(name, &default);
        }
    }
}

impl Menu {
    /// The touch page fills the available landscape area, including sizes
    /// between the integer zoom levels of the classic menus.
    pub fn canvas_layout(&self, c: &mut Canvas) {
        if !matches!(self.screen, Screen::OptionsPage(Page::Touch) | Screen::TouchEditor) {
            c.menu_layout();
            return;
        }
        c.reset_origin();
        let [left, top, right, bottom] = self.touch_insets;
        let (width, height) = (c.vw() * c.scale, c.vh() * c.scale);
        let safe_width = width * (1.0 - left - right);
        let safe_height = height * (1.0 - top - bottom);
        c.scale = (safe_width / 640.0).min(safe_height / 360.0).max(0.1);
        c.ox = ((width * left + safe_width * 0.5) / c.scale - 320.0).floor();
        c.oy = ((height * top + safe_height * 0.5) / c.scale - 180.0).floor();
    }

    pub(super) fn touch_shortcut_index(&self) -> Option<usize> {
        self.touch_shortcut.then(|| MAIN_ITEMS + usize::from(self.custom_running))
    }

    pub fn set_touch_shortcut(&mut self, show: bool) {
        if self.touch_shortcut && !show {
            self.main_cursor = self.main_cursor.min(MAIN_ITEMS + usize::from(self.custom_running) - 1);
        }
        self.touch_shortcut = show;
    }

    pub(super) fn open_touch_from_main(&mut self) {
        self.open(Screen::OptionsPage(Page::Touch));
        self.opt_cursor = 0;
        self.touch_from_main = true;
    }

    pub(super) fn draw_touch_shortcut(&self, c: &mut Canvas) {
        let Some(index) = self.touch_shortcut_index() else { return };
        let [x, y, w, h] = SHORTCUT;
        let selected = self.main_cursor == index;
        touch_button(c, [x - CLASSIC.0, y - CLASSIC.1, w, h], "Touch Controls", selected);
    }

    pub fn touch_slider_at(&self, pos: (f32, f32)) -> Option<usize> {
        if self.screen != Screen::OptionsPage(Page::Touch) || self.prompt.is_some() {
            return None;
        }
        let row = item_at(pos)?;
        (matches!(ITEMS[row], Item::Slider { .. }) && (SLIDER.0 - 10.0..SLIDER.1 + 10.0).contains(&pos.0)).then_some(row)
    }

    pub(super) fn pick_touch_choice(&mut self, row: usize, pos: (f32, f32), cv: &mut Cvars) -> bool {
        let item = ITEMS[row];
        let count = self.options(item).len();
        if count == 0 || !(CHOICE_X..CHOICE_X + CHOICE_WIDTH).contains(&pos.0) {
            return false;
        }
        let col = ((pos.0 - CHOICE_X) / (CHOICE_WIDTH / count as f32)) as usize;
        self.set_option(item, col.min(count - 1), cv);
        true
    }

    pub(super) fn draw_touch_settings(&self, c: &mut Canvas, cv: &Cvars) {
        c.round_box(12.0, 10.0, 616.0, 350.0, 9.0, [0.045, 0.037, 0.031, 0.96], Some(BORDER));
        c.text(ROW_X, 21.0, "Touch Controls", 19.0, GOLD_FG);
        for (row, &item) in ITEMS[..ROWS].iter().enumerate() {
            let y = ROW_Y + row as f32 * ROW_HEIGHT;
            let selected = self.opt_cursor == row;
            let fill = if selected { [0.23, 0.16, 0.095, 0.98] } else { [0.092, 0.075, 0.06, 1.0] };
            c.round_box(ROW_X, y + 1.0, ROW_WIDTH, ROW_HEIGHT - 2.0, 5.0, fill, selected.then_some(BORDER));
            let label = match item {
                Item::Slider { label, cvar, min, max, .. } => {
                    draw_slider(c, y, cv.value_of(cvar), min, max);
                    label
                }
                Item::Choice { label, .. } | Item::Toggle { label, .. } => {
                    let opts = self.options(item);
                    let current = self.option_index(item, cv);
                    let width = CHOICE_WIDTH / opts.len() as f32;
                    for (i, opt) in opts.iter().enumerate() {
                        let x = CHOICE_X + i as f32 * width;
                        touch_button(c, [x + 1.0, y + 3.0, width - 2.0, ROW_HEIGHT - 6.0], opt, current == i);
                    }
                    label
                }
                _ => continue,
            };
            let mut label = qcore::lang::tr(label);
            if row < 2 {
                label.to_mut().push_str("  °/cm");
            }
            let size = fitted_size(c, &label, 12.0, 130.0);
            c.text_raw(ROW_X + 10.0, y + (ROW_HEIGHT - size) / 2.0, &label, size, if selected { GOLD_FG } else { FG });
        }
        self.draw_touch_preview(c, cv);
        touch_button(c, EDIT, "Edit Layout", self.opt_cursor == ROWS);
        touch_button(c, GYRO, "Gyro & Flick Stick", self.opt_cursor == ROWS + 1);
        touch_button(c, RESET, "Reset", self.opt_cursor == ROWS + 2);
    }

    fn draw_touch_preview(&self, c: &mut Canvas, cv: &Cvars) {
        let [x, y, w, h] = PREVIEW;
        c.text_center(x + w / 2.0, y - 25.0, "Preview", 13.0, FG);
        c.round_box(x, y, w, h, 6.0, [0.08, 0.064, 0.051, 1.0], Some(BORDER));
        let aspect = self.touch_aspect;
        let layout = TouchLayout::from_cvars(cv, aspect, self.touch_insets);
        let h = (h - 8.0).min((w - 8.0) / aspect);
        let area = [x + (w - h * aspect) / 2.0, y + (PREVIEW[3] - h) / 2.0, h * aspect, h];
        let [ax, ay, aw, ah] = area;
        c.fill(ax, ay, aw, ah, [0.17, 0.133, 0.101, 1.0]);
        let center = (ax + aw / 2.0, ay + ah / 2.0);
        c.fill(center.0 - 3.0, center.1, 7.0, 1.0, MUTED);
        c.fill(center.0, center.1 - 3.0, 1.0, 7.0, MUTED);
        let menu_only = cv.value_of("touch_enable") == 0.0;
        crate::touch_controls::draw(c, &layout, None, area, cv.value_of("touch_opacity"), menu_only);
        let item = ITEMS[self.opt_cursor.min(ITEMS.len() - 1)];
        if let Some(tip) = tooltip(item) {
            for (i, line) in super::navigation::wrap_tooltip_lines(c, &qcore::lang::tr(tip), w - 12.0, 11.0).iter().enumerate() {
                c.text_raw(x + 6.0, y + PREVIEW[3] + 8.0 + i as f32 * 14.0, line, 11.0, MUTED);
            }
        }
        c.text_center(x + w / 2.0, 321.0, "Tap to swap weapons.", 11.0, FG);
        c.text_center(x + w / 2.0, 336.0, "Hold and slide to select.", 11.0, MUTED);
    }
}

fn draw_slider(c: &mut Canvas, y: f32, value: f32, min: f32, max: f32) {
    let (x, end) = SLIDER;
    let fraction = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let w = end - x;
    let cy = y + ROW_HEIGHT / 2.0;
    c.round_box(x, cy - 3.0, w, 6.0, 3.0, [0.30, 0.23, 0.16, 1.0], None);
    c.round_box(x, cy - 3.0, w * fraction, 6.0, 3.0, GOLD_FG, None);
    let thumb = x + w * fraction;
    c.round_box(thumb - 6.0, cy - 6.0, 12.0, 12.0, 6.0, GOLD_FG, None);
    let value = qcore::cvar::format_float(value.clamp(min, max));
    c.text_right(363.0, cy - 5.5, &value, 11.0, FG);
}

pub(super) fn touch_button(c: &mut Canvas, [x, y, w, h]: [f32; 4], label: &str, selected: bool) {
    let fill = if selected { [0.37, 0.23, 0.12, 1.0] } else { [0.13, 0.10, 0.077, 1.0] };
    c.round_box(x, y, w, h, 5.0, fill, Some(BORDER));
    let label = qcore::lang::tr(label);
    let size = fitted_size(c, &label, (h * 0.43).min(16.0), w - 8.0);
    let color = if selected { GOLD_FG } else { FG };
    c.text_raw(x + (w - c.text_w_raw(&label, size)) / 2.0, y + (h - size) / 2.0, &label, size, color);
}

fn fitted_size(c: &Canvas, text: &str, size: f32, width: f32) -> f32 {
    size * (width / c.text_w_raw(text, size).max(1.0)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_shortcut_slider_choices_reset_and_back() {
        let mut menu = Menu::new();
        let mut cv = Cvars::new();
        qinput::Input::new(&mut cv);
        menu.touch_shortcut = true;
        menu.open(Screen::Main);
        let click = |menu: &mut Menu, cv: &mut Cvars, pos| {
            menu.mouse_move(pos, cv);
            menu.key(K_MOUSE1, cv, true);
        };
        click(&mut menu, &mut cv, (SHORTCUT[0] + 3.0, SHORTCUT[1] + 3.0));
        assert_eq!(menu.screen, Screen::OptionsPage(Page::Touch));
        click(&mut menu, &mut cv, (SLIDER.0, ROW_Y + 10.0));
        assert_eq!(cv.value_of("touch_yaw_sensitivity"), 7.0);
        menu.mouse_move((SLIDER.1 + 40.0, ROW_Y + 60.0), &mut cv);
        assert_eq!(cv.value_of("touch_yaw_sensitivity"), 84.0);
        menu.mouse_up();
        click(&mut menu, &mut cv, (CHOICE_X + CHOICE_WIDTH - 5.0, ROW_Y + 4.0 * ROW_HEIGHT + 10.0));
        assert_eq!(cv.value_of("touch_aim_assist"), 2.0);
        click(&mut menu, &mut cv, (CHOICE_X + CHOICE_WIDTH - 5.0, ROW_Y + 5.0 * ROW_HEIGHT + 10.0));
        assert_eq!(cv.value_of("touch_left_handed"), 1.0);
        click(&mut menu, &mut cv, (RESET[0] + 3.0, RESET[1] + 3.0));
        assert_eq!(cv.value_of("touch_yaw_sensitivity"), 28.0);
        assert_eq!(cv.value_of("touch_aim_assist"), 1.0);
        assert_eq!(cv.value_of("touch_left_handed"), 0.0);
        click(&mut menu, &mut cv, (BACK[0] + 3.0, BACK[1] + 3.0));
        assert_eq!(menu.screen, Screen::Main);
        click(&mut menu, &mut cv, (RESUME[0] + 3.0, RESUME[1] + 3.0));
        assert_eq!(menu.screen, Screen::None);
    }
}
