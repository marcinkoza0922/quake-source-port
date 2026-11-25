//! Main menu → Demos: saved demos (`record`, kept auto-demos) and
//! automatic ones (`cl_autodemo`), a list and an info panel. The host fills
//! it (`_scandemos`); maps and lengths arrive from a background scan.
//! Typing filters both lists (fuzzy, like Map Packs) and Tab cycles the
//! sort; the actions are buttons in the info panel (Play, Export Video,
//! Rename, Keep, Delete), with Enter, Del and pad buttons as shortcuts.
//! Drawing: `demos/draw.rs`; the export and rename dialogs: `demos/dialogs.rs`.

mod dialogs;
mod draw;
mod touch;

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use qinput::keys::*;

use crate::demo::library::{DemoFile, kept_name};
use crate::modmenu::fuzzy_score;

use super::pages::{Item, Run, toggle};
use super::{Action, Page, Screen};

const ROW_H: f32 = 10.0;
const TAB_Y: f32 = 38.0;
const TAB: (f32, f32) = (130.0, 14.0);
/// Filter box and sort button, over the list.
const BAR_Y: f32 = 56.0;
const SORT_W: f32 = 96.0;
const TOP: f32 = 76.0;
const ROWS: usize = 24;
const LIST: (f32, f32) = (40.0, 264.0);
const INFO: (f32, f32) = (312.0, 288.0);
/// Action buttons in the info panel: a two-column grid.
const BTN_Y: f32 = TOP + 8.0 * ROW_H;
const BTN: (f32, f32) = (133.0, 15.0);
const BTN_GAP: f32 = 4.0;
pub const TABS: [&str; 2] = [qcore::t!("Saved"), qcore::t!("Automatic")];
/// Back button, under the list.
pub const BACK: (f32, f32) = (LIST.0, TOP + ROWS as f32 * ROW_H + 8.0);

// Options > Demos and its Video Recording page.
pub(super) const DEMO_SLOTS: &[(&str, f32)] = &[("5", 5.0), ("10", 10.0), ("20", 20.0), ("50", 50.0), ("100", 100.0), ("unlimited", 0.0)];
pub(super) const PB_DEMOS: &[(&str, f32)] = &[("off", 0.0), ("any%", 1.0), ("no-hit", 4.0), ("100%", 2.0), ("100% no-hit", 5.0)];
pub(super) const DEMO_BAR: &[(&str, f32)] = &[("off", 0.0), ("auto", 1.0), ("always", 2.0)];
pub(super) const DEMOS: &[Item] = &[
    toggle("Auto-Record Demos", "cl_autodemo"),
    Item::Choice { label: "Auto-Demo Slots", cvar: "cl_autodemo_slots", choices: DEMO_SLOTS },
    Item::Choice { label: "Keep Personal Bests", cvar: "cl_autodemo_pb", choices: PB_DEMOS },
    Item::Choice { label: "Show Ghost", cvar: "cl_demo_ghost", choices: PB_DEMOS },
    Item::Choice { label: "Playback Bar", cvar: "cl_demobar", choices: DEMO_BAR },
    toggle("Pause at End", "cl_demo_endpause"),
    Item::Sub { label: "Video Recording", page: Page::Video },
];

pub(super) const VIDEO_FPS: &[(&str, f32)] = &[("30", 30.0), ("60", 60.0), ("120", 120.0)];
pub(super) const VIDEO_CODEC: &[(&str, f32)] = &[("H.264 (MP4)", 0.0), ("H.265 (MP4)", 1.0), ("AV1 (MP4)", 2.0), ("VP9 (WebM)", 3.0)];
pub(super) const VIDEO_QUALITY: &[(&str, f32)] = &[("high", 0.0), ("medium", 1.0), ("small file", 2.0)];
pub(super) const VIDEO_SIZE: &[(&str, f32)] = &[("Same as window", 0.0), ("720p", 1.0), ("1080p", 2.0), ("1440p", 3.0), ("4K", 4.0)];
pub(super) const VIDEO: &[Item] = &[
    Item::Choice { label: "Frame Rate", cvar: "capture_fps", choices: VIDEO_FPS },
    Item::Choice { label: "Resolution", cvar: "capture_size", choices: VIDEO_SIZE },
    Item::Choice { label: "Video Format", cvar: "capture_codec", choices: VIDEO_CODEC },
    Item::Choice { label: "Quality", cvar: "capture_quality", choices: VIDEO_QUALITY },
    toggle("GPU Encoding", "capture_gpu"),
    toggle("Record Audio", "capture_audio"),
    Item::Text { label: "ffmpeg Program", cvar: "capture_ffmpeg", max: 60 },
];

/// The export dialog (`menu_export`): the video settings, the folder and
/// file name to write (the extension follows the format), and what to export.
pub(super) const EXPORT: &[Item] = &[
    Item::Choice { label: "Frame Rate", cvar: "capture_fps", choices: VIDEO_FPS },
    Item::Choice { label: "Resolution", cvar: "capture_size", choices: VIDEO_SIZE },
    Item::Choice { label: "Video Format", cvar: "capture_codec", choices: VIDEO_CODEC },
    toggle("GPU Encoding", "capture_gpu"),
    Item::Text { label: "Destination", cvar: "_export_dest", max: 240 },
    Item::Text { label: "File Name", cvar: "_export_name", max: 100 },
    Item::Run { label: "Export Highlighted Clip", run: Run::ExportClip },
    Item::Run { label: "Export Whole Demo", run: Run::ExportWhole },
];
/// Rows of the export buttons.
pub(super) const EXPORT_CLIP_ROW: usize = 6;

/// The rename dialog (Demos → Rename).
pub(super) const RENAME: &[Item] = &[Item::Text { label: "Name", cvar: "_demo_rename", max: 60 }, Item::Run { label: "Rename", run: Run::RenameDemo }];

/// Text fields holding a file name: no path separators or characters
/// file systems refuse.
pub(super) fn file_name_field(cvar: &str) -> bool {
    matches!(cvar, "_demo_rename" | "_export_name")
}

pub(super) fn file_name_char(ch: char) -> bool {
    !"/\\:*?<>|".contains(ch)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DemoSort {
    #[default]
    Newest,
    Name,
    Map,
    Length,
}

impl DemoSort {
    fn next(self) -> Self {
        match self {
            Self::Newest => Self::Name,
            Self::Name => Self::Map,
            Self::Map => Self::Length,
            Self::Length => Self::Newest,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Newest => qcore::t!("Newest"),
            Self::Name => qcore::t!("Name"),
            Self::Map => qcore::t!("Map"),
            Self::Length => qcore::t!("Length"),
        }
    }
}

/// The info panel's buttons, in grid order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoBtn {
    Play,
    Export,
    Rename,
    Keep,
    Delete,
}

pub const BUTTONS: [(DemoBtn, &str); 5] = [
    (DemoBtn::Play, qcore::t!("Play")),
    (DemoBtn::Export, qcore::t!("Export Video")),
    (DemoBtn::Rename, qcore::t!("Rename")),
    (DemoBtn::Keep, qcore::t!("Keep")),
    (DemoBtn::Delete, qcore::t!("Delete")),
];

fn button_rect(i: usize) -> (f32, f32, f32, f32) {
    let (col, row) = ((i % 2) as f32, (i / 2) as f32);
    (INFO.0 + 8.0 + col * (BTN.0 + 6.0), BTN_Y + row * (BTN.1 + BTN_GAP), BTN.0, BTN.1)
}

/// What a key or click asks the menu to do.
#[derive(Debug, PartialEq)]
pub enum DemoAction {
    None,
    /// The cursor, tab, filter or sort changed.
    Moved,
    Back,
    Play(String),
    Export(String),
    Keep(String),
    /// The `.dem` (user-dir relative), the suggested name, and whether it's
    /// an auto-demo (renaming one keeps it).
    Rename { file: String, name: String, auto: bool },
    /// Asks first: (play name, shown name).
    Delete(String, String),
}

/// What is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Row(usize),
    Tab(usize),
    Sort,
    Button(DemoBtn),
}

#[derive(Default)]
pub struct DemoBrowser {
    pub lists: [Vec<DemoFile>; 2],
    /// Indices into `lists` that pass the filter, in the shown order.
    view: [Vec<usize>; 2],
    pub tab: usize,
    /// Positions in `view`.
    cursor: [usize; 2],
    scroll: [usize; 2],
    filter: String,
    sort: DemoSort,
    /// `cl_autodemo`, for the empty-list note.
    pub auto_on: bool,
    info_rx: Option<Receiver<(PathBuf, String, f64)>>,
}

impl DemoBrowser {
    /// New lists from the host; keeps the selection by file where possible.
    pub fn set(&mut self, saved: Vec<DemoFile>, auto: Vec<DemoFile>, auto_on: bool) {
        let keep = self.selected_paths();
        self.lists = [saved, auto];
        self.auto_on = auto_on;
        self.rebuild(Some(keep));
        // map names and lengths: one background pass over the files
        let todo: Vec<PathBuf> = self.lists.iter().flatten().filter(|d| d.duration.is_none()).map(|d| d.path.clone()).collect();
        if todo.is_empty() || cfg!(target_arch = "wasm32") {
            self.info_rx = None;
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for p in todo {
                if let Some((map, secs)) = crate::demo::library::describe(&p)
                    && tx.send((p, map, secs)).is_err()
                {
                    return;
                }
            }
        });
        self.info_rx = Some(rx);
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.info_rx else { return };
        let mut any = false;
        for (p, map, secs) in rx.try_iter() {
            for d in self.lists.iter_mut().flatten().filter(|d| d.path == p) {
                if !map.is_empty() {
                    d.map = Some(map.clone());
                }
                d.duration = Some(secs);
                any = true;
            }
        }
        // maps and lengths can reorder or match the filter now
        if any && (self.sort != DemoSort::Newest || !self.filter.is_empty()) {
            let keep = self.selected_paths();
            self.rebuild(Some(keep));
        }
    }

    /// Selects the demo at `path` (after a rename), on its tab.
    pub fn focus(&mut self, path: &Path) {
        for t in 0..2 {
            if let Some(v) = self.view[t].iter().position(|&i| self.lists[t][i].path == path) {
                self.tab = t;
                self.cursor[t] = v;
                self.clamp_scroll(t);
            }
        }
    }

    fn selected_paths(&self) -> [Option<PathBuf>; 2] {
        std::array::from_fn(|t| self.selected_in(t).map(|d| d.path.clone()))
    }

    /// Re-filters and re-sorts both lists; `keep` holds each tab's selection
    /// by file (else the best match is selected).
    fn rebuild(&mut self, keep: Option<[Option<PathBuf>; 2]>) {
        let q = self.filter.trim().to_string();
        for t in 0..2 {
            let list = &self.lists[t];
            let mut scored: Vec<(usize, i32)> = list
                .iter()
                .enumerate()
                .filter_map(|(i, d)| {
                    let label = label(d);
                    let s = [Some(label.as_str()), Some(d.name.as_str()), d.map.as_deref()].into_iter().flatten().filter_map(|s| fuzzy_score(&q, s)).max()?;
                    Some((i, s))
                })
                .collect();
            scored.sort_by(|&(ia, sa), &(ib, sb)| {
                if !q.is_empty() && sa != sb {
                    return sb.cmp(&sa);
                }
                let (a, b) = (&list[ia], &list[ib]);
                let lower = |d: &DemoFile| label(d).to_lowercase();
                let map = |d: &DemoFile| d.map.as_deref().unwrap_or("").to_ascii_lowercase();
                match self.sort {
                    DemoSort::Newest => std::cmp::Ordering::Equal,
                    DemoSort::Name => lower(a).cmp(&lower(b)),
                    DemoSort::Map => map(a).cmp(&map(b)),
                    // longest first; unknown lengths last
                    DemoSort::Length => b.duration.unwrap_or(-1.0).total_cmp(&a.duration.unwrap_or(-1.0)),
                }
                .then(ia.cmp(&ib))
            });
            self.view[t] = scored.into_iter().map(|(i, _)| i).collect();
            let kept = keep.as_ref().and_then(|k| k[t].as_ref());
            let n = self.view[t].len();
            self.cursor[t] = match kept {
                Some(p) => self.view[t].iter().position(|&i| &self.lists[t][i].path == p).unwrap_or(self.cursor[t]),
                None => 0,
            }
            .min(n.saturating_sub(1));
            if kept.is_none() {
                self.scroll[t] = 0;
            }
            self.scroll[t] = self.scroll[t].min(n.saturating_sub(ROWS));
            self.clamp_scroll(t);
        }
    }

    fn selected_in(&self, tab: usize) -> Option<&DemoFile> {
        self.view[tab].get(self.cursor[tab]).and_then(|&i| self.lists[tab].get(i))
    }

    pub fn selected(&self) -> Option<&DemoFile> {
        self.selected_in(self.tab)
    }

    /// Rows in the current tab's view.
    fn rows(&self) -> usize {
        self.view[self.tab].len()
    }

    fn row(&self, v: usize) -> Option<&DemoFile> {
        self.view[self.tab].get(v).and_then(|&i| self.lists[self.tab].get(i))
    }

    fn clamp_scroll(&mut self, t: usize) {
        let (c, s) = (self.cursor[t], &mut self.scroll[t]);
        if c < *s {
            *s = c;
        } else if c >= *s + ROWS {
            *s = c + 1 - ROWS;
        }
    }

    /// Opens on the automatic tab when there are no saved demos.
    pub fn open(&mut self) {
        if self.lists[0].is_empty() && !self.lists[1].is_empty() {
            self.tab = 1;
        }
    }

    /// Whether a button can be used for the selection.
    pub fn enabled(&self, b: DemoBtn) -> bool {
        self.selected().is_some_and(|d| b != DemoBtn::Keep || d.auto)
    }

    /// A button's action on the selected demo.
    pub fn action(&self, b: DemoBtn) -> DemoAction {
        let Some(d) = self.selected().filter(|_| self.enabled(b)) else { return DemoAction::None };
        match b {
            DemoBtn::Play => DemoAction::Play(d.play_name()),
            DemoBtn::Export => DemoAction::Export(d.play_name()),
            DemoBtn::Keep => DemoAction::Keep(d.play_name()),
            DemoBtn::Rename => DemoAction::Rename { file: d.rel_name(), name: if d.auto { kept_name(&d.name) } else { d.name.clone() }, auto: d.auto },
            DemoBtn::Delete => DemoAction::Delete(d.play_name(), d.name.clone()),
        }
    }

    fn set_filter(&mut self, f: impl FnOnce(&mut String) -> bool) -> DemoAction {
        if !f(&mut self.filter) {
            return DemoAction::None;
        }
        self.rebuild(None);
        DemoAction::Moved
    }

    fn cycle_sort(&mut self) -> DemoAction {
        self.sort = self.sort.next();
        let keep = self.selected_paths();
        self.rebuild(Some(keep));
        DemoAction::Moved
    }

    fn switch_tab(&mut self, t: usize) -> DemoAction {
        if std::mem::replace(&mut self.tab, t) == t { DemoAction::None } else { DemoAction::Moved }
    }

    /// Keys: arrows move, typing filters, Tab sorts, Left/Right switch list;
    /// Enter plays and Del deletes; pad: X export, Y rename, LT keep, Back
    /// delete, RT sort.
    pub fn key(&mut self, key: i32) -> DemoAction {
        let n = self.rows();
        let t = self.tab;
        let step = match key {
            K_UPARROW | K_DPAD_UP => -1,
            K_DOWNARROW | K_DPAD_DOWN => 1,
            K_PGUP => -(ROWS as i32 - 1),
            K_PGDN => ROWS as i32 - 1,
            K_HOME => -(n as i32),
            K_END => n as i32,
            _ => 0,
        };
        if step != 0 {
            if n == 0 {
                return DemoAction::None;
            }
            let old = self.cursor[t];
            // arrows wrap, page keys stop at the ends
            self.cursor[t] = if step.abs() == 1 { (old as i32 + step).rem_euclid(n as i32) as usize } else { (old as i32 + step).clamp(0, n as i32 - 1) as usize };
            self.clamp_scroll(t);
            return if self.cursor[t] == old { DemoAction::None } else { DemoAction::Moved };
        }
        match key {
            K_ESCAPE if !self.filter.is_empty() => self.set_filter(|f| !std::mem::take(f).is_empty()),
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => DemoAction::Back,
            K_TAB | K_RTRIGGER => self.cycle_sort(),
            K_LEFTARROW | K_DPAD_LEFT | K_LSHOULDER => self.switch_tab(0),
            K_RIGHTARROW | K_DPAD_RIGHT | K_RSHOULDER => self.switch_tab(1),
            K_BACKSPACE => self.set_filter(|f| f.pop().is_some()),
            K_ENTER | K_KP_ENTER | K_ABUTTON => self.action(DemoBtn::Play),
            K_XBUTTON => self.action(DemoBtn::Export),
            K_YBUTTON => self.action(DemoBtn::Rename),
            K_LTRIGGER => self.action(DemoBtn::Keep),
            K_DEL | K_BACK => self.action(DemoBtn::Delete),
            0x20..=0x7e => self.set_filter(|f| {
                f.push(key as u8 as char);
                true
            }),
            _ => DemoAction::None,
        }
    }

    /// What is under the pointer.
    pub fn hit(&self, (x, y): (f32, f32)) -> Option<Hit> {
        for t in 0..2 {
            let tx = 320.0 - TAB.0 - 4.0 + t as f32 * (TAB.0 + 8.0);
            if (tx..tx + TAB.0).contains(&x) && (TAB_Y..TAB_Y + TAB.1).contains(&y) {
                return Some(Hit::Tab(t));
            }
        }
        let sx = LIST.0 + LIST.1 - SORT_W;
        if (sx..LIST.0 + LIST.1).contains(&x) && (BAR_Y..BAR_Y + crate::modmenu::BAR_H).contains(&y) {
            return Some(Hit::Sort);
        }
        for (i, &(b, _)) in BUTTONS.iter().enumerate() {
            let r = button_rect(i);
            if (r.0..r.0 + r.2).contains(&x) && (r.1..r.1 + r.3).contains(&y) {
                return Some(Hit::Button(b));
            }
        }
        let (lx, lw) = LIST;
        if !(lx..lx + lw).contains(&x) || y < TOP {
            return None;
        }
        let r = ((y - TOP) / ROW_H) as usize;
        let i = self.scroll[self.tab] + r;
        (r < ROWS && i < self.rows()).then_some(Hit::Row(i))
    }

    /// Left click: a tab or the sort button switches; a row is selected, and
    /// a click on the selected row plays it; a button acts when it can.
    pub fn click(&mut self, pos: (f32, f32)) -> DemoAction {
        match self.hit(pos) {
            Some(Hit::Tab(t)) => self.switch_tab(t),
            Some(Hit::Sort) => self.cycle_sort(),
            Some(Hit::Row(i)) if i == self.cursor[self.tab] => self.action(DemoBtn::Play),
            Some(Hit::Row(i)) => {
                self.cursor[self.tab] = i;
                DemoAction::Moved
            }
            Some(Hit::Button(b)) => self.action(b),
            None => DemoAction::None,
        }
    }

    /// Scrollbar: (x, track top, track h, thumb y, thumb h).
    pub fn scrollbar(&self) -> Option<(f32, f32, f32, f32, f32)> {
        let n = self.rows();
        if n <= ROWS {
            return None;
        }
        let h = ROWS as f32 * ROW_H;
        let th = (h * ROWS as f32 / n as f32).max(8.0);
        let ty = TOP + (h - th) * self.scroll[self.tab] as f32 / (n - ROWS) as f32;
        Some((LIST.0 + LIST.1 - 5.0, TOP, h, ty, th))
    }

    /// Scrollbar drag: the thumb top follows `y`. The selection stays.
    pub fn drag_to(&mut self, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.scrollbar() else { return };
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.scroll[self.tab] = (f * (self.rows() - ROWS) as f32).round() as usize;
    }

    /// Wheel: scrolls the view by rows. The selection stays.
    pub fn wheel(&mut self, up: bool) {
        let sc = self.scroll[self.tab];
        let sc = if up { sc.saturating_sub(3) } else { sc + 3 };
        self.scroll[self.tab] = sc.min(self.rows().saturating_sub(ROWS));
    }
}

/// How a demo is listed: auto-demos by date and map.
fn label(d: &DemoFile) -> String {
    if d.auto { auto_label(d) } else { d.name.clone() }
}

/// `<stamp>_<map>` as "2024-01-31 12:00 e1m1".
fn auto_label(d: &DemoFile) -> String {
    let (stamp, map) = d.name.split_once('_').unwrap_or((&d.name, ""));
    let b = stamp.as_bytes();
    if b.len() == 15 && b[8] == b'-' && stamp.is_ascii() {
        format!("{}-{}-{} {}:{}  {map}", &stamp[..4], &stamp[4..6], &stamp[6..8], &stamp[9..11], &stamp[11..13])
    } else {
        d.name.clone()
    }
}

#[cfg(test)]
mod tests;
