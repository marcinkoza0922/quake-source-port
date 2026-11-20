//! Undo/redo history and drag-commit logic for the HUD editor.

use qcore::cvar::Cvars;

use super::editor::{CVAR, Drag, Editor, Out, PRESET, layout, save, sound};
use super::place;
use super::presets::PRESET_NAME;

/// Maximum number of undo steps kept in memory.
const MAX_UNDO: usize = 64;

/// A saved editor state that can be restored by undo or redo.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Snapshot {
    pub layout: String,
    pub preset: String,
    pub preset_name: String,
    pub editing_preset: Option<String>,
    pub sel: Option<usize>,
}

impl Editor {
    /// Whether there is at least one action to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub(super) fn snapshot(&self, cv: &Cvars) -> Snapshot {
        Snapshot {
            layout: cv.by_name(CVAR).map_or_else(String::new, |c| c.string.clone()),
            preset: cv.by_name(PRESET).map_or_else(|| "0".into(), |c| c.string.clone()),
            preset_name: cv.by_name(PRESET_NAME).map_or_else(String::new, |c| c.string.clone()),
            editing_preset: self.editing_preset.clone(),
            sel: self.sel,
        }
    }

    pub(super) fn push_undo(&mut self, snap: Snapshot) {
        if self.undo_stack.last() == Some(&snap) {
            return;
        }
        if self.undo_stack.len() >= MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(snap);
        self.redo_stack.clear();
    }

    pub(super) fn push_undo_now(&mut self, cv: &Cvars) {
        let snap = self.snapshot(cv);
        self.push_undo(snap);
    }

    fn apply_snapshot(&mut self, snap: Snapshot, cv: &mut Cvars) {
        cv.set(CVAR, &snap.layout);
        cv.set(PRESET, &snap.preset);
        cv.set(PRESET_NAME, &snap.preset_name);
        self.editing_preset = snap.editing_preset;
        self.popup = None;
        self.typing = None;
        self.confirm_reset = false;
        self.drag = None;
        self.snap_guides.0.clear();
        self.snap_guides.1.clear();
        let l = layout(cv);
        if let Some(i) = snap.sel.filter(|&i| i < l.elems.len()) {
            self.select(i, &l);
        } else {
            self.sel = None;
            self.cursor = 0;
        }
    }

    pub(super) fn undo(&mut self, cv: &mut Cvars) -> Out {
        let Some(prev) = self.undo_stack.pop() else {
            return sound("misc/menu3.wav");
        };
        let cur = self.snapshot(cv);
        self.redo_stack.push(cur);
        self.apply_snapshot(prev, cv);
        sound("misc/menu2.wav")
    }

    pub(super) fn redo(&mut self, cv: &mut Cvars) -> Out {
        let Some(next) = self.redo_stack.pop() else {
            return sound("misc/menu3.wav");
        };
        let cur = self.snapshot(cv);
        self.undo_stack.push(cur);
        self.apply_snapshot(next, cv);
        sound("misc/menu2.wav")
    }

    /// Commits a finished mouse or controller element drag: reflows displaced
    /// siblings into `hud_layout` and records one undo entry if anything moved.
    pub(super) fn commit_drag(&mut self, drag: Drag, cv: &mut Cvars) {
        self.snap_guides.0.clear();
        self.snap_guides.1.clear();
        let (elem, origin, before) = match drag {
            Drag::Elem { elem, moved: true, origin, before, .. } | Drag::Pad { elem, origin, before, .. } => (elem, origin, before),
            Drag::Elem { moved: false, .. } | Drag::Dialog(_) => return,
        };
        let mut l = layout(cv);
        if elem < l.elems.len() && self.rects.len() == l.elems.len() && self.geo.vw > 0.0 && self.geo.vh > 0.0 {
            let sizes: Vec<(f32, f32)> = self.rects.iter().map(|r| (r.w, r.h)).collect();
            let final_rects = place::place_drag(&l.elems, &sizes, self.geo.vw, self.geo.vh, elem, origin);
            let mut changed = false;
            for (e, r) in l.elems.iter_mut().zip(&final_rects) {
                let (nx, ny) = place::offset_for(e, *r, self.geo.vw, self.geo.vh);
                if (nx, ny) != (e.x, e.y) {
                    (e.x, e.y) = (nx, ny);
                    changed = true;
                }
            }
            if changed {
                save(cv, &l);
            }
            self.rects = final_rects;
        }
        let after = cv.by_name(CVAR).map_or("", |c| c.string.as_str());
        if after != before.layout {
            self.push_undo(before);
        }
    }
}
