//! Touch Add-ons browser: the add-ons as rows with an on/off switch (a tap
//! elsewhere on the row opens its details), then Play when setting up a
//! new game. The details show the looping preview, the description,
//! weapon slot steppers, key bindings and the parameters page.

use super::rows::{CTRL_H, SWITCH};
use super::*;
use crate::menu::addon_browser::{ADDON_COUNT, AddonInfo, addon_info};

const PREVIEW_MAX_H: f32 = 190.0;

/// What a tap on the details hit.
#[derive(Clone, Copy)]
enum Hit {
    Toggle,
    Slot(usize, i32),
    Bind(usize),
    Settings,
}

impl Menu {
    fn tv_addon_page(&self) -> Page {
        if self.screen == Screen::OptionsPage(Page::NewGameAddons) { Page::NewGameAddons } else { Page::Addons }
    }

    fn tv_addon_row(&self, i: usize) -> Rect {
        let (x0, cw) = self.tv_col();
        [x0, BAR_H + 4.0 - self.tv_scroll() + i as f32 * ROW, cw, ROW]
    }

    fn tv_addon_switch(r: Rect) -> Rect {
        [r[0] + r[2] - 16.0 - SWITCH.0 - 8.0, r[1], SWITCH.0 + 24.0, r[3]]
    }

    /// Play (new game only) under the list.
    fn tv_addon_play(&self) -> Option<Rect> {
        (self.tv_addon_page() == Page::NewGameAddons).then(|| {
            let (x0, cw) = self.tv_col();
            let bw = (cw - 32.0).min(360.0);
            [x0 + (cw - bw) / 2.0, BAR_H + 4.0 - self.tv_scroll() + ADDON_COUNT as f32 * ROW + 14.0, bw, ROW]
        })
    }

    pub(super) fn tv_addons_h(&self) -> f32 {
        match self.tv.addon_detail {
            Some(i) => self.tv_addon_detail_h(&addon_info(i)),
            None => 4.0 + ADDON_COUNT as f32 * ROW + if self.tv_addon_page() == Page::NewGameAddons { 14.0 + ROW } else { 0.0 } + 70.0,
        }
    }

    fn tv_switch(c: &mut Canvas, r: Rect, on: bool, off: bool) {
        let (sw, sh) = SWITCH;
        let sx = r[0] + (r[2] - sw) / 2.0;
        let sy = r[1] + (r[3] - sh) / 2.0;
        let track = match (on, off) {
            (true, false) => [0.80, 0.55, 0.22, 1.0],
            (true, true) => [0.42, 0.38, 0.33, 1.0],
            _ => [0.22, 0.20, 0.18, 1.0],
        };
        c.round_box(sx, sy, sw, sh, sh / 2.0, track, Some(if off { [0.3, 0.28, 0.25, 1.0] } else { BORDER }));
        let kx = if on { sx + sw - sh + 3.0 } else { sx + 3.0 };
        c.round_box(kx, sy + 3.0, sh - 6.0, sh - 6.0, (sh - 6.0) / 2.0, if off { MUTED } else { [0.95, 0.88, 0.75, 1.0] }, None);
    }

    pub(super) fn tv_draw_addons(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        if let Some(i) = self.tv.addon_detail.filter(|_| self.tv.screen == Some(self.screen)) {
            return self.tv_draw_addon_detail(c, pic, cv, realtime, i);
        }
        let (_, h) = self.tv_size();
        let page = self.tv_addon_page();
        self.tv_title(c, page_name(page));
        c.clip_rows(Some((BAR_H, h)));
        let locked = self.toys_locked(cv);
        let first = self.tv_addon_row(0);
        c.round_box(first[0], first[1], first[2], ADDON_COUNT as f32 * ROW, 8.0, PANEL_BG, Some(BORDER));
        for i in 0..ADDON_COUNT {
            let r = self.tv_addon_row(i);
            if r[1] > h || r[1] + r[3] < BAR_H {
                continue;
            }
            let a = addon_info(i);
            if i > 0 {
                c.fill(r[0] + 12.0, r[1], r[2] - 24.0, 1.0, LINE);
            }
            let sw = Self::tv_addon_switch(r);
            if self.tv_pressed(r) && !self.tv_pressed(sw) {
                c.fill(r[0] + 1.0, r[1], r[2] - 2.0, r[3], PRESS);
            }
            fit_left(c, r[0] + 20.0, r[1] + (ROW - LABEL) / 2.0, a.name, LABEL, sw[0] - r[0] - 60.0, if locked { MUTED } else { FG });
            c.shape(sw[0] - 22.0, r[1] + ROW / 2.0 - 6.0, 12.0, 12.0, crate::font::Shape::ArrowRight, GOLD_FG);
            Self::tv_switch(c, sw, cv.value_of(a.cvar) != 0.0, locked);
        }
        let mut y = first[1] + ADDON_COUNT as f32 * ROW + 14.0;
        if let Some(r) = self.tv_addon_play() {
            tv_button(c, r, qcore::t!("Play"), if self.tv_pressed(r) { Look::Pressed } else { Look::Primary });
            y = r[1] + r[3] + 14.0;
        }
        match self.hud_conflict_note(cv) {
            Some(s) => self.tv_note(c, y, &s),
            None => self.tv_note(c, y, self.addon_note(cv, page)),
        }
        c.clip_rows(None);
    }

    /// Centred small print wrapped to the column, from `y`.
    fn tv_note(&self, c: &mut Canvas, mut y: f32, s: &str) {
        let (x0, cw) = self.tv_col();
        for line in navigation::wrap_tooltip_lines(c, &qcore::lang::tr(s), cw - 24.0, SMALL) {
            let w = c.text_w_raw(&line, SMALL);
            c.text_raw(x0 + (cw - w) / 2.0, y, &line, SMALL, MUTED);
            y += SMALL + 6.0;
        }
    }

    pub(super) fn tv_addons_click(&mut self, pos: (f32, f32), cv: &mut Cvars, out: &mut Vec<Action>) {
        if pos.1 < BAR_H {
            return;
        }
        if let Some(i) = self.tv.addon_detail {
            return self.tv_addon_detail_click(i, pos, cv, out);
        }
        if self.tv_addon_play().is_some_and(|r| inside(r, pos)) {
            out.push(Action::Sound("misc/menu2.wav"));
            self.launch_play(cv, out);
            return;
        }
        let Some(i) = (0..ADDON_COUNT).find(|&i| inside(self.tv_addon_row(i), pos)) else { return };
        self.addon_ui_select(i);
        if inside(Self::tv_addon_switch(self.tv_addon_row(i)), pos) {
            let a = addon_info(i);
            if self.adjust(toggle(a.name, a.cvar), 1, true, cv, out) {
                out.push(Action::Sound("misc/menu3.wav"));
            }
        } else {
            self.tv.addon_detail = Some(i);
            self.tv.scroll = 0.0;
            out.push(Action::Sound("misc/menu2.wav"));
        }
    }

    /// Details layout: preview, then description lines, then the rows.
    fn tv_addon_detail_layout(&self, a: &AddonInfo) -> (Rect, Vec<String>, Vec<(Rect, Hit)>) {
        let (x0, cw) = self.tv_col();
        let top = BAR_H + 4.0 - self.tv_scroll();
        let ph = (cw * 116.0 / 404.0).min(PREVIEW_MAX_H);
        let pw = ph * 404.0 / 116.0;
        let preview = [x0 + (cw - pw) / 2.0, top, pw, ph];
        let est = |t: &str| t.chars().count() as f32 * VALUE * 0.52;
        let lines = qcore::lang::wrap(&qcore::lang::tr(a.description), cw - 24.0, est);
        let mut y = top + ph + 12.0 + lines.len() as f32 * (VALUE + 6.0) + 10.0;
        let mut rows = vec![([x0, y, cw, ROW], Hit::Toggle)];
        y += ROW;
        for k in 0..a.slots.len() {
            rows.push(([x0, y, cw, ROW], Hit::Slot(k, 0)));
            y += ROW;
        }
        for k in 0..a.binds.len() {
            rows.push(([x0, y, cw, ROW], Hit::Bind(k)));
            y += ROW;
        }
        if a.page.is_some() {
            rows.push(([x0, y + 8.0, cw, ROW], Hit::Settings));
        }
        (preview, lines, rows)
    }

    fn tv_addon_detail_h(&self, a: &AddonInfo) -> f32 {
        let (_, _, rows) = self.tv_addon_detail_layout(a);
        rows.last().map_or(0.0, |(r, _)| r[1] + r[3]) + self.tv_scroll() - BAR_H + 70.0
    }

    /// − and + of a slot row.
    fn tv_slot_steps(r: Rect) -> (Rect, Rect) {
        let plus = [r[0] + r[2] - 16.0 - 44.0, r[1] + (r[3] - CTRL_H) / 2.0, 44.0, CTRL_H];
        let minus = [plus[0] - 52.0 - 44.0, plus[1], 44.0, CTRL_H];
        (minus, plus)
    }

    fn tv_draw_addon_detail(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64, i: usize) {
        let (_, h) = self.tv_size();
        let a = addon_info(i);
        self.tv_title(c, a.name);
        c.clip_rows(Some((BAR_H, h)));
        let (preview, lines, rows) = self.tv_addon_detail_layout(&a);
        crate::menu::addon_preview::draw(c, pic, a.id, realtime, preview);
        let (x0, cw) = self.tv_col();
        let mut y = preview[1] + preview[3] + 12.0;
        c.round_box(x0, y - 6.0, cw, lines.len() as f32 * (VALUE + 6.0) + 10.0, 8.0, PANEL_BG, Some(BORDER));
        for l in &lines {
            c.text_raw(x0 + 12.0, y, l, VALUE, FG);
            y += VALUE + 6.0;
        }
        let locked = self.toys_locked(cv);
        let (first, _) = rows[0];
        let list_h = rows.iter().filter(|(_, hit)| !matches!(hit, Hit::Settings)).count() as f32 * ROW;
        c.round_box(first[0], first[1], first[2], list_h, 8.0, PANEL_BG, Some(BORDER));
        for (k, (r, hit)) in rows.iter().enumerate() {
            let ty = r[1] + (ROW - LABEL) / 2.0;
            if k > 0 && !matches!(hit, Hit::Settings) {
                c.fill(r[0] + 12.0, r[1], r[2] - 24.0, 1.0, LINE);
            }
            match *hit {
                Hit::Toggle => {
                    fit_left(c, r[0] + 20.0, ty, a.name, LABEL, r[2] - 140.0, if locked { MUTED } else { GOLD_FG });
                    Self::tv_switch(c, Self::tv_addon_switch(*r), cv.value_of(a.cvar) != 0.0, locked);
                }
                Hit::Slot(s, _) => {
                    let (name, slot) = a.slots[s];
                    fit_left(c, r[0] + 20.0, ty, &format!("{}  ·  {}", qcore::lang::tr(name), qcore::lang::tr("Weapon Slot")), LABEL, r[2] - 220.0, FG);
                    let (minus, plus) = Self::tv_slot_steps(*r);
                    for (b, sign) in [(minus, false), (plus, true)] {
                        c.round_box(b[0], b[1], b[2], b[3], 6.0, if self.tv_pressed(b) { PRESS } else { [0.0, 0.0, 0.0, 0.6] }, Some(BORDER));
                        let (cx, cy) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
                        c.fill(cx - 8.0, cy - 1.0, 16.0, 2.0, GOLD_FG);
                        if sign {
                            c.fill(cx - 1.0, cy - 8.0, 2.0, 16.0, GOLD_FG);
                        }
                    }
                    let v = cv.value_of(slot).round().clamp(1.0, 10.0) as i32 % 10;
                    let t = v.to_string();
                    let tw = c.text_w_raw(&t, LABEL);
                    c.text_raw(minus[0] + minus[2] + (plus[0] - minus[0] - minus[2] - tw) / 2.0, ty, &t, LABEL, GOLD_FG);
                }
                Hit::Bind(b) => {
                    let (label, cmd) = a.binds[b];
                    fit_left(c, r[0] + 20.0, ty, label, LABEL, r[2] * 0.4, FG);
                    let text = if self.addon_capture == Some(cmd) {
                        qcore::lang::tr("Press key...").into_owned()
                    } else {
                        let keys: Vec<String> = keys_for(&self.bindings, cmd).into_iter().map(keynum_to_string).collect();
                        if keys.is_empty() { "---".into() } else { keys.join(", ") }
                    };
                    let tw = c.text_w_raw(&text, VALUE).min(r[2] * 0.5);
                    clip_raw(c, r[0] + r[2] - 20.0 - tw, r[1] + (ROW - VALUE) / 2.0, &text, VALUE, r[2] * 0.5, GOLD_FG);
                }
                Hit::Settings => {
                    let bw = (r[2] - 32.0).min(360.0);
                    let br = [r[0] + (r[2] - bw) / 2.0, r[1], bw, r[3]];
                    tv_button(c, br, page_name(a.page.unwrap_or(Page::Addons)), if self.tv_pressed(br) { Look::Pressed } else { Look::Primary });
                }
            }
        }
        c.clip_rows(None);
    }

    fn tv_addon_detail_click(&mut self, i: usize, pos: (f32, f32), cv: &mut Cvars, out: &mut Vec<Action>) {
        let a = addon_info(i);
        let (_, _, rows) = self.tv_addon_detail_layout(&a);
        let Some(&(r, hit)) = rows.iter().find(|(r, _)| inside(*r, pos)) else { return };
        let hit = match hit {
            Hit::Slot(s, _) => {
                let (minus, plus) = Self::tv_slot_steps(r);
                Hit::Slot(s, if inside(minus, pos) { -1 } else if inside(plus, pos) { 1 } else { 0 })
            }
            h => h,
        };
        match hit {
            Hit::Toggle => {
                if self.adjust(toggle(a.name, a.cvar), 1, true, cv, out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Hit::Slot(s, dir) if dir != 0 => {
                let slot = a.slots[s].1;
                let v = cv.value_of(slot).round().clamp(1.0, 10.0) as i32;
                set_num(cv, slot, ((v - 1 + dir).rem_euclid(10) + 1) as f32);
                out.push(Action::Sound("misc/menu3.wav"));
            }
            Hit::Slot(..) => {}
            Hit::Bind(b) => {
                // the next key or button pressed is bound (the classic capture)
                self.addon_capture = Some(a.binds[b].1);
                out.push(Action::Sound("misc/menu2.wav"));
            }
            Hit::Settings => {
                if let Some(page) = a.page {
                    let parent = self.tv_addon_page();
                    self.sub_return = Some((parent, page, self.opt_cursor));
                    self.screen = Screen::OptionsPage(page);
                    self.opt_cursor = 0;
                    out.push(Action::Sound("misc/menu2.wav"));
                }
            }
        }
    }
}
