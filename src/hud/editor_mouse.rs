//! Pointer hit-testing, clicking, and dragging in the HUD editor.

use qcore::cvar::Cvars;

use super::editor::{BUTTONS, Drag, Editor, LIST_ROWS, Out, ROW, Row, TITLE_H, W, button_row_col, layout, save, sound};
use super::editor_rows;
use super::layout::Layout;
use super::place::{self, Rect};

/// Snap radius in HUD units when dragging with the mouse (disabled while Shift is held).
const SNAP_THRESHOLD: f32 = 3.0;
/// Pointer travel in HUD units before a click on an element becomes an active drag.
const DRAG_THRESHOLD: f32 = 2.0;

enum Hit {
    Toggle,
    Undo,
    Preview,
    Title,
    List(usize),
    Button(usize),
    Prop(usize, f32),
    Inside,
    Outside,
}

impl Editor {
    fn hit(&self, (mx, my): (f32, f32), l: &Layout) -> Hit {
        let nprops = self.sel.and_then(|i| l.elems.get(i)).map_or(0, |e| editor_rows::props(e.kind).len());
        let d = self.dims(nprops, self.sel.is_some());
        if !(d.x..d.x + W).contains(&mx) || !(d.y..d.y + d.h).contains(&my) {
            return Hit::Outside;
        }
        if my < d.y + TITLE_H {
            if Self::toggle_rect(&d).contains((mx, my)) {
                return Hit::Toggle;
            }
            if !self.pad_ui && Self::undo_rect(&d).contains((mx, my)) {
                return Hit::Undo;
            }
            if !self.pad_ui && Self::preview_rect(&d).contains((mx, my)) {
                return Hit::Preview;
            }
            return Hit::Title;
        }
        if self.collapsed {
            return Hit::Outside;
        }
        if (d.list_y..d.list_y + LIST_ROWS as f32 * ROW).contains(&my) {
            let i = self.scroll + ((my - d.list_y) / ROW) as usize;
            return if i < l.elems.len() { Hit::List(i) } else { Hit::Inside };
        }
        if let Some(b) = (0..BUTTONS.len()).find(|&b| Self::button_rect(&d, b).contains((mx, my))) {
            return Hit::Button(b);
        }
        if self.sel.is_some() && my >= d.props_y {
            let j = ((my - d.props_y) / ROW) as usize;
            if j < nprops {
                return Hit::Prop(j, mx - d.x);
            }
        }
        Hit::Inside
    }

    fn to_hud(&self, (mx, my): (f32, f32)) -> (f32, f32) {
        let k = self.geo.menu / self.geo.hud.max(1e-3);
        (mx * k, my * k)
    }

    pub(super) fn click(&mut self, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(m) = self.mouse else { return Out::default() };
        match self.hit(m, l) {
            Hit::Toggle => self.toggle_collapse(),
            Hit::Undo => self.undo(cv),
            Hit::Preview => {
                self.preview = self.preview.next();
                sound("misc/menu1.wav")
            }
            Hit::Title => {
                let (x, y) = self.pos.unwrap_or_default();
                self.drag = Some(Drag::Dialog((m.0 - x, m.1 - y)));
                Out::default()
            }
            Hit::List(i) => {
                self.select(i, l);
                sound("misc/menu1.wav")
            }
            Hit::Button(b) => {
                let (row, col) = button_row_col(b);
                self.cursor_to(Row::Buttons(row), l);
                self.button = col;
                if b != 5 {
                    self.confirm_reset = false;
                }
                self.press(b, l, cv)
            }
            Hit::Prop(j, x) => {
                let Some(p) = self.sel.and_then(|i| l.elems.get(i)).and_then(|e| editor_rows::props(e.kind).get(j).copied()) else {
                    return Out::default();
                };
                self.cursor_to(Row::Prop(p), l);
                // the value's left half steps back
                let dir = if (W - 92.0..W - 50.0).contains(&x) { -1 } else { 1 };
                self.activate(p, dir, l, cv)
            }
            Hit::Inside => Out::default(),
            Hit::Outside => {
                let (hx, hy) = self.to_hud(m);
                let Some(i) = (0..self.rects.len().min(l.elems.len())).rev().find(|&i| self.rects[i].contains((hx, hy))) else {
                    if self.sel.is_some() {
                        self.sel = None;
                        return sound("misc/menu1.wav");
                    }
                    return Out::default();
                };
                let r = self.rects[i];
                let before = self.snapshot(cv);
                self.select(i, l);
                self.drag = Some(Drag::Elem { elem: i, grab: (hx - r.x, hy - r.y), start: (hx, hy), moved: false, origin: r, before });
                sound("misc/menu1.wav")
            }
        }
    }

    /// Pointer at `pos` (menu units, centred origin): drags the dialog or
    /// the grabbed element (with magnetic snap guides and website-style reflow).
    pub fn mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) {
        let m = (pos.0 + self.geo.ox, pos.1 + self.geo.oy);
        self.mouse = Some(m);
        match self.drag.clone() {
            Some(Drag::Dialog((gx, gy))) => {
                let (sw, sh) = (self.geo.sw, self.geo.sh);
                self.pos = Some(((m.0 - gx).clamp(0.0, (sw - W).max(0.0)), (m.1 - gy).clamp(0.0, (sh - TITLE_H).max(0.0))));
            }
            Some(Drag::Elem { elem: i, grab: (gx, gy), start, mut moved, origin, before }) => {
                let (hx, hy) = self.to_hud(m);
                if !moved {
                    if (hx - start.0).hypot(hy - start.1) < DRAG_THRESHOLD {
                        return;
                    }
                    moved = true;
                    self.drag = Some(Drag::Elem { elem: i, grab: (gx, gy), start, moved, origin, before });
                }
                let mut l = layout(cv);
                let (Some(e), Some(r)) = (l.elems.get_mut(i), self.rects.get(i).copied()) else { return };
                let want = Rect { x: hx - gx, y: hy - gy, ..r };
                let others: Vec<Rect> = self.rects.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, &o)| o).collect();
                let threshold = if self.shift { 0.0 } else { SNAP_THRESHOLD };
                let snapped = place::snap_rect(want, &others, self.geo.vw, self.geo.vh, threshold);
                self.snap_guides = (snapped.guides_x, snapped.guides_y);
                let (x, y) = place::offset_for(e, snapped.rect, self.geo.vw, self.geo.vh);
                if (x, y) != (e.x, e.y) {
                    (e.x, e.y) = (x, y);
                    save(cv, &l);
                }
            }
            Some(Drag::Pad { .. }) | None => {}
        }
    }

    /// Finishes a mouse drag, committing reflowed sibling positions and pushing an undo step.
    pub fn mouse_up(&mut self, cv: &mut Cvars) {
        if !self.moving()
            && let Some(drag) = self.drag.take()
        {
            self.commit_drag(drag, cv);
        }
    }

    /// Ends dialog-only drag when called without cvars (e.g. generic menu mouse-up).
    pub fn cancel_dialog_drag(&mut self) {
        if matches!(self.drag, Some(Drag::Dialog(_))) {
            self.drag = None;
        }
    }
}
