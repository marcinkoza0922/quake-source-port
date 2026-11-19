//! Mouse on the demo playback bar. While a demo the player started is
//! playing the cursor is free (demos don't use mouse look): moving it shows
//! the bar, buttons act on click (Rewind and Forward while held), a click on
//! the timeline seeks there and a drag across it marks that segment as the
//! clip (`Playback::marks`).

use super::demo_bar::{Btn, inside, layout};
use super::*;

/// Drags shorter than this (virtual units) are clicks.
const DRAG_MIN: f32 = 3.0;

#[derive(Default)]
pub(super) struct Pointer {
    /// Virtual-unit position and the canvas size it was measured on.
    pub pos: Option<(f32, f32)>,
    pub size: (f32, f32),
    /// Rewind/Forward held with the mouse.
    pub held: Option<Btn>,
    /// Timeline drag: start time, current time, start x.
    pub drag: Option<f64>,
    pub drag_to: Option<f64>,
    drag_x: f32,
    /// Pad triggers past their dead zone (they mark a clip).
    pub triggers: [bool; 2],
    /// The player's Hide/Show (H, View, the Hide button, the strip) and the
    /// `cl_demobar` it was made under: it wins over `cl_demobar` for the
    /// session, so a hidden bar stays hidden when control keys are used,
    /// until the player shows it or picks another Playback Bar setting.
    pub choice: Option<(bool, f32)>,
}

impl Host {
    /// A demo the player started, not exported: the cursor is free.
    pub(super) fn demo_cursor(&self) -> bool {
        self.demo.is_some() && self.demonum < 0 && self.timedemo.is_none() && !self.exporting()
    }

    /// Relative mouse only in game mode without a demo cursor; applied when
    /// it changes (called every frame and on key-destination changes).
    pub(super) fn sync_mouse_grab(&mut self) {
        // the cursor is free over a user demo and on the export screen
        let free = self.demo_cursor() || self.exporting();
        let want = self.key_dest() == KeyDest::Game && !free;
        if self.mouse_relative != Some(want) {
            self.mouse_relative = Some(want);
            if let Some(w) = &self.window {
                let _ = w.set_relative_mouse(want);
            }
        }
        if !free {
            self.demo_ui = Pointer::default();
        }
    }

    /// Timeline time under virtual x, if the index is ready.
    fn track_time(&self, x: f32) -> Option<f64> {
        let d = self.demo.as_ref()?;
        let len = d.play.duration()?;
        let l = layout(self.demo_ui.size.0, self.demo_ui.size.1, d.play.auto);
        Some(((x - l.track.0) / l.track.2).clamp(0.0, 1.0) as f64 * len)
    }

    pub(super) fn demo_pointer_move(&mut self, pos: (f32, f32), size: (f32, f32)) {
        self.demo_ui.pos = Some(pos);
        self.demo_ui.size = size;
        if self.demo_ui.drag.is_some() {
            self.demo_ui.drag_to = self.track_time(pos.0);
        }
    }

    /// Mouse buttons while the demo cursor is free; never reach the binds.
    pub(super) fn demo_pointer_button(&mut self, button: u8, down: bool) {
        if button != 1 {
            return;
        }
        if !down {
            if let Some(b) = self.demo_ui.held.take()
                && let Some(d) = &mut self.demo
                && matches!(b, Btn::Rewind | Btn::Forward)
            {
                d.play.scrub = 0;
            }
            if let Some(start) = self.demo_ui.drag.take() {
                let end = self.demo_ui.drag_to.take().unwrap_or(start);
                let dragged = self.demo_ui.pos.is_some_and(|p| (p.0 - self.demo_ui.drag_x).abs() >= DRAG_MIN);
                if dragged {
                    if let Some(d) = &mut self.demo {
                        d.play.marks = [Some(start.min(end)), Some(start.max(end))];
                    }
                } else {
                    self.demo_seek(start);
                }
            }
            return;
        }
        let (Some(pos), Some(d)) = (self.demo_ui.pos, &self.demo) else { return };
        // the hidden bar's strip: a click there brings the bar back
        if self.demo_peek().is_some() {
            self.toggle_demo_bar();
            return;
        }
        let l = layout(self.demo_ui.size.0, self.demo_ui.size.1, d.play.auto);
        if self.demo_bar_visible() && inside(l.hit, pos) {
            self.demo_ui.drag = self.track_time(pos.0);
            self.demo_ui.drag_to = self.demo_ui.drag;
            self.demo_ui.drag_x = pos.0;
            return;
        }
        if !self.demo_bar_visible() {
            return;
        }
        if let Some(&(b, _)) = l.buttons.iter().find(|(_, r)| inside(*r, pos)) {
            self.demo_button(b);
        }
    }

    /// One bar button (H and the Hide button share `toggle_demo_bar`).
    pub(super) fn demo_button(&mut self, b: Btn) {
        let Some(d) = &mut self.demo else { return };
        match b {
            Btn::Start => self.demo_seek(0.0),
            Btn::Back10 => self.demo_jump(-10.0),
            Btn::Fwd10 => self.demo_jump(10.0),
            Btn::NextMap => self.demo_next_map(),
            Btn::Rewind | Btn::Forward => {
                d.play.scrub = if b == Btn::Rewind { -1 } else { 1 };
                self.demo_ui.held = Some(b);
            }
            Btn::PlayPause => {
                d.paused = !d.paused;
                d.play.scrub = 0;
            }
            Btn::Slower => d.play.slower(&mut d.paused),
            Btn::Faster => d.play.faster(&mut d.paused),
            Btn::MarkIn => self.demo_mark(0),
            Btn::MarkOut => self.demo_mark(1),
            Btn::ClearMarks => d.play.marks = [None; 2],
            Btn::Export => self.cbuf.add_text("menu_export\n"),
            Btn::Keep => self.cbuf.add_text("demo_keep\n"),
            Btn::Hide => self.toggle_demo_bar(),
        }
    }

    /// H / Hide / the strip: shown (2) or hidden (1), at once.
    pub(super) fn toggle_demo_bar(&mut self) {
        let show = !self.demo_bar_visible();
        self.demo_ui.choice = Some((show, self.cv("cl_demobar")));
        if let Some(d) = &mut self.demo {
            d.play.shown_until = 0.0;
        }
    }
}
