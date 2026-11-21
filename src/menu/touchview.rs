//! Touch layout of the menus. While touch is the active device, the home
//! (pause) menu, the Options hub and the options pages are laid out in
//! the platform's UI units (Android dp, Windows/Linux logical pixels, CSS
//! pixels) instead of the scaled 640x360 canvas, so rows and buttons keep
//! a finger-sized height (48+ units) on every screen size. The pages and
//! their logic are shared with the classic layout (`Item`, `adjust`,
//! `options`, `set_option`, ...); only drawing and hit testing differ.
//! Screens without a touch layout yet keep the classic one, and dialogs
//! (prompts, on-screen keyboard, key capture) are drawn over it classically.

use super::*;
use std::cell::Cell;

mod addons;
mod cards;
mod dialog;
mod help;
mod home;
mod hub;
mod list;
mod page;
mod rows;
mod weapons;
#[cfg(test)]
mod tests;

/// Top bar (Back button and title) height.
pub(super) const BAR_H: f32 = 56.0;
/// Settings row, heading row and a row whose control sits under its label.
pub(super) const ROW: f32 = 52.0;
pub(super) const HEAD: f32 = 36.0;
pub(super) const STACK_ROW: f32 = 92.0;
/// Space kept from the safe area's edges, and the widest content column.
pub(super) const MARGIN: f32 = 16.0;
pub(super) const MAX_W: f32 = 760.0;
/// Description bar at the bottom of a page.
pub(super) const FOOT_H: f32 = 60.0;
/// Text sizes: row labels, values and small print.
pub(super) const LABEL: f32 = 17.0;
pub(super) const VALUE: f32 = 16.0;
pub(super) const SMALL: f32 = 13.5;
/// The safe area is always at least this many units on its short side.
const MIN_UNITS: f32 = 320.0;
/// Back button in the top bar.
const BACK_W: f32 = 112.0;

pub(super) const PANEL_BG: [f32; 4] = [0.045, 0.037, 0.031, 0.94];
/// Dialogs, sheets and the footer hide what is under them completely.
pub(super) const DIALOG_BG: [f32; 4] = [0.05, 0.04, 0.033, 1.0];
pub(super) const PRESS: [f32; 4] = [0.6, 0.38, 0.12, 0.55];
pub(super) const SELECT: [f32; 4] = [0.6, 0.38, 0.12, 0.22];
pub(super) const LINE: [f32; 4] = [1.0, 1.0, 1.0, 0.07];

/// x, y, w, h in units.
pub(super) type Rect = [f32; 4];

/// One row of a touch list screen, filled by that screen's module.
#[derive(Clone, Debug, Default)]
pub struct ListRow {
    pub title: String,
    pub sub: String,
    pub right: String,
    pub color: [f32; 4],
    pub right_color: [f32; 4],
}

pub(super) fn inside([x, y, w, h]: Rect, (px, py): (f32, f32)) -> bool {
    (x..x + w).contains(&px) && (y..y + h).contains(&py)
}

/// Per-screen state of the touch layout.
#[derive(Default)]
pub struct TouchView {
    /// Screen the state below belongs to (reset when it changes).
    screen: Option<Screen>,
    /// Content scroll in units.
    scroll: f32,
    /// Choice list open over a page row.
    sheet: Option<Sheet>,
    /// Page row whose description shows in the footer.
    footer: Option<usize>,
    /// The Options search field has focus (the OS keyboard is up).
    search_focus: bool,
    /// Page text row that was tapped (it takes the OS keyboard).
    text_focus: Option<usize>,
    /// The Load screen listed autosaves (a different list: back to the top).
    autosaves: bool,
    /// Add-ons browser: the add-on whose details are open.
    addon_detail: Option<usize>,
    /// Help: a section's text is open (else the section list).
    help_reading: bool,
    /// Help: height of the open section's text (set while drawing).
    help_h: Cell<f32>,
    /// Safe-area size in units at the last layout.
    size: Cell<(f32, f32)>,
}

/// The list of a choice row's values.
#[derive(Clone, Copy, Debug)]
struct Sheet {
    row: usize,
    scroll: f32,
}


impl Menu {
    /// Touch is the active device and the screen has a touch layout.
    pub(super) fn tv_screen(&self) -> bool {
        self.touch_ui
            && match self.screen {
                Screen::Main | Screen::SinglePlayer | Screen::Multiplayer | Screen::Options | Screen::Quit | Screen::Death
                | Screen::Load | Screen::Save | Screen::Mods | Screen::ModMaps | Screen::ModOnline | Screen::Demos | Screen::Help => true,
                Screen::ServerBrowser => !self.server_filters_open(),
                Screen::OptionsPage(_) => true,
                _ => false,
            }
    }

    /// The on-screen keyboard or a key capture is up: they keep the
    /// classic layout (a key or the pad answers them, not a tap).
    fn tv_classic_modal(&self) -> bool {
        self.osk.is_some() || self.capture.is_some() || self.addon_capture.is_some()
    }

    /// Pointer input goes to the touch layout: its screen, or a touch
    /// dialog over any screen.
    pub(super) fn tv_input(&self) -> bool {
        self.touch_ui && !self.tv_classic_modal() && (self.tv_screen() || self.tv_ask().is_some())
    }

    /// Pixels per layout unit for a safe area of `sw` x `sh` pixels: the
    /// display scale, reduced so the short side keeps [`MIN_UNITS`].
    fn tv_scale(&self, sw: f32, sh: f32) -> f32 {
        let s = if self.display_scale.is_finite() && self.display_scale > 0.0 { self.display_scale } else { 1.0 };
        s.min(sw.min(sh) / MIN_UNITS).max(0.25)
    }

    /// Canvas origin at the safe area's top-left, in layout units.
    pub(super) fn tv_layout(&self, c: &mut Canvas) {
        c.reset_origin();
        let [l, t, r, b] = self.touch_insets;
        let (sw, sh) = (c.w * (1.0 - l - r), c.h * (1.0 - t - b));
        c.scale = self.tv_scale(sw, sh);
        c.ox = c.w * l / c.scale;
        c.oy = c.h * t / c.scale;
        self.tv.size.set((sw / c.scale, sh / c.scale));
    }

    pub(super) fn tv_size(&self) -> (f32, f32) {
        self.tv.size.get()
    }

    /// Content column: left edge and width.
    pub(super) fn tv_col(&self) -> (f32, f32) {
        let (w, _) = self.tv_size();
        let cw = (w - 2.0 * MARGIN).clamp(100.0, MAX_W);
        ((w - cw) / 2.0, cw)
    }

    /// Scroll of the current screen (0 until its state is set up).
    pub(super) fn tv_scroll(&self) -> f32 {
        if self.tv.screen == Some(self.screen) && self.tv.autosaves == self.autosaves { self.tv.scroll } else { 0.0 }
    }

    fn tv_footer(&self) -> Option<usize> {
        self.tv.footer.filter(|_| self.tv.screen == Some(self.screen))
    }

    fn tv_sheet(&self) -> Option<Sheet> {
        self.tv.sheet.filter(|_| self.tv.screen == Some(self.screen))
    }

    /// In the touch layout a text row takes typing only once tapped.
    pub(super) fn tv_text_focus(&self) -> Option<bool> {
        (self.tv_screen() && matches!(self.screen, Screen::OptionsPage(_)))
            .then(|| self.tv.screen == Some(self.screen) && self.tv.text_focus == Some(self.opt_cursor))
    }

    /// The search or filter field has focus (the OS keyboard is up).
    pub(super) fn tv_search_focus(&self) -> bool {
        self.tv.search_focus && self.tv_input() && !self.tv_dialog_open() && (self.screen == Screen::Options || self.tv_list_filters())
    }

    /// Resets the per-screen state after a screen change, scrolling a
    /// page so its selected row (the one a sub-page returns to) shows.
    pub(super) fn tv_sync(&mut self) {
        if self.tv.screen == Some(self.screen) && self.tv.autosaves == self.autosaves {
            return;
        }
        let back_from = self.tv.screen;
        self.tv.screen = Some(self.screen);
        self.tv.autosaves = self.autosaves;
        // back from an add-on's parameters page: its details again
        let sel = self.addon_ui_selected();
        self.tv.addon_detail = (matches!(self.screen, Screen::OptionsPage(Page::Addons | Page::NewGameAddons))
            && back_from.is_some_and(|s| s == Screen::OptionsPage(addon_browser::addon_info(sel).page.unwrap_or(Page::Addons))))
            .then_some(sel);
        self.tv.scroll = 0.0;
        self.tv.sheet = None;
        self.tv.footer = None;
        self.tv.search_focus = false;
        self.tv.text_focus = None;
        self.tv.help_reading = false;
        if let Screen::OptionsPage(p) = self.screen
            && self.opt_cursor > 0
            && self.opt_cursor < self.page_len(p)
        {
            let (_, h) = self.tv_size();
            let (top, rh) = self.tv_row_span(p, self.opt_cursor);
            let view = h - BAR_H - FOOT_H;
            if top + rh > view {
                self.tv.scroll = (top + rh / 2.0 - view / 2.0).clamp(0.0, self.tv_max_scroll());
            }
        }
    }

    /// Touch leaves (keyboard, mouse or pad used): close touch-only state.
    pub(super) fn tv_leave(&mut self) {
        self.tv.sheet = None;
        self.tv.search_focus = false;
    }

    /// Height of the scrolling content of the current screen.
    fn tv_content_h(&self) -> f32 {
        match self.screen {
            Screen::OptionsPage(Page::WeaponPriority) => self.tv_wp_h(),
            Screen::OptionsPage(Page::Addons | Page::NewGameAddons) => self.tv_addons_h(),
            Screen::OptionsPage(p) => self.tv_page_h(p),
            Screen::Options => self.tv_hub_h(),
            Screen::Main | Screen::SinglePlayer | Screen::Multiplayer => self.tv_home_h(),
            Screen::Load | Screen::Save => self.tv_cards_h(),
            _ => 0.0,
        }
    }

    fn tv_max_scroll(&self) -> f32 {
        let (_, h) = self.tv_size();
        if let Some(v) = self.tv_list() {
            return self.tv_list_max_scroll(&v);
        }
        if self.screen == Screen::Help {
            return (self.tv.help_h.get() + MARGIN - (h - BAR_H)).max(0.0);
        }
        let foot = if matches!(self.screen, Screen::OptionsPage(_)) { FOOT_H + 8.0 } else { MARGIN };
        (self.tv_content_h() + foot - (h - BAR_H)).max(0.0)
    }

    /// Drag scrolling; returns whether anything moved.
    pub(super) fn tv_scroll_by(&mut self, dy: f32) -> bool {
        if self.tv_dialog_open() {
            return false;
        }
        self.tv_sync();
        if let Some(mut s) = self.tv.sheet {
            let before = s.scroll;
            s.scroll = (s.scroll + dy).clamp(0.0, self.tv_sheet_max_scroll(s));
            self.tv.sheet = Some(s);
            return s.scroll != before;
        }
        let before = self.tv.scroll;
        self.tv.scroll = (self.tv.scroll + dy).clamp(0.0, self.tv_max_scroll());
        self.tv.scroll != before
    }

    /// Pointer motion while a finger is down (slider drags).
    pub(super) fn tv_mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) -> Option<&'static str> {
        self.tv_sync();
        if let (Some(i), Screen::OptionsPage(p)) = (self.slider_drag, self.screen) {
            self.tv_set_slider(p, i, pos.0, cv);
        }
        None
    }

    /// Keys on a touch screen: taps (`K_MOUSE1` at `self.mouse`) and Back
    /// with a list or the search open. False lets the classic handling run.
    pub(super) fn tv_key(&mut self, key: i32, cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) -> bool {
        self.tv_sync();
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);
        if self.tv.sheet.is_some() {
            if key == K_MOUSE1 {
                self.tv_sheet_click(cv, out);
            } else if back {
                self.tv.sheet = None;
                out.push(Action::Sound("misc/menu3.wav"));
            }
            return true;
        }
        if back && std::mem::take(&mut self.tv.search_focus) && self.search.query.is_empty() {
            return true;
        }
        // typing comes from the OS keyboard's text events, not its keys
        if self.tv_search_focus() && (0x20..=0x7e).contains(&key) {
            return true;
        }
        let back_tap = key == K_MOUSE1 && self.tv_has_back() && self.mouse.is_some_and(|p| inside(self.tv_back_rect(), p));
        if (back || back_tap) && self.screen == Screen::Help && std::mem::take(&mut self.tv.help_reading) {
            self.tv.scroll = 0.0;
            out.push(Action::Sound("misc/menu3.wav"));
            return true;
        }
        if (back || back_tap) && self.tv.addon_detail.take().is_some() {
            self.tv.scroll = 0.0;
            out.push(Action::Sound("misc/menu3.wav"));
            return true;
        }
        if key != K_MOUSE1 {
            return false;
        }
        let Some(pos) = self.mouse else { return true };
        if back_tap {
            out.push(Action::Sound("misc/menu3.wav"));
            out.extend(self.key(K_ESCAPE, cv, server_active));
            return true;
        }
        match self.screen {
            Screen::Main | Screen::SinglePlayer | Screen::Multiplayer => self.tv_home_click(pos, cv, server_active, out),
            Screen::Options => self.tv_hub_click(pos, cv, out),
            Screen::Load | Screen::Save => self.tv_cards_click(pos, cv, server_active, out),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline | Screen::Demos | Screen::ServerBrowser | Screen::Help => {
                self.tv_list_click(pos, cv, server_active, out);
            }
            Screen::OptionsPage(Page::WeaponPriority) => self.tv_wp_click(pos, cv, out),
            Screen::OptionsPage(Page::Addons | Page::NewGameAddons) => self.tv_addons_click(pos, cv, out),
            Screen::OptionsPage(p) => self.tv_page_click(p, pos, cv, server_active, out),
            _ => {}
        }
        true
    }

    fn tv_has_back(&self) -> bool {
        !matches!(self.screen, Screen::Main | Screen::Quit | Screen::Death)
    }

    /// Back button of the top bar.
    pub(super) fn tv_back_rect(&self) -> Rect {
        let x0 = match self.screen {
            Screen::OptionsPage(p) => self.tv_geom(p).x0,
            _ => self.tv_col().0,
        };
        [x0, 4.0, BACK_W, BAR_H - 8.0]
    }

    /// Draws the touch layout of the current screen, then any classic
    /// dialog over it.
    pub(super) fn tv_draw(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        let base_scale = c.scale;
        c.fade_screen();
        self.tv_layout(c);
        // first, so a page's sheet covers it (the content is clipped below the bar)
        if self.tv_has_back() {
            self.tv_draw_back(c);
        }
        match self.screen {
            Screen::Main | Screen::SinglePlayer | Screen::Multiplayer | Screen::Quit => self.tv_draw_home(c, pic),
            Screen::Options => self.tv_draw_hub(c, pic, cv, realtime),
            Screen::Load | Screen::Save => self.tv_draw_cards(c, pic),
            Screen::Help if self.tv.help_reading && self.tv.screen == Some(Screen::Help) => self.tv_draw_help(c),
            _ if self.tv_list().is_some() => {
                if let Some(v) = self.tv_list() {
                    self.tv_draw_list(c, &v, realtime);
                }
            }
            Screen::OptionsPage(Page::WeaponPriority) => self.tv_draw_wp(c, pic, cv),
            Screen::OptionsPage(Page::Addons | Page::NewGameAddons) => self.tv_draw_addons(c, pic, cv, realtime),
            Screen::OptionsPage(p) => self.tv_draw_page(c, pic, cv, p, realtime),
            _ => {}
        }
        if let Some(o) = self.osk {
            self.classic_layout(c);
            self.draw_osk(c, o);
        }
        self.tv_overlays(c, cv);
        c.scale = base_scale;
    }

    /// Dialogs over any screen in touch mode: the touch dialog, or the
    /// classic ones while the on-screen keyboard or a key capture is up.
    /// Leaves the canvas in whichever layout drew last.
    pub(super) fn tv_overlays(&self, c: &mut Canvas, cv: &Cvars) {
        if self.tv_dialog_open() {
            self.tv_layout(c);
            self.tv_draw_dialog(c, cv);
            return;
        }
        self.classic_layout(c);
        self.draw_prompt(c, cv);
        c.reset_origin();
        if self.autoaim_prompt {
            self.draw_autoaim_prompt(c);
        }
        self.draw_binding_prompt(c);
    }

    fn tv_draw_back(&self, c: &mut Canvas) {
        let r = self.tv_back_rect();
        let pressed = self.touch_press.is_some_and(|p| inside(r, p));
        tv_button(c, r, "Back", if pressed { Look::Pressed } else { Look::Plain });
        c.shape(r[0] + 14.0, r[1] + r[3] / 2.0 - 6.0, 12.0, 12.0, crate::font::Shape::ArrowLeft, GOLD_FG);
    }

    /// Page title centred in the top bar, clear of the Back button.
    pub(super) fn tv_title(&self, c: &mut Canvas, title: &str) {
        let (x0, cw) = self.tv_col();
        let room = cw - 2.0 * (BACK_W + 12.0);
        let mut size = 22.0;
        let w = c.text_w(title, size);
        if w > room {
            size *= (room / w).max(0.6);
        }
        let tw = c.text_w(title, size);
        c.text(x0 + (cw - tw) / 2.0, (BAR_H - size) / 2.0, title, size, GOLD_FG);
    }

    /// Whether the finger currently down is over `r` (pressed look).
    pub(super) fn tv_pressed(&self, r: Rect) -> bool {
        self.touch_press.is_some_and(|p| inside(r, p))
    }
}

/// How a touch button is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    Plain,
    /// Gold label (main actions).
    Primary,
    Pressed,
    Dim,
}

/// A bordered button with its label centred (shrunk to fit).
pub(super) fn tv_button(c: &mut Canvas, [x, y, w, h]: Rect, label: &str, look: Look) {
    let bg = match look {
        Look::Pressed => PRESS,
        Look::Primary => [0.16, 0.10, 0.05, 0.95],
        _ => [0.0, 0.0, 0.0, 0.72],
    };
    let edge = if look == Look::Dim { [BORDER[0], BORDER[1], BORDER[2], 0.45] } else { BORDER };
    c.round_box(x, y, w, h, 6.0, bg, Some(edge));
    let fg = match look {
        Look::Dim => MUTED,
        Look::Plain => FG,
        _ => GOLD_FG,
    };
    let size = if h >= 56.0 { 19.0 } else { LABEL };
    fit_center(c, x + w / 2.0, y + (h - size) / 2.0, label, size, w - 16.0, fg);
}

/// Translated text centred on `cx`, shrunk to `max_w` if needed.
pub(super) fn fit_center(c: &mut Canvas, cx: f32, y: f32, s: &str, size: f32, max_w: f32, color: [f32; 4]) {
    let w = c.text_w(s, size);
    let size2 = if w > max_w { size * (max_w / w).max(0.55) } else { size };
    let w2 = c.text_w(s, size2);
    c.text(cx - w2 / 2.0, y + (size - size2) / 2.0, s, size2, color);
}

/// Translated text from `x`, shrunk to `max_w` if needed; returns its width.
pub(super) fn fit_left(c: &mut Canvas, x: f32, y: f32, s: &str, size: f32, max_w: f32, color: [f32; 4]) -> f32 {
    let w = c.text_w(s, size);
    let size2 = if w > max_w { size * (max_w / w).max(0.55) } else { size };
    c.text(x, y + (size - size2) / 2.0, s, size2, color)
}

/// Untranslated text from `x`, cut with "…" to `max_w`.
pub(super) fn clip_raw(c: &mut Canvas, x: f32, y: f32, s: &str, size: f32, max_w: f32, color: [f32; 4]) -> f32 {
    if c.text_w_raw(s, size) <= max_w {
        return c.text_raw(x, y, s, size, color);
    }
    let ell = c.text_w_raw("…", size);
    let chars: Vec<char> = s.chars().collect();
    let mut n = chars.len();
    while n > 0 && c.text_w_raw(&chars[..n].iter().collect::<String>(), size) + ell > max_w {
        n -= 1;
    }
    let cut: String = chars[..n].iter().collect::<String>() + "…";
    c.text_raw(x, y, &cut, size, color)
}
