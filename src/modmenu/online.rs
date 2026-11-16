//! Map Packs > "Get more maps": the Quaddicted archive browser. Lists the
//! archive's single-player releases (filter by typing, Tab sorts), shows the
//! highlighted one in the info panel, and Enter downloads and installs it, or
//! opens its map list once installed. Data and downloads: `crate::quaddicted`.

use qinput::keys::*;

use super::{INFO, LIST, ROW_H, ROWS, TOP, box_, center, center_gold, clamp_scroll, clip, draw_filter_bar, fuzzy_score, wrap};
use crate::draw::Canvas;
use crate::menu::{Action, FG, GOLD_FG, HILITE, MUTED, TXT, txt, txt_gold};
use crate::quaddicted::{Job, Online, Release, Status};

const WARN_FG: [f32; 4] = [0.95, 0.55, 0.35, 1.0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    Newest,
    Title,
    Author,
    Size,
}

impl Sort {
    fn next(self) -> Self {
        match self {
            Self::Newest => Self::Title,
            Self::Title => Self::Author,
            Self::Author => Self::Size,
            Self::Size => Self::Newest,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest",
            Self::Title => "Title",
            Self::Author => "Author",
            Self::Size => "Size",
        }
    }
}

#[derive(Default)]
pub struct OnlinePage {
    pub data: Online,
    pub cursor: usize,
    pub scroll: usize,
    view: Vec<usize>,
    filter: String,
    sort: Sort,
}

impl OnlinePage {
    /// Takes a freshly loaded index from the worker (call once per frame).
    pub fn poll(&mut self) {
        if self.data.poll() {
            self.rebuild();
        }
    }

    pub fn rows(&self) -> usize {
        self.view.len()
    }

    fn at(&self, row: usize) -> Option<&Release> {
        self.view.get(row).and_then(|&i| self.data.releases.get(i))
    }

    fn rebuild(&mut self) {
        let q = self.filter.trim();
        let rs = &self.data.releases;
        let mut scored: Vec<(usize, i32)> = rs
            .iter()
            .enumerate()
            .filter(|(_, r)| r.singleplayer)
            .filter_map(|(i, r)| Some((i, [fuzzy_score(q, &r.title), fuzzy_score(q, &r.author), fuzzy_score(q, &r.stem)].into_iter().flatten().max()?)))
            .collect();
        scored.sort_by(|&(ia, sa), &(ib, sb)| {
            if !q.is_empty() && sa != sb {
                return sb.cmp(&sa);
            }
            let (a, b) = (&rs[ia], &rs[ib]);
            let low = |s: &str| s.to_ascii_lowercase();
            match self.sort {
                Sort::Newest => ia.cmp(&ib),
                Sort::Title => low(&a.title).cmp(&low(&b.title)),
                Sort::Author => low(&a.author).cmp(&low(&b.author)),
                Sort::Size => b.bytes.cmp(&a.bytes),
            }
            .then_with(|| ia.cmp(&ib))
        });
        self.view = scored.into_iter().map(|(i, _)| i).collect();
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.rebuild();
    }

    /// Returns false to leave the page (back to the mod list).
    pub fn key(&mut self, key: i32, out: &mut Vec<Action>) -> bool {
        if key == K_TAB {
            self.cycle_sort();
            out.push(Action::Sound("misc/menu3.wav"));
            return true;
        }
        if key == K_F1 + 4 {
            out.push(Action::Command("_quaddicted force\n".into()));
            out.push(Action::Sound("misc/menu3.wav"));
            return true;
        }
        if key == K_BACKSPACE || (0x20..=0x7e).contains(&key) {
            let changed = if key == K_BACKSPACE {
                self.filter.pop().is_some()
            } else {
                self.filter.push(key as u8 as char);
                true
            };
            if changed {
                self.rebuild();
                out.push(Action::Sound("misc/menu1.wav"));
            }
            return true;
        }
        let n = self.rows();
        let step = match key {
            K_UPARROW | K_DPAD_UP | K_MWHEELUP => -1,
            K_DOWNARROW | K_DPAD_DOWN | K_MWHEELDOWN => 1,
            K_PGUP | K_LSHOULDER => -(ROWS as i32 - 1),
            K_PGDN | K_RSHOULDER => ROWS as i32 - 1,
            K_HOME => -(n as i32),
            K_END => n as i32,
            _ => 0,
        };
        if step != 0 {
            if n > 0 {
                let old = self.cursor;
                self.cursor = if matches!(key, K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN) {
                    (self.cursor as i32 + step).rem_euclid(n as i32) as usize
                } else {
                    (self.cursor as i32 + step).clamp(0, n as i32 - 1) as usize
                };
                clamp_scroll(self.cursor, &mut self.scroll);
                if self.cursor != old {
                    out.push(Action::Sound("misc/menu1.wav"));
                }
            }
            return true;
        }
        if key == K_ESCAPE && !self.filter.is_empty() {
            self.filter.clear();
            self.rebuild();
            out.push(Action::Sound("misc/menu1.wav"));
            return true;
        }
        if matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2 | K_LEFTARROW | K_DPAD_LEFT) {
            // new installs show up in the rescanned mod list
            out.push(Action::Command("_mods\n".into()));
            return false;
        }
        if matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1) {
            let Some(r) = self.at(self.cursor).cloned() else { return true };
            if let Some(dir) = self.data.installed(&r.sha) {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("menu_mods \"{dir}\"\n")));
            } else if !matches!(self.data.job(&r.sha), Some(Job::Downloading(_) | Job::Installing)) {
                out.push(Action::Sound("misc/menu2.wav"));
                self.data.download(&r);
            }
        }
        true
    }

    /// One-word state of a release for the list and info panel.
    fn state(&self, r: &Release) -> Option<(String, [f32; 4])> {
        match self.data.job(&r.sha) {
            Some(Job::Downloading(f)) => return Some((format!("{:>3}%", (f * 100.0) as u32), GOLD_FG)),
            Some(Job::Installing) => return Some(("unpacking".into(), GOLD_FG)),
            Some(Job::Failed(_)) => return Some(("failed".into(), WARN_FG)),
            None => {}
        }
        self.data.installed(&r.sha).map(|_| ("installed".to_string(), GOLD_FG))
    }

    pub fn draw(&self, c: &mut Canvas) {
        center_gold(c, 24.0, "Quaddicted Map Archive");
        let sub = match self.data.status() {
            Status::Idle | Status::Loading => "Loading the map list...".to_string(),
            Status::Failed(e) => format!("Couldn't load the map list: {}", clip(&e, 50)),
            Status::Ready if self.filter.is_empty() => format!("{} single-player releases  -  F5 refreshes", self.view.len()),
            Status::Ready => format!("{} matching", self.view.len()),
        };
        center(c, 36.0, &sub);
        let (x, w) = LIST;
        draw_filter_bar(c, x, w, 88.0, &self.filter, self.sort.label());
        box_(c, x, w, ROWS);
        for i in (self.scroll..self.rows()).take(ROWS) {
            let Some(r) = self.at(i) else { continue };
            let y = TOP + (i - self.scroll) as f32 * ROW_H;
            if i == self.cursor {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            }
            let (tag, col) = self.state(r).unwrap_or_else(|| (r.date.get(..4).unwrap_or("").to_string(), MUTED));
            txt(c, x + 6.0, y + 1.0, &clip(&r.title, ((w - 76.0) / 6.0) as usize));
            c.text_right(x + w - 10.0, y + 1.0, &tag, TXT, col);
        }
        if let Some((bx, top, h, ty, th)) = self.scrollbar() {
            c.fill(bx, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(bx, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
        if let Some(r) = self.at(self.cursor) {
            self.draw_info(c, r);
        }
    }

    fn draw_info(&self, c: &mut Canvas, r: &Release) {
        let (ix, iw) = INFO;
        box_(c, ix, iw, ROWS);
        let cols = ((iw - 16.0) / 6.0) as usize;
        let mut y = TOP + 1.0;
        let line = |c: &mut Canvas, s: &str, col: [f32; 4], y: &mut f32| {
            c.text(ix + 8.0, *y, &clip(s, cols), TXT, col);
            *y += ROW_H;
        };
        for l in wrap(&r.title, cols).iter().take(2) {
            txt_gold(c, ix + 8.0, y, l);
            y += ROW_H;
        }
        y += 4.0;
        line(c, &format!("by {}", r.author), FG, &mut y);
        line(c, &format!("{}  {:.1} MB  {}", r.date, r.bytes as f64 / 1e6, r.kinds.join(", ")), MUTED, &mut y);
        if !r.deps.is_empty() {
            line(c, &format!("requires: {}", r.deps.join(", ")), WARN_FG, &mut y);
        }
        if !r.startmaps.is_empty() {
            line(c, &format!("maps: {}", r.startmaps.join(" ")), MUTED, &mut y);
        }
        let action = match (self.data.job(&r.sha), self.data.installed(&r.sha)) {
            (Some(Job::Failed(e)), _) => (format!("Failed: {e}"), WARN_FG),
            (Some(Job::Downloading(f)), _) => (format!("Downloading... {:.0}%", f * 100.0), GOLD_FG),
            (Some(Job::Installing), _) => ("Unpacking...".into(), GOLD_FG),
            (None, Some(dir)) => (format!("Installed in {dir}/  -  Enter: play"), GOLD_FG),
            (None, None) => ("Enter: download and install".into(), GOLD_FG),
        };
        y += 4.0;
        for l in wrap(&action.0, cols).iter().take(3) {
            line(c, l, action.1, &mut y);
        }
        y += 6.0;
        let bottom = TOP + ROWS as f32 * ROW_H - ROW_H;
        for l in wrap(&r.description, cols) {
            if y > bottom {
                break;
            }
            txt(c, ix + 8.0, y, &l);
            y += ROW_H;
        }
    }

    /// (x, track top, track h, thumb y, thumb h).
    pub fn scrollbar(&self) -> Option<(f32, f32, f32, f32, f32)> {
        let n = self.rows();
        if n <= ROWS {
            return None;
        }
        let h = ROWS as f32 * ROW_H;
        let th = (h * ROWS as f32 / n as f32).max(8.0);
        let ty = TOP + (h - th) * self.scroll as f32 / (n - ROWS) as f32;
        Some((LIST.0 + LIST.1 - 5.0, TOP, h, ty, th))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(title: &str, date: &str) -> Release {
        Release { sha: title.into(), title: title.into(), date: date.into(), singleplayer: true, stem: title.into(), ..Default::default() }
    }

    #[test]
    fn filters_and_sorts() {
        let mut p = OnlinePage::default();
        p.data.releases = vec![rel("Zeta", "2024-01-01"), rel("Alpha", "2020-01-01"), Release { singleplayer: false, ..rel("DM", "2025") }];
        p.rebuild();
        assert_eq!(p.rows(), 2, "deathmatch-only releases are hidden");
        assert_eq!(p.at(0).unwrap().title, "Zeta");
        let mut out = Vec::new();
        p.key(K_TAB, &mut out);
        assert_eq!(p.at(0).unwrap().title, "Alpha");
        p.key(b'z' as i32, &mut out);
        assert_eq!((p.rows(), p.at(0).unwrap().title.as_str()), (1, "Zeta"));
        assert!(p.key(K_ESCAPE, &mut out), "Escape clears the filter first");
        assert!(!p.key(K_ESCAPE, &mut out));
    }
}
