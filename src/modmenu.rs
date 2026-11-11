//! Mod menu: installed mods (left list + info panel) and a per-mod map
//! browser with completion stats from qsp_stats.json. The host fills the
//! data (`_mods`, `_modmaps <dir>`); picking a map runs `game <dir> <map>`.

use qinput::keys::*;
use qui::mods::{MapInfo, ModInfo};
use qui::stats::Stats;

use crate::draw::Canvas;
use crate::menu::{Action, BORDER, HILITE, gold};

/// Which page is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    List,
    Maps,
}

const ROW_H: f32 = 10.0;
const TOP: f32 = 56.0;
const ROWS: usize = 28;
/// Mod list column and info panel (menu units, 640x360).
const LIST: (f32, f32) = (40.0, 264.0);
const INFO: (f32, f32) = (312.0, 288.0);
/// Map page panel.
const MAPS: (f32, f32) = (40.0, 560.0);

#[derive(Default)]
pub struct ModMenu {
    pub mods: Vec<ModInfo>,
    /// Mounted game dir (lowercase).
    pub current: String,
    cursor: usize,
    scroll: usize,
    /// Mod whose maps are listed.
    sel: usize,
    pub maps: Vec<MapInfo>,
    pub stats: Stats,
    /// Map page cursor: 0 = "Start", 1.. = maps.
    map_cursor: usize,
    map_scroll: usize,
}

fn clamp_scroll(cursor: usize, scroll: &mut usize) {
    if cursor < *scroll {
        *scroll = cursor;
    } else if cursor >= *scroll + ROWS {
        *scroll = cursor + 1 - ROWS;
    }
}

impl ModMenu {
    pub fn set_mods(&mut self, mods: Vec<ModInfo>, current: &str) {
        self.current = current.to_ascii_lowercase();
        self.mods = mods;
        // start on the mounted mod
        self.cursor = self.mods.iter().position(|m| m.dir.eq_ignore_ascii_case(current)).unwrap_or(0);
        self.scroll = 0;
        clamp_scroll(self.cursor, &mut self.scroll);
    }

    pub fn set_maps(&mut self, maps: Vec<MapInfo>, stats: Stats) {
        self.maps = maps;
        self.stats = stats;
        self.map_cursor = 0;
        self.map_scroll = 0;
    }

    /// Shows the map page of `dir` (for `menu_mods <dir>`). False if unknown.
    pub fn select(&mut self, dir: &str) -> bool {
        let Some(i) = self.mods.iter().position(|m| m.dir.eq_ignore_ascii_case(dir)) else { return false };
        self.cursor = i;
        self.sel = i;
        clamp_scroll(i, &mut self.scroll);
        true
    }

    fn selected(&self) -> Option<&ModInfo> {
        self.mods.get(self.sel)
    }

    fn rows(&self, page: Page) -> usize {
        match page {
            Page::List => self.mods.len(),
            Page::Maps => self.maps.len() + 1,
        }
    }

    /// Row index (absolute) under a menu-space point.
    pub fn item_at(&self, page: Page, (x, y): (f32, f32)) -> Option<usize> {
        let (x0, w, scroll) = match page {
            Page::List => (LIST.0, LIST.1, self.scroll),
            Page::Maps => (MAPS.0, MAPS.1, self.map_scroll),
        };
        if x < x0 || x >= x0 + w || y < TOP {
            return None;
        }
        let r = ((y - TOP) / ROW_H) as usize;
        (r < ROWS && r + scroll < self.rows(page)).then_some(r + scroll)
    }

    pub fn cursor_mut(&mut self, page: Page) -> &mut usize {
        match page {
            Page::List => &mut self.cursor,
            Page::Maps => &mut self.map_cursor,
        }
    }

    /// Returns the next page (`None` = leave the mod menu to the main menu).
    pub fn key(&mut self, page: Page, key: i32, out: &mut Vec<Action>) -> Option<Page> {
        let n = self.rows(page);
        let (cursor, scroll) = match page {
            Page::List => (&mut self.cursor, &mut self.scroll),
            Page::Maps => (&mut self.map_cursor, &mut self.map_scroll),
        };
        let step = match key {
            K_UPARROW | K_DPAD_UP | K_MWHEELUP => -1,
            K_DOWNARROW | K_DPAD_DOWN | K_MWHEELDOWN => 1,
            K_PGUP | K_LSHOULDER => -(ROWS as i32 - 1),
            K_PGDN | K_RSHOULDER => ROWS as i32 - 1,
            K_HOME => -(n as i32),
            K_END => n as i32,
            _ => 0,
        };
        if step != 0 && n > 0 {
            let old = *cursor;
            *cursor = if matches!(key, K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN) {
                // arrows wrap, page keys stop at the ends
                (*cursor as i32 + step).rem_euclid(n as i32) as usize
            } else {
                (*cursor as i32 + step).clamp(0, n as i32 - 1) as usize
            };
            clamp_scroll(*cursor, scroll);
            if *cursor != old {
                out.push(Action::Sound("misc/menu1.wav"));
            }
            return Some(page);
        }
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1);
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);
        match page {
            Page::List if back => None,
            Page::List if enter || matches!(key, K_RIGHTARROW | K_DPAD_RIGHT) => {
                let Some(m) = self.mods.get(self.cursor) else { return Some(page) };
                self.sel = self.cursor;
                self.maps.clear();
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("_modmaps \"{}\"\n", m.dir)));
                Some(Page::Maps)
            }
            Page::Maps if back || matches!(key, K_LEFTARROW | K_DPAD_LEFT) => Some(Page::List),
            Page::Maps if enter => {
                let Some(m) = self.selected() else { return Some(Page::List) };
                let current = m.dir.eq_ignore_ascii_case(&self.current);
                let cmd = match self.map_cursor {
                    // Start: another mod boots through its own quake.rc; the
                    // mounted one starts its first map.
                    0 if !current => format!("game \"{}\"\n", m.dir),
                    0 => match self.maps.first() {
                        Some(f) => format!("game \"{}\" \"{}\"\n", m.dir, f.name),
                        None => return Some(page),
                    },
                    i => format!("game \"{}\" \"{}\"\n", m.dir, self.maps[i - 1].name),
                };
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Close);
                out.push(Action::Command(cmd));
                None
            }
            _ => Some(page),
        }
    }

    pub fn draw(&self, c: &mut Canvas, page: Page) {
        match page {
            Page::List => self.draw_list(c),
            Page::Maps => self.draw_maps(c),
        }
    }

    fn draw_list(&self, c: &mut Canvas) {
        center_gold(c, 24.0, "Mods");
        center(c, 36.0, &format!("{} installed", self.mods.len()));
        let (x, w) = LIST;
        box_(c, x, w, ROWS);
        for (i, m) in self.mods.iter().enumerate().skip(self.scroll).take(ROWS) {
            let y = TOP + (i - self.scroll) as f32 * ROW_H;
            if i == self.cursor {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            let cols = ((w - 24.0) / 8.0) as usize;
            let t = clip(&m.title, cols);
            if m.dir.eq_ignore_ascii_case(&self.current) {
                gold(c, x + 6.0, y + 1.0, &t);
                c.ch(x + w - 14.0, y + 1.0, b'*' | 128);
            } else {
                c.string(x + 6.0, y + 1.0, &t);
            }
        }
        scroll_hints(c, x, w, self.scroll, self.mods.len());
        // info panel for the highlighted mod
        let Some(m) = self.mods.get(self.cursor) else { return };
        let (ix, iw) = INFO;
        box_(c, ix, iw, ROWS);
        let cols = ((iw - 16.0) / 8.0) as usize;
        let mut y = TOP + 1.0;
        for l in wrap(&m.title, cols).iter().take(2) {
            gold(c, ix + 8.0, y, l);
            y += ROW_H;
        }
        y += 4.0;
        c.string(ix + 8.0, y, &clip(&format!("dir: {}", m.dir), cols));
        y += ROW_H;
        if let Some(b) = m.base.dir() {
            c.string(ix + 8.0, y, &format!("base: {b}"));
            y += ROW_H;
        }
        if m.dir.eq_ignore_ascii_case(&self.current) {
            gold(c, ix + 8.0, y, "(running)");
            y += ROW_H;
        }
        y += 6.0;
        let bottom = TOP + ROWS as f32 * ROW_H - ROW_H;
        'blurb: for line in &m.blurb {
            for l in wrap(line, cols) {
                if y > bottom {
                    break 'blurb;
                }
                c.string(ix + 8.0, y, &l);
                y += ROW_H;
            }
        }
    }

    fn draw_maps(&self, c: &mut Canvas) {
        let Some(m) = self.selected() else { return };
        center_gold(c, 24.0, &clip(&m.title, 70));
        let done = self.maps.iter().filter(|x| self.stats.get(&x.name).is_some_and(|s| s.completions > 0)).count();
        center(c, 36.0, &format!("{} maps, {done} completed", self.maps.len()));
        let (x, w) = MAPS;
        box_(c, x, w, ROWS);
        let stat_x = x + w - 176.0;
        for i in (self.map_scroll..self.maps.len() + 1).take(ROWS) {
            let y = TOP + (i - self.map_scroll) as f32 * ROW_H;
            if i == self.map_cursor {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            if i == 0 {
                let running = m.dir.eq_ignore_ascii_case(&self.current);
                gold(c, x + 6.0, y + 1.0, if running { "Start first map" } else { "Launch mod" });
                continue;
            }
            let map = &self.maps[i - 1];
            c.string(x + 6.0, y + 1.0, &clip(&map.name, 14));
            let tcols = ((stat_x - x - 128.0) / 8.0) as usize;
            c.string(x + 120.0, y + 1.0, &clip(&map.title, tcols));
            if let Some(s) = self.stats.get(&map.name) {
                let k = format!("K{:>3}/{:<3}", s.kills, s.total_kills);
                let sec = format!("S{:>2}/{:<2}", s.secrets, s.total_secrets);
                let t = s.best_time as u32;
                let parts = [(k, s.all_kills()), (sec, s.all_secrets()), (format!("{:>2}:{:02}", t / 60, t % 60), false)];
                let mut sx = stat_x;
                for (txt, full) in parts {
                    if full {
                        gold(c, sx, y + 1.0, &txt);
                    } else {
                        c.string(sx, y + 1.0, &txt);
                    }
                    sx += (txt.len() + 1) as f32 * 8.0;
                }
            }
        }
        if self.maps.is_empty() {
            center(c, TOP + 2.0 * ROW_H + 1.0, "no maps of its own");
        }
        scroll_hints(c, x, w, self.map_scroll, self.maps.len() + 1);
    }
}

fn box_(c: &mut Canvas, x: f32, w: f32, rows: usize) {
    let h = rows as f32 * ROW_H + 8.0;
    c.fill(x, TOP - 4.0, w, h, [0.0, 0.0, 0.0, 0.8]);
    c.frame(x, TOP - 4.0, w, h, BORDER);
}

fn scroll_hints(c: &mut Canvas, x: f32, w: f32, scroll: usize, n: usize) {
    if scroll > 0 {
        c.ch(x + w - 10.0, TOP - 2.0, b'^' | 128);
    }
    if scroll + ROWS < n {
        c.ch(x + w - 10.0, TOP + ROWS as f32 * ROW_H - 6.0, b'v' | 128);
    }
}

fn center(c: &mut Canvas, y: f32, s: &str) {
    c.string(((640.0 - s.len() as f32 * 8.0) / 2.0).floor(), y, s);
}

fn center_gold(c: &mut Canvas, y: f32, s: &str) {
    gold(c, ((640.0 - s.len() as f32 * 8.0) / 2.0).floor(), y, s);
}

/// ASCII-only, at most `n` columns (with a trailing `~` when cut).
fn clip(s: &str, n: usize) -> String {
    let s: String = s.chars().filter(char::is_ascii).collect();
    if s.len() <= n || n == 0 {
        s
    } else {
        format!("{}~", &s[..n - 1])
    }
}

/// Greedy word wrap to `n` columns (long words are cut).
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for w in s.split_whitespace() {
        let w = clip(w, n);
        if !line.is_empty() && line.len() + 1 + w.len() > n {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line += &w;
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use qui::mods::Base;

    fn menu() -> ModMenu {
        let m = |d: &str| ModInfo { dir: d.into(), title: d.into(), base: Base::Id, blurb: vec![] };
        let mut mm = ModMenu::default();
        mm.set_mods(vec![m("id1"), m("ad"), m("mg1")], "ad");
        mm
    }

    fn cmds(out: &[Action]) -> Vec<String> {
        out.iter().filter_map(|a| if let Action::Command(c) = a { Some(c.clone()) } else { None }).collect()
    }

    #[test]
    fn pick_mod_and_map() {
        let mut mm = menu();
        assert_eq!(mm.cursor, 1, "starts on the running mod");
        let mut out = Vec::new();
        assert_eq!(mm.key(Page::List, K_DOWNARROW, &mut out), Some(Page::List));
        assert_eq!(mm.key(Page::List, K_ENTER, &mut out), Some(Page::Maps));
        assert_eq!(cmds(&out), ["_modmaps \"mg1\"\n"]);
        let maps = ["start", "mg1m1"].map(|n| MapInfo { name: n.into(), title: String::new() }).to_vec();
        mm.set_maps(maps, Stats::default());
        out.clear();
        mm.key(Page::Maps, K_DOWNARROW, &mut out);
        mm.key(Page::Maps, K_DOWNARROW, &mut out);
        out.clear();
        assert_eq!(mm.key(Page::Maps, K_ENTER, &mut out), None);
        assert_eq!(cmds(&out), ["game \"mg1\" \"mg1m1\"\n"]);
        // "Launch mod" on another mod boots it without a map
        mm.map_cursor = 0;
        out.clear();
        mm.key(Page::Maps, K_ENTER, &mut out);
        assert_eq!(cmds(&out), ["game \"mg1\"\n"]);
        assert_eq!(mm.key(Page::Maps, K_ESCAPE, &mut out), Some(Page::List));
        assert_eq!(mm.key(Page::List, K_ESCAPE, &mut out), None);
    }

    #[test]
    fn text_helpers() {
        assert_eq!(clip("abcdef", 4), "abc~");
        assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
    }
}
