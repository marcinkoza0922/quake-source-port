use super::*;

const LIST_COLUMNS: [(f32, &str); 5] = [
    (X + 8.0, "Name"), (X + 286.0, "Map"), (X + 384.0, "Protocol"), (X + 442.0, "Players"), (X + 500.0, "Ping"),
];

impl Menu {
    pub(in crate::menu) fn draw_server_browser_button(&self, c: &mut Canvas, y: f32, label: &str) {
        let selected = matches!(self.page_item(Page::Join, self.opt_cursor), Item::Run { run: Run::BrowseServers, .. });
        let rect = server_browser_button(y - 2.0);
        self.server_button(c, rect, label, selected.then_some(K_ABUTTON), selected, true);
    }

    pub(in crate::menu) fn draw_server_browser(&self, c: &mut Canvas) {
        let browser = &self.server_browser;
        let list = browser.current();
        c.text_center(320.0, 24.0, "Server Browser", 12.0, GOLD_FG);
        for (i, (label, key)) in [("LAN", K_LSHOULDER), (qcore::t!("Internet"), K_RSHOULDER)].into_iter().enumerate() {
            self.server_button(c, TABS[i], label, Some(key), i == browser.tab.index(), true);
        }
        if browser.tab == ServerTab::Internet {
            self.server_button(c, FILTERS, qcore::t!("Filters"), Some(K_LTRIGGER), list.filter.has_constraints(), true);
        }
        let busy = list.status == SearchStatus::Searching;
        self.server_button(c, REFRESH, qcore::t!("Refresh"), Some(K_XBUTTON), false, !busy);
        if !list.servers.is_empty() && matches!(list.status, SearchStatus::Searching | SearchStatus::Failed) {
            let status = if busy { qcore::t!("Searching...") } else { qcore::t!("Server search failed") };
            let right = if browser.tab == ServerTab::Internet { FILTERS[0] } else { REFRESH[0] };
            c.text_right(right - 8.0, 48.0, status, TXT, MUTED);
        }
        modmenu::filter_bar_at(
            c, X, QUERY_Y, W, SORT_W, &list.filter.query, SORT_LABELS[list.filter.sort],
            self.pad_ui.then_some((Some(K_YBUTTON), Some(K_RTRIGGER), &self.glyphs)),
        );

        c.fill(X, 85.0, W, BOTTOM - 85.0, [0.0, 0.0, 0.0, 0.72]);
        c.frame(X, 85.0, W, BOTTOM - 85.0, BORDER);
        c.fill(X + 1.0, 103.0, W - 2.0, 1.0, BORDER);
        for (i, (_, label)) in LIST_COLUMNS.iter().enumerate() {
            self.draw_server_cell(c, 90.0, i, &qcore::lang::tr(label), GOLD_FG);
        }
        for (i, &index) in list.visible.iter().enumerate().skip(list.scroll).take(ROWS) {
            self.draw_server_row(c, list, i, &list.servers[index]);
        }
        if list.visible.is_empty() {
            let message = match (list.status, list.servers.is_empty()) {
                (SearchStatus::Searching, true) => "Looking for Quake servers...",
                (SearchStatus::Failed, true) => qcore::t!("Server search failed"),
                (_, true) => qcore::t!("No servers found"),
                (_, false) => qcore::t!("No matching servers"),
            };
            c.text_center(320.0, 178.0, message, TXT, MUTED);
        }
        if let Some((x, ty, th)) = list.scrollbar() {
            c.fill(x, TOP, 3.0, BOTTOM - TOP, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
        if !list.servers.is_empty() {
            let count = qcore::tr!("{}/{} matching", list.visible.len(), list.servers.len());
            c.text_raw(X, 289.0, &count, TXT, MUTED);
        }
        if let Some(entry) = list.selected() {
            let details = format!("{}   /   {}   /   {}", entry.connect_address(), entry.effective_gamedir(), entry.mode.tag());
            let details = crate::glyphs::fit_label(c, &details, W, TXT);
            c.text_raw(X, 302.0, &details, TXT, FG);
        }
        let favorite = list.selected().is_some_and(|entry| entry.favorite);
        let can_join = self.selected_server_address().is_some();
        self.server_button(c, FAVORITE, qcore::t!("Favorites"), Some(K_RTHUMB), favorite, can_join);
        self.server_button(c, JOIN_SERVER, "Connect", Some(K_ABUTTON), can_join, can_join);
        if let Some(cursor) = browser.filter_cursor {
            self.draw_server_filters(c, cursor);
        }
    }

    fn draw_server_row(&self, c: &mut Canvas, list: &ServerList, row: usize, entry: &ServerEntry) {
        let y = TOP + (row - list.scroll) as f32 * PITCH;
        let hovered = self.mouse.is_some_and(|pos| list.row_at(pos) == Some(row));
        if row == list.cursor || hovered {
            let color = if row == list.cursor { HILITE } else { [0.6, 0.38, 0.12, 0.22] };
            c.fill(X + 2.0, y, W - 13.0, PITCH, color);
        }
        let hostname = if entry.hostname.is_empty() { &entry.address } else { &entry.hostname };
        let name = if entry.favorite { format!("* {hostname}") } else { hostname.clone() };
        let players = if entry.ping_ms.is_some() {
            format!("{}/{}", entry.players, entry.max_players)
        } else {
            "—".into()
        };
        let ping = entry.ping_ms.map_or_else(|| "—".into(), |ms| format!("{ms} ms"));
        let fields = [&name, &entry.map, entry.protocol.tag(), &players, &ping];
        for (i, field) in fields.into_iter().enumerate() {
            let color = if entry.favorite && i == 0 { GOLD_FG } else { FG };
            self.draw_server_cell(c, y + 3.0, i, field, color);
        }
    }

    fn draw_server_cell(&self, c: &mut Canvas, y: f32, column: usize, text: &str, color: [f32; 4]) {
        let x = LIST_COLUMNS[column].0;
        let right = LIST_COLUMNS.get(column + 1).map_or(X + W - 12.0, |col| col.0 - 7.0);
        let text = crate::glyphs::fit_label(c, text, right - x, TXT);
        c.text_raw(x, y, &text, TXT, color);
    }

    pub(super) fn server_button(&self, c: &mut Canvas, rect: [f32; 4], label: &str, key: Option<i32>, active: bool, enabled: bool) {
        let [x, y, w, h] = rect;
        let hover = enabled && !self.pad_ui && self.mouse.is_some_and(|pos| hit(rect, pos));
        let background = if active || hover { HILITE } else { [0.04, 0.03, 0.02, 0.9] };
        c.round_box(x, y, w, h, 3.0, background, Some(BORDER));
        let color = if !enabled { MUTED } else if active || hover { GOLD_FG } else { FG };
        self.glyphs.button_label(c, rect, if self.pad_ui { key } else { None }, label, TXT, color);
    }
}

pub(in crate::menu) fn server_browser_button(y: f32) -> [f32; 4] {
    [250.0, y, 140.0, ROW_H]
}
