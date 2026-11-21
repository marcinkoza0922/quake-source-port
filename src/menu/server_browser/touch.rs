//! The server browser as rows for the touch layout (`menu::touchview`).
//! Its filters page keeps the classic layout.

use super::*;

impl Menu {
    /// The filters page is open (classic layout).
    pub(in crate::menu) fn server_filters_open(&self) -> bool {
        self.server_browser.filter_cursor.is_some()
    }

    pub(in crate::menu) fn tv_server_rows(&self) -> Vec<ListRow> {
        let list = self.server_browser.current();
        list.visible
            .iter()
            .map(|&i| {
                let e = &list.servers[i];
                let host = if e.hostname.is_empty() { &e.address } else { &e.hostname };
                let players = if e.ping_ms.is_some() { format!("{}/{}", e.players, e.max_players) } else { "—".into() };
                let ping = e.ping_ms.map_or_else(|| "—".into(), |ms| format!("{ms} ms"));
                ListRow {
                    title: if e.favorite { format!("* {host}") } else { host.clone() },
                    sub: format!("{}  ·  {}  ·  {}", e.map, e.protocol.tag(), e.effective_gamedir()),
                    right: format!("{players}   {ping}"),
                    color: if e.favorite { GOLD_FG } else { FG },
                    right_color: MUTED,
                }
            })
            .collect()
    }

    pub(in crate::menu) fn tv_server_cursor(&self) -> usize {
        self.server_browser.current().cursor
    }

    pub(in crate::menu) fn tv_server_select(&mut self, row: usize) {
        let list = self.server_browser.current_mut();
        if row < list.visible.len() {
            list.cursor = row;
        }
    }

    /// Tab labels and the open one.
    pub(in crate::menu) fn tv_server_tabs(&self) -> (Vec<String>, usize) {
        (vec!["LAN".into(), qcore::lang::tr("Internet").into_owned()], self.server_browser.tab.index())
    }

    pub(in crate::menu) fn tv_server_filter(&self) -> &str {
        &self.server_browser.current().filter.query
    }

    pub(in crate::menu) fn tv_server_sort(&self) -> &'static str {
        SORT_LABELS[self.server_browser.current().filter.sort]
    }

    /// Count or search status under the title.
    pub(in crate::menu) fn tv_server_sub(&self) -> String {
        let list = self.server_browser.current();
        match (list.status, list.servers.is_empty()) {
            (SearchStatus::Searching, true) => qcore::lang::tr("Looking for Quake servers...").into_owned(),
            (SearchStatus::Failed, _) => qcore::lang::tr("Server search failed").into_owned(),
            (SearchStatus::Searching, false) => qcore::lang::tr("Searching...").into_owned(),
            (_, true) => qcore::lang::tr("No servers found").into_owned(),
            (_, false) => qcore::tr!("{}/{} matching", list.visible.len(), list.servers.len()),
        }
    }

    pub(in crate::menu) fn tv_server_info(&self) -> Vec<(String, [f32; 4])> {
        let Some(e) = self.server_browser.current().selected() else { return Vec::new() };
        let host = if e.hostname.is_empty() { e.address.clone() } else { e.hostname.clone() };
        vec![
            (host, GOLD_FG),
            (e.connect_address(), FG),
            (format!("{}  ·  {}  ·  {}", e.map, e.effective_gamedir(), e.mode.tag()), FG),
        ]
    }

    /// Buttons: (label, key the classic browser takes).
    pub(in crate::menu) fn tv_server_actions(&self) -> Vec<(&'static str, i32)> {
        let mut out = Vec::new();
        if self.selected_server_address().is_some() {
            out.push(("Connect", K_ENTER));
            out.push((qcore::t!("Favorites"), FAVORITE_KEY));
        }
        if self.server_browser.current().status != SearchStatus::Searching {
            out.push((qcore::t!("Refresh"), REFRESH_KEY));
        }
        if self.server_browser.tab == ServerTab::Internet {
            out.push((qcore::t!("Filters"), FILTER_KEY));
        }
        out
    }

    /// Tab key: LAN or Internet.
    pub(in crate::menu) fn tv_server_tab_key(t: usize) -> i32 {
        if t == 0 { K_LSHOULDER } else { K_RSHOULDER }
    }
}
