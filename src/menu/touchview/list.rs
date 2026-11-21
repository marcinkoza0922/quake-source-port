//! Touch list screens: Map Packs (installed packs, a pack's maps, the
//! online archive), Demos, the server browser and the Help sections. Each
//! screen's own module supplies its rows, info and buttons
//! (`ListRow`); this lays them out with tabs, a filter field (the OS
//! keyboard) and a sort button above a scrolling list, and the
//! highlighted row's info and buttons beside it (below on narrow screens).
//! A tap selects a row, a second tap runs its first button; the buttons,
//! tabs and sort send the keys the classic screens take.

use super::*;

/// Row height, filter row height, sort button and info pane widths.
const LROW: f32 = 60.0;
const FIELD_H: f32 = 46.0;
const SORT_W: f32 = 180.0;
const TAB_W: f32 = 170.0;
const BTN_H: f32 = 48.0;
const WIDE: f32 = 1100.0;

pub(super) struct ListView {
    pub(super) title: String,
    pub(super) sub: String,
    /// Tab labels, the open one, and the key that opens each.
    pub(super) tabs: Vec<(String, i32)>,
    pub(super) tab: usize,
    pub(super) filter: Option<String>,
    /// Sort label and the key that cycles it.
    pub(super) sort: Option<(&'static str, i32)>,
    pub(super) rows: Vec<ListRow>,
    pub(super) cursor: Option<usize>,
    pub(super) info: Vec<(String, [f32; 4])>,
    /// Buttons for the highlighted row: (label, key).
    pub(super) actions: Vec<(&'static str, i32)>,
    /// A tap on a row opens it at once (Help).
    pub(super) opens: bool,
}

/// Where everything goes.
pub(super) struct ListGeom {
    sub_y: f32,
    pub(super) tabs: Vec<Rect>,
    pub(super) field: Option<Rect>,
    pub(super) sort: Option<Rect>,
    pub(super) list: Rect,
    info: Option<Rect>,
    pub(super) actions: Vec<Rect>,
}

impl Menu {
    fn tv_mods_page(&self) -> Option<modmenu::Page> {
        match self.screen {
            Screen::Mods => Some(modmenu::Page::List),
            Screen::ModMaps => Some(modmenu::Page::Maps),
            Screen::ModOnline => Some(modmenu::Page::Online),
            _ => None,
        }
    }

    /// The list screen showing, if any.
    pub(super) fn tv_list(&self) -> Option<ListView> {
        if let Some(p) = self.tv_mods_page() {
            let (title, sub) = self.mods.tv_heading(p);
            return Some(ListView {
                title,
                sub,
                tabs: Vec::new(),
                tab: 0,
                filter: Some(self.mods.tv_filter(p).to_string()),
                sort: Some((self.mods.tv_sort(p), K_TAB)),
                rows: self.mods.tv_rows(p),
                cursor: Some(self.mods.tv_cursor(p)),
                info: self.mods.tv_info(p),
                actions: self.mods.tv_actions(p),
                opens: false,
            });
        }
        match self.screen {
            Screen::Demos => Some(ListView {
                title: qcore::lang::tr("Demos").into_owned(),
                sub: String::new(),
                tabs: self.demos.tv_tabs().into_iter().zip([K_LEFTARROW, K_RIGHTARROW]).collect(),
                tab: self.demos.tab,
                filter: Some(self.demos.tv_filter().to_string()),
                sort: Some((self.demos.tv_sort(), K_TAB)),
                rows: self.demos.tv_rows(),
                cursor: Some(self.demos.tv_cursor()),
                info: self.demos.tv_info(),
                actions: self.demos.tv_actions(),
                opens: false,
            }),
            Screen::ServerBrowser if !self.server_filters_open() => {
                let (tabs, tab) = self.tv_server_tabs();
                Some(ListView {
                    title: qcore::lang::tr("Server Browser").into_owned(),
                    sub: self.tv_server_sub(),
                    tabs: tabs.into_iter().enumerate().map(|(t, l)| (l, Self::tv_server_tab_key(t))).collect(),
                    tab,
                    filter: Some(self.tv_server_filter().to_string()),
                    sort: Some((self.tv_server_sort(), K_TAB)),
                    rows: self.tv_server_rows(),
                    cursor: Some(self.tv_server_cursor()),
                    info: self.tv_server_info(),
                    actions: self.tv_server_actions(),
                    opens: false,
                })
            }
            Screen::Help if !self.tv.help_reading => Some(ListView {
                title: qcore::lang::tr("Help").into_owned(),
                sub: String::new(),
                tabs: Vec::new(),
                tab: 0,
                filter: None,
                sort: None,
                rows: crate::menu::help::sections()
                    .iter()
                    .map(|s| ListRow { title: s.title.clone(), color: FG, ..Default::default() })
                    .collect(),
                cursor: Some(self.help.section),
                info: Vec::new(),
                actions: Vec::new(),
                opens: true,
            }),
            _ => None,
        }
    }

    pub(super) fn tv_list_geom(&self, v: &ListView) -> ListGeom {
        let (w, h) = self.tv_size();
        let cw = (w - 2.0 * MARGIN).clamp(100.0, WIDE);
        let x0 = (w - cw) / 2.0;
        let mut y = BAR_H;
        let sub_y = y;
        if !v.sub.is_empty() {
            y += 26.0;
        }
        let mut tabs = Vec::new();
        let mut x = x0;
        // tabs share the filter's row when there is room
        let same_row = !v.tabs.is_empty() && v.filter.is_some() && cw >= 640.0;
        if !v.tabs.is_empty() {
            let tw = TAB_W.min((cw - 8.0 * (v.tabs.len() - 1) as f32) / v.tabs.len() as f32);
            for k in 0..v.tabs.len() {
                tabs.push([x0 + k as f32 * (tw + 8.0), y, tw, FIELD_H]);
            }
            if same_row {
                x = x0 + v.tabs.len() as f32 * (tw + 8.0) + 4.0;
            } else {
                y += FIELD_H + 8.0;
            }
        }
        let mut field = None;
        let mut sort = None;
        if v.filter.is_some() || v.sort.is_some() {
            let sw = if v.sort.is_some() { SORT_W.min(cw * 0.35) } else { 0.0 };
            if v.filter.is_some() {
                field = Some([x, y, x0 + cw - x - if sw > 0.0 { sw + 8.0 } else { 0.0 }, FIELD_H]);
            }
            if v.sort.is_some() {
                sort = Some([x0 + cw - sw, y, sw, FIELD_H]);
            }
            y += FIELD_H + 8.0;
        }
        let bottom = h - 8.0;
        let two_pane = cw >= 640.0 && !v.opens;
        let mut actions = Vec::new();
        let (list, info) = if two_pane {
            let lw = (cw * 0.58).floor();
            let info = [x0 + lw + 12.0, y, cw - lw - 12.0, bottom - y];
            // more than three buttons: two columns, so the info keeps room
            let cols = if v.actions.len() > 3 { 2 } else { 1 };
            let rows = v.actions.len().div_ceil(cols);
            let bw = (info[2] - 20.0 - 8.0 * (cols - 1) as f32) / cols as f32;
            let top = bottom - 8.0 - rows as f32 * (BTN_H + 8.0) + 8.0;
            for k in 0..v.actions.len() {
                let (col, row) = (k % cols, k / cols);
                actions.push([info[0] + 10.0 + col as f32 * (bw + 8.0), top + row as f32 * (BTN_H + 8.0), bw, BTN_H]);
            }
            ([x0, y, lw, bottom - y], Some(info))
        } else {
            // at most three buttons per row under the list
            let n = v.actions.len();
            let cols = n.clamp(1, 3);
            let rows = n.div_ceil(cols);
            let bar_h = rows as f32 * (BTN_H + 8.0);
            let list_h = if n > 0 { bottom - y - bar_h - 2.0 } else { bottom - y };
            let bw = (cw - 8.0 * (cols - 1) as f32) / cols as f32;
            for k in 0..n {
                let (col, row) = (k % cols, k / cols);
                actions.push([x0 + col as f32 * (bw + 8.0), bottom - bar_h + 8.0 + row as f32 * (BTN_H + 8.0), bw, BTN_H]);
            }
            ([x0, y, cw, list_h], None)
        };
        ListGeom { sub_y, tabs, field, sort, list, info, actions }
    }

    /// Rows that don't fit the list pane.
    pub(super) fn tv_list_max_scroll(&self, v: &ListView) -> f32 {
        let g = self.tv_list_geom(v);
        (v.rows.len() as f32 * LROW - g.list[3] + 8.0).max(0.0)
    }

    fn tv_list_row(&self, g: &ListGeom, i: usize) -> Rect {
        [g.list[0], g.list[1] + 4.0 + i as f32 * LROW - self.tv_scroll(), g.list[2], LROW]
    }

    pub(super) fn tv_draw_list(&self, c: &mut Canvas, v: &ListView, realtime: f64) {
        let g = self.tv_list_geom(v);
        self.tv_title(c, &v.title);
        if !v.sub.is_empty() {
            let x0 = g.list[0];
            clip_raw(c, x0 + 4.0, g.sub_y + 4.0, &v.sub, SMALL, g.list[2] + g.info.map_or(0.0, |i| i[2] + 12.0), MUTED);
        }
        for (k, (r, (label, _))) in g.tabs.iter().zip(&v.tabs).enumerate() {
            let look = if k == v.tab { Look::Primary } else if self.tv_pressed(*r) { Look::Pressed } else { Look::Plain };
            tv_button(c, *r, label, look);
        }
        if let (Some(f), Some(q)) = (g.field, &v.filter) {
            let focus = self.tv_search_focus();
            c.round_box(f[0], f[1], f[2], f[3], 6.0, [0.0, 0.0, 0.0, 0.75], Some(if focus { GOLD_FG } else { BORDER }));
            let ty = f[1] + (FIELD_H - VALUE) / 2.0;
            if q.is_empty() {
                fit_left(c, f[0] + 14.0, ty, "Type to filter...", VALUE, f[2] - 28.0, MUTED);
            } else {
                let tw = clip_raw(c, f[0] + 14.0, ty, q, VALUE, f[2] - FIELD_H - 20.0, FG);
                if focus && (realtime * 2.0) as i64 & 1 == 0 {
                    c.fill(f[0] + 15.0 + tw, ty, 1.5, VALUE, FG);
                }
                c.text_raw(f[0] + f[2] - FIELD_H + (FIELD_H - c.text_w_raw("×", 22.0)) / 2.0, f[1] + (FIELD_H - 22.0) / 2.0 - 1.0, "×", 22.0, GOLD_FG);
            }
        }
        if let (Some(r), Some((label, _))) = (g.sort, v.sort) {
            let s = qcore::tr!("Sort: {}", qcore::lang::tr(label));
            tv_button(c, r, &s, if self.tv_pressed(r) { Look::Pressed } else { Look::Plain });
        }
        // the list
        let l = g.list;
        c.round_box(l[0], l[1], l[2], l[3], 8.0, PANEL_BG, Some(BORDER));
        c.clip_rows(Some((l[1] + 2.0, l[1] + l[3] - 2.0)));
        for (i, row) in v.rows.iter().enumerate() {
            let r = self.tv_list_row(&g, i);
            if r[1] > l[1] + l[3] || r[1] + r[3] < l[1] {
                continue;
            }
            if v.cursor == Some(i) && !v.opens {
                c.fill(r[0] + 2.0, r[1], r[2] - 4.0, r[3], HILITE);
            } else if self.tv_pressed(r) {
                c.fill(r[0] + 2.0, r[1], r[2] - 4.0, r[3], PRESS);
            } else if i > 0 {
                c.fill(r[0] + 12.0, r[1], r[2] - 24.0, 1.0, LINE);
            }
            let rw = if row.right.is_empty() { 0.0 } else { c.text_w_raw(&row.right, VALUE).min(r[2] * 0.4) + 16.0 };
            let room = r[2] - 32.0 - rw - if v.opens { 24.0 } else { 0.0 };
            if row.sub.is_empty() {
                clip_raw(c, r[0] + 16.0, r[1] + (LROW - LABEL) / 2.0, &row.title, LABEL, room, row.color);
            } else {
                clip_raw(c, r[0] + 16.0, r[1] + 9.0, &row.title, LABEL, room, row.color);
                clip_raw(c, r[0] + 16.0, r[1] + 15.0 + LABEL, &row.sub, SMALL, room, MUTED);
            }
            if !row.right.is_empty() {
                clip_raw(c, r[0] + r[2] - rw, r[1] + (LROW - VALUE) / 2.0, &row.right, VALUE, rw - 16.0, row.right_color);
            }
            if v.opens {
                c.shape(r[0] + r[2] - 30.0, r[1] + LROW / 2.0 - 6.0, 12.0, 12.0, crate::font::Shape::ArrowRight, GOLD_FG);
            }
        }
        c.clip_rows(None);
        // info and buttons
        let info_lines = |c: &mut Canvas, area: Rect, max_bottom: f32| {
            let mut y = area[1] + 12.0;
            for (k, (s, col)) in v.info.iter().enumerate() {
                let size = if k == 0 { LABEL } else { VALUE };
                for line in qcore::lang::wrap(s, area[2] - 24.0, |t| c.text_w_raw(t, size)) {
                    if y + size > max_bottom {
                        return;
                    }
                    c.text_raw(area[0] + 12.0, y, &line, size, *col);
                    y += size + 6.0;
                }
                y += 4.0;
            }
        };
        if let Some(info) = g.info {
            c.round_box(info[0], info[1], info[2], info[3], 8.0, PANEL_BG, Some(BORDER));
            let top_btn = g.actions.first().map_or(info[1] + info[3] - 8.0, |r| r[1] - 8.0);
            info_lines(c, info, top_btn);
        } else if v.rows.is_empty() {
            info_lines(c, l, l[1] + l[3] - 8.0);
        }
        if v.rows.is_empty() && g.info.is_some() {
            fit_center(c, l[0] + l[2] / 2.0, l[1] + 24.0, "---", VALUE, l[2], MUTED);
        }
        for (k, (r, (label, _))) in g.actions.iter().zip(&v.actions).enumerate() {
            let look = if self.tv_pressed(*r) { Look::Pressed } else if k == 0 { Look::Primary } else { Look::Plain };
            tv_button(c, *r, label, look);
        }
    }

    pub(super) fn tv_list_click(&mut self, pos: (f32, f32), cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) {
        let Some(v) = self.tv_list() else { return };
        let g = self.tv_list_geom(&v);
        let key = |m: &mut Self, k: i32, cv: &mut Cvars, out: &mut Vec<Action>| out.extend(m.key(k, cv, server_active));
        if let Some(t) = g.tabs.iter().position(|r| inside(*r, pos)) {
            self.tv.search_focus = false;
            key(self, v.tabs[t].1, cv, out);
            self.tv.scroll = 0.0;
            return;
        }
        if let Some(f) = g.field
            && inside(f, pos)
        {
            let clear = [f[0] + f[2] - FIELD_H, f[1], FIELD_H, FIELD_H];
            if v.filter.as_ref().is_some_and(|q| !q.is_empty()) && inside(clear, pos) {
                // Back clears a filter first on every list screen
                key(self, K_ESCAPE, cv, out);
                self.tv.scroll = 0.0;
            }
            self.tv.search_focus = true;
            return;
        }
        self.tv.search_focus = false;
        if let (Some(r), Some((_, k))) = (g.sort, v.sort)
            && inside(r, pos)
        {
            key(self, k, cv, out);
            return;
        }
        if let Some(k) = g.actions.iter().position(|r| inside(*r, pos)) {
            key(self, v.actions[k].1, cv, out);
            return;
        }
        let l = g.list;
        if !inside(l, pos) {
            return;
        }
        let Some(i) = (0..v.rows.len()).find(|&i| inside(self.tv_list_row(&g, i), pos)) else { return };
        if v.opens {
            self.tv_list_select(i);
            self.tv_list_open();
            out.push(Action::Sound("misc/menu2.wav"));
        } else if v.cursor == Some(i) {
            if let Some(&(_, k)) = v.actions.first() {
                key(self, k, cv, out);
            }
        } else {
            self.tv_list_select(i);
            out.push(Action::Sound("misc/menu1.wav"));
        }
    }

    fn tv_list_select(&mut self, i: usize) {
        if let Some(p) = self.tv_mods_page() {
            *self.mods.cursor_mut(p) = i;
            return;
        }
        match self.screen {
            Screen::Demos => self.demos.tv_select(i),
            Screen::ServerBrowser => self.tv_server_select(i),
            Screen::Help if i != self.help.section => {
                self.help.section = i;
                self.help.scroll.set(0);
            }
            _ => {}
        }
    }

    /// Help: a section was tapped.
    fn tv_list_open(&mut self) {
        if self.screen == Screen::Help {
            self.tv.help_reading = true;
            self.tv.scroll = 0.0;
        }
    }

    /// The list screen has a filter field.
    pub(super) fn tv_list_filters(&self) -> bool {
        self.tv_mods_page().is_some() || self.screen == Screen::Demos || self.screen == Screen::ServerBrowser && !self.server_filters_open()
    }

    /// A character from the OS keyboard into the list's filter.
    pub(in crate::menu) fn tv_list_type(&mut self, ch: char) {
        if let Some(p) = self.tv_mods_page() {
            self.mods.filter_push(p, ch);
        } else if self.screen == Screen::Demos {
            if (' '..='~').contains(&ch) {
                self.demos.key(ch as i32);
            }
        } else if self.screen == Screen::ServerBrowser {
            self.server_browser_type(ch);
        }
        self.tv.scroll = 0.0;
    }
}
