//! Main menu → Demos: saved demos (`record`, kept auto-demos) and
//! automatic ones (`cl_autodemo`), a list and an info panel. The host fills
//! it (`_scandemos`); maps and lengths arrive from a background scan.
//! Enter plays, E exports to video, K keeps an auto-demo, Del deletes.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use qinput::keys::*;

use crate::demo::library::DemoFile;
use crate::demo::playback::format_time;
use qcore::cvar::Cvars;

use super::pages::{Item, toggle};
use super::{Action, Page, Screen};
use crate::draw::Canvas;
use crate::menu::{BORDER, GOLD_FG, HILITE, MUTED, TXT, txt, txt_gold};
use crate::modmenu::clip;

const ROW_H: f32 = 10.0;
const TOP: f32 = 64.0;
const ROWS: usize = 26;
const TAB_Y: f32 = 44.0;
const TAB: (f32, f32) = (130.0, 14.0);
const LIST: (f32, f32) = (40.0, 264.0);
const INFO: (f32, f32) = (312.0, 288.0);
pub const TABS: [&str; 2] = [qcore::t!("Saved"), qcore::t!("Automatic")];
/// Back button, under the list.
pub const BACK: (f32, f32) = (LIST.0, TOP + ROWS as f32 * ROW_H + 8.0);

// Options > Demos and its Video Recording page.
pub(super) const DEMO_SLOTS: &[(&str, f32)] = &[("5", 5.0), ("10", 10.0), ("20", 20.0), ("50", 50.0), ("100", 100.0), ("unlimited", 0.0)];
pub(super) const DEMO_BAR: &[(&str, f32)] = &[("off", 0.0), ("auto", 1.0), ("always", 2.0)];
pub(super) const DEMOS: &[Item] = &[
    toggle("Auto-Record Demos", "cl_autodemo"),
    Item::Choice { label: "Auto-Demo Slots", cvar: "cl_autodemo_slots", choices: DEMO_SLOTS },
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

/// The export dialog (`menu_export`): the video settings, where to write
/// it, and what to export.
pub(super) const EXPORT: &[Item] = &[
    Item::Choice { label: "Frame Rate", cvar: "capture_fps", choices: VIDEO_FPS },
    Item::Choice { label: "Resolution", cvar: "capture_size", choices: VIDEO_SIZE },
    Item::Choice { label: "Video Format", cvar: "capture_codec", choices: VIDEO_CODEC },
    toggle("GPU Encoding", "capture_gpu"),
    Item::Text { label: "Destination", cvar: "_export_dest", max: 240 },
    Item::Run { label: "Export Highlighted Clip", run: super::pages::Run::ExportClip },
    Item::Run { label: "Export Whole Demo", run: super::pages::Run::ExportWhole },
];

/// What a key asks the menu to do.
#[derive(Debug, PartialEq)]
pub enum DemoAction {
    None,
    Back,
    Play(String),
    Export(String),
    Keep(String),
    /// Asks first: (play name, shown name).
    Delete(String, String),
}

#[derive(Default)]
pub struct DemoBrowser {
    pub lists: [Vec<DemoFile>; 2],
    pub tab: usize,
    cursor: [usize; 2],
    scroll: [usize; 2],
    /// `cl_autodemo`, for the empty-list note.
    pub auto_on: bool,
    info_rx: Option<Receiver<(PathBuf, String, f64)>>,
}

impl DemoBrowser {
    /// New lists from the host; keeps the selection by file where possible.
    pub fn set(&mut self, saved: Vec<DemoFile>, auto: Vec<DemoFile>, auto_on: bool) {
        let keep: [Option<PathBuf>; 2] = std::array::from_fn(|t| self.selected_in(t).map(|d| d.path.clone()));
        self.lists = [saved, auto];
        self.auto_on = auto_on;
        for (t, kept) in keep.iter().enumerate() {
            let n = self.lists[t].len();
            self.cursor[t] = kept.as_ref().and_then(|p| self.lists[t].iter().position(|d| &d.path == p)).unwrap_or(self.cursor[t]).min(n.saturating_sub(1));
            self.clamp_scroll(t);
        }
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
        for (p, map, secs) in rx.try_iter() {
            for d in self.lists.iter_mut().flatten().filter(|d| d.path == p) {
                if !map.is_empty() {
                    d.map = Some(map.clone());
                }
                d.duration = Some(secs);
            }
        }
    }

    fn selected_in(&self, tab: usize) -> Option<&DemoFile> {
        self.lists[tab].get(self.cursor[tab])
    }

    pub fn selected(&self) -> Option<&DemoFile> {
        self.selected_in(self.tab)
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

    pub fn key(&mut self, key: i32) -> DemoAction {
        let k = if (b'A' as i32..=b'Z' as i32).contains(&key) { key + 32 } else { key };
        let n = self.lists[self.tab].len();
        let t = self.tab;
        let step = match k {
            K_UPARROW | K_DPAD_UP | K_MWHEELUP => -1,
            K_DOWNARROW | K_DPAD_DOWN | K_MWHEELDOWN => 1,
            K_PGUP => -(ROWS as i32 - 1),
            K_PGDN => ROWS as i32 - 1,
            K_HOME => -(n as i32),
            K_END => n as i32,
            _ => 0,
        };
        if step != 0 {
            if n > 0 {
                self.cursor[t] = if step.abs() == 1 { (self.cursor[t] as i32 + step).rem_euclid(n as i32) as usize } else { (self.cursor[t] as i32 + step).clamp(0, n as i32 - 1) as usize };
                self.clamp_scroll(t);
            }
            return DemoAction::None;
        }
        let sel = self.selected().map(|d| (d.play_name(), d.name.clone(), d.auto));
        match k {
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => DemoAction::Back,
            K_TAB | K_LEFTARROW | K_RIGHTARROW | K_DPAD_LEFT | K_DPAD_RIGHT | K_LSHOULDER | K_RSHOULDER => {
                self.tab ^= 1;
                DemoAction::None
            }
            K_ENTER | K_KP_ENTER | K_ABUTTON => sel.map_or(DemoAction::None, |s| DemoAction::Play(s.0)),
            K_XBUTTON => sel.map_or(DemoAction::None, |s| DemoAction::Export(s.0)),
            k if k == b'e' as i32 => sel.map_or(DemoAction::None, |s| DemoAction::Export(s.0)),
            K_YBUTTON => sel.filter(|s| s.2).map_or(DemoAction::None, |s| DemoAction::Keep(s.0)),
            k if k == b'k' as i32 => sel.filter(|s| s.2).map_or(DemoAction::None, |s| DemoAction::Keep(s.0)),
            K_DEL | K_BACKSPACE | K_BACK => sel.map_or(DemoAction::None, |s| DemoAction::Delete(s.0, s.1)),
            _ => DemoAction::None,
        }
    }

    /// Row or tab under the pointer: `Ok(row)` or `Err(tab)`.
    pub fn item_at(&self, (x, y): (f32, f32)) -> Option<Result<usize, usize>> {
        for t in 0..2 {
            let tx = 320.0 - TAB.0 - 4.0 + t as f32 * (TAB.0 + 8.0);
            if (tx..tx + TAB.0).contains(&x) && (TAB_Y..TAB_Y + TAB.1).contains(&y) {
                return Some(Err(t));
            }
        }
        let (lx, lw) = LIST;
        if !(lx..lx + lw).contains(&x) || y < TOP {
            return None;
        }
        let r = ((y - TOP) / ROW_H) as usize;
        let i = self.scroll[self.tab] + r;
        (r < ROWS && i < self.lists[self.tab].len()).then_some(Ok(i))
    }

    /// Pointer hover / click: select the row or switch tab; true when the
    /// selection changed.
    pub fn point(&mut self, pos: (f32, f32), click: bool) -> bool {
        match self.item_at(pos) {
            Some(Ok(i)) => std::mem::replace(&mut self.cursor[self.tab], i) != i,
            Some(Err(t)) if click => std::mem::replace(&mut self.tab, t) != t,
            _ => false,
        }
    }

    pub fn draw(&self, c: &mut Canvas) {
        c.text_center(320.0, 24.0, "Demos", 13.0, GOLD_FG);
        for (t, label) in TABS.iter().enumerate() {
            let x = 320.0 - TAB.0 - 4.0 + t as f32 * (TAB.0 + 8.0);
            let on = t == self.tab;
            c.fill(x, TAB_Y, TAB.0, TAB.1, if on { HILITE } else { [0.0, 0.0, 0.0, 0.7] });
            c.frame(x, TAB_Y, TAB.0, TAB.1, BORDER);
            let s = qcore::tr!("{} ({})", qcore::lang::tr(label), self.lists[t].len());
            c.text_center(x + TAB.0 / 2.0, TAB_Y + 3.0, &s, TXT, if on { GOLD_FG } else { MUTED });
        }
        let (x, w) = LIST;
        panel(c, x, w);
        let list = &self.lists[self.tab];
        if list.is_empty() {
            let note = if self.tab == 0 {
                qcore::t!("No saved demos yet. Record one with the console command 'record <name>', or keep an automatic demo.")
            } else if self.auto_on {
                qcore::t!("No automatic demos yet. Each level you play is recorded here.")
            } else {
                qcore::t!("Automatic demos are off. Turn on Auto-Record Demos in Options > Demos to record every level you play.")
            };
            paragraph(c, x + 8.0, TOP + 1.0, w - 16.0, note, MUTED);
        }
        for (i, d) in list.iter().enumerate().skip(self.scroll[self.tab]).take(ROWS) {
            let y = TOP + (i - self.scroll[self.tab]) as f32 * ROW_H;
            if i == self.cursor[self.tab] {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            let label = if d.auto { auto_label(d) } else { d.name.clone() };
            txt(c, x + 6.0, y + 1.0, &clip(&label, ((w - 60.0) / 6.0) as usize));
            if let Some(t) = d.duration {
                c.text_right(x + w - 8.0, y + 1.0, &format_time(t), TXT, MUTED);
            }
        }
        if list.len() > ROWS {
            let h = ROWS as f32 * ROW_H;
            let th = (h * ROWS as f32 / list.len() as f32).max(8.0);
            let ty = TOP + (h - th) * self.scroll[self.tab] as f32 / (list.len() - ROWS) as f32;
            c.fill(x + w - 4.0, TOP, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x + w - 4.0, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
        let (ix, iw) = INFO;
        panel(c, ix, iw);
        let Some(d) = self.selected() else { return };
        let cols = ((iw - 16.0) / 6.0) as usize;
        let mut y = TOP + 1.0;
        txt_gold(c, ix + 8.0, y, &clip(&if d.auto { auto_label(d) } else { d.name.clone() }, cols));
        y += ROW_H + 4.0;
        let mut row = |c: &mut Canvas, s: String| {
            txt(c, ix + 8.0, y, &clip(&s, cols));
            y += ROW_H;
        };
        row(c, qcore::tr!("Map: {}", d.map.as_deref().unwrap_or("...")));
        row(c, qcore::tr!("Length: {}", d.duration.map_or("...".into(), format_time)));
        row(c, qcore::tr!("Recorded: {}", date(d.modified)));
        row(c, qcore::tr!("Size: {} KB", d.bytes.div_ceil(1024)));
        if d.auto {
            row(c, qcore::tr!("Automatic: replaced when the slots are full"));
        }
        y += 8.0;
        let keys: &[(&str, &str)] = if d.auto {
            &[("Enter", "Play"), ("E", "Export Video"), ("K", "Keep"), ("Del", "Delete"), ("Tab", "Switch List")]
        } else {
            &[("Enter", "Play"), ("E", "Export Video"), ("Del", "Delete"), ("Tab", "Switch List")]
        };
        for (k, a) in keys {
            c.text(ix + 8.0, y, k, TXT, GOLD_FG);
            txt(c, ix + 56.0, y, a);
            y += ROW_H;
        }
        y += 6.0;
        paragraph(c, ix + 8.0, y, iw - 16.0, qcore::t!("While a demo plays: Space pauses, the arrows scrub and change speed, I and O mark a clip and E exports it."), MUTED);
    }
}

impl super::Menu {
    /// Opens the export dialog; Back returns to `back` (the Demos list, or
    /// the demo itself). `info` lines go under the rows.
    pub fn open_export(&mut self, back: Screen, info: Vec<String>, clip: bool) {
        self.export_back = back;
        self.export_info = info;
        self.opt_cursor = if clip { 5 } else { 6 };
        self.screen = Screen::OptionsPage(Page::Export);
    }

    /// Lines under the export dialog's rows.
    pub(super) fn draw_export_notes(&self, c: &mut Canvas, rows: usize) {
        let mut y = super::PAGE_ROW0 + rows as f32 * super::ROW_H + 30.0;
        for l in &self.export_info {
            c.text_center(320.0, y, l, TXT, MUTED);
            y += 11.0;
        }
    }

    /// A key on the Demos screen: list keys, then the chosen action.
    pub(super) fn demos_key(&mut self, key: i32, out: &mut Vec<Action>) {
        match self.demos.key(key) {
            DemoAction::None => {}
            DemoAction::Back => {
                self.screen = Screen::Main;
                out.push(Action::Sound("misc/menu3.wav"));
            }
            DemoAction::Play(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("playdemo \"{n}\"\n")));
                out.push(Action::Close);
                self.screen = Screen::None;
            }
            DemoAction::Export(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("menu_export \"{n}\"\n")));
            }
            DemoAction::Keep(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("_demo_keep_file \"{n}\"\n")));
            }
            DemoAction::Delete(n, shown) => {
                self.confirm(vec![qcore::tr!("Delete {}?", shown), qcore::tr!("This can't be undone.")], qcore::t!("Delete"), format!("_demo_delete \"{n}\"\n"), false);
            }
        }
    }

    /// Left click: Back, a tab, or a row (which plays it).
    pub(super) fn demos_click(&mut self, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let Some(pos) = self.mouse else { return Vec::new() };
        if let Some((bx, by, bw, bh)) = self.back_button()
            && (bx..bx + bw).contains(&pos.0)
            && (by..by + bh).contains(&pos.1)
        {
            return self.key(K_ESCAPE, cv, server_active);
        }
        self.demos.point(pos, true);
        if matches!(self.demos.item_at(pos), Some(Ok(_))) {
            return self.key(K_ENTER, cv, server_active);
        }
        Vec::new()
    }
}

/// `<stamp>_<map>` as "2024-01-31 12:00 e1m1".
fn auto_label(d: &DemoFile) -> String {
    let (stamp, map) = d.name.split_once('_').unwrap_or((&d.name, ""));
    let b = stamp.as_bytes();
    if b.len() == 15 && b[8] == b'-' {
        format!("{}-{}-{} {}:{}  {map}", &stamp[..4], &stamp[4..6], &stamp[6..8], &stamp[9..11], &stamp[11..13])
    } else {
        d.name.clone()
    }
}

fn date(secs: u64) -> String {
    let s = crate::demo::stamp(secs);
    format!("{}-{}-{} {}:{} UTC", &s[..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13])
}

/// Translated text wrapped to `w`.
fn paragraph(c: &mut Canvas, x: f32, y: f32, w: f32, s: &str, color: [f32; 4]) {
    let s = qcore::lang::tr(s);
    for (i, l) in qcore::lang::wrap(&s, w, |t| c.text_w_raw(t, TXT)).iter().enumerate() {
        c.text_raw(x, y + i as f32 * ROW_H, l, TXT, color);
    }
}

fn panel(c: &mut Canvas, x: f32, w: f32) {
    let h = ROWS as f32 * ROW_H + 8.0;
    c.fill(x, TOP - 4.0, w, h, [0.0, 0.0, 0.0, 0.8]);
    c.frame(x, TOP - 4.0, w, h, BORDER);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, auto: bool) -> DemoFile {
        DemoFile { path: PathBuf::from(format!("/u/{name}.dem")), name: name.into(), auto, bytes: 2048, modified: 0, map: None, duration: Some(61.0) }
    }

    #[test]
    fn keys_pick_actions() {
        let mut b = DemoBrowser::default();
        b.set(vec![file("a", false), file("b", false)], vec![file("20240101-120000_e1m1", true)], true);
        assert_eq!(b.key(K_DOWNARROW), DemoAction::None);
        assert_eq!(b.key(K_ENTER), DemoAction::Play("b.dem".into()));
        // saved demos can't be kept
        assert_eq!(b.key(b'k' as i32), DemoAction::None);
        assert_eq!(b.key(K_TAB), DemoAction::None);
        assert_eq!(b.key(b'K' as i32), DemoAction::Keep("autodemos/20240101-120000_e1m1.dem".into()));
        assert_eq!(b.key(b'e' as i32), DemoAction::Export("autodemos/20240101-120000_e1m1.dem".into()));
        assert_eq!(b.key(K_DEL), DemoAction::Delete("autodemos/20240101-120000_e1m1.dem".into(), "20240101-120000_e1m1".into()));
        assert_eq!(b.key(K_ESCAPE), DemoAction::Back);
        assert_eq!(auto_label(&b.lists[1][0]), "2024-01-01 12:00  e1m1");
    }

    #[test]
    fn selection_survives_rescan_and_opens_on_auto_when_empty() {
        let mut b = DemoBrowser::default();
        b.set(vec![file("a", false), file("b", false)], vec![], false);
        b.key(K_DOWNARROW);
        b.set(vec![file("new", false), file("a", false), file("b", false)], vec![], false);
        assert_eq!(b.selected().map(|d| d.name.as_str()), Some("b"));
        let mut e = DemoBrowser::default();
        e.set(vec![], vec![file("20240101-120000_e1m1", true)], true);
        e.open();
        assert_eq!(e.tab, 1);
    }
}
