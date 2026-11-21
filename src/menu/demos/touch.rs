//! The Demos screen as rows for the touch layout (`menu::touchview`).

use super::*;
use crate::demo::playback::format_time;
use crate::menu::{FG, GOLD_FG, ListRow, MUTED};

impl DemoBrowser {
    pub fn tv_rows(&self) -> Vec<ListRow> {
        (0..self.rows())
            .filter_map(|v| {
                let d = self.row(v)?;
                Some(ListRow {
                    title: label(d),
                    sub: d.map.clone().unwrap_or_default(),
                    right: d.duration.map(format_time).unwrap_or_default(),
                    color: FG,
                    right_color: MUTED,
                })
            })
            .collect()
    }

    pub fn tv_cursor(&self) -> usize {
        self.cursor[self.tab]
    }

    /// Selects row `v` of the current tab.
    pub fn tv_select(&mut self, v: usize) {
        if v < self.rows() {
            self.cursor[self.tab] = v;
        }
    }

    /// Tab names with their counts.
    pub fn tv_tabs(&self) -> Vec<String> {
        TABS.iter()
            .enumerate()
            .map(|(t, name)| {
                let (shown, all) = (self.view[t].len(), self.lists[t].len());
                if self.filter.is_empty() {
                    qcore::tr!("{} ({})", qcore::lang::tr(name), all)
                } else {
                    qcore::tr!("{} ({}/{})", qcore::lang::tr(name), shown, all)
                }
            })
            .collect()
    }

    pub fn tv_filter(&self) -> &str {
        &self.filter
    }

    pub fn tv_sort(&self) -> &'static str {
        self.sort.label()
    }

    /// The info panel's text, or the note of an empty list.
    pub fn tv_info(&self) -> Vec<(String, [f32; 4])> {
        let Some(d) = self.selected() else {
            let note = if !self.filter.is_empty() && !self.lists[self.tab].is_empty() {
                qcore::t!("No demos match the filter. Backspace or Esc clears it.")
            } else if self.tab == 0 {
                qcore::t!("No saved demos yet. Record one with the console command 'record <name>', or keep an automatic demo.")
            } else if self.auto_on {
                qcore::t!("No automatic demos yet. Each level you play is recorded here.")
            } else {
                qcore::t!("Automatic demos are off. Turn on Auto-Record Demos in Options > Demos to record every level you play.")
            };
            return vec![(qcore::lang::tr(note).into_owned(), MUTED)];
        };
        let mut out = vec![
            (label(d), GOLD_FG),
            (qcore::tr!("Map: {}", d.map.as_deref().unwrap_or("...")), FG),
            (qcore::tr!("Length: {}", d.duration.map_or("...".into(), format_time)), FG),
            (qcore::tr!("Recorded: {}", draw::date(d.modified)), FG),
        ];
        if d.auto {
            out.push((qcore::tr!("Automatic: replaced when the slots are full"), MUTED));
        }
        out
    }

    /// The buttons that apply to the selection: (label, pad key).
    pub fn tv_actions(&self) -> Vec<(&'static str, i32)> {
        BUTTONS.iter().filter(|(b, _)| self.enabled(*b)).map(|&(b, name)| (name, b.pad_key())).collect()
    }
}
