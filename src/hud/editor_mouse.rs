//! Pointer hit-testing, clicking, and dragging in the HUD editor.

use qcore::cvar::Cvars;

use super::editor::{Drag, Editor, LIST_ROWS, Out, ROW, Row, TITLE_H, W, layout, save, sound};
use super::editor_rows;
use super::layout::Layout;
use super::place::{self, Rect};

enum Hit {
    Toggle,
    Title,
    List(usize),
    Button(usize),
    Prop(usize, f32),
    Inside,
    Outside,
}

impl Editor {
    fn hit(&self, (mx, my): (f32, f32), l: &Layout) -> Hit {
        let nprops = l.elems.get(self.sel).map_or(0, |e| editor_rows::props(e.kind).len());
        let d = self.dims(nprops);
        if !(d.x..d.x + W).contains(&mx) || !(d.y..d.y + d.h).contains(&my) {
            return Hit::Outside;
        }
        if my < d.y + TITLE_H {
            return if Self::toggle_rect(&d).contains((mx, my)) { Hit::Toggle } else { Hit::Title };
        }
        if self.collapsed {
            return Hit::Outside;
        }
        if (d.list_y..d.list_y + LIST_ROWS as f32 * ROW).contains(&my) {
            let i = self.scroll + ((my - d.list_y) / ROW) as usize;
            return if i < l.elems.len() { Hit::List(i) } else { Hit::Inside };
        }
        if let Some(b) = (0..4).find(|&b| Self::button_rect(&d, b).contains((mx, my))) {
            return Hit::Button(b);
        }
        if my >= d.props_y {
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
                self.cursor_to(Row::Buttons(b / 2), l);
                self.button = b % 2;
                if b != 2 {
                    self.confirm_reset = false;
                }
                self.press(b, l, cv)
            }
            Hit::Prop(j, x) => {
                let Some(p) = l.elems.get(self.sel).and_then(|e| editor_rows::props(e.kind).get(j).copied()) else {
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
                    return Out::default();
                };
                let r = self.rects[i];
                self.select(i, l);
                self.drag = Some(Drag::Elem(i, (hx - r.x, hy - r.y)));
                sound("misc/menu1.wav")
            }
        }
    }

    /// Pointer at `pos` (menu units, centred origin): drags the dialog or
    /// the grabbed element.
    pub fn mouse_move(&mut self, pos: (f32, f32), cv: &mut Cvars) {
        let m = (pos.0 + self.geo.ox, pos.1 + self.geo.oy);
        self.mouse = Some(m);
        match self.drag {
            Some(Drag::Dialog((gx, gy))) => {
                let (sw, sh) = (self.geo.sw, self.geo.sh);
                self.pos = Some(((m.0 - gx).clamp(0.0, (sw - W).max(0.0)), (m.1 - gy).clamp(0.0, (sh - TITLE_H).max(0.0))));
            }
            Some(Drag::Elem(i, (gx, gy))) => {
                let mut l = layout(cv);
                let (Some(e), Some(r)) = (l.elems.get_mut(i), self.rects.get(i).copied()) else { return };
                let (hx, hy) = self.to_hud(m);
                let want = Rect { x: hx - gx, y: hy - gy, ..r };
                let (x, y) = place::offset_for(e, want, self.geo.vw, self.geo.vh);
                if (x, y) != (e.x, e.y) {
                    (e.x, e.y) = (x, y);
                    save(cv, &l);
                }
            }
            Some(Drag::Pad { .. }) | None => {}
        }
    }

    pub fn mouse_up(&mut self) {
        if !self.moving() {
            self.drag = None;
        }
    }
}
