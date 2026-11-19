//! The HUD editor's popups: the Add tree of element kinds by category,
//! and the anchor grid behind the position button.

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::editor::{ADD_W, AddRow, CELL, Dims, Editor, Out, Popup, ROW, W, save, sound};
use super::editor_rows;
use super::kind::{CATEGORIES, Kind};
use super::layout::{Anchor, Elem, Layout};

pub(super) fn add_rows(open: &[bool]) -> Vec<AddRow> {
    let mut out = Vec::new();
    for (i, (_, kinds)) in CATEGORIES.iter().enumerate() {
        out.push(AddRow::Cat(i));
        if open.get(i).copied().unwrap_or(false) {
            out.extend(kinds.iter().map(|&k| AddRow::Kind(k)));
        }
    }
    out
}

impl Editor {
    /// Top left of the open popup.
    pub(super) fn popup_at(&self, d: &Dims, nrows: usize) -> (f32, f32) {
        match self.popup {
            Some(Popup::Grid(_)) => {
                let y = d.props_y + 6.0 * ROW;
                (d.x + W - 3.0 * CELL.0 - 14.0, y.min(self.geo.sh - 3.0 * CELL.1 - 10.0))
            }
            _ => {
                let h = (nrows + 1) as f32 * ROW + 6.0;
                let x = if d.x + W + 4.0 + ADD_W <= self.geo.sw { d.x + W + 4.0 } else { (d.x - ADD_W - 4.0).max(0.0) };
                (x, d.btn_y.min(self.geo.sh - h).max(0.0))
            }
        }
    }
    pub(super) fn open_grid(&mut self, l: &Layout) -> Out {
        let a = l.elems.get(self.sel).map_or(Anchor::Center, |e| e.anchor);
        self.popup = Some(Popup::Grid(a as usize));
        sound("misc/menu2.wav")
    }
    /// Adds `k` at the screen centre; placement moves it to free room.
    pub(super) fn add(&mut self, k: Kind, l: &mut Layout, cv: &mut Cvars) -> Out {
        let anchor = if k == Kind::StatusBar { Anchor::BottomCenter } else { Anchor::Center };
        l.elems.push(Elem::new(k, anchor));
        save(cv, l);
        self.popup = None;
        self.select(l.elems.len() - 1, l);
        sound("misc/menu2.wav")
    }

    pub(super) fn set_anchor(&mut self, cell: usize, l: &mut Layout, cv: &mut Cvars) -> Out {
        if let Some(e) = l.elems.get_mut(self.sel) {
            let fresh = Elem::new(e.kind, Anchor::ALL[cell.min(8)]);
            (e.anchor, e.x, e.y) = (fresh.anchor, fresh.x, fresh.y);
            save(cv, l);
        }
        self.popup = None;
        sound("misc/menu2.wav")
    }

    pub(super) fn popup_key(&mut self, key: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        if key == K_MOUSE1 {
            return self.popup_click(l, cv);
        }
        let back = matches!(key, K_ESCAPE | K_BBUTTON);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON);
        let (dx, dy) = match key {
            K_UPARROW | K_DPAD_UP | K_KP_UPARROW | K_MWHEELUP => (0, -1),
            K_DOWNARROW | K_DPAD_DOWN | K_KP_DOWNARROW | K_MWHEELDOWN => (0, 1),
            K_LEFTARROW | K_DPAD_LEFT => (-1, 0),
            K_RIGHTARROW | K_DPAD_RIGHT => (1, 0),
            _ => (0, 0),
        };
        if back {
            self.popup = None;
            return sound("misc/menu3.wav");
        }
        match self.popup.clone() {
            Some(Popup::Grid(c)) => {
                if enter {
                    return self.set_anchor(c, l, cv);
                }
                let (col, row) = ((c % 3) as i32 + dx, (c / 3) as i32 + dy);
                self.popup = Some(Popup::Grid((row.clamp(0, 2) * 3 + col.clamp(0, 2)) as usize));
                sound("misc/menu1.wav")
            }
            Some(Popup::Add { mut open, mut cursor }) => {
                let rows = add_rows(&open);
                match (rows.get(cursor).copied(), enter, dx) {
                    (Some(AddRow::Kind(k)), true, _) | (Some(AddRow::Kind(k)), _, 1) => return self.add(k, l, cv),
                    (Some(AddRow::Cat(i)), true, _) => open[i] = !open[i],
                    (Some(AddRow::Cat(i)), _, 1) => open[i] = true,
                    (Some(AddRow::Cat(i)), _, -1) => open[i] = false,
                    (Some(AddRow::Kind(k)), _, -1) => {
                        // back to its category, closed
                        let ci = CATEGORIES.iter().position(|c| c.1.contains(&k)).unwrap_or(0);
                        open[ci] = false;
                        cursor = add_rows(&open).iter().position(|&r| r == AddRow::Cat(ci)).unwrap_or(0);
                    }
                    _ => {}
                }
                let n = add_rows(&open).len() as i32;
                cursor = (cursor as i32 + dy).rem_euclid(n.max(1)) as usize;
                self.popup = Some(Popup::Add { open, cursor });
                sound("misc/menu1.wav")
            }
            None => Out::default(),
        }
    }

    /// The popup row or grid cell under the pointer.
    pub(super) fn popup_hit(&self, l: &Layout) -> Option<usize> {
        let (mx, my) = self.mouse?;
        let nprops = l.elems.get(self.sel).map_or(0, |e| editor_rows::props(e.kind).len());
        let d = self.dims(nprops);
        match &self.popup {
            Some(Popup::Grid(_)) => {
                let (x, y) = self.popup_at(&d, 0);
                let (cx, cy) = (((mx - x - 4.0) / CELL.0).floor(), ((my - y - 4.0) / CELL.1).floor());
                ((0.0..3.0).contains(&cx) && (0.0..3.0).contains(&cy)).then(|| cy as usize * 3 + cx as usize)
            }
            Some(Popup::Add { open, .. }) => {
                let n = add_rows(open).len();
                let (x, y) = self.popup_at(&d, n);
                let i = ((my - y - 3.0 - ROW) / ROW).floor();
                ((x..x + ADD_W).contains(&mx) && i >= 0.0 && (i as usize) < n).then_some(i as usize)
            }
            None => None,
        }
    }

    pub(super) fn popup_click(&mut self, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(i) = self.popup_hit(l) else {
            self.popup = None;
            return sound("misc/menu3.wav");
        };
        match self.popup.clone() {
            Some(Popup::Grid(_)) => self.set_anchor(i, l, cv),
            Some(Popup::Add { mut open, .. }) => match add_rows(&open)[i] {
                AddRow::Kind(k) => self.add(k, l, cv),
                AddRow::Cat(c) => {
                    open[c] = !open[c];
                    self.popup = Some(Popup::Add { open, cursor: i });
                    sound("misc/menu1.wav")
                }
            },
            None => Out::default(),
        }
    }
}
