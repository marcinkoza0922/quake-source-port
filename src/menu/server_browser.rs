//! Multiplayer server lists and input. Each tab keeps its own results and selection.

use super::*;
use crate::net::browser::{BrowserFilter, SavedServers, ServerTab};

mod draw;
pub(super) use draw::server_browser_button;
mod filters;

const X: f32 = 48.0;
const W: f32 = 544.0;
const TOP: f32 = 104.0;
const PITCH: f32 = 15.0;
const ROWS: usize = 12;
const BOTTOM: f32 = TOP + ROWS as f32 * PITCH;
const ACTION_Y: f32 = 318.0;
const TABS: [[f32; 4]; 2] = [[X, 44.0, 92.0, 16.0], [X + 96.0, 44.0, 92.0, 16.0]];
const REFRESH: [f32; 4] = [X + W - 82.0, 44.0, 82.0, 16.0];
const FILTERS: [f32; 4] = [X + W - 170.0, 44.0, 84.0, 16.0];
const QUERY_Y: f32 = 66.0;
const SORT_W: f32 = 120.0;
const SORT: [f32; 4] = [X + W - SORT_W, QUERY_Y, SORT_W, modmenu::BAR_H];
const FAVORITE: [f32; 4] = [X + W - 201.0, ACTION_Y, 116.0, 16.0];
const JOIN_SERVER: [f32; 4] = [X + W - 81.0, ACTION_Y, 81.0, 16.0];
const SORT_LABELS: &[&str] = &["Ping", "Players", "Name", "Map"];
const FILTER_KEY: i32 = K_F1 + 1;
const FAVORITE_CHAR: i32 = b'*' as i32;
const FAVORITE_KEY: i32 = K_F1 + 2;
const REFRESH_KEY: i32 = K_F1 + 4;
const SELECT: [i32; 3] = [K_ENTER, K_KP_ENTER, K_ABUTTON];
const BACK: [i32; 3] = [K_ESCAPE, K_BBUTTON, K_MOUSE2];

pub(super) fn hit([x, y, w, h]: [f32; 4], (mx, my): (f32, f32)) -> bool {
    (x..x + w).contains(&mx) && (y..y + h).contains(&my)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SearchStatus {
    #[default]
    Ready,
    Searching,
    Complete,
    Failed,
}

#[derive(Default)]
pub(super) struct ServerBrowser {
    tab: ServerTab,
    tabs: [ServerList; 2],
    filter_cursor: Option<usize>,
}

impl ServerBrowser {
    fn current(&self) -> &ServerList {
        &self.tabs[self.tab.index()]
    }

    fn current_mut(&mut self) -> &mut ServerList {
        &mut self.tabs[self.tab.index()]
    }
}

#[derive(Default)]
struct ServerList {
    servers: Vec<ServerEntry>,
    visible: Vec<usize>,
    filter: BrowserFilter,
    cursor: usize,
    scroll: usize,
    status: SearchStatus,
}

impl ServerList {
    fn selected(&self) -> Option<&ServerEntry> {
        self.visible.get(self.cursor).map(|&idx| &self.servers[idx])
    }

    fn update_servers(&mut self, update: impl FnOnce(&mut Vec<ServerEntry>)) {
        let selected = self.selected().map(|entry| entry.address.clone());
        update(&mut self.servers);
        self.visible = self.filter.filter_and_sort(&self.servers);
        if let Some(address) = selected
            && let Some(row) = self.visible.iter().position(|&i| self.servers[i].address == address)
        {
            self.cursor = row;
        }
        self.reveal_cursor();
    }

    fn filter_changed(&mut self) {
        self.cursor = 0;
        self.scroll = 0;
        self.visible = self.filter.filter_and_sort(&self.servers);
    }

    fn reveal_cursor(&mut self) {
        self.cursor = self.cursor.min(self.visible.len().saturating_sub(1));
        self.scroll = self.scroll.min(self.max_scroll()).min(self.cursor)
            .max((self.cursor + 1).saturating_sub(ROWS));
    }

    fn max_scroll(&self) -> usize {
        self.visible.len().saturating_sub(ROWS)
    }

    fn set_scroll(&mut self, scroll: usize) {
        self.scroll = scroll.min(self.max_scroll());
        self.cursor = self.cursor.clamp(self.scroll, self.scroll + ROWS - 1);
    }

    fn row_at(&self, pos: (f32, f32)) -> Option<usize> {
        if !hit([X + 2.0, TOP, W - 13.0, BOTTOM - TOP], pos) {
            return None;
        }
        let row = ((pos.1 - TOP) / PITCH) as usize + self.scroll;
        (row < self.visible.len()).then_some(row)
    }

    fn scrollbar(&self) -> Option<(f32, f32, f32)> {
        let max = self.max_scroll();
        if max == 0 {
            return None;
        }
        let h = BOTTOM - TOP;
        let thumb = (h * ROWS as f32 / self.visible.len() as f32).max(8.0);
        Some((X + W - 6.0, TOP + (h - thumb) * self.scroll as f32 / max as f32, thumb))
    }

    fn drag_to(&mut self, y: f32, grab: f32) {
        if let Some((_, _, thumb)) = self.scrollbar() {
            let fraction = ((y - grab - TOP) / (BOTTOM - TOP - thumb)).clamp(0.0, 1.0);
            self.set_scroll((fraction * self.max_scroll() as f32).round() as usize);
        }
    }
}

impl Menu {
    pub fn set_server_search_status(&mut self, tab: ServerTab, status: SearchStatus) {
        self.server_browser.tabs[tab.index()].status = status;
    }

    pub fn replace_server_results(&mut self, tab: ServerTab, results: Vec<ServerEntry>, saved: &SavedServers) {
        self.server_browser.tabs[tab.index()].update_servers(|servers| {
            *servers = results;
            match tab {
                ServerTab::Lan => saved.mark_entries(servers),
                ServerTab::Internet => saved.sync_into(servers),
            }
        });
    }

    pub fn update_saved_servers(&mut self, saved: &SavedServers) {
        self.server_browser.tabs[ServerTab::Lan.index()].update_servers(|servers| saved.mark_entries(servers));
        self.server_browser.tabs[ServerTab::Internet.index()].update_servers(|servers| saved.sync_into(servers));
    }

    pub fn find_server(&self, address: &str) -> Option<&ServerEntry> {
        self.server_browser.tabs.iter()
            .flat_map(|list| &list.servers)
            .find(|entry| entry.bare_address().eq_ignore_ascii_case(address))
    }

    pub(super) fn open_server_browser(&mut self, out: &mut Vec<Action>) {
        self.open(Screen::ServerBrowser);
        self.server_browser.filter_cursor = None;
        self.refresh_server_tab(out);
    }

    fn refresh_server_tab(&self, out: &mut Vec<Action>) {
        if self.server_browser.current().status != SearchStatus::Searching {
            out.push(Action::Command(format!("{}\n", self.server_browser.tab.command())));
            out.push(Action::Sound("misc/menu2.wav"));
        }
    }

    fn switch_server_tab(&mut self, tab: ServerTab, out: &mut Vec<Action>) {
        if self.server_browser.tab != tab {
            self.server_browser.tab = tab;
            self.drag = None;
            if matches!(self.server_browser.current().status, SearchStatus::Ready | SearchStatus::Failed) {
                self.refresh_server_tab(out);
            }
            out.push(Action::Sound("misc/menu1.wav"));
        }
    }

    pub(super) fn server_browser_back_position(&self) -> (f32, f32) {
        if self.server_browser.filter_cursor.is_some() { filters::BACK_POS } else { (X, ACTION_Y) }
    }

    pub(super) fn server_browser_accepts_text(&self) -> bool {
        self.server_browser.filter_cursor.is_none() && self.osk.is_none()
    }

    pub(super) fn server_browser_type(&mut self, ch: char) {
        let list = self.server_browser.current_mut();
        if (' '..='~').contains(&ch) && list.filter.query.len() < 48 {
            list.filter.query.push(ch.to_ascii_lowercase());
            list.filter_changed();
        }
    }

    pub(super) fn server_browser_backspace(&mut self) {
        let list = self.server_browser.current_mut();
        if list.filter.query.pop().is_some() {
            list.filter_changed();
        }
    }

    pub(super) fn server_browser_hover(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        if self.osk.is_some() {
            return None;
        }
        if let Some(cursor) = &mut self.server_browser.filter_cursor {
            let (row, _) = filters::control_at(pos)?;
            let moved = *cursor != row;
            *cursor = row;
            return moved.then_some("misc/menu1.wav");
        }
        let list = self.server_browser.current_mut();
        if let Some(grab) = self.drag {
            list.drag_to(pos.1, grab);
        }
        None
    }

    pub(super) fn server_browser_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) {
        if self.server_browser.filter_cursor.is_some() {
            self.server_filter_key(key, out);
            return;
        }
        if key == K_MOUSE1 {
            if let Some(pos) = self.mouse {
                self.server_browser_click(pos, cv, out);
            }
            return;
        }
        if BACK.contains(&key) {
            let list = self.server_browser.current_mut();
            if list.filter.query.is_empty() {
                self.screen = Screen::OptionsPage(Page::Join);
            } else {
                list.filter.query.clear();
                list.filter_changed();
            }
            self.drag = None;
            out.push(Action::Sound("misc/menu3.wav"));
            return;
        }
        match key {
            K_LEFTARROW | K_DPAD_LEFT | K_LSHOULDER => self.switch_server_tab(ServerTab::Lan, out),
            K_RIGHTARROW | K_DPAD_RIGHT | K_RSHOULDER => self.switch_server_tab(ServerTab::Internet, out),
            K_TAB | K_RTRIGGER => self.server_sort_next(out),
            FILTER_KEY | K_LTRIGGER => self.open_server_filters(out),
            REFRESH_KEY | K_XBUTTON => self.refresh_server_tab(out),
            FAVORITE_KEY | K_RTHUMB | FAVORITE_CHAR => self.favorite_selected(out),
            K_YBUTTON => {
                self.osk = Some(Osk::default());
                out.push(Action::Sound("misc/menu2.wav"));
            }
            k if SELECT.contains(&k) => self.join_selected(cv, out),
            K_BACKSPACE => self.server_browser_backspace(),
            0x20..=0x7e => self.server_browser_type(key as u8 as char),
            K_MWHEELUP | K_MWHEELDOWN => {
                let list = self.server_browser.current_mut();
                list.set_scroll(if key == K_MWHEELUP { list.scroll.saturating_sub(3) } else { list.scroll + 3 });
            }
            K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN | K_PGUP | K_PGDN | K_HOME | K_END => {
                let list = self.server_browser.current_mut();
                if list.visible.is_empty() {
                    return;
                }
                let old = list.cursor;
                list.cursor = match key {
                    K_UPARROW | K_DPAD_UP => old.saturating_sub(1),
                    K_DOWNARROW | K_DPAD_DOWN => old + 1,
                    K_PGUP => old.saturating_sub(ROWS),
                    K_PGDN => old + ROWS,
                    K_HOME => 0,
                    _ => list.visible.len() - 1,
                };
                list.reveal_cursor();
                if old != list.cursor {
                    out.push(Action::Sound("misc/menu1.wav"));
                }
            }
            _ => {}
        }
    }

    fn server_browser_click(&mut self, pos: (f32, f32), cv: &mut Cvars, out: &mut Vec<Action>) {
        if let Some((x, y, w, h)) = self.back_button()
            && hit([x, y, w, h], pos)
        {
            self.server_browser_key(K_ESCAPE, cv, out);
        } else if let Some(i) = TABS.iter().position(|&rect| hit(rect, pos)) {
            self.switch_server_tab(ServerTab::ALL[i], out);
        } else if hit(REFRESH, pos) {
            self.refresh_server_tab(out);
        } else if hit(SORT, pos) {
            self.server_sort_next(out);
        } else if hit(FILTERS, pos) {
            self.open_server_filters(out);
        } else if hit(FAVORITE, pos) {
            self.favorite_selected(out);
        } else if hit(JOIN_SERVER, pos) {
            self.join_selected(cv, out);
        } else {
            let list = self.server_browser.current_mut();
            if let Some((x, ty, th)) = list.scrollbar()
                && hit([x - 4.0, TOP, 10.0, BOTTOM - TOP], pos)
            {
                let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
                list.drag_to(pos.1, grab);
                self.drag = Some(grab);
            } else if let Some(row) = list.row_at(pos) {
                list.cursor = row;
                out.push(Action::Sound("misc/menu1.wav"));
            }
        }
    }

    fn server_sort_next(&mut self, out: &mut Vec<Action>) {
        let list = self.server_browser.current_mut();
        list.filter.sort = (list.filter.sort + 1) % SORT_LABELS.len();
        list.update_servers(|_| {});
        out.push(Action::Sound("misc/menu1.wav"));
    }

    fn selected_server_address(&self) -> Option<String> {
        let entry = self.server_browser.current().selected()?;
        // Saved addresses may have been entered at the console or edited on disk.
        let addr = entry.connect_address();
        (!addr.is_empty() && !addr.contains(|ch: char| ch.is_whitespace() || matches!(ch, '"' | ';')))
            .then_some(addr)
    }

    fn join_selected(&mut self, cv: &mut Cvars, out: &mut Vec<Action>) {
        if let Some(address) = self.selected_server_address() {
            cv.set("_mp_address", &address);
            self.run(Run::Connect, cv, out);
        }
    }

    fn favorite_selected(&self, out: &mut Vec<Action>) {
        if let Some(address) = self.selected_server_address() {
            out.push(Action::Command(format!("server_fav \"{address}\"\n")));
            out.push(Action::Sound("misc/menu2.wav"));
        }
    }
}

#[cfg(test)]
mod tests;
