//! Options search and breadcrumbs. Typing on the Options hub (or Y with a
//! controller, through the on-screen keyboard) lists the settings whose
//! name, page or search words (`search_keywords.rs`) match; picking one
//! opens its page with that row selected. Page titles show the path to
//! the page, earlier pages muted.

use super::search_keywords::{keywords, page_keywords};
use super::*;
use crate::modmenu::fuzzy_score;

/// Result rows shown at once (they stay above the on-screen keyboard).
const SEARCH_ROWS: usize = 14;
/// The search field, between the hub title and its panel.
const BAR_Y: f32 = 46.0;
const BAR_H: f32 = 12.0;
/// Breadcrumb separator: gap, arrow, gap.
const SEP_W: f32 = 13.0;

/// One searchable row of an options page, or a top-level page itself.
#[derive(Clone, Copy)]
pub(super) struct Hit {
    pub(super) label: &'static str,
    /// Pages from the hub down to the page holding the row.
    path: [Option<Page>; 4],
    /// The row (`None`: the top-level page `path[0]`).
    item: Option<Item>,
}

impl Hit {
    fn pages(&self) -> impl Iterator<Item = Page> + '_ {
        self.path.iter().map_while(|p| *p)
    }

    fn page(&self) -> Page {
        self.pages().last().expect("hit without a page")
    }

    /// Opens a page (a sub-page row or a top-level page) rather than
    /// holding a value.
    fn opens(&self) -> bool {
        self.item.is_none_or(|i| matches!(i, Item::Sub { .. }))
    }
}

#[derive(Default)]
pub(super) struct Search {
    pub(super) query: String,
    pub(super) cursor: usize,
    scroll: usize,
    hits: Vec<Hit>,
}

/// Name shown for a row (`None`: rows without a fixed name).
pub(super) fn item_label(item: Item) -> Option<&'static str> {
    Some(match item {
        Item::Slider { label, .. }
        | Item::Toggle { label, .. }
        | Item::Choice { label, .. }
        | Item::Text { label, .. }
        | Item::Color { label, .. }
        | Item::Run { label, .. }
        | Item::Bind { label, .. }
        | Item::Sub { label, .. } => label,
        Item::DisplayMode => "Display Mode",
        Item::Resolution => "Resolution",
        Item::Apply => "Apply Display Changes",
        Item::Language => "Language",
        Item::UserDataDir => "User Data Directory",
        Item::Soundtrack => "Soundtrack",
        Item::HudPreset => "HUD Preset",
        Item::ShaderPreset => "Preset",
        Item::ResetBinds => "Reset to Defaults",
        Item::ResetShaderParams => "Reset Shader Settings",
        Item::Map => "Map",
        Item::Server(_) | Item::ShaderParam(_) | Item::Browser(_) => return None,
    })
}

/// Every row reachable from the Options hub, page by page; a page reached
/// from two parents is listed once, under the first.
fn index() -> Vec<Hit> {
    fn walk(path: &[Page], seen: &mut Vec<Page>, out: &mut Vec<Hit>) {
        let p = *path.last().unwrap();
        if seen.contains(&p) || path.len() > 4 {
            return;
        }
        seen.push(p);
        let mut arr = [None; 4];
        for (a, &q) in arr.iter_mut().zip(path) {
            *a = Some(q);
        }
        if path.len() == 1 {
            out.push(Hit { label: page_name(p), path: arr, item: None });
        }
        for &item in page_items(p) {
            if let Some(label) = item_label(item) {
                out.push(Hit { label, path: arr, item: Some(item) });
            }
        }
        for &item in page_items(p) {
            if let Item::Sub { page, .. } = item {
                walk(&[path, &[page]].concat(), seen, out);
            }
        }
    }
    let (mut seen, mut out) = (Vec::new(), Vec::new());
    for &(_, p) in &PAGES {
        walk(&[p], &mut seen, &mut out);
    }
    out
}

fn norm(s: &str) -> String {
    s.to_lowercase().replace("colour", "color")
}

fn words(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty())
}

/// How well `hit` matches the (normalised) query; higher is better.
/// Every query word must start a word of the row's name (best), its search
/// words, or its pages' names. Without that, a loose in-order letter match
/// on the name still counts, below every word match.
fn score(q: &str, hit: &Hit) -> Option<i32> {
    let shown = norm(&qcore::lang::tr(hit.label));
    let en = norm(hit.label);
    let kw = norm(hit.item.map_or_else(|| page_keywords(hit.page()), keywords));
    let crumbs: Vec<String> = if hit.item.is_none() {
        Vec::new()
    } else {
        hit.pages().flat_map(|p| [norm(&qcore::lang::tr(page_name(p))), norm(page_name(p))]).collect()
    };
    let starts = |s: &str, w: &str| words(s).any(|x| x.starts_with(w));
    let (mut tier, mut sum) = (3, 0);
    for w in words(q) {
        if starts(&shown, w) || starts(&en, w) {
            sum += 3;
        } else if starts(&kw, w) {
            tier = tier.min(2);
            sum += 2;
        } else if crumbs.iter().any(|c| starts(c, w)) {
            tier = tier.min(1);
            sum += 1;
        } else {
            tier = 0;
            break;
        }
    }
    let fuzzy = fuzzy_score(q, &shown).or_else(|| fuzzy_score(q, &en));
    if tier > 0 && sum > 0 {
        return Some(tier * 10_000 + sum * 100 + fuzzy.unwrap_or(0).max(0));
    }
    // a piece of the name, or an abbreviation like "mblur"
    fuzzy.filter(|_| !q.contains(' ') && q.chars().count() >= 2 && (abbrev(q, &shown) || abbrev(q, &en)))
}

/// `q` is part of `t`, or each of its letters starts a word of `t` or
/// follows the previous one ("mblur" in "motion blur", not "sens" in
/// "screen shader").
fn abbrev(q: &str, t: &str) -> bool {
    fn go(q: &[char], t: &[char], next: usize) -> bool {
        let Some(&c) = q.first() else { return true };
        (next < t.len() && next > 0 && t[next] == c && go(&q[1..], t, next + 1))
            || (next..t.len()).any(|j| t[j] == c && (j == 0 || !t[j - 1].is_alphanumeric()) && go(&q[1..], t, j + 1))
    }
    let (qc, tc): (Vec<char>, Vec<char>) = (q.chars().collect(), t.chars().collect());
    t.contains(q) || go(&qc, &tc, 0)
}

/// Rows matching `query`, best first; ties keep shallower pages first.
pub(super) fn matches(query: &str) -> Vec<Hit> {
    let q = norm(query.trim());
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(i32, usize, Hit)> =
        index().into_iter().filter_map(|h| Some((score(&q, &h)?, h.pages().count(), h))).collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|h| h.2).collect()
}

/// First page on the hit's path (below the hub page) that its parent's
/// setting switches off.
fn gate(hit: &Hit, cv: &Cvars) -> Option<usize> {
    hit.pages().enumerate().skip(1).find(|&(_, p)| sub_disabled(p, cv)).map(|(k, _)| k)
}

impl Menu {
    /// The hub shows search results instead of its pages.
    pub(super) fn searching(&self) -> bool {
        self.screen == Screen::Options && !self.search.query.trim().is_empty()
    }

    /// Rows in the hub panel.
    pub(super) fn hub_rows(&self) -> usize {
        if self.searching() { self.search.hits.len().clamp(1, SEARCH_ROWS) } else { PAGES.len() }
    }

    pub(super) fn search_scroll(&self) -> usize {
        self.search.scroll
    }

    pub(super) fn search_hits(&self) -> usize {
        self.search.hits.len()
    }

    fn set_query(&mut self, f: impl FnOnce(&mut String)) {
        f(&mut self.search.query);
        self.search.hits = matches(&self.search.query);
        self.search.cursor = 0;
        self.search.scroll = 0;
    }

    pub(super) fn search_type(&mut self, ch: char) {
        if (' '..='~').contains(&ch) && self.search.query.len() < 40 {
            self.set_query(|q| q.push(ch));
        }
    }

    pub(super) fn search_backspace(&mut self) {
        self.set_query(|q| {
            q.pop();
        });
    }

    pub(super) fn clear_search(&mut self) {
        self.search = Search::default();
    }

    /// Character typed on the on-screen keyboard: the hub's search field, a
    /// mod/map filter box, or a page's text row.
    pub(super) fn osk_char(&mut self, ch: char, cv: &mut Cvars) {
        match self.screen {
            Screen::Options => self.search_type(ch),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => self.mods.filter_push(self.mods_page(), ch),
            _ => self.text(ch, cv),
        }
    }

    /// Hub keys for the search; true when handled.
    pub(super) fn search_key(&mut self, key: i32, cv: &Cvars, out: &mut Vec<Action>) -> bool {
        match key {
            K_YBUTTON => {
                self.osk = Some(Osk::default());
                out.push(Action::Sound("misc/menu2.wav"));
                return true;
            }
            K_BACKSPACE if !self.search.query.is_empty() => {
                self.search_backspace();
                return true;
            }
            K_ESCAPE | K_BBUTTON | K_MOUSE2 if !self.search.query.is_empty() => {
                self.clear_search();
                out.push(Action::Sound("misc/menu3.wav"));
                return true;
            }
            // printable keys type, as in the Demos and Map Packs filters (SDL
            // text input stays off outside the console, so no text events)
            0x20..=0x7e => {
                self.search_type(key as u8 as char);
                return true;
            }
            _ => {}
        }
        if !self.searching() {
            return false;
        }
        let n = self.search.hits.len();
        let s = &mut self.search;
        match key {
            _ if n == 0 => return true,
            K_UPARROW | K_DPAD_UP => s.cursor = (s.cursor + n - 1) % n,
            K_DOWNARROW | K_DPAD_DOWN => s.cursor = (s.cursor + 1) % n,
            K_PGUP => s.cursor = s.cursor.saturating_sub(SEARCH_ROWS - 1),
            K_PGDN => s.cursor = (s.cursor + SEARCH_ROWS - 1).min(n - 1),
            K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1 => {
                let hit = s.hits[s.cursor.min(n - 1)];
                self.jump(hit, cv, out);
                return true;
            }
            _ => return true,
        }
        if s.cursor < s.scroll {
            s.scroll = s.cursor;
        } else if s.cursor >= s.scroll + SEARCH_ROWS {
            s.scroll = s.cursor + 1 - SEARCH_ROWS;
        }
        out.push(Action::Sound("misc/menu1.wav"));
        true
    }

    /// First row of `p` that `f` accepts (0 when none does).
    fn find_row(&self, p: Page, f: impl Fn(Item) -> bool) -> usize {
        (0..self.page_len(p)).find(|&i| f(self.page_item(p, i))).unwrap_or(0)
    }

    /// Opens the hit's page with its row selected. A row on a page that is
    /// switched off selects that page's row on its parent instead, next to
    /// the setting that switches it on.
    fn jump(&mut self, hit: Hit, cv: &Cvars, out: &mut Vec<Action>) {
        let pages: Vec<Page> = hit.pages().collect();
        let (path, row): (&[Page], Box<dyn Fn(Item) -> bool>) = match gate(&hit, cv) {
            Some(k) => {
                let off = pages[k];
                (&pages[..k], Box::new(move |i| matches!(i, Item::Sub { page, .. } if page == off)))
            }
            None if hit.item.is_none() => (&pages[..], Box::new(|_| true)),
            None => (&pages[..], Box::new(move |i| item_label(i) == Some(hit.label))),
        };
        let page = *path.last().unwrap();
        let cursor = self.find_row(page, row);
        self.sub_return = path.len().checked_sub(2).map(|k| {
            let parent = path[k];
            (parent, page, self.find_row(parent, |i| matches!(i, Item::Sub { page: s, .. } if s == page)))
        });
        self.open(Screen::OptionsPage(page));
        self.opt_cursor = cursor;
        self.opt_scroll.set(cursor.saturating_sub(PAGE_ROWS / 2));
        if page == Page::Graphics {
            out.push(Action::Command("_vid_menu\n".into()));
        }
        out.push(Action::Sound("misc/menu2.wav"));
    }

    /// Path to `page` for its title: the menu it hangs off, then each page.
    pub(super) fn crumbs(&self, page: Page) -> Vec<&'static str> {
        let mut chain = vec![page];
        let mut up = self.sub_return.filter(|r| r.1 == page).map(|r| r.0).or_else(|| parent_page(page).map(|r| r.0));
        while let Some(p) = up.filter(|p| !chain.contains(p)) {
            chain.push(p);
            up = parent_page(p).map(|r| r.0);
        }
        let top = *chain.last().unwrap();
        let root = if PAGES.iter().any(|x| x.1 == top) {
            Some(qcore::t!("Options"))
        } else if is_net_page(top) {
            Some(qcore::t!("Multiplayer"))
        } else if top == Page::RenameDemo {
            Some(qcore::t!("Demos"))
        } else {
            None
        };
        root.into_iter().chain(chain.iter().rev().map(|&p| page_name(p))).collect()
    }

    /// Options page title: the breadcrumb path, earlier pages muted.
    pub(super) fn draw_page_title(&self, c: &mut Canvas, page: Page) {
        let segs = self.crumbs(page);
        let max = 600.0;
        let w = trail_w(c, &segs).min(max);
        let x = if page == Page::Controls { LABEL_R - w } else { 320.0 - w / 2.0 };
        draw_trail(c, x.max(20.0), PAGE_ROW0 - 16.0, &segs, max, GOLD_FG);
    }

    pub(super) fn draw_search_bar(&self, c: &mut Canvas, realtime: f64) {
        c.fill(PANEL_X, BAR_Y, PANEL_W, BAR_H, [0.0, 0.0, 0.0, 0.8]);
        c.frame(PANEL_X, BAR_Y, PANEL_W, BAR_H, BORDER);
        let y = BAR_Y + 2.0;
        let tx = if self.pad_ui {
            let gh = crate::glyphs::H.min(BAR_H - 2.0);
            let gw = self.glyphs.key_h(c, PANEL_X + 3.0, BAR_Y + (BAR_H - gh) / 2.0, K_YBUTTON, gh);
            PANEL_X + 3.0 + gw + 3.0
        } else {
            PANEL_X + 6.0
        };
        if self.search.query.is_empty() {
            c.text(tx, y, "Type to search settings...", TXT, MUTED);
            return;
        }
        let max_w = (PANEL_X + PANEL_W - tx - 10.0).max(20.0);
        let mut q: &str = &self.search.query;
        while c.text_w_raw(q, TXT) > max_w && !q.is_empty() {
            q = &q[1..];
        }
        let tw = c.text_raw(tx, y, q, TXT, FG);
        if (realtime * 4.0) as i64 & 1 == 0 {
            c.fill(tx + 1.0 + tw, y, 1.0, 8.0, FG);
        }
    }

    pub(super) fn draw_search_results(&self, c: &mut Canvas, cv: &Cvars) {
        let hits = &self.search.hits;
        let vis = self.hub_rows();
        panel(c, ROW0, vis);
        if hits.is_empty() {
            c.text_center(320.0, ROW0 + 2.0, "No matching settings", TXT, MUTED);
            return;
        }
        let sc = self.search.scroll;
        highlight(c, ROW0 + (self.search.cursor.saturating_sub(sc)) as f32 * ROW_H);
        for (k, h) in hits.iter().enumerate().skip(sc).take(vis) {
            let y = ROW0 + (k - sc) as f32 * ROW_H + 2.0;
            let off = gate(h, cv).is_some()
                || h.item.is_some_and(|i| matches!(i, Item::Sub { page, .. } if sub_disabled(page, cv)));
            let col = if off { MUTED } else if h.opens() { GOLD_FG } else { FG };
            c.text_right(LABEL_R, y, h.label, TXT, col);
            if h.item.is_some() {
                let segs: Vec<&str> = h.pages().map(page_name).collect();
                draw_trail(c, VALUE_X, y, &segs, PANEL_X + PANEL_W - VALUE_X - 10.0, MUTED);
            } else {
                c.shape(VALUE_X, y + 1.0, 7.0, 7.0, crate::font::Shape::ArrowRight, col);
            }
        }
        if hits.len() > vis {
            let (x, h) = (PANEL_X + PANEL_W - 5.0, vis as f32 * ROW_H);
            let th = (h * vis as f32 / hits.len() as f32).max(8.0);
            let ty = ROW0 + (h - th) * sc as f32 / (hits.len() - vis) as f32;
            c.fill(x, ROW0, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
    }
}

fn trail_w(c: &Canvas, segs: &[&str]) -> f32 {
    segs.iter().map(|s| c.text_w(s, TXT)).sum::<f32>() + SEP_W * segs.len().saturating_sub(1) as f32
}

/// Breadcrumbs from `x`: every page but the last muted, separated by small
/// arrows. Leading pages that do not fit within `max_w` become "…".
fn draw_trail(c: &mut Canvas, x: f32, y: f32, segs: &[&str], max_w: f32, last: [f32; 4]) {
    let mut start = 0;
    let ell = c.text_w_raw("…", TXT) + SEP_W;
    while start + 1 < segs.len() && trail_w(c, &segs[start..]) + if start > 0 { ell } else { 0.0 } > max_w {
        start += 1;
    }
    let mut x = x;
    if start > 0 {
        x += c.text_raw(x, y, "…", TXT, MUTED);
        x = sep(c, x, y);
    }
    for (i, s) in segs[start..].iter().enumerate() {
        if i > 0 {
            x = sep(c, x, y);
        }
        let col = if start + i + 1 == segs.len() { last } else { MUTED };
        x += c.text(x, y, s, TXT, col);
    }
}

fn sep(c: &mut Canvas, x: f32, y: f32) -> f32 {
    c.shape(x + 4.0, y + 2.0, 5.0, 5.0, crate::font::Shape::ArrowRight, MUTED);
    x + SEP_W
}

#[cfg(test)]
mod tests;
