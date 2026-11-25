//! Controller support for the HUD editor: moving elements on screen with the
//! left stick or D-pad, moving the editor panel with the right stick, and
//! context-sensitive gamepad hints.

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::editor::{CVAR, Drag, Editor, Out, PRESET, Row, TITLE_H, W, sound};
use super::editor_rows::Prop;
use super::layout::Layout;
use super::place::{self, Rect};

const STICK_DEADZONE: f32 = 0.18;
/// Right-stick panel movement speed in menu units per second.
const PANEL_SPEED: f32 = 260.0;
/// Left-stick element movement speed in HUD units per second.
const ELEM_SPEED: f32 = 220.0;

/// Radial deadzone + gentle curve for smooth precision at light tilt.
fn stick_vec([x, y]: [f32; 2]) -> (f32, f32) {
    let mag = (x * x + y * y).sqrt();
    if mag <= STICK_DEADZONE {
        return (0.0, 0.0);
    }
    let norm = ((mag - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).min(1.0);
    let curved = norm.powf(1.5);
    (x / mag * curved, y / mag * curved)
}

impl Editor {
    /// True while the controller is actively moving an element on screen.
    pub fn moving(&self) -> bool {
        matches!(self.drag, Some(Drag::Pad { .. }))
    }

    pub(super) fn start_move(&mut self, l: &Layout, cv: &Cvars) -> Out {
        let Some(sel) = self.sel else { return Out::default() };
        let Some(e) = l.elems.get(sel) else { return Out::default() };
        self.popup = None;
        self.typing = None;
        self.confirm_reset = false;
        let origin = self.rects.get(sel).copied().unwrap_or_default();
        let want = self.rects.get(sel).map(|r| (r.x, r.y));
        let before = self.snapshot(cv);
        self.drag = Some(Drag::Pad { elem: sel, orig: (e.x, e.y), origin, want, before });
        sound("misc/menu2.wav")
    }

    /// Nudge or smoothly move the element currently in controller Move mode.
    fn move_elem_by(&mut self, dx: f32, dy: f32, l: &mut Layout, cv: &mut Cvars) {
        let Some(Drag::Pad { elem, orig, origin, mut want, before }) = self.drag.clone() else { return };
        let Some(e) = l.elems.get_mut(elem) else {
            self.drag = None;
            return;
        };
        if let Some(r) = self.rects.get(elem).copied() {
            let (wx, wy) = want.unwrap_or((r.x, r.y));
            let nx = (wx + dx).clamp(0.0, (self.geo.vw - r.w).max(0.0));
            let ny = (wy + dy).clamp(0.0, (self.geo.vh - r.h).max(0.0));
            want = Some((nx, ny));
            self.drag = Some(Drag::Pad { elem, orig, origin, want, before });
            let moved = Rect { x: nx, y: ny, ..r };
            let others: Vec<Rect> = self.rects.iter().enumerate().filter(|&(j, _)| j != elem).map(|(_, &o)| o).collect();
            let guides = place::snap_rect(moved, &others, self.geo.vw, self.geo.vh, 0.0);
            self.snap_guides = (guides.guides_x, guides.guides_y);
            let (ox, oy) = place::offset_for(e, moved, self.geo.vw, self.geo.vh);
            if (ox, oy) != (e.x, e.y) {
                (e.x, e.y) = (ox, oy);
                self.save(cv, l);
            }
        } else {
            e.x = (e.x + dx).round();
            e.y = (e.y + dy).round();
            self.save(cv, l);
        }
    }

    /// Keys handled while moving an element with the controller.
    pub(super) fn pad_move_key(&mut self, key: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        let nudge = if self.shift { 8.0 } else { 1.0 };
        match key {
            K_ABUTTON | K_ENTER | K_KP_ENTER => {
                if let Some(drag) = self.drag.take() {
                    self.commit_drag(drag, cv);
                }
                sound("misc/menu2.wav")
            }
            K_BBUTTON | K_ESCAPE => {
                self.snap_guides.0.clear();
                self.snap_guides.1.clear();
                if let Some(Drag::Pad { before, .. }) = self.drag.take() {
                    cv.set(CVAR, &before.layout);
                    cv.set(super::addons::CVAR, &before.addons);
                    cv.set(PRESET, &before.preset);
                    cv.set(super::presets::PRESET_NAME, &before.preset_name);
                }
                sound("misc/menu3.wav")
            }
            K_XBUTTON => {
                if let Some(drag) = self.drag.take() {
                    self.commit_drag(drag, cv);
                }
                let updated = self.layout(cv);
                self.focus_props(&updated)
            }
            K_RSHOULDER => self.toggle_collapse(),
            K_LTRIGGER | K_RTRIGGER => {
                let dir = if key == K_LTRIGGER { -1 } else { 1 };
                self.scale_selected(dir, l, cv)
            }
            K_START => {
                if let Some(drag) = self.drag.take() {
                    self.commit_drag(drag, cv);
                }
                Out { sound: Some("misc/menu2.wav"), exit: true }
            }
            K_DPAD_UP | K_UPARROW | K_KP_UPARROW => {
                self.move_elem_by(0.0, -nudge, l, cv);
                Out::default()
            }
            K_DPAD_DOWN | K_DOWNARROW | K_KP_DOWNARROW => {
                self.move_elem_by(0.0, nudge, l, cv);
                Out::default()
            }
            K_DPAD_LEFT | K_LEFTARROW => {
                self.move_elem_by(-nudge, 0.0, l, cv);
                Out::default()
            }
            K_DPAD_RIGHT | K_RIGHTARROW => {
                self.move_elem_by(nudge, 0.0, l, cv);
                Out::default()
            }
            K_MOUSE1 => {
                if let Some(drag) = self.drag.take() {
                    self.commit_drag(drag, cv);
                }
                let mut updated = self.layout(cv);
                self.click(&mut updated, cv)
            }
            _ => Out::default(),
        }
    }

    /// Per-frame analog stick input while the HUD editor is open.
    /// Right stick moves the editor dialog; left stick moves the grabbed element.
    pub fn pad_stick(&mut self, ls: [f32; 2], rs: [f32; 2], dt: f32, cv: &mut Cvars) {
        if dt <= 0.0 {
            return;
        }
        let (rx, ry) = stick_vec(rs);
        if rx != 0.0 || ry != 0.0 {
            let (x, y) = self.pos.unwrap_or((8.0, 8.0));
            let nx = (x + rx * PANEL_SPEED * dt).clamp(0.0, (self.geo.sw - W).max(0.0));
            let ny = (y + ry * PANEL_SPEED * dt).clamp(0.0, (self.geo.sh - TITLE_H).max(0.0));
            self.pos = Some((nx, ny));
        }
        if self.moving() {
            let (lx, ly) = stick_vec(ls);
            if lx != 0.0 || ly != 0.0 {
                let mut l = self.layout(cv);
                self.move_elem_by(lx * ELEM_SPEED * dt, ly * ELEM_SPEED * dt, &mut l, cv);
            }
        }
    }

    /// Step scale of the selected element (if scalable).
    pub(super) fn scale_selected(&mut self, dir: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(sel) = self.sel else { return Out::default() };
        let Some(e) = l.elems.get(sel) else { return Out::default() };
        if e.kind.is_message() {
            return Out::default();
        }
        self.step(Prop::Scale, dir, l, cv)
    }

    /// Expand the dialog (if collapsed) and focus the first property of the selected element.
    pub(super) fn focus_props(&mut self, l: &Layout) -> Out {
        self.collapsed = false;
        if self.sel.is_none() && !l.elems.is_empty() {
            self.select(0, l);
        }
        if let Some(p) = self.sel.and_then(|i| l.elems.get(i)).and_then(|e| super::editor_rows::props(e.kind).first().copied()) {
            self.cursor_to(Row::Prop(p), l);
            sound("misc/menu1.wav")
        } else {
            Out::default()
        }
    }
}
