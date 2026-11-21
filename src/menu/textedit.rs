//! Editing of options text rows (`Item::Text`: player name, server
//! address, demo names, video paths): a caret and selection over the
//! cvar's value, moved by arrows/Home/End (Shift extends), placed by a
//! click and stretched by dragging; typing and pasting replace the
//! selection. The host handles Ctrl+A/C/X/V with the system clipboard
//! (`Host::menu_clipboard`).

use super::*;

/// Caret and selection anchor (char indices) of the text row `row`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Edit {
    row: Option<(Page, usize)>,
    caret: usize,
    anchor: usize,
}

impl Edit {
    fn range(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }
}

/// Where a text row's visible characters were drawn: the row, the first
/// visible char and the x of each char boundary from there (for clicks).
#[derive(Clone, Debug, Default)]
pub(super) struct TextLayout {
    row: usize,
    first: usize,
    xs: Vec<f32>,
}

/// Characters a text row takes.
fn allowed(cvar: &str, ch: char) -> bool {
    (' '..='~').contains(&ch) && ch != '"' && ch != ';' && (!demos::file_name_field(cvar) || demos::file_name_char(ch))
}

fn value(cv: &Cvars, cvar: &str) -> Vec<char> {
    cv.by_name(cvar).map_or_else(Vec::new, |c| c.string.chars().collect())
}

impl Menu {
    /// The text row under the cursor: (page, row, cvar, max length).
    fn text_row(&self) -> Option<(Page, usize, &'static str, usize)> {
        let Screen::OptionsPage(p) = self.screen else { return None };
        match self.page_item(p, self.opt_cursor) {
            Item::Text { cvar, max, .. } => Some((p, self.opt_cursor, cvar, max)),
            _ => None,
        }
    }

    /// The edit state of a row: the stored one, or the caret at the end.
    fn edit_of(&self, p: Page, row: usize, len: usize) -> Edit {
        match self.edit {
            e if e.row == Some((p, row)) => Edit { caret: e.caret.min(len), anchor: e.anchor.min(len), ..e },
            _ => Edit { row: Some((p, row)), caret: len, anchor: len },
        }
    }

    /// Runs `f` on the cursor row's value and edit state, then stores both.
    fn edit_text(&mut self, cv: &mut Cvars, f: impl FnOnce(&mut Vec<char>, &mut Edit, &'static str, usize)) -> bool {
        let Some((p, row, cvar, max)) = self.text_row() else { return false };
        let mut v = value(cv, cvar);
        let mut e = self.edit_of(p, row, v.len());
        let old = v.clone();
        f(&mut v, &mut e, cvar, max);
        if v != old {
            cv.set(cvar, &v.iter().collect::<String>());
        }
        self.edit = e;
        true
    }

    /// Replaces the selection with `s` (characters the row refuses are
    /// dropped, the rest cut at the row's length).
    pub fn text_insert(&mut self, s: &str, cv: &mut Cvars) {
        self.edit_text(cv, |v, e, cvar, max| {
            let (a, b) = e.range();
            v.drain(a..b);
            let room = max.saturating_sub(v.len());
            let ins: Vec<char> = s.chars().filter(|&c| allowed(cvar, c)).take(room).collect();
            let n = ins.len();
            v.splice(a..a, ins);
            e.caret = a + n;
            e.anchor = e.caret;
        });
    }

    /// The selected text of the cursor's text row.
    pub fn text_selection(&self, cv: &Cvars) -> Option<String> {
        let (p, row, cvar, _) = self.text_row()?;
        let v = value(cv, cvar);
        let (a, b) = self.edit_of(p, row, v.len()).range();
        (a < b).then(|| v[a..b].iter().collect())
    }

    pub fn text_select_all(&mut self, cv: &mut Cvars) {
        self.edit_text(cv, |v, e, _, _| {
            e.anchor = 0;
            e.caret = v.len();
        });
    }

    /// Backspace (`forward`: Delete): the selection, else one character.
    pub(super) fn text_delete(&mut self, forward: bool, cv: &mut Cvars) {
        self.edit_text(cv, |v, e, _, _| {
            let (mut a, mut b) = e.range();
            if a == b {
                if forward { b = (b + 1).min(v.len()) } else { a = a.saturating_sub(1) }
            }
            v.drain(a..b);
            e.caret = a;
            e.anchor = a;
        });
    }

    /// Caret keys on a text row; true when `key` is one.
    pub(super) fn text_nav(&mut self, key: i32, cv: &mut Cvars) -> bool {
        let shift = self.shift;
        let mv = |v: &Vec<char>, e: &Edit| match key {
            K_LEFTARROW | K_DPAD_LEFT if !shift && e.caret != e.anchor => Some(e.range().0),
            K_RIGHTARROW | K_DPAD_RIGHT if !shift && e.caret != e.anchor => Some(e.range().1),
            K_LEFTARROW | K_DPAD_LEFT => Some(e.caret.saturating_sub(1)),
            K_RIGHTARROW | K_DPAD_RIGHT => Some((e.caret + 1).min(v.len())),
            K_HOME | K_KP_HOME => Some(0),
            K_END | K_KP_END => Some(v.len()),
            _ => None,
        };
        let mut hit = false;
        self.edit_text(cv, |v, e, _, _| {
            if let Some(c) = mv(v, e) {
                hit = true;
                e.caret = c;
                if !shift {
                    e.anchor = c;
                }
            }
        });
        hit
    }

    /// Char boundary of text row `row` nearest to menu x (`None`: the
    /// row was not drawn, or `x` is left of its box).
    fn text_hit(&self, row: usize, x: f32) -> Option<usize> {
        let lay = self.text_layouts.borrow();
        let l = lay.iter().find(|l| l.row == row)?;
        if x < VALUE_X - 3.0 {
            return None;
        }
        let k = l.xs.iter().enumerate().min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs())).map_or(0, |(k, _)| k);
        Some(l.first + k)
    }

    /// Click on text row `row` (already the cursor): places the caret and
    /// starts a drag selection. Shift+click extends the selection.
    pub(super) fn text_click(&mut self, row: usize, x: f32, cv: &mut Cvars) {
        let Some(k) = self.text_hit(row, x) else { return };
        let shift = self.shift;
        self.edit_text(cv, |_, e, _, _| {
            e.caret = k;
            if !shift {
                e.anchor = k;
            }
        });
        self.text_drag = Some(row);
    }

    pub(super) fn text_drag_to(&mut self, row: usize, x: f32, cv: &mut Cvars) {
        if let Some(k) = self.text_hit(row, x) {
            self.edit_text(cv, |_, e, _, _| e.caret = k);
        }
    }

    /// A text row: its value (scrolled to keep the caret in view on the
    /// cursor row), selection, caret and, for video names, the extension.
    pub(super) fn draw_text_row(&self, c: &mut Canvas, cv: &Cvars, p: Page, row: usize, y: f32, blink: bool) {
        let Item::Text { label, cvar, max } = self.page_item(p, row) else { return };
        row_label(c, 0.0, y, label, FG);
        let v = value(cv, cvar);
        // fits inside the panel: 20 visible characters, or for long fields
        // (paths) the panel's width
        let w = if max > 40 { PANEL_X + PANEL_W - VALUE_X - 4.0 } else { (max.min(20) as f32 * 7.0 + 14.0).min(PANEL_X + PANEL_W - VALUE_X - 4.0) };
        c.fill(VALUE_X - 3.0, y - 1.0, w, BOX_H, [0.0, 0.0, 0.0, 0.5]);
        // the export file name's extension follows the video format
        let ext = (cvar == "_export_name").then(|| crate::capture::Codec::from_cvar(cv.value_of("capture_codec")).extension());
        let avail = w - 6.0 - ext.map_or(0.0, |e| c.text_w_raw(e, TXT) + 3.0);
        let focus = row == self.opt_cursor && self.drop.is_none();
        let e = if focus { self.edit_of(p, row, v.len()) } else { Edit { row: None, caret: v.len(), anchor: v.len() } };
        let width = |a: usize, b: usize| c.text_w_raw(&v[a..b].iter().collect::<String>(), TXT);
        let ell = c.text_w_raw("…", TXT);
        // first visible char: as few cut as keep the caret in view
        let first = (0..=e.caret).find(|&f| width(f, e.caret) + if f > 0 { ell } else { 0.0 } <= avail).unwrap_or(e.caret);
        let x0 = VALUE_X + if first > 0 { ell } else { 0.0 };
        let mut xs = vec![x0];
        for k in first + 1..=v.len() {
            let x = x0 + width(first, k);
            if x > VALUE_X + avail {
                break;
            }
            xs.push(x);
        }
        let last = first + xs.len() - 1;
        let (a, b) = e.range();
        if focus && a < b {
            let (sa, sb) = (a.clamp(first, last), b.clamp(first, last));
            c.fill(xs[sa - first], y - 1.0, xs[sb - first] - xs[sa - first], BOX_H, [0.85, 0.6, 0.25, 0.45]);
        }
        if first > 0 {
            c.text_raw(VALUE_X, y, "…", TXT, MUTED);
        }
        c.text_raw(x0, y, &v[first..last].iter().collect::<String>(), TXT, FG);
        if blink && focus && (first..=last).contains(&e.caret) {
            c.fill(xs[e.caret - first], y, 1.0, 8.0, FG);
        }
        if let Some(ext) = ext {
            c.text_raw(xs[last - first] + 3.0, y, ext, TXT, MUTED);
        }
        let mut lay = self.text_layouts.borrow_mut();
        lay.retain(|l| l.row != row);
        lay.push(TextLayout { row, first, xs });
    }
}

#[cfg(test)]
mod tests;
