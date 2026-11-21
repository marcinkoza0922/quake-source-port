//! The mod menu pages as rows for the touch layout (`menu::touchview`):
//! the same lists, filters and info panels as the classic pages draw.

use super::*;
use crate::menu::ListRow;

const ROW_FG: [f32; 4] = FG;

impl ModMenu {
    /// Title and count line of a page.
    pub fn tv_heading(&self, page: Page) -> (String, String) {
        match page {
            Page::List => (
                qcore::lang::tr("Map Packs").into_owned(),
                if self.mod_filter.is_empty() {
                    qcore::tr!("{} installed", self.mods.len())
                } else {
                    qcore::tr!("{}/{} matching", self.mod_view.len(), self.mods.len())
                },
            ),
            Page::Maps => {
                let title = self.selected().map_or_else(String::new, |m| m.title.clone());
                let done = self.maps.iter().filter(|x| self.stats.get(&x.name).is_some_and(|s| s.completions > 0)).count();
                let sub = if self.map_filter.is_empty() {
                    qcore::tr!("{} maps, {} completed", self.maps.len(), done)
                } else {
                    qcore::tr!("{}/{} matching, {} completed", self.map_view.len(), self.maps.len(), done)
                };
                (title, sub)
            }
            Page::Online => (qcore::lang::tr("Quaddicted Map Archive").into_owned(), self.online.tv_sub()),
        }
    }

    pub fn tv_filter(&self, page: Page) -> &str {
        match page {
            Page::List => &self.mod_filter,
            Page::Maps => &self.map_filter,
            Page::Online => self.online.tv_filter(),
        }
    }

    pub fn tv_sort(&self, page: Page) -> &'static str {
        match page {
            Page::List => self.mod_sort.label(),
            Page::Maps => self.map_sort.label(),
            Page::Online => self.online.tv_sort(),
        }
    }

    pub fn tv_cursor(&self, page: Page) -> usize {
        match page {
            Page::List => self.cursor,
            Page::Maps => self.map_cursor,
            Page::Online => self.online.cursor,
        }
    }

    pub fn tv_rows(&self, page: Page) -> Vec<ListRow> {
        match page {
            Page::List => (0..self.rows(Page::List))
                .map(|i| match self.mod_at_row(i) {
                    None => ListRow {
                        title: qcore::lang::tr("Get more maps (Quaddicted)...").into_owned(),
                        sub: qcore::lang::tr("Needs an internet connection.").into_owned(),
                        color: GOLD_FG,
                        ..Default::default()
                    },
                    Some((_, m)) => {
                        let running = m.dir.eq_ignore_ascii_case(&self.current);
                        ListRow {
                            title: m.title.clone(),
                            sub: m.dir.clone(),
                            right: if running { qcore::lang::tr("running").into_owned() } else { String::new() },
                            color: if running { GOLD_FG } else if is_official_expansion(&m.dir) { OFFICIAL_FG } else { ROW_FG },
                            right_color: MUTED,
                        }
                    }
                })
                .collect(),
            Page::Maps => {
                let running = self.selected().is_some_and(|m| m.dir.eq_ignore_ascii_case(&self.current));
                (0..self.rows(Page::Maps))
                    .filter_map(|i| {
                        if self.map_filter.is_empty() && i == 0 {
                            let t = if running { "Start first map" } else { "Launch mod" };
                            return Some(ListRow { title: qcore::lang::tr(t).into_owned(), color: GOLD_FG, ..Default::default() });
                        }
                        let map = self.map_at_row(i)?;
                        let right = self.stats.get(&map.name).map_or_else(String::new, |s| {
                            let t = s.best_time as u32;
                            format!("K {}/{}   S {}/{}   {}:{:02}", s.kills, s.total_kills, s.secrets, s.total_secrets, t / 60, t % 60)
                        });
                        let done = self.stats.get(&map.name).is_some_and(|s| s.completions > 0);
                        Some(ListRow {
                            title: if map.title.is_empty() { map.name.clone() } else { map.title.clone() },
                            sub: map.name.clone(),
                            right,
                            color: ROW_FG,
                            right_color: if done { GOLD_FG } else { MUTED },
                        })
                    })
                    .collect()
            }
            Page::Online => self.online.tv_rows(),
        }
    }

    /// The info panel of the highlighted row.
    pub fn tv_info(&self, page: Page) -> Vec<(String, [f32; 4])> {
        match page {
            Page::List => match self.mod_at_row(self.cursor) {
                None => [
                    "Browse the Quaddicted archive of",
                    "single-player maps and episodes,",
                    "then download and install them",
                    "here. Each release gets its own",
                    "entry in this list.",
                ]
                .iter()
                .map(|l| qcore::lang::tr(l).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
                .lines()
                .map(|l| (l.to_string(), ROW_FG))
                .collect(),
                Some((_, m)) => {
                    let mut out = vec![(m.title.clone(), GOLD_FG), (qcore::tr!("dir: {}", m.dir), MUTED)];
                    if let Some(b) = m.base.dir() {
                        out.push((qcore::tr!("base: {}", b), MUTED));
                    }
                    out.extend(m.blurb.iter().map(|l| (l.clone(), ROW_FG)));
                    out
                }
            },
            Page::Maps => self.map_at_row(self.map_cursor).map_or_else(Vec::new, |map| {
                let mut out = vec![(map.title.clone(), GOLD_FG), (map.name.clone(), MUTED)];
                if let Some(s) = self.stats.get(&map.name) {
                    let t = s.best_time as u32;
                    out.push((format!("K {}/{}   S {}/{}   {}:{:02}", s.kills, s.total_kills, s.secrets, s.total_secrets, t / 60, t % 60), ROW_FG));
                }
                out
            }),
            Page::Online => self.online.tv_info(),
        }
    }

    /// Buttons for the highlighted row: (label, key the classic page takes).
    pub fn tv_actions(&self, page: Page) -> Vec<(&'static str, i32)> {
        match page {
            Page::List if self.mod_at_row(self.cursor).is_none() => vec![(qcore::t!("Select"), K_ENTER)],
            Page::List => vec![(qcore::t!("Launch mod"), K_ENTER), (qcore::t!("Level Select"), K_RIGHTARROW)],
            Page::Maps if self.map_filter.is_empty() && self.map_cursor == 0 => {
                let running = self.selected().is_some_and(|m| m.dir.eq_ignore_ascii_case(&self.current));
                vec![(if running { qcore::t!("Start first map") } else { qcore::t!("Launch mod") }, K_ENTER)]
            }
            Page::Maps => vec![(qcore::t!("Play"), K_ENTER)],
            Page::Online if self.online.rows() > 0 => vec![(qcore::t!("Select"), K_ENTER)],
            Page::Online => Vec::new(),
        }
    }
}

impl online::OnlinePage {
    fn tv_sub(&self) -> String {
        use crate::quaddicted::Status;
        match self.data.status() {
            Status::Idle | Status::Loading => qcore::tr!("Loading the map list..."),
            Status::Failed(e) => qcore::tr!("Couldn't load the map list: {}", clip(&qcore::lang::tr(&e), 50)),
            Status::Ready => qcore::tr!("{} matching", self.rows()),
        }
    }
}
