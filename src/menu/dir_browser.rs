//! In-game directory explorer for choosing the User Data Directory
//! (Options > Gameplay > User Data Directory).
//!
//! Displays the active path, a live preview of the `<selected>/quake-source-port`
//! folder that will be created, a scrollable list of subdirectories, action
//! buttons (Select Folder, Default, Show Hidden, Cancel), and a 3-way
//! Copy / Switch Only / Cancel modal when existing user data is present.

use std::path::{Path, PathBuf};

use super::*;

mod draw;
pub(super) use draw::fit_path_tail;

const LIST_X: f32 = 64.0;
const LIST_W: f32 = 512.0;
const PATH_Y: f32 = 38.0;
const PATH_H: f32 = 16.0;
const PREVIEW_Y: f32 = 58.0;
const LIST_Y: f32 = 74.0;
const DIR_ROW_H: f32 = 15.0;
const VIS_ROWS: usize = 13;
const LIST_H: f32 = VIS_ROWS as f32 * DIR_ROW_H + 8.0;
const ERR_Y: f32 = LIST_Y + LIST_H + 4.0;
const BTN_Y: f32 = 296.0;
const BTN_H: f32 = 18.0;

/// Bottom action buttons: (x, width).
const BTNS: [(f32, f32); 4] = [
    (64.0, 136.0),
    (213.0, 100.0),
    (327.0, 136.0),
    (476.0, 100.0),
];

/// Copy-or-switch confirmation modal geometry.
const MODAL: (f32, f32, f32, f32) = (120.0, 112.0, 400.0, 136.0);
const MODAL_BTN_Y: f32 = MODAL.1 + 102.0;
const MODAL_BTN_H: f32 = 18.0;
const MODAL_BTNS: [(f32, f32); 3] = [(140.0, 112.0), (264.0, 112.0), (388.0, 112.0)];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    Parent,
    Dir { name: String, path: PathBuf },
}

/// Result of an input event handled by the directory explorer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DirOutcome {
    None,
    Cancel,
    /// Selected parent directory and whether to copy existing user data.
    Apply { parent: PathBuf, copy: bool },
}

#[derive(Default)]
pub(super) struct DirBrowser {
    current: PathBuf,
    entries: Vec<Entry>,
    cursor: usize,
    scroll: usize,
    focus_buttons: bool,
    btn_cursor: usize,
    show_hidden: bool,
    read_error: bool,
    error: Option<String>,
    /// Copy-or-switch modal: (chosen parent, highlighted button 0..3).
    confirm: Option<(PathBuf, usize)>,
}

impl DirBrowser {
    /// Opens the browser at `start` (falling back to the nearest existing ancestor).
    pub(super) fn open(&mut self, start: PathBuf) {
        let mut dir = start;
        while !dir.is_dir() {
            match dir.parent().filter(|p| !p.as_os_str().is_empty()) {
                Some(p) => dir = p.to_path_buf(),
                None => {
                    dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                    break;
                }
            }
        }
        self.focus_buttons = false;
        self.btn_cursor = 0;
        self.error = None;
        self.confirm = None;
        self.set_dir(dir, None);
    }

    pub(super) fn has_modal(&self) -> bool {
        self.confirm.is_some()
    }

    fn set_dir(&mut self, dir: PathBuf, select_name: Option<&str>) {
        self.current = dir;
        self.error = None;
        self.reload(select_name);
    }

    fn reload(&mut self, select_name: Option<&str>) {
        self.entries.clear();
        self.read_error = false;
        if self.current.parent().is_some() {
            self.entries.push(Entry::Parent);
        }
        let mut subdirs: Vec<(String, PathBuf)> = Vec::new();
        match std::fs::read_dir(&self.current) {
            Ok(rd) => {
                for e in rd.flatten() {
                    let path = e.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else { continue };
                    if !self.show_hidden && name.starts_with('.') {
                        continue;
                    }
                    subdirs.push((name, path));
                }
            }
            Err(_) => self.read_error = true,
        }
        subdirs.sort_by_key(|(n, _)| n.to_ascii_lowercase());
        for (name, path) in subdirs {
            self.entries.push(Entry::Dir { name, path });
        }
        self.cursor = select_name
            .and_then(|want| {
                self.entries.iter().position(|e| matches!(e, Entry::Dir { name, .. } if name == want))
            })
            .unwrap_or(0)
            .min(self.entries.len().saturating_sub(1));
        self.scroll = 0;
        self.fix_scroll();
    }

    fn fix_scroll(&mut self) {
        let n = self.entries.len();
        if n <= VIS_ROWS {
            self.scroll = 0;
            return;
        }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + VIS_ROWS {
            self.scroll = self.cursor + 1 - VIS_ROWS;
        }
        self.scroll = self.scroll.min(n - VIS_ROWS);
    }

    fn go_up(&mut self) -> bool {
        let Some(parent) = self.current.parent().map(Path::to_path_buf) else {
            return false;
        };
        let prev_name = self.current.file_name().and_then(|n| n.to_str()).map(str::to_string);
        if prev_name.as_deref().is_some_and(|n| n.starts_with('.')) {
            self.show_hidden = true;
        }
        self.set_dir(parent, prev_name.as_deref());
        true
    }

    fn activate_entry(&mut self) -> bool {
        match self.entries.get(self.cursor).cloned() {
            Some(Entry::Parent) => self.go_up(),
            Some(Entry::Dir { path, .. }) => {
                self.set_dir(path, None);
                true
            }
            None => false,
        }
    }

    fn select_current(&mut self) -> DirOutcome {
        let chosen = crate::userdata::normalize_parent(&self.current);
        let new_root = crate::userdata::resolve_root(&chosen);
        let old_root = crate::userdata::root();
        if new_root == old_root
            || std::fs::canonicalize(&new_root).ok().zip(std::fs::canonicalize(&old_root).ok()).is_some_and(|(a, b)| a == b)
        {
            return DirOutcome::Cancel;
        }
        if let Err(e) = crate::userdata::check_writable(&new_root) {
            self.error = Some(e);
            return DirOutcome::None;
        }
        if crate::userdata::has_existing_data(&old_root) {
            self.confirm = Some((chosen, 0));
            return DirOutcome::None;
        }
        DirOutcome::Apply { parent: chosen, copy: false }
    }

    fn activate_button(&mut self, idx: usize) -> DirOutcome {
        match idx {
            0 => self.select_current(),
            1 => {
                let def = crate::userdata::default_parent();
                if def.is_dir() {
                    self.set_dir(def, None);
                } else {
                    self.open(def);
                }
                DirOutcome::None
            }
            2 => {
                self.show_hidden = !self.show_hidden;
                let keep = match self.entries.get(self.cursor) {
                    Some(Entry::Dir { name, .. }) => Some(name.clone()),
                    _ => None,
                };
                self.reload(keep.as_deref());
                DirOutcome::None
            }
            _ => DirOutcome::Cancel,
        }
    }

    /// Scrollbar geometry: (x, track top, track h, thumb y, thumb h).
    pub(super) fn scrollbar(&self) -> Option<(f32, f32, f32, f32, f32)> {
        let total = self.entries.len();
        if total <= VIS_ROWS {
            return None;
        }
        let (x, top, h) = (LIST_X + LIST_W - 5.0, LIST_Y + 4.0, VIS_ROWS as f32 * DIR_ROW_H);
        let th = (h * VIS_ROWS as f32 / total as f32).max(10.0);
        let ty = top + (h - th) * self.scroll as f32 / (total - VIS_ROWS) as f32;
        Some((x, top, h, ty, th))
    }

    pub(super) fn drag_to(&mut self, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.scrollbar() else { return };
        let max = self.entries.len().saturating_sub(VIS_ROWS);
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.scroll = (f * max as f32).round() as usize;
        self.cursor = self.cursor.clamp(self.scroll, (self.scroll + VIS_ROWS - 1).min(self.entries.len().saturating_sub(1)));
    }

    pub(super) fn wheel(&mut self, up: bool) {
        if self.confirm.is_some() {
            return;
        }
        let max = self.entries.len().saturating_sub(VIS_ROWS);
        self.scroll = if up { self.scroll.saturating_sub(2) } else { (self.scroll + 2).min(max) };
    }

    fn row_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let top = LIST_Y + 4.0;
        let vis = self.entries.len().min(VIS_ROWS);
        if (LIST_X + 2.0..LIST_X + LIST_W - 10.0).contains(&x) && (top..top + vis as f32 * DIR_ROW_H).contains(&y) {
            Some(self.scroll + ((y - top) / DIR_ROW_H) as usize)
        } else {
            None
        }
    }

    fn btn_at((x, y): (f32, f32)) -> Option<usize> {
        (BTN_Y..BTN_Y + BTN_H)
            .contains(&y)
            .then(|| BTNS.iter().position(|&(bx, bw)| (bx..bx + bw).contains(&x)))
            .flatten()
    }

    fn modal_btn_at((x, y): (f32, f32)) -> Option<usize> {
        (MODAL_BTN_Y..MODAL_BTN_Y + MODAL_BTN_H)
            .contains(&y)
            .then(|| MODAL_BTNS.iter().position(|&(bx, bw)| (bx..bx + bw).contains(&x)))
            .flatten()
    }

    pub(super) fn mouse_move(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        if let Some((_, cur)) = self.confirm.as_mut() {
            let i = Self::modal_btn_at(pos)?;
            return (*cur != i).then(|| {
                *cur = i;
                "misc/menu1.wav"
            });
        }
        if let Some(i) = self.row_at(pos) {
            let changed = self.focus_buttons || self.cursor != i;
            self.focus_buttons = false;
            self.cursor = i;
            return changed.then_some("misc/menu1.wav");
        }
        if let Some(i) = Self::btn_at(pos) {
            let changed = !self.focus_buttons || self.btn_cursor != i;
            self.focus_buttons = true;
            self.btn_cursor = i;
            return changed.then_some("misc/menu1.wav");
        }
        None
    }

    pub(super) fn key(&mut self, key: i32, mouse: Option<(f32, f32)>, out: &mut Vec<Action>) -> DirOutcome {
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON);
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);

        if let Some((chosen, mut cur)) = self.confirm.clone() {
            if left {
                cur = (cur + 2) % 3;
                self.confirm = Some((chosen, cur));
                out.push(Action::Sound("misc/menu1.wav"));
                return DirOutcome::None;
            }
            if right {
                cur = (cur + 1) % 3;
                self.confirm = Some((chosen, cur));
                out.push(Action::Sound("misc/menu1.wav"));
                return DirOutcome::None;
            }
            if back {
                self.confirm = None;
                out.push(Action::Sound("misc/menu3.wav"));
                return DirOutcome::None;
            }
            let pick = if key == K_MOUSE1 {
                mouse.and_then(Self::modal_btn_at)
            } else if enter {
                Some(cur)
            } else {
                None
            };
            if let Some(idx) = pick {
                self.confirm = None;
                out.push(Action::Sound("misc/menu2.wav"));
                return match idx {
                    0 => DirOutcome::Apply { parent: chosen, copy: true },
                    1 => DirOutcome::Apply { parent: chosen, copy: false },
                    _ => DirOutcome::None,
                };
            }
            return DirOutcome::None;
        }

        if back {
            out.push(Action::Sound("misc/menu3.wav"));
            return DirOutcome::Cancel;
        }
        if key == K_MOUSE1 {
            let Some(pos) = mouse else { return DirOutcome::None };
            if let Some(i) = self.row_at(pos) {
                self.focus_buttons = false;
                self.cursor = i;
                if self.activate_entry() {
                    out.push(Action::Sound("misc/menu2.wav"));
                }
                return DirOutcome::None;
            }
            if let Some(i) = Self::btn_at(pos) {
                self.focus_buttons = true;
                self.btn_cursor = i;
                out.push(Action::Sound("misc/menu2.wav"));
                return self.activate_button(i);
            }
            return DirOutcome::None;
        }
        if key == K_XBUTTON {
            out.push(Action::Sound("misc/menu2.wav"));
            return self.select_current();
        }
        if key == K_YBUTTON {
            out.push(Action::Sound("misc/menu3.wav"));
            return self.activate_button(2);
        }
        if key == K_TAB {
            self.focus_buttons = !self.focus_buttons;
            if self.focus_buttons {
                self.btn_cursor = 0;
            }
            out.push(Action::Sound("misc/menu1.wav"));
            return DirOutcome::None;
        }
        if matches!(key, K_BACKSPACE | K_LSHOULDER) {
            if self.go_up() {
                self.focus_buttons = false;
                out.push(Action::Sound("misc/menu2.wav"));
            }
            return DirOutcome::None;
        }
        if self.focus_buttons {
            if left {
                self.btn_cursor = (self.btn_cursor + BTNS.len() - 1) % BTNS.len();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if right {
                self.btn_cursor = (self.btn_cursor + 1) % BTNS.len();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if up {
                self.focus_buttons = false;
                out.push(Action::Sound("misc/menu1.wav"));
            } else if enter {
                out.push(Action::Sound("misc/menu2.wav"));
                return self.activate_button(self.btn_cursor);
            }
            return DirOutcome::None;
        }
        let n = self.entries.len();
        if n > 0 {
            if up {
                self.cursor = if self.cursor == 0 { n - 1 } else { self.cursor - 1 };
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if down {
                self.cursor = (self.cursor + 1) % n;
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if key == K_PGUP {
                self.cursor = self.cursor.saturating_sub(VIS_ROWS);
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if key == K_PGDN {
                self.cursor = (self.cursor + VIS_ROWS).min(n - 1);
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if key == K_HOME {
                self.cursor = 0;
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if key == K_END {
                self.cursor = n - 1;
                self.fix_scroll();
                out.push(Action::Sound("misc/menu1.wav"));
            } else if left {
                if self.go_up() {
                    out.push(Action::Sound("misc/menu2.wav"));
                }
            } else if (enter || right) && self.activate_entry() {
                out.push(Action::Sound("misc/menu2.wav"));
            }
        } else if left && self.go_up() {
            out.push(Action::Sound("misc/menu2.wav"));
        }
        DirOutcome::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigates_into_and_back_up_from_subfolders() {
        let base = std::env::temp_dir().join(format!("qsp-dirbrowser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("alpha/nested")).unwrap();
        std::fs::create_dir_all(base.join(".hidden")).unwrap();
        std::fs::write(base.join("plain_file.txt"), "x").unwrap();

        let mut db = DirBrowser::default();
        db.open(base.clone());
        // Hidden folder and plain files are omitted by default; Parent + alpha shown.
        assert!(db.entries.iter().any(|e| matches!(e, Entry::Parent)));
        assert!(db.entries.iter().any(|e| matches!(e, Entry::Dir { name, .. } if name == "alpha")));
        assert!(!db.entries.iter().any(|e| matches!(e, Entry::Dir { name, .. } if name == ".hidden")));

        // Enter alpha via keyboard
        let mut out = Vec::new();
        db.key(K_DOWNARROW, None, &mut out);
        db.key(K_ENTER, None, &mut out);
        assert_eq!(db.current, base.join("alpha"));

        // Backspace returns to parent with alpha still selected
        db.key(K_BACKSPACE, None, &mut out);
        assert_eq!(db.current, base);
        assert!(matches!(db.entries.get(db.cursor), Some(Entry::Dir { name, .. }) if name == "alpha"));

        // Toggling Show Hidden exposes `.hidden`
        db.key(K_YBUTTON, None, &mut out);
        assert!(db.entries.iter().any(|e| matches!(e, Entry::Dir { name, .. } if name == ".hidden")));

        let _ = std::fs::remove_dir_all(&base);
    }
}
