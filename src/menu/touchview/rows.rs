//! Touch options page rows: their kinds, geometry and drawing. A row is
//! the label on the left and its control on the right; on narrow screens
//! the control goes under the label.

use super::*;

/// How a row shows and edits its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Heading,
    Toggle,
    /// Sliders and shader parameters: a track with − and + buttons.
    Slider,
    /// Player colours: a swatch with − and + buttons.
    Stepper,
    /// A few short values, all shown as buttons.
    Segments(usize),
    /// Longer lists open a sheet.
    List,
    /// A value that can't be changed here (borderless resolution).
    Value,
    Text,
    Bind,
    Path,
    Sub,
    Button,
}

/// Height of a control and of − / + buttons.
pub(super) const CTRL_H: f32 = 40.0;
const STEP_W: f32 = 44.0;
const VALUE_W: f32 = 64.0;
pub(super) const SWITCH: (f32, f32) = (56.0, 32.0);
/// The crosshair preview's classic box (`reticle::preview`).
const PREVIEW: Rect = [508.0, 72.0, 128.0, 266.0];
/// Extra content below the rows of the Customize pages (their notes).
const NOTES_H: f32 = 84.0;
/// Player Setup: width of the player preview beside the rows.
const PLAYER_W: f32 = 120.0;

/// Geometry of a page: column, control width and whether controls go
/// under their labels.
#[derive(Clone, Copy, Debug)]
pub(super) struct Geom {
    pub(super) x0: f32,
    pub(super) cw: f32,
    pub(super) ctrl_w: f32,
    pub(super) stacked: bool,
    /// Crosshair preview: where it goes and its scale.
    preview: Option<(f32, f32, f32)>,
}

impl Menu {
    pub(super) fn tv_geom(&self, p: Page) -> Geom {
        let (w, h) = self.tv_size();
        let (mut x0, mut cw) = self.tv_col();
        let mut preview = None;
        let k = ((h - BAR_H - 12.0) / PREVIEW[3]).clamp(0.8, 1.4);
        let side = match p {
            Page::Crosshair => Some(PREVIEW[2] * k),
            Page::Setup => Some(PLAYER_W),
            _ => None,
        };
        if let Some(side) = side {
            let room = w - 2.0 * MARGIN - side - 12.0;
            if room >= 420.0 {
                cw = room.min(MAX_W);
                x0 = (w - cw - side - 12.0) / 2.0;
                preview = Some((x0 + cw + 12.0, BAR_H + 4.0, k));
            }
        }
        let stacked = cw < 520.0;
        let ctrl_w = if stacked { cw - 32.0 } else { (cw * 0.5).min(380.0) };
        Geom { x0, cw, ctrl_w, stacked, preview }
    }

    pub(super) fn tv_kind(&self, item: Item, g: &Geom) -> Kind {
        match item {
            Item::Heading(_) => Kind::Heading,
            Item::Toggle { .. } => Kind::Toggle,
            Item::Slider { .. } | Item::ShaderParam(_) => Kind::Slider,
            Item::Color { .. } => Kind::Stepper,
            Item::Text { .. } => Kind::Text,
            Item::Bind { .. } => Kind::Bind,
            Item::UserDataDir => Kind::Path,
            Item::Sub { .. } => Kind::Sub,
            Item::Run { .. } | Item::Apply | Item::ResetBinds | Item::ResetShaderParams => Kind::Button,
            _ => {
                let opts = self.options(item);
                let n = opts.len();
                // all values as buttons when two or three short ones fit
                let longest = opts.iter().map(|o| qcore::lang::tr(o).chars().count()).max().unwrap_or(0);
                if n == 0 {
                    Kind::Value
                } else if (2..=3).contains(&n) && longest as f32 * VALUE * 0.56 + 20.0 <= g.ctrl_w / n as f32 {
                    Kind::Segments(n)
                } else {
                    Kind::List
                }
            }
        }
    }

    fn tv_row_h(&self, kind: Kind, g: &Geom) -> f32 {
        match kind {
            Kind::Heading => HEAD,
            Kind::Sub | Kind::Button | Kind::Toggle => ROW,
            _ if g.stacked => STACK_ROW,
            _ => ROW,
        }
    }

    /// Top (from the first row) and height of every row.
    fn tv_spans(&self, p: Page, g: &Geom) -> Vec<(f32, f32)> {
        let mut top = 0.0;
        (0..self.page_len(p))
            .map(|k| {
                let h = self.tv_row_h(self.tv_kind(self.page_item(p, k), g), g);
                top += h;
                (top - h, h)
            })
            .collect()
    }

    fn tv_rows_h(spans: &[(f32, f32)]) -> f32 {
        spans.last().map_or(0.0, |(t, h)| t + h)
    }

    /// Top (from the first row) and height of row `i`.
    pub(super) fn tv_row_span(&self, p: Page, i: usize) -> (f32, f32) {
        let spans = self.tv_spans(p, &self.tv_geom(p));
        spans.get(i).copied().unwrap_or((Self::tv_rows_h(&spans), 0.0))
    }

    pub(super) fn tv_page_h(&self, p: Page) -> f32 {
        let rows = Self::tv_rows_h(&self.tv_spans(p, &self.tv_geom(p)));
        let notes = if matches!(p, Page::Customize | Page::NewGameCustomize | Page::Modifiers | Page::Export | Page::RenameDemo) { NOTES_H } else { 0.0 };
        4.0 + rows + notes
    }

    fn tv_list_top(&self) -> f32 {
        BAR_H + 4.0 - self.tv_scroll()
    }

    /// Screen rectangle of row `i`.
    pub(super) fn tv_row_rect(&self, p: Page, i: usize) -> Rect {
        let g = self.tv_geom(p);
        let (top, h) = self.tv_row_span(p, i);
        [g.x0, self.tv_list_top() + top, g.cw, h]
    }

    /// Row under a point (headings excluded), within the list area.
    pub(super) fn tv_row_at(&self, p: Page, pos: (f32, f32)) -> Option<usize> {
        let (_, h) = self.tv_size();
        if pos.1 < BAR_H || pos.1 >= h {
            return None;
        }
        let g = self.tv_geom(p);
        let top = self.tv_list_top();
        self.tv_spans(p, &g)
            .iter()
            .position(|&(t, rh)| inside([g.x0, top + t, g.cw, rh], pos))
            .filter(|&i| !matches!(self.page_item(p, i), Item::Heading(_)))
    }

    /// The control of a row (right of the label, or under it).
    pub(super) fn tv_ctrl_rect(&self, g: &Geom, r: Rect, kind: Kind) -> Rect {
        let [x, y, w, h] = r;
        match kind {
            Kind::Button => {
                let bw = (w - 32.0).min(360.0);
                [x + (w - bw) / 2.0, y + 6.0, bw, h - 12.0]
            }
            // a switch fits beside its label even on narrow screens
            Kind::Toggle => [x + w - 16.0 - SWITCH.0, y + (h - CTRL_H) / 2.0, SWITCH.0, CTRL_H],
            _ if g.stacked => [x + 16.0, y + h - CTRL_H - 10.0, w - 32.0, CTRL_H],
            _ => [x + w - 16.0 - g.ctrl_w, y + (h - CTRL_H) / 2.0, g.ctrl_w, CTRL_H],
        }
    }

    /// − and + buttons and the track of a slider or stepper control.
    pub(super) fn tv_steps(ctrl: Rect, kind: Kind) -> (Rect, Rect, Rect) {
        let [x, y, w, h] = ctrl;
        if kind == Kind::Stepper {
            let plus = [x + w - STEP_W, y, STEP_W, h];
            let minus = [plus[0] - 8.0 - STEP_W, y, STEP_W, h];
            return (minus, plus, [x, y, minus[0] - x - 8.0, h]);
        }
        let minus = [x, y, STEP_W, h];
        let plus = [x + w - VALUE_W - STEP_W, y, STEP_W, h];
        let tx = minus[0] + STEP_W + 8.0;
        (minus, plus, [tx, y, plus[0] - 8.0 - tx, h])
    }

    /// Fill (0..1) and value text of a slider row.
    pub(super) fn tv_slider_value(&self, item: Item, cv: &Cvars) -> Option<(f32, String)> {
        match item {
            Item::Slider { cvar, min, max, invert, .. } => {
                let v = cv.value_of(cvar);
                let f = ((v - min) / (max - min)).clamp(0.0, 1.0);
                Some((if invert { 1.0 - f } else { f }, slider_text(cvar, v)))
            }
            Item::ShaderParam(k) => {
                let (p, v) = self.shader_param(k, cv)?;
                let f = if p.max > p.min { ((v - p.min) / (p.max - p.min)).clamp(0.0, 1.0) } else { 0.0 };
                Some((f, qcore::cvar::format_float((v * 1000.0).round() / 1000.0)))
            }
            _ => None,
        }
    }

    pub(super) fn tv_label(&self, item: Item) -> String {
        match item {
            Item::ShaderParam(k) => self
                .shader_params
                .get(k)
                .map(|p| if p.description.trim().is_empty() { p.id.clone() } else { p.description.trim().to_string() })
                .unwrap_or_default(),
            _ => qcore::lang::tr(search::item_label(item).unwrap_or("")).into_owned(),
        }
    }

    /// Current value of a list or segmented row, as shown.
    fn tv_value_text(&self, item: Item, cv: &Cvars) -> String {
        match item {
            Item::Choice { cvar: "cl_autodemo_pb", .. } if cv.value_of("cl_autodemo_pb").round() as i32 == 3 => {
                qcore::lang::tr(qcore::t!("any% and 100%")).into_owned()
            }
            Item::Resolution if self.pending_mode == 2 => qcore::tr!("desktop"),
            Item::Map => {
                let m = self.host_map(cv).unwrap_or_else(|| "(none)".into());
                let t = self.net_maps.iter().find(|x| x.name == m).map_or(String::new(), |x| x.title.clone());
                format!("{m} {t}")
            }
            _ => {
                let opts = self.options(item);
                opts.get(self.option_index(item, cv)).map(|o| qcore::lang::tr(o).into_owned()).unwrap_or_default()
            }
        }
    }

    pub(super) fn tv_draw_page(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, p: Page, realtime: f64) {
        let (_, h) = self.tv_size();
        let g = self.tv_geom(p);
        let n = self.page_len(p);
        self.tv_title(c, page_name(p));
        c.clip_rows(Some((BAR_H, h)));
        let top = self.tv_list_top();
        let spans = self.tv_spans(p, &g);
        let rows_h = Self::tv_rows_h(&spans);
        c.round_box(g.x0, top, g.cw, rows_h, 8.0, PANEL_BG, Some(BORDER));
        let mut prev_heading = true;
        for (i, &(t, rh)) in spans.iter().enumerate().take(n) {
            let r = [g.x0, top + t, g.cw, rh];
            let item = self.page_item(p, i);
            let heading = matches!(item, Item::Heading(_));
            if r[1] < h && r[1] + r[3] > BAR_H {
                if !prev_heading && !heading {
                    c.fill(r[0] + 12.0, r[1], r[2] - 24.0, 1.0, LINE);
                }
                self.tv_draw_row(c, cv, p, i, &g, r, realtime);
            }
            prev_heading = heading;
        }
        // notes under the Customize pages and the demo dialogs
        let mut y = top + rows_h + 14.0;
        let dialog = matches!(p, Page::Export | Page::RenameDemo).then(|| self.dialog_info.iter().map(|l| (l.clone(), MUTED)));
        for (s, col) in self.custom_notes(cv, p).into_iter().chain(dialog.into_iter().flatten()) {
            for line in navigation::wrap_tooltip_lines(c, &s, g.cw - 24.0, SMALL) {
                let w = c.text_w_raw(&line, SMALL);
                c.text_raw(g.x0 + (g.cw - w) / 2.0, y, &line, SMALL, col);
                y += SMALL + 6.0;
            }
        }
        c.clip_rows(None);
        match (g.preview, p) {
            (Some((x, y, k)), Page::Crosshair) => self.tv_draw_preview(c, cv, x, y, k),
            (Some((x, y, _)), Page::Setup) => tv_draw_player(c, pic, cv, x, y),
            _ => {}
        }
        self.tv_draw_footer(c, cv, p, &g);
        if let Some(s) = self.tv_sheet() {
            self.tv_draw_sheet(c, cv, p, s);
        }
    }

    /// The classic crosshair preview, scaled into the touch layout.
    fn tv_draw_preview(&self, c: &mut Canvas, cv: &Cvars, x: f32, y: f32, k: f32) {
        let (scale, ox, oy) = (c.scale, c.ox, c.oy);
        c.scale = scale * k;
        c.ox = (x + ox) / k - PREVIEW[0];
        c.oy = (y + oy) / k - PREVIEW[1];
        crate::reticle::preview(c, cv);
        (c.scale, c.ox, c.oy) = (scale, ox, oy);
    }

    #[allow(clippy::too_many_arguments)]
    fn tv_draw_row(&self, c: &mut Canvas, cv: &Cvars, p: Page, i: usize, g: &Geom, r: Rect, realtime: f64) {
        let item = self.page_item(p, i);
        let kind = self.tv_kind(item, g);
        let [x, y, w, h] = r;
        if let Item::Heading(l) = item {
            let ty = y + h - SMALL - 8.0;
            let tw = fit_left(c, x + 20.0, ty, l, SMALL, w - 40.0, GOLD_FG);
            c.fill(x + 28.0 + tw, ty + SMALL / 2.0, (w - 48.0 - tw).max(0.0), 1.0, [GOLD_FG[0], GOLD_FG[1], GOLD_FG[2], 0.35]);
            return;
        }
        let locked = self.game_locked(item, cv);
        let off = self.item_disabled(item, cv) || locked || matches!(item, Item::Sub { page, .. } if sub_disabled(page, cv));
        let slider_drag = self.slider_drag == Some(i);
        if (self.tv_pressed(r) && kind != Kind::Slider) || slider_drag {
            c.fill(x + 1.0, y, w - 2.0, h, PRESS);
        } else if self.tv_footer() == Some(i) {
            c.fill(x + 1.0, y, w - 2.0, h, SELECT);
        }
        let ctrl = self.tv_ctrl_rect(g, r, kind);
        let dep = self.is_dependent(p, i);
        let indent = if dep { 16.0 } else { 0.0 };
        let lx = x + 20.0 + indent;
        let beside = !g.stacked || matches!(kind, Kind::Sub | Kind::Toggle);
        let room = if kind == Kind::Sub { w - 60.0 - indent } else if beside { ctrl[0] - 12.0 - lx } else { w - 40.0 - indent };
        let ly = if beside { y + (h - LABEL) / 2.0 } else { y + 12.0 };
        if self.changed(item, cv) {
            c.round_box(x + 7.0, ly + LABEL / 2.0 - 3.0, 6.0, 6.0, 3.0, if off { MUTED } else { GOLD_FG }, None);
        }
        let lc = if off { MUTED } else if kind == Kind::Sub { GOLD_FG } else if dep { DEP_FG } else { FG };
        if kind != Kind::Button {
            let label = self.tv_label(item);
            fit_left(c, lx, ly, &label, LABEL, room, lc);
        }
        let vc = if off { MUTED } else { FG };
        let ty = ctrl[1] + (ctrl[3] - VALUE) / 2.0;
        if locked {
            fit_left(c, ctrl[0], ty, "(fixed for this game)", VALUE, ctrl[2], MUTED);
            return;
        }
        match kind {
            Kind::Heading => {}
            Kind::Toggle => {
                let on = matches!(item, Item::Toggle { cvar, .. } if cv.value_of(cvar) != 0.0);
                let (sw, sh) = SWITCH;
                let sx = ctrl[0] + ctrl[2] - sw;
                let sy = ctrl[1] + (ctrl[3] - sh) / 2.0;
                let track = match (on, off) {
                    (true, false) => [0.80, 0.55, 0.22, 1.0],
                    (true, true) => [0.42, 0.38, 0.33, 1.0],
                    _ => [0.22, 0.20, 0.18, 1.0],
                };
                c.round_box(sx, sy, sw, sh, sh / 2.0, track, Some(if off { [0.3, 0.28, 0.25, 1.0] } else { BORDER }));
                let kx = if on { sx + sw - sh + 3.0 } else { sx + 3.0 };
                c.round_box(kx, sy + 3.0, sh - 6.0, sh - 6.0, (sh - 6.0) / 2.0, if off { MUTED } else { [0.95, 0.88, 0.75, 1.0] }, None);
            }
            Kind::Slider => {
                let Some((f, text)) = self.tv_slider_value(item, cv) else { return };
                let (minus, plus, track) = Self::tv_steps(ctrl, kind);
                step_button(c, minus, false, off, self.tv_pressed(minus));
                step_button(c, plus, true, off, self.tv_pressed(plus));
                let (fill, knob) = if off { ([0.42, 0.38, 0.33, 1.0], MUTED) } else { ([0.80, 0.55, 0.22, 1.0], [0.95, 0.82, 0.6, 1.0]) };
                let kr = 12.0;
                let (tx, tw) = (track[0] + kr, track[2] - 2.0 * kr);
                let cy = track[1] + track[3] / 2.0;
                c.round_box(tx, cy - 3.0, tw, 6.0, 3.0, [0.25, 0.22, 0.19, 1.0], None);
                c.round_box(tx, cy - 3.0, (tw * f).max(6.0), 6.0, 3.0, fill, None);
                let kr2 = if slider_drag { kr + 3.0 } else { kr };
                c.round_box(tx + tw * f - kr2, cy - kr2, 2.0 * kr2, 2.0 * kr2, kr2, knob, Some([0.2, 0.12, 0.05, 0.8]));
                let vx = ctrl[0] + ctrl[2] - VALUE_W + 6.0;
                clip_raw(c, vx, ty, &text, VALUE, VALUE_W - 6.0, vc);
            }
            Kind::Stepper => {
                let (minus, plus, _) = Self::tv_steps(ctrl, kind);
                if let Item::Color { high, .. } = item {
                    let v = cv.value_of("_cl_color") as i32;
                    let k = if high { v >> 4 } else { v & 15 }.clamp(0, 13) as usize;
                    let [cr, cg, cb] = crate::draw::PLAYER_COLORS[k].map(|x| x as f32 / 255.0);
                    let sx = minus[0] - 8.0 - 56.0;
                    c.round_box(sx, ctrl[1] + 4.0, 56.0, ctrl[3] - 8.0, 4.0, [cr, cg, cb, 1.0], Some(BORDER));
                    c.text_raw(sx - 12.0 - c.text_w_raw(&k.to_string(), VALUE), ty, &k.to_string(), VALUE, vc);
                }
                step_button(c, minus, false, off, self.tv_pressed(minus));
                step_button(c, plus, true, off, self.tv_pressed(plus));
            }
            Kind::Segments(n) => {
                let cur = self.option_index(item, cv);
                let opts = self.options(item);
                let sw = ctrl[2] / n as f32;
                c.round_box(ctrl[0], ctrl[1], ctrl[2], ctrl[3], 6.0, [0.0, 0.0, 0.0, 0.6], Some(if off { MUTED } else { BORDER }));
                for (k, o) in opts.iter().enumerate().take(n) {
                    let sr = [ctrl[0] + k as f32 * sw, ctrl[1], sw, ctrl[3]];
                    if k == cur {
                        let fill = if off { [0.35, 0.32, 0.28, 0.9] } else { [0.62, 0.40, 0.14, 0.95] };
                        c.round_box(sr[0] + 3.0, sr[1] + 3.0, sr[2] - 6.0, sr[3] - 6.0, 4.0, fill, None);
                    } else if self.tv_pressed(sr) {
                        c.round_box(sr[0] + 3.0, sr[1] + 3.0, sr[2] - 6.0, sr[3] - 6.0, 4.0, PRESS, None);
                    }
                    let col = if off { MUTED } else if k == cur { [1.0, 0.95, 0.85, 1.0] } else { FG };
                    fit_center(c, sr[0] + sw / 2.0, ty, o, VALUE, sw - 10.0, col);
                }
            }
            Kind::List => {
                c.round_box(ctrl[0], ctrl[1], ctrl[2], ctrl[3], 6.0, [0.0, 0.0, 0.0, 0.6], Some(if off { MUTED } else { BORDER }));
                let text = self.tv_value_text(item, cv);
                clip_raw(c, ctrl[0] + 12.0, ty, &text, VALUE, ctrl[2] - 44.0, vc);
                c.shape(ctrl[0] + ctrl[2] - 26.0, ctrl[1] + ctrl[3] / 2.0 - 5.0, 12.0, 10.0, crate::font::Shape::ArrowDown, if off { MUTED } else { GOLD_FG });
            }
            Kind::Value => {
                let text = self.tv_value_text(item, cv);
                clip_raw(c, ctrl[0] + 12.0, ty, &text, VALUE, ctrl[2] - 12.0, MUTED);
            }
            Kind::Text => {
                let Item::Text { cvar, .. } = item else { return };
                let focus = i == self.opt_cursor && self.wants_text();
                c.round_box(ctrl[0], ctrl[1], ctrl[2], ctrl[3], 6.0, [0.0, 0.0, 0.0, 0.7], Some(if focus { GOLD_FG } else { BORDER }));
                let v = cv.by_name(cvar).map_or(String::new(), |c| c.string.clone());
                // keep the end (where typing goes) in view
                let room = ctrl[2] - 24.0;
                let mut shown: String = v.clone();
                while c.text_w_raw(&shown, VALUE) > room && !shown.is_empty() {
                    shown.remove(0);
                }
                let tw = c.text_raw(ctrl[0] + 12.0, ty, &shown, VALUE, vc);
                if focus && (realtime * 2.0) as i64 & 1 == 0 {
                    c.fill(ctrl[0] + 13.0 + tw, ty, 1.5, VALUE, FG);
                }
            }
            Kind::Bind => {
                let Item::Bind { cmd, .. } = item else { return };
                let keys: Vec<String> = keys_for(&self.bindings, cmd).into_iter().map(keynum_to_string).collect();
                let text = if keys.is_empty() { "---".to_string() } else { keys.join(", ") };
                let tw = c.text_w_raw(&text, VALUE).min(ctrl[2]);
                clip_raw(c, ctrl[0] + ctrl[2] - tw, ty, &text, VALUE, ctrl[2], if keys.is_empty() { MUTED } else { vc });
            }
            Kind::Path => {
                let path = crate::userdata::parent().to_string_lossy().into_owned();
                let shown = dir_browser::fit_path_tail(c, &path, ctrl[2] - 16.0, VALUE);
                c.text_raw(ctrl[0] + 8.0, ty, &shown, VALUE, if cfg!(target_arch = "wasm32") { MUTED } else { GOLD_FG });
            }
            Kind::Sub => {
                if off {
                    let ow = c.text_w("(off)", VALUE);
                    c.text(x + w - 44.0 - ow, y + (h - VALUE) / 2.0, "(off)", VALUE, MUTED);
                }
                c.shape(x + w - 30.0, y + h / 2.0 - 6.0, 12.0, 12.0, crate::font::Shape::ArrowRight, lc);
            }
            Kind::Button => {
                let live = match item {
                    Item::Apply => self.display_dirty(cv),
                    Item::ResetShaderParams => cv.by_name("r_shader_params").is_some_and(|c| !c.string.trim().is_empty()),
                    _ => !off,
                };
                let label = match item {
                    Item::Run { label, .. } => label,
                    Item::Apply => "Apply Display Changes",
                    Item::ResetBinds => "Reset to Defaults",
                    Item::ResetShaderParams => "Reset Shader Settings",
                    _ => "",
                };
                let look = if !live { Look::Dim } else if self.tv_pressed(ctrl) { Look::Pressed } else { Look::Primary };
                tv_button(c, ctrl, label, look);
            }
        }
    }
}

/// Player Setup: the player in the chosen colours in the classic box
/// (M_Setup_Draw), scaled to the side column.
fn tv_draw_player(c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, x: f32, y: f32) {
    let v = cv.value_of("_cl_color") as i32;
    let Some(bx @ (_, bw, bh)) = pic("gfx/bigbox.lmp") else { return };
    let k = PLAYER_W / bw as f32;
    let (ox, oy, s) = (c.ox, c.oy, c.scale);
    let px = |x: f32, y: f32| ((x + ox) * s, (y + oy) * s);
    let (sx, sy) = px(x, y);
    c.pic_stretch_px([sx, sy, bw as f32 * k * c.scale, bh as f32 * k * c.scale], bx.0);
    if let Some(p @ (_, pw, ph)) = pic(&format!("gfx/menuplyr.lmp#{},{}", (v >> 4) & 15, v & 15)) {
        let (sx, sy) = px(x + (PLAYER_W - pw as f32 * k) / 2.0, y + 8.0 * k);
        c.pic_stretch_px([sx, sy, pw as f32 * k * c.scale, ph as f32 * k * c.scale], p.0);
    }
}

/// A − or + button.
fn step_button(c: &mut Canvas, [x, y, w, h]: Rect, plus: bool, off: bool, pressed: bool) {
    let bg = if pressed { PRESS } else { [0.0, 0.0, 0.0, 0.6] };
    c.round_box(x, y, w, h, 6.0, bg, Some(if off { MUTED } else { BORDER }));
    let col = if off { MUTED } else { GOLD_FG };
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    c.fill(cx - 8.0, cy - 1.0, 16.0, 2.0, col);
    if plus {
        c.fill(cx - 1.0, cy - 8.0, 2.0, 16.0, col);
    }
}
