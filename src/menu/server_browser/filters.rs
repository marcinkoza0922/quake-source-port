use super::*;
use crate::net::browser::PingFilter;

const BOX: [f32; 4] = [166.0, 92.0, 308.0, 165.0];
const FIRST_ROW: f32 = BOX[1] + 26.0;
const ROW_H: f32 = 17.0;
const CHOICE_X: f32 = 321.0;
pub(super) const BACK_POS: (f32, f32) = (174.0, 233.0);
const RESET: [f32; 4] = [366.0, BACK_POS.1, 100.0, 16.0];
const PING_CHOICES: [(PingFilter, &str); 7] = [
    (PingFilter::Any, "All"),
    (PingFilter::Below(50), "<50 ms"),
    (PingFilter::Below(100), "<100 ms"),
    (PingFilter::Below(150), "<150 ms"),
    (PingFilter::Below(200), "<200 ms"),
    (PingFilter::Below(250), "<250 ms"),
    (PingFilter::Unknown, "Unknown"),
];

#[derive(Clone, Copy)]
enum FilterRow {
    Ping,
    Protocol,
    Population,
    Mode,
    Mod,
    Saved,
}

const FILTER_ROWS: [FilterRow; 6] = [
    FilterRow::Ping, FilterRow::Protocol, FilterRow::Population, FilterRow::Mode, FilterRow::Mod, FilterRow::Saved,
];

impl FilterRow {
    fn label(self) -> &'static str {
        match self {
            Self::Ping => "Ping",
            Self::Protocol => "Protocol",
            Self::Population => "Players",
            Self::Mode => "Game Type",
            Self::Mod => "Mod",
            Self::Saved => "Status",
        }
    }

    fn choices(self, list: &ServerList) -> (usize, Vec<&str>) {
        let filter = &list.filter;
        let (index, choices): (usize, &[&str]) = match self {
            Self::Ping => {
                let index = PING_CHOICES.iter().position(|&(ping, _)| ping == filter.ping).unwrap_or(0);
                return (index, PING_CHOICES.iter().map(|&(_, label)| label).collect());
            }
            Self::Protocol => (filter.protocol, &["All", "NetQuake", "QuakeWorld", "FTE / DP"]),
            Self::Population => (filter.population, &["All", "Empty", "Populated", "Open slots", "Full"]),
            Self::Mode => (filter.mode, &["All", "Deathmatch", "Co-op", "Teamplay"]),
            Self::Saved => (filter.saved, &["All", "Favorites", "Recent"]),
            Self::Mod => {
                let mut mods: Vec<_> = list.servers.iter().map(ServerEntry::effective_gamedir).collect();
                if !filter.mod_filter.is_empty() {
                    mods.push(&filter.mod_filter);
                }
                mods.sort_unstable_by_key(|s| s.to_ascii_lowercase());
                mods.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
                let index = mods.iter().position(|s| s.eq_ignore_ascii_case(&filter.mod_filter)).map_or(0, |i| i + 1);
                mods.insert(0, "All");
                return (index, mods);
            }
        };
        (index, choices.to_vec())
    }

    fn cycle(self, list: &mut ServerList, direction: i32) {
        let (index, choices) = self.choices(list);
        let next = (index as i32 + direction).rem_euclid(choices.len() as i32) as usize;
        match self {
            Self::Ping => list.filter.ping = PING_CHOICES[next].0,
            Self::Protocol => list.filter.protocol = next,
            Self::Population => list.filter.population = next,
            Self::Mode => list.filter.mode = next,
            Self::Saved => list.filter.saved = next,
            Self::Mod => list.filter.mod_filter = if next == 0 { String::new() } else { choices[next].to_string() },
        }
        list.filter_changed();
    }
}

fn rect(row: usize) -> [f32; 4] {
    [BOX[0] + 8.0, FIRST_ROW + row as f32 * ROW_H, BOX[2] - 16.0, ROW_H]
}

pub(super) fn control_at(pos: (f32, f32)) -> Option<(usize, i32)> {
    if hit(RESET, pos) {
        return Some((FILTER_ROWS.len(), 1));
    }
    FILTER_ROWS.iter().enumerate().find_map(|(i, _)| {
        hit(rect(i), pos).then_some((i, if (CHOICE_X..CHOICE_X + 24.0).contains(&pos.0) { -1 } else { 1 }))
    })
}

impl Menu {
    pub(super) fn open_server_filters(&mut self, out: &mut Vec<Action>) {
        if self.server_browser.tab == ServerTab::Internet {
            self.server_browser.filter_cursor = Some(0);
            self.drag = None;
            out.push(Action::Sound("misc/menu2.wav"));
        }
    }

    pub(super) fn server_filter_key(&mut self, key: i32, out: &mut Vec<Action>) {
        if BACK.contains(&key) || matches!(key, FILTER_KEY | K_LTRIGGER) {
            self.server_browser.filter_cursor = None;
            out.push(Action::Sound("misc/menu3.wav"));
            return;
        }
        let cursor = self.server_browser.filter_cursor.unwrap_or_default();
        let count = FILTER_ROWS.len() + 1;
        let (row, direction) = match key {
            K_UPARROW | K_DPAD_UP => ((cursor + count - 1) % count, 0),
            K_DOWNARROW | K_DPAD_DOWN => ((cursor + 1) % count, 0),
            K_LEFTARROW | K_DPAD_LEFT if cursor < FILTER_ROWS.len() => (cursor, -1),
            K_RIGHTARROW | K_DPAD_RIGHT if cursor < FILTER_ROWS.len() => (cursor, 1),
            k if SELECT.contains(&k) => (cursor, 1),
            K_MOUSE1 => {
                let Some(pos) = self.mouse else { return };
                if let Some((x, y, w, h)) = self.back_button()
                    && hit([x, y, w, h], pos)
                {
                    return self.server_filter_key(K_ESCAPE, out);
                }
                let Some(hit) = control_at(pos) else { return };
                hit
            }
            _ => return,
        };
        self.server_browser.filter_cursor = Some(row);
        let list = self.server_browser.current_mut();
        if direction != 0 {
            if let Some(&control) = FILTER_ROWS.get(row) {
                control.cycle(list, direction);
            } else {
                let filter = &mut list.filter;
                *filter = BrowserFilter { query: std::mem::take(&mut filter.query), sort: filter.sort, ..Default::default() };
                list.filter_changed();
            }
        }
        out.push(Action::Sound(if direction == 0 { "misc/menu1.wav" } else { "misc/menu3.wav" }));
    }

    pub(super) fn draw_server_filters(&self, c: &mut Canvas, cursor: usize) {
        c.fill(0.0, 0.0, 640.0, MENU_H, [0.0, 0.0, 0.0, 0.55]);
        let [x, y, w, h] = BOX;
        c.fill(x, y, w, h, [0.05, 0.04, 0.03, 0.98]);
        c.frame(x, y, w, h, BORDER);
        c.text_center(320.0, y + 9.0, "Filters", TXT * 1.15, GOLD_FG);
        let list = self.server_browser.current();
        for (i, &control) in FILTER_ROWS.iter().enumerate() {
            let [rx, ry, rw, rh] = rect(i);
            if i == cursor {
                c.fill(rx, ry, rw, rh, HILITE);
            }
            txt(c, rx + 5.0, ry + 4.0, control.label());
            let (index, choices) = control.choices(list);
            let choice = if matches!(control, FilterRow::Mod) && index > 0 {
                std::borrow::Cow::Borrowed(choices[index])
            } else {
                qcore::lang::tr(choices[index])
            };
            let label = crate::glyphs::fit_label(c, &choice, 91.0, TXT);
            let tx = CHOICE_X + 69.0 - c.text_w_raw(&label, TXT) / 2.0;
            c.text_raw(tx, ry + 4.0, &label, TXT, if i == cursor { GOLD_FG } else { FG });
            txt(c, CHOICE_X + 5.0, ry + 4.0, "<");
            txt(c, CHOICE_X + 125.0, ry + 4.0, ">");
        }
        let reset_selected = cursor == FILTER_ROWS.len();
        self.server_button(c, RESET, "Reset", reset_selected.then_some(K_ABUTTON), reset_selected, true);
    }
}
