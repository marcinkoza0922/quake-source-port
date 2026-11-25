//! Mod menu: installed mods (left list + info panel) and a per-mod map
//! browser with completion stats from qsp_stats.json. The host fills the
//! data (`_mods`, `_modmaps <dir>`). Enter launches the mod (`game <dir>`);
//! the map page is the level selector, reached with Right (or from New Game's
//! Level Select button). Both lists scroll: arrows, wheel and a draggable bar.

mod online;
mod touch;

use qinput::keys::*;
use qui::mods::{MapInfo, ModInfo};
use qui::stats::Stats;

use crate::draw::Canvas;
use crate::menu::{Action, BORDER, FG, GOLD_FG, HILITE, MUTED, TXT, txt, txt_gold};

/// Which page is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    List,
    Maps,
    /// Quaddicted archive browser (`online.rs`).
    Online,
}

const ROW_H: f32 = 10.0;
const OFFICIAL_FG: [f32; 4] = [0.62, 0.80, 0.95, 1.0];
const TOP: f32 = 64.0;
const ROWS: usize = 27;
const BAR_Y: f32 = 46.0;
pub(crate) const BAR_H: f32 = 12.0;
/// Mod list column and info panel (menu units, 640x360).
const LIST: (f32, f32) = (40.0, 264.0);
const INFO: (f32, f32) = (312.0, 288.0);
/// Map page panel.
const MAPS: (f32, f32) = (40.0, 560.0);
/// Sort button widths per page (wide enough for controller glyphs).
const SORT_W_LIST: f32 = 96.0;
const SORT_W_MAPS: f32 = 104.0;
const SORT_W_ONLINE: f32 = 96.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModSort {
    #[default]
    Default,
    Name,
    Dir,
}

impl ModSort {
    fn next(self) -> Self {
        match self {
            Self::Default => Self::Name,
            Self::Name => Self::Dir,
            Self::Dir => Self::Default,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Default => qcore::t!("Recent"),
            Self::Name => qcore::t!("Name"),
            Self::Dir => qcore::t!("Dir"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapSort {
    #[default]
    Default,
    Name,
    Title,
    Completed,
}

impl MapSort {
    fn next(self) -> Self {
        match self {
            Self::Default => Self::Name,
            Self::Name => Self::Title,
            Self::Title => Self::Completed,
            Self::Completed => Self::Default,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Default => qcore::t!("Default"),
            Self::Name => qcore::t!("Name"),
            Self::Title => qcore::t!("Title"),
            Self::Completed => qcore::t!("Done"),
        }
    }
}

#[derive(Default)]
pub struct ModMenu {
    pub mods: Vec<ModInfo>,
    /// Mounted game dir (lowercase).
    pub current: String,
    cursor: usize,
    scroll: usize,
    mod_view: Vec<usize>,
    mod_filter: String,
    mod_sort: ModSort,
    mod_last_played: Vec<u64>,
    /// Mod index (in `self.mods`) whose maps are listed.
    sel: usize,
    pub maps: Vec<MapInfo>,
    pub stats: Stats,
    /// Map page cursor: 0 = "Start" (when unfiltered), or maps.
    map_cursor: usize,
    map_scroll: usize,
    map_view: Vec<usize>,
    map_filter: String,
    map_sort: MapSort,
    pub online: online::OnlinePage,
}

fn clamp_scroll(cursor: usize, scroll: &mut usize) {
    if cursor < *scroll {
        *scroll = cursor;
    } else if cursor >= *scroll + ROWS {
        *scroll = cursor + 1 - ROWS;
    }
}

impl ModMenu {
    /// Back sits directly beneath the list panel, in the shared menu coordinate space.
    pub fn back_position(&self) -> (f32, f32) {
        (LIST.0, TOP + ROWS as f32 * ROW_H + 8.0)
    }

    pub fn set_mods(&mut self, mods: Vec<ModInfo>, current: &str) {
        self.current = current.to_ascii_lowercase();
        self.mod_last_played.resize(mods.len(), 0);
        self.mod_last_played.fill(0);
        self.mods = mods;
        self.rebuild_mod_view();
        // start on the mounted mod
        self.cursor = self.mod_view.iter().position(|&i| self.mods[i].dir.eq_ignore_ascii_case(current)).map_or(0, |v| v + self.pin());
        self.scroll = 0;
        clamp_scroll(self.cursor, &mut self.scroll);
    }

    /// Sets per-mod last-played timestamps (unix seconds, 0 = never) and re-sorts.
    pub fn set_last_played(&mut self, times: Vec<u64>) {
        self.mod_last_played = times;
        self.rebuild_mod_view();
        self.cursor = self.mod_view.iter().position(|&i| self.mods[i].dir.eq_ignore_ascii_case(&self.current)).map_or(0, |v| v + self.pin());
        self.scroll = 0;
        clamp_scroll(self.cursor, &mut self.scroll);
    }

    /// Title of a listed map (its name when the map has no message).
    pub fn map_title(&self, name: &str) -> Option<String> {
        let m = self.maps.iter().find(|m| m.name.eq_ignore_ascii_case(name))?;
        Some(if m.title.trim().is_empty() { m.name.clone() } else { m.title.clone() })
    }

    pub fn set_maps(&mut self, maps: Vec<MapInfo>, stats: Stats) {
        self.maps = maps;
        self.stats = stats;
        self.map_filter.clear();
        self.rebuild_map_view();
    }

    /// Shows the map page of `dir` (for `menu_mods <dir>`). False if unknown.
    pub fn select(&mut self, dir: &str) -> bool {
        let Some(i) = self.mods.iter().position(|m| m.dir.eq_ignore_ascii_case(dir)) else { return false };
        self.sel = i;
        if let Some(v) = self.mod_view.iter().position(|&idx| idx == i) {
            self.cursor = v + self.pin();
            clamp_scroll(v, &mut self.scroll);
        }
        true
    }

    fn rebuild_mod_view(&mut self) {
        let q = self.mod_filter.trim();
        let mut scored: Vec<(usize, i32)> = self
            .mods
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let s = [fuzzy_score(q, &m.title), fuzzy_score(q, &m.dir)].into_iter().flatten().max()?;
                Some((i, s))
            })
            .collect();
        scored.sort_by(|&(ia, sa), &(ib, sb)| {
            if !q.is_empty() && sa != sb {
                return sb.cmp(&sa);
            }
            let (a, b) = (&self.mods[ia], &self.mods[ib]);
            match self.mod_sort {
                ModSort::Default => {
                    let ta = self.mod_last_played.get(ia).copied().unwrap_or(0);
                    let tb = self.mod_last_played.get(ib).copied().unwrap_or(0);
                    tb.cmp(&ta)
                        .then_with(|| mod_fallback_key(a).cmp(&mod_fallback_key(b)))
                        .then_with(|| ia.cmp(&ib))
                }
                ModSort::Name => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()).then_with(|| ia.cmp(&ib)),
                ModSort::Dir => a.dir.to_ascii_lowercase().cmp(&b.dir.to_ascii_lowercase()).then_with(|| ia.cmp(&ib)),
            }
        });
        self.mod_view = scored.into_iter().map(|(i, _)| i).collect();
        self.cursor = 0;
        self.scroll = 0;
    }

    fn rebuild_map_view(&mut self) {
        let q = self.map_filter.trim();
        let mut scored: Vec<(usize, i32)> = self
            .maps
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let s = [fuzzy_score(q, &m.name), fuzzy_score(q, &m.title)].into_iter().flatten().max()?;
                Some((i, s))
            })
            .collect();
        scored.sort_by(|&(ia, sa), &(ib, sb)| {
            if !q.is_empty() && sa != sb {
                return sb.cmp(&sa);
            }
            let (a, b) = (&self.maps[ia], &self.maps[ib]);
            match self.map_sort {
                MapSort::Default => ia.cmp(&ib),
                MapSort::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()).then_with(|| ia.cmp(&ib)),
                MapSort::Title => a.title.to_ascii_lowercase().cmp(&b.title.to_ascii_lowercase()).then_with(|| ia.cmp(&ib)),
                MapSort::Completed => {
                    let da = self.stats.get(&a.name).is_some_and(|s| s.completions > 0);
                    let db = self.stats.get(&b.name).is_some_and(|s| s.completions > 0);
                    db.cmp(&da).then_with(|| ia.cmp(&ib))
                }
            }
        });
        self.map_view = scored.into_iter().map(|(i, _)| i).collect();
        self.map_cursor = 0;
        self.map_scroll = 0;
    }

    /// 1 while the "Get more maps" row is pinned above the mod list.
    fn pin(&self) -> usize {
        usize::from(self.mod_filter.is_empty())
    }

    fn mod_at_row(&self, row: usize) -> Option<(usize, &ModInfo)> {
        let &idx = self.mod_view.get(row.checked_sub(self.pin())?)?;
        Some((idx, self.mods.get(idx)?))
    }

    fn map_at_row(&self, row: usize) -> Option<&MapInfo> {
        let idx = if self.map_filter.is_empty() { row.checked_sub(1)? } else { row };
        self.map_view.get(idx).and_then(|&i| self.maps.get(i))
    }

    /// Scrollbar of a page: (x, track top, track h, thumb y, thumb h).
    pub fn scrollbar(&self, page: Page) -> Option<(f32, f32, f32, f32, f32)> {
        let (x0, w, scroll) = match page {
            Page::List => (LIST.0, LIST.1, self.scroll),
            Page::Maps => (MAPS.0, MAPS.1, self.map_scroll),
            Page::Online => return self.online.scrollbar(),
        };
        let n = self.rows(page);
        if n <= ROWS {
            return None;
        }
        let h = ROWS as f32 * ROW_H;
        let th = (h * ROWS as f32 / n as f32).max(8.0);
        let ty = TOP + (h - th) * scroll as f32 / (n - ROWS) as f32;
        Some((x0 + w - 5.0, TOP, h, ty, th))
    }

    /// Scrollbar drag: the thumb top follows `y`; the cursor is pulled into view.
    pub fn drag_to(&mut self, page: Page, y: f32, grab: f32) {
        let Some((_, top, h, _, th)) = self.scrollbar(page) else { return };
        let max = self.rows(page) - ROWS;
        let f = ((y - grab - top) / (h - th).max(1.0)).clamp(0.0, 1.0);
        self.set_scroll(page, (f * max as f32).round() as usize);
    }

    /// Wheel: scrolls the view by rows, keeping the cursor visible.
    pub fn wheel(&mut self, page: Page, up: bool) {
        let sc = match page {
            Page::List => self.scroll,
            Page::Maps => self.map_scroll,
            Page::Online => self.online.scroll,
        };
        self.set_scroll(page, if up { sc.saturating_sub(3) } else { sc + 3 });
    }

    fn set_scroll(&mut self, page: Page, sc: usize) {
        let sc = sc.min(self.rows(page).saturating_sub(ROWS));
        let (cursor, scroll) = match page {
            Page::List => (&mut self.cursor, &mut self.scroll),
            Page::Maps => (&mut self.map_cursor, &mut self.map_scroll),
            Page::Online => (&mut self.online.cursor, &mut self.online.scroll),
        };
        *scroll = sc;
        *cursor = (*cursor).clamp(sc, sc + ROWS - 1);
    }

    fn selected(&self) -> Option<&ModInfo> {
        self.mods.get(self.sel)
    }

    fn rows(&self, page: Page) -> usize {
        match page {
            Page::List => self.mod_view.len() + self.pin(),
            Page::Maps => self.map_view.len() + usize::from(self.map_filter.is_empty()),
            Page::Online => self.online.rows(),
        }
    }

    /// Row index (absolute) under a menu-space point.
    pub fn item_at(&self, page: Page, (x, y): (f32, f32)) -> Option<usize> {
        let (x0, w, scroll) = match page {
            Page::List | Page::Online => (LIST.0, LIST.1, if page == Page::List { self.scroll } else { self.online.scroll }),
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
            Page::Online => &mut self.online.cursor,
        }
    }

    fn cycle_sort(&mut self, page: Page) {
        match page {
            Page::List => {
                self.mod_sort = self.mod_sort.next();
                self.rebuild_mod_view();
            }
            Page::Maps => {
                self.map_sort = self.map_sort.next();
                self.rebuild_map_view();
            }
            Page::Online => self.online.cycle_sort(),
        }
    }

    /// Whether the active page currently has a non-empty filter query.
    pub fn has_filter(&self, page: Page) -> bool {
        match page {
            Page::List => !self.mod_filter.is_empty(),
            Page::Maps => !self.map_filter.is_empty(),
            Page::Online => self.online.has_filter(),
        }
    }

    /// Appends a character to the active page's filter (used by the on-screen keyboard).
    pub fn filter_push(&mut self, page: Page, ch: char) {
        if !(' '..='~').contains(&ch) {
            return;
        }
        match page {
            Page::List => {
                self.mod_filter.push(ch);
                self.rebuild_mod_view();
            }
            Page::Maps => {
                self.map_filter.push(ch);
                self.rebuild_map_view();
            }
            Page::Online => self.online.filter_push(ch),
        }
    }

    /// Removes the last character from the active page's filter.
    pub fn filter_pop(&mut self, page: Page) {
        match page {
            Page::List => {
                if self.mod_filter.pop().is_some() {
                    self.rebuild_mod_view();
                }
            }
            Page::Maps => {
                if self.map_filter.pop().is_some() {
                    self.rebuild_map_view();
                }
            }
            Page::Online => self.online.filter_pop(),
        }
    }

    /// Handles clicks on the search/sort bar.
    pub fn click(&mut self, page: Page, (x, y): (f32, f32), out: &mut Vec<Action>) -> bool {
        if !(BAR_Y..BAR_Y + BAR_H).contains(&y) {
            return false;
        }
        let (bx, bw, sw) = match page {
            Page::List => (LIST.0, LIST.1, SORT_W_LIST),
            Page::Maps => (MAPS.0, MAPS.1, SORT_W_MAPS),
            Page::Online => (LIST.0, LIST.1, SORT_W_ONLINE),
        };
        let sx = bx + bw - sw;
        if (sx..sx + sw).contains(&x) {
            self.cycle_sort(page);
            out.push(Action::Sound("misc/menu3.wav"));
            return true;
        }
        false
    }

    /// Returns the next page (`None` = leave the mod menu to the main menu).
    pub fn key(&mut self, page: Page, key: i32, out: &mut Vec<Action>) -> Option<Page> {
        if page == Page::Online {
            return Some(if self.online.key(key, out) { Page::Online } else { Page::List });
        }
        if matches!(key, K_TAB | K_RTRIGGER) {
            self.cycle_sort(page);
            out.push(Action::Sound("misc/menu3.wav"));
            return Some(page);
        }
        if key == K_BACKSPACE {
            let filter = if page == Page::List { &mut self.mod_filter } else { &mut self.map_filter };
            if filter.pop().is_some() {
                if page == Page::List { self.rebuild_mod_view() } else { self.rebuild_map_view() }
                out.push(Action::Sound("misc/menu1.wav"));
            }
            return Some(page);
        }
        if (0x20..=0x7e).contains(&key) {
            let ch = key as u8 as char;
            if page == Page::List {
                self.mod_filter.push(ch);
                self.rebuild_mod_view();
            } else {
                self.map_filter.push(ch);
                self.rebuild_map_view();
            }
            out.push(Action::Sound("misc/menu1.wav"));
            return Some(page);
        }
        let n = self.rows(page);
        let (cursor, scroll) =
            if page == Page::List { (&mut self.cursor, &mut self.scroll) } else { (&mut self.map_cursor, &mut self.map_scroll) };
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
            Page::List if back && !self.mod_filter.is_empty() => {
                self.mod_filter.clear();
                self.rebuild_mod_view();
                out.push(Action::Sound("misc/menu1.wav"));
                Some(Page::List)
            }
            Page::Maps if back && !self.map_filter.is_empty() => {
                self.map_filter.clear();
                self.rebuild_map_view();
                out.push(Action::Sound("misc/menu1.wav"));
                Some(Page::Maps)
            }
            Page::List if back => None,
            Page::List if enter && self.pin() == 1 && self.cursor == 0 => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command("_quaddicted\n".into()));
                Some(Page::Online)
            }
            Page::List if enter => {
                let Some((_, m)) = self.mod_at_row(self.cursor) else { return Some(page) };
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Close);
                // the running mod is already loaded: just return to it
                if !m.dir.eq_ignore_ascii_case(&self.current) {
                    out.push(Action::command("game", &[&m.dir]));
                }
                None
            }
            Page::List if matches!(key, K_RIGHTARROW | K_DPAD_RIGHT) => {
                let Some((idx, m)) = self.mod_at_row(self.cursor) else { return Some(page) };
                let cmd = Action::command("_modmaps", &[&m.dir]);
                self.sel = idx;
                self.maps.clear();
                self.map_view.clear();
                self.map_filter.clear();
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(cmd);
                Some(Page::Maps)
            }
            Page::Maps if back || matches!(key, K_LEFTARROW | K_DPAD_LEFT) => Some(Page::List),
            Page::Maps if enter => {
                let Some(m) = self.selected() else { return Some(Page::List) };
                let current = m.dir.eq_ignore_ascii_case(&self.current);
                let cmd = if self.map_filter.is_empty() && self.map_cursor == 0 {
                    // Start: another mod boots through its own quake.rc; the
                    // mounted one starts its first map.
                    if !current {
                        Action::command("game", &[&m.dir])
                    } else {
                        match self.map_view.first().and_then(|&i| self.maps.get(i)) {
                            Some(f) => Action::command("game", &[&m.dir, &f.name]),
                            None => return Some(page),
                        }
                    }
                } else {
                    let Some(map) = self.map_at_row(self.map_cursor) else { return Some(page) };
                    Action::command("game", &[&m.dir, &map.name])
                };
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Close);
                out.push(cmd);
                None
            }
            _ => Some(page),
        }
    }

    pub fn draw(&self, c: &mut Canvas, page: Page, pad: bool, glyphs: &crate::glyphs::Glyphs) {
        match page {
            Page::List => self.draw_list(c, pad, glyphs),
            Page::Maps => self.draw_maps(c, pad, glyphs),
            Page::Online => self.online.draw(c, pad, glyphs),
        }
    }

    fn draw_list(&self, c: &mut Canvas, pad: bool, glyphs: &crate::glyphs::Glyphs) {
        center_gold(c, 24.0, "Map Packs");
        let count = if self.mod_filter.is_empty() {
            qcore::tr!("{} installed", self.mods.len())
        } else {
            qcore::tr!("{}/{} matching", self.mod_view.len(), self.mods.len())
        };
        center(c, 36.0, &count);
        let (x, w) = LIST;
        draw_filter_bar(c, x, w, SORT_W_LIST, &self.mod_filter, self.mod_sort.label(), pad, glyphs);
        box_(c, x, w, ROWS);
        for i in (self.scroll..self.rows(Page::List)).take(ROWS) {
            let y = TOP + (i - self.scroll) as f32 * ROW_H;
            if i == self.cursor {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            let Some((_, m)) = self.mod_at_row(i) else {
                txt_gold(c, x + 6.0, y + 1.0, "Get more maps (Quaddicted)...");
                continue;
            };
            let cols = ((w - 60.0) / 6.0) as usize;
            let t = clip(&m.title, cols);
            if m.dir.eq_ignore_ascii_case(&self.current) {
                txt_gold(c, x + 6.0, y + 1.0, &t);
                c.text_right(x + w - 8.0, y + 1.0, "running", TXT, MUTED);
            } else if is_official_expansion(&m.dir) {
                c.text(x + 6.0, y + 1.0, &t, TXT, OFFICIAL_FG);
            } else {
                txt(c, x + 6.0, y + 1.0, &t);
            }
        }
        self.draw_bar(c, Page::List);
        // info panel for the highlighted mod
        let (ix, iw) = INFO;
        let Some((_, m)) = self.mod_at_row(self.cursor) else {
            box_(c, ix, iw, ROWS);
            let mut y = TOP + 1.0;
            for l in [qcore::t!("Browse the Quaddicted archive of"), qcore::t!("single-player maps and episodes,"), qcore::t!("then download and install them"), qcore::t!("here. Each release gets its own"), qcore::t!("entry in this list."), "", qcore::t!("Needs an internet connection.")] {
                txt(c, ix + 8.0, y, l);
                y += ROW_H;
            }
            return;
        };
        box_(c, ix, iw, ROWS);
        let cols = ((iw - 16.0) / 6.0) as usize;
        let mut y = TOP + 1.0;
        for l in wrap(&m.title, cols).iter().take(2) {
            txt_gold(c, ix + 8.0, y, l);
            y += ROW_H;
        }
        y += 4.0;
        txt(c, ix + 8.0, y, &clip(&qcore::tr!("dir: {}", m.dir), cols));
        y += ROW_H;
        if let Some(b) = m.base.dir() {
            txt(c, ix + 8.0, y, &qcore::tr!("base: {}", b));
            y += ROW_H;
        }
        if m.dir.eq_ignore_ascii_case(&self.current) {
            txt_gold(c, ix + 8.0, y, "(running)");
            y += ROW_H;
        }
        y += 6.0;
        let bottom = TOP + ROWS as f32 * ROW_H - ROW_H;
        'blurb: for line in &m.blurb {
            for l in wrap(line, cols) {
                if y > bottom {
                    break 'blurb;
                }
                txt(c, ix + 8.0, y, &l);
                y += ROW_H;
            }
        }
    }

    fn draw_maps(&self, c: &mut Canvas, pad: bool, glyphs: &crate::glyphs::Glyphs) {
        let Some(m) = self.selected() else { return };
        center_gold(c, 24.0, &clip(&m.title, 70));
        let done = self.maps.iter().filter(|x| self.stats.get(&x.name).is_some_and(|s| s.completions > 0)).count();
        let subtitle = if self.map_filter.is_empty() {
            qcore::tr!("{} maps, {} completed", self.maps.len(), done)
        } else {
            qcore::tr!("{}/{} matching, {} completed", self.map_view.len(), self.maps.len(), done)
        };
        center(c, 36.0, &subtitle);
        let (x, w) = MAPS;
        draw_filter_bar(c, x, w, SORT_W_MAPS, &self.map_filter, self.map_sort.label(), pad, glyphs);
        box_(c, x, w, ROWS);
        let stat_x = x + w - 176.0;
        let total = self.rows(Page::Maps);
        for i in (self.map_scroll..total).take(ROWS) {
            let y = TOP + (i - self.map_scroll) as f32 * ROW_H;
            if i == self.map_cursor {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            if self.map_filter.is_empty() && i == 0 {
                let running = m.dir.eq_ignore_ascii_case(&self.current);
                txt_gold(c, x + 6.0, y + 1.0, if running { "Start first map" } else { "Launch mod" });
                continue;
            }
            let Some(map) = self.map_at_row(i) else { continue };
            txt(c, x + 6.0, y + 1.0, &clip(&map.name, 14));
            let tcols = ((stat_x - x - 128.0) / 6.0) as usize;
            txt(c, x + 120.0, y + 1.0, &clip(&map.title, tcols));
            if let Some(s) = self.stats.get(&map.name) {
                let k = format!("K{:>3}/{:<3}", s.kills, s.total_kills);
                let sec = format!("S{:>2}/{:<2}", s.secrets, s.total_secrets);
                let t = s.best_time as u32;
                let parts = [(k, s.all_kills()), (sec, s.all_secrets()), (format!("{:>2}:{:02}", t / 60, t % 60), false)];
                let mut sx = stat_x;
                for (t, full) in parts {
                    c.text(sx, y + 1.0, &t, TXT, if full { GOLD_FG } else { FG });
                    sx += 60.0;
                }
            }
        }
        if self.maps.is_empty() {
            center(c, TOP + 2.0 * ROW_H + 1.0, "no maps of its own");
        }
        self.draw_bar(c, Page::Maps);
    }

    fn draw_bar(&self, c: &mut Canvas, page: Page) {
        if let Some((x, top, h, ty, th)) = self.scrollbar(page) {
            c.fill(x, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_filter_bar(c: &mut Canvas, x: f32, w: f32, sort_w: f32, filter: &str, sort_label: &str, pad: bool, glyphs: &crate::glyphs::Glyphs) {
    filter_bar_at(c, x, BAR_Y, w, sort_w, filter, sort_label, pad.then_some((Some(K_YBUTTON), Some(K_RTRIGGER), glyphs)));
}

/// The type-to-filter box and the sort button at its right, `BAR_H` tall at `y`.
/// `pad_glyphs` optionally supplies `(filter_key, sort_key, glyphs)` to draw
/// controller glyphs directly inside those controls.
#[allow(clippy::too_many_arguments)]
pub(crate) fn filter_bar_at(
    c: &mut Canvas,
    x: f32,
    y: f32,
    w: f32,
    sort_w: f32,
    filter: &str,
    sort_label: &str,
    pad_glyphs: Option<(Option<i32>, Option<i32>, &crate::glyphs::Glyphs)>,
) {
    let fw = w - sort_w - 4.0;
    c.fill(x, y, fw, BAR_H, [0.0, 0.0, 0.0, 0.8]);
    c.frame(x, y, fw, BAR_H, BORDER);
    let mut tx = x + 6.0;
    if let Some((Some(fk), _, glyphs)) = pad_glyphs {
        let gh = crate::glyphs::H.min(BAR_H - 2.0);
        let gw = glyphs.key_h(c, x + 3.0, y + (BAR_H - gh) / 2.0, fk, gh);
        tx = x + 3.0 + gw + 3.0;
    }
    let cols = (((x + fw - tx - 4.0).max(6.0)) / 6.0) as usize;
    if filter.is_empty() {
        c.text(tx, y + 2.0, "Type to filter...", TXT, MUTED);
    } else {
        txt(c, tx, y + 2.0, &clip(&format!("{filter}_"), cols));
    }
    let sx = x + w - sort_w;
    c.fill(sx, y, sort_w, BAR_H, [0.08, 0.08, 0.08, 0.9]);
    c.frame(sx, y, sort_w, BAR_H, BORDER);
    let label = qcore::tr!("Sort: {}", qcore::lang::tr(sort_label));
    if let Some((_, Some(sk), glyphs)) = pad_glyphs {
        glyphs.button_label_raw(c, [sx, y, sort_w, BAR_H], Some(sk), &label, TXT, GOLD_FG);
    } else {
        c.text_center(sx + sort_w * 0.5, y + 2.0, &label, TXT, GOLD_FG);
    }
}

const OFFICIAL_EXPANSIONS: &[&str] = &["hipnotic", "rogue", "dopa", "mg1", "mg3"];

fn is_official_expansion(dir: &str) -> bool {
    let d = dir.to_ascii_lowercase();
    OFFICIAL_EXPANSIONS.contains(&d.as_str())
}

fn mod_fallback_key(m: &ModInfo) -> (u8, usize, String) {
    let d = m.dir.to_ascii_lowercase();
    if d == "id1" {
        return (0, 0, String::new());
    }
    if let Some(pos) = OFFICIAL_EXPANSIONS.iter().position(|&e| e == d) {
        return (1, pos, String::new());
    }
    (2, 0, m.title.to_ascii_lowercase())
}

/// Case-insensitive fuzzy subsequence score; `None` if `query` does not match.
pub(crate) fn fuzzy_score(query: &str, target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
    let t: Vec<char> = target.chars().map(|c| c.to_ascii_lowercase()).collect();
    let mut qi = 0;
    let mut score = 0i32;
    let mut prev_match: Option<usize> = None;
    for (ti, &tc) in t.iter().enumerate() {
        if tc == q[qi] {
            score += 10;
            if ti > 0 && prev_match == Some(ti - 1) {
                score += 15;
            }
            if ti == 0 || matches!(t[ti - 1], ' ' | '_' | '-' | '/' | '.') {
                score += 20;
            }
            prev_match = Some(ti);
            qi += 1;
            if qi == q.len() {
                let q_str: String = q.iter().collect();
                let t_str: String = t.iter().collect();
                if t_str.contains(&q_str) {
                    score += 50;
                }
                return Some(score - t.len() as i32);
            }
        }
    }
    None
}

fn box_(c: &mut Canvas, x: f32, w: f32, rows: usize) {
    let h = rows as f32 * ROW_H + 8.0;
    c.fill(x, TOP - 4.0, w, h, [0.0, 0.0, 0.0, 0.8]);
    c.frame(x, TOP - 4.0, w, h, BORDER);
}

fn center(c: &mut Canvas, y: f32, s: &str) {
    c.text_center(320.0, y, s, TXT, MUTED);
}

fn center_gold(c: &mut Canvas, y: f32, s: &str) {
    c.text_center(320.0, y, s, 13.0, GOLD_FG);
}

/// At most `n` columns (wide CJK characters count two), with a trailing
/// `~` when cut; control characters are dropped.
pub(crate) fn clip(s: &str, n: usize) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if n == 0 || qcore::lang::wrap::cols(&s) <= n {
        return s;
    }
    let mut out = String::new();
    for ch in s.chars() {
        if qcore::lang::wrap::cols(&out) + qcore::lang::wrap::cols(ch.encode_utf8(&mut [0; 4])) > n - 1 {
            break;
        }
        out.push(ch);
    }
    out + "~"
}

/// Greedy word wrap to `n` columns (long words are split; Chinese and
/// Japanese break between characters).
fn wrap(s: &str, n: usize) -> Vec<String> {
    let s = clip(s, usize::MAX);
    let mut v = qcore::lang::wrap(&s, n as f32, |t| qcore::lang::wrap::cols(t) as f32);
    v.retain(|l| !l.is_empty());
    v
}

#[cfg(test)]
mod tests;
