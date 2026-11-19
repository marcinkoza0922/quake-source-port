//! Drawing the Demos screen: tabs, filter/sort bar, the list with its
//! scrollbar, and the info panel with the action buttons.

use super::*;
use crate::demo::playback::format_time;
use crate::draw::Canvas;
use crate::menu::{BORDER, FG, GOLD_FG, HILITE, MUTED, TXT, txt, txt_gold};
use crate::modmenu::clip;

const HOVER: [f32; 4] = [1.0, 1.0, 1.0, 0.07];

impl DemoBtn {
    /// Gamepad button that triggers this action (matches [`DemoBrowser::key`]).
    pub fn pad_key(self) -> i32 {
        match self {
            Self::Play => K_ABUTTON,
            Self::Export => K_XBUTTON,
            Self::Rename => K_YBUTTON,
            Self::Keep => K_LTRIGGER,
            Self::Delete => K_BACK,
        }
    }
}

impl DemoBrowser {
    /// `mouse` lights the row or button under the pointer; `pad` draws
    /// controller glyphs on buttons and picks controller wording in the note.
    pub fn draw(&self, c: &mut Canvas, mouse: Option<(f32, f32)>, pad: bool, glyphs: &crate::glyphs::Glyphs) {
        let hover = mouse.and_then(|p| self.hit(p));
        c.text_center(320.0, 22.0, "Demos", 13.0, GOLD_FG);
        let tab_keys = [K_LSHOULDER, K_RSHOULDER];
        for (t, name) in TABS.iter().enumerate() {
            let x = 320.0 - TAB.0 - 4.0 + t as f32 * (TAB.0 + 8.0);
            let on = t == self.tab;
            let bg = if on { HILITE } else if hover == Some(Hit::Tab(t)) { [0.25, 0.18, 0.08, 0.8] } else { [0.0, 0.0, 0.0, 0.7] };
            c.fill(x, TAB_Y, TAB.0, TAB.1, bg);
            c.frame(x, TAB_Y, TAB.0, TAB.1, BORDER);
            let (shown, all) = (self.view[t].len(), self.lists[t].len());
            let s = if self.filter.is_empty() { qcore::tr!("{} ({})", qcore::lang::tr(name), all) } else { qcore::tr!("{} ({}/{})", qcore::lang::tr(name), shown, all) };
            glyphs.button_label_raw(c, [x, TAB_Y, TAB.0, TAB.1], pad.then_some(tab_keys[t]), &s, TXT, if on { GOLD_FG } else { MUTED });
        }
        let (x, w) = LIST;
        crate::modmenu::filter_bar_at(c, x, BAR_Y, w, SORT_W, &self.filter, self.sort.label(), pad.then_some((None, Some(K_RTRIGGER), glyphs)));
        panel(c, x, w);
        self.draw_list(c, hover);
        self.draw_info(c, hover, pad, glyphs);
    }

    fn draw_list(&self, c: &mut Canvas, hover: Option<Hit>) {
        let (x, w) = LIST;
        let list = &self.lists[self.tab];
        if list.is_empty() {
            let note = if self.tab == 0 {
                qcore::t!("No saved demos yet. Record one with the console command 'record <name>', or keep an automatic demo.")
            } else if self.auto_on {
                qcore::t!("No automatic demos yet. Each level you play is recorded here.")
            } else {
                qcore::t!("Automatic demos are off. Turn on Auto-Record Demos in Options > Demos to record every level you play.")
            };
            paragraph(c, x + 8.0, TOP + 1.0, w - 16.0, note, MUTED);
        } else if self.rows() == 0 {
            paragraph(c, x + 8.0, TOP + 1.0, w - 16.0, qcore::t!("No demos match the filter. Backspace or Esc clears it."), MUTED);
        }
        let scroll = self.scroll[self.tab];
        let bar = self.scrollbar();
        let right = if bar.is_some() { x + w - 10.0 } else { x + w - 8.0 };
        for v in (scroll..self.rows()).take(ROWS) {
            let Some(d) = self.row(v) else { continue };
            let y = TOP + (v - scroll) as f32 * ROW_H;
            if v == self.cursor[self.tab] {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HILITE);
            } else if hover == Some(Hit::Row(v)) {
                c.fill(x + 2.0, y, w - 4.0, ROW_H, HOVER);
            }
            txt(c, x + 6.0, y + 1.0, &clip(&label(d), ((w - 60.0) / 6.0) as usize));
            if let Some(t) = d.duration {
                c.text_right(right, y + 1.0, &format_time(t), TXT, MUTED);
            }
        }
        if let Some((bx, top, h, ty, th)) = bar {
            c.fill(bx, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(bx, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
    }

    fn draw_info(&self, c: &mut Canvas, hover: Option<Hit>, pad: bool, glyphs: &crate::glyphs::Glyphs) {
        let (ix, iw) = INFO;
        panel(c, ix, iw);
        let cols = ((iw - 16.0) / 6.0) as usize;
        let mut y = TOP + 1.0;
        if let Some(d) = self.selected() {
            txt_gold(c, ix + 8.0, y, &clip(&label(d), cols));
            y += ROW_H + 4.0;
            let mut row = |c: &mut Canvas, s: String| {
                txt(c, ix + 8.0, y, &clip(&s, cols));
                y += ROW_H;
            };
            row(c, qcore::tr!("Map: {}", d.map.as_deref().unwrap_or("...")));
            row(c, qcore::tr!("Length: {}", d.duration.map_or("...".into(), format_time)));
            row(c, qcore::tr!("Recorded: {}", date(d.modified)));
            row(c, qcore::tr!("Size: {} KB", d.bytes.div_ceil(1024)));
            if d.auto {
                row(c, qcore::tr!("Automatic: replaced when the slots are full"));
            }
        }
        for (i, &(b, name)) in BUTTONS.iter().enumerate() {
            let (bx, by, bw, bh) = button_rect(i);
            let on = self.enabled(b);
            let lit = on && hover == Some(Hit::Button(b));
            let bg = if lit { HILITE } else if on { [0.0, 0.0, 0.0, 0.7] } else { [0.0, 0.0, 0.0, 0.35] };
            c.round_box(bx, by, bw, bh, 3.0, bg, Some(if on { BORDER } else { [0.35, 0.3, 0.25, 0.5] }));
            let fg = if lit { GOLD_FG } else if on { FG } else { [0.45, 0.42, 0.38, 0.7] };
            glyphs.button_label(c, [bx, by, bw, bh], (pad && on).then_some(b.pad_key()), name, TXT, fg);
        }
        let (_, last_y, _, last_h) = button_rect(BUTTONS.len() - 1);
        let note = if pad {
            qcore::t!("While a demo plays: A pauses, the d-pad scrubs and changes speed, LT and RT mark a clip and X exports it.")
        } else {
            qcore::t!("While a demo plays: Space pauses, the arrows scrub and change speed, I and O mark a clip and E exports it.")
        };
        paragraph(c, ix + 8.0, last_y + last_h + 10.0, iw - 16.0, note, MUTED);
    }
}

fn date(secs: u64) -> String {
    let s = crate::demo::stamp(secs);
    format!("{}-{}-{} {}:{} UTC", &s[..4], &s[4..6], &s[6..8], &s[9..11], &s[11..13])
}

/// Translated text wrapped to `w`.
fn paragraph(c: &mut Canvas, x: f32, y: f32, w: f32, s: &str, color: [f32; 4]) {
    let s = qcore::lang::tr(s);
    for (i, l) in qcore::lang::wrap(&s, w, |t| c.text_w_raw(t, TXT)).iter().enumerate() {
        c.text_raw(x, y + i as f32 * ROW_H, l, TXT, color);
    }
}

fn panel(c: &mut Canvas, x: f32, w: f32) {
    let h = ROWS as f32 * ROW_H + 8.0;
    c.fill(x, TOP - 4.0, w, h, [0.0, 0.0, 0.0, 0.8]);
    c.frame(x, TOP - 4.0, w, h, BORDER);
}
