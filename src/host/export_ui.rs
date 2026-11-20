//! What the window shows while a demo exports. Export frames are drawn
//! offscreen (`Renderer::set_offscreen`) and only read back, so the window
//! keeps a still of the clip's first frame under a panel: progress, time
//! left, the encoder, and Cancel. The panel is redrawn about 30 times a
//! second of real time. Esc does nothing here; Cancel (click, Enter or A)
//! stops the export and deletes what it wrote.

use super::*;
use crate::demo::playback::format_time;
use crate::menu::{BORDER, GOLD_FG, HILITE, MUTED, TXT};

/// Real-time interval between panel redraws.
const UI_EVERY: f64 = 1.0 / 30.0;
const PANEL: (f32, f32) = (300.0, 92.0);

/// The panel's Cancel button for a canvas of `vw`×`vh`.
pub(super) fn cancel_rect(vw: f32, vh: f32) -> demo_bar::Rect {
    let (x, y) = (((vw - PANEL.0) / 2.0).floor(), ((vh - PANEL.1) / 2.0).floor());
    (x + PANEL.0 / 2.0 - 40.0, y + PANEL.1 - 22.0, 80.0, 15.0)
}

/// Progress of an export: done fraction and seconds left (wall clock).
pub(super) struct Progress {
    pub frac: f64,
    pub at: f64,
    pub len: f64,
    pub left: Option<f64>,
    pub file: String,
    pub encoder: String,
}

impl Host {
    /// Keeps the first exported frame as the panel's background.
    pub(super) fn freeze_export_frame(&mut self, w: u32, h: u32, px: &[u8]) {
        let pic = match self.export_pic {
            Some((id, pw, ph)) if (pw, ph) == (w, h) => self.r.update_pic(id, px).ok().map(|_| id),
            Some((id, ..)) => self.r.resize_pic(id, w, h).and_then(|_| self.r.update_pic(id, px)).ok().map(|_| id),
            None => self.r.register_pic_rgba_linear(w, h, px).ok(),
        };
        self.export_pic = pic.map(|id| (id, w, h));
    }

    /// Draws and presents the panel when it's due (windowed exports only).
    pub(super) fn render_export_ui(&mut self, progress: Option<Progress>, force: bool) {
        if self.window.is_none() {
            return;
        }
        let now = qcore::time::Instant::now();
        if !force && self.export_ui_at.is_some_and(|t| now.duration_since(t).as_secs_f64() < UI_EVERY) {
            return;
        }
        self.export_ui_at = Some(now);
        let (w, h) = self.r.size();
        let mut c = Canvas::new(w, h);
        self.sync_ui_text();
        c.ui = Some(self.ui_text.clone());
        match self.export_pic.filter(|_| self.export_frozen()) {
            Some((pic, ..)) => c.pic_stretch_px([0.0, 0.0, w as f32, h as f32], pic),
            None => c.fill(0.0, 0.0, c.vw(), c.vh(), [0.0, 0.0, 0.0, 1.0]),
        }
        c.fill(0.0, 0.0, c.vw(), c.vh(), [0.0, 0.0, 0.0, 0.45]);
        let (vw, vh) = (c.vw(), c.vh());
        let (x, y) = (((vw - PANEL.0) / 2.0).floor(), ((vh - PANEL.1) / 2.0).floor());
        c.round_box(x, y, PANEL.0, PANEL.1, 4.0, [0.0, 0.0, 0.0, 0.85], Some(BORDER));
        c.text_center(vw / 2.0, y + 6.0, "Exporting video", 11.0, GOLD_FG);
        let (bx, by, bw) = (x + 14.0, y + 38.0, PANEL.0 - 28.0);
        c.fill(bx, by, bw, 6.0, [1.0, 1.0, 1.0, 0.18]);
        match &progress {
            Some(p) => {
                let file = crate::modmenu::clip(&p.file, 52);
                c.text_center(vw / 2.0, y + 22.0, &file, TXT, MUTED);
                c.fill(bx, by, bw * p.frac.clamp(0.0, 1.0) as f32, 6.0, [0.85, 0.65, 0.30, 0.95]);
                let line = format!("{} / {}   {}%", format_time(p.at), format_time(p.len), (p.frac * 100.0).floor() as i32);
                c.text_raw(bx, by + 10.0, &line, 8.0, crate::menu::FG);
                if let Some(left) = p.left {
                    c.text_right(bx + bw, by + 10.0, &qcore::tr!("About {} left", format_time(left)), 8.0, crate::menu::FG);
                }
                if !p.encoder.is_empty() {
                    let ew = c.text_w_raw(&p.encoder, 7.0);
                    c.text_raw(x + PANEL.0 - 8.0 - ew, y + PANEL.1 - 11.0, &p.encoder, 7.0, MUTED);
                }
            }
            None => c.text_center(vw / 2.0, y + 22.0, "Preparing...", TXT, MUTED),
        }
        let r = cancel_rect(vw, vh);
        let hover = self.demo_ui.pos.is_some_and(|p| demo_bar::inside(r, p));
        c.round_box(r.0, r.1, r.2, r.3, 3.0, if hover { HILITE } else { [1.0, 1.0, 1.0, 0.1] }, Some(BORDER));
        let glyphs = self.glyphs();
        glyphs.button_label(&mut c, [r.0, r.1, r.2, r.3], self.pad_ui().then_some(qinput::keys::K_ABUTTON), qcore::t!("Cancel"), TXT, GOLD_FG);
        self.upload_ui_text();
        let Some(mut settings) = self.last_settings else { return };
        // the still already has gamma and filters in it
        settings.gamma = 1.0;
        settings.contrast = 1.0;
        settings.colorblind = 0;
        settings.dither = 0.0;
        let frame = qrender::RenderFrame {
            gun_portals: &[],
            teleport_ripples: &[],
            time: self.cl.time,
            view: None,
            entities: &[],
            particles: &[],
            fte_vertices: &[],
            fte_indices: &[],
            fte_batches: &[],
            dlights: &[],
            lightstyles: &[],
            fog: Default::default(),
            blend: [0.0; 4],
            draw2d: &c.list,
            settings,
            shot_impacts: &[],
            dyn_verts: &[],
            splits: &[],
            player: None,
        };
        self.r.present_next(true);
        if let Err(e) = self.r.render(&frame) {
            self.con.print(&format!("render error: {e:?}\n"));
        }
    }

    /// Input while exporting: Cancel by click, Enter or A; everything else
    /// (Esc included) is swallowed.
    pub(super) fn export_input(&mut self, ie: &InputEvent) {
        let press = match *ie {
            InputEvent::MouseButton { button: 1, down: true } => {
                let r = cancel_rect(self.demo_ui.size.0, self.demo_ui.size.1);
                self.demo_ui.pos.is_some_and(|p| demo_bar::inside(r, p))
            }
            InputEvent::Key { key, down: true } => matches!(key, qinput::keys::K_ENTER | qinput::keys::K_KP_ENTER),
            InputEvent::GamepadButton { button, down: true } => button.key() == qinput::keys::K_ABUTTON,
            _ => false,
        };
        if press {
            self.cancel_export();
        }
    }
}
