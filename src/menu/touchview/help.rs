//! Touch Help: a tapped section of the manual as full-width text (the
//! section list is a touch list, `list.rs`). Back returns to the list.

use super::*;
use crate::menu::help::{Block, sections};

impl Menu {
    pub(super) fn tv_draw_help(&self, c: &mut Canvas) {
        let (_, h) = self.tv_size();
        let (x0, cw) = self.tv_col();
        let secs = sections();
        let sec = &secs[self.help.section.min(secs.len() - 1)];
        self.tv_title(c, &sec.title);
        let width = cw - 40.0;
        let mut lines: Vec<(f32, String, bool)> = Vec::new();
        for b in &sec.blocks {
            match b {
                Block::Gap => lines.push((0.0, String::new(), false)),
                Block::Heading(t) => lines.push((0.0, t.clone(), true)),
                Block::Para { text, bullet } => {
                    let indent = bullet.as_ref().map_or(0.0, |b| c.text_w_raw(b, VALUE) + 8.0);
                    for (k, l) in qcore::lang::wrap(text, width - indent, |t| c.text_w_raw(t, VALUE)).into_iter().enumerate() {
                        let s = match (k, bullet) {
                            (0, Some(b)) => format!("{b} {l}"),
                            _ => l,
                        };
                        lines.push((if k == 0 { 0.0 } else { indent }, s, false));
                    }
                }
            }
        }
        let lh = |head: bool| if head { LABEL + 12.0 } else { VALUE + 8.0 };
        let total: f32 = lines.iter().map(|l| lh(l.2)).sum::<f32>() + 24.0;
        self.tv.help_h.set(total);
        let top = BAR_H + 4.0 - self.tv_scroll();
        c.round_box(x0, top, cw, total, 8.0, PANEL_BG, Some(BORDER));
        c.clip_rows(Some((BAR_H, h)));
        let rtl = qcore::lang::rtl();
        let mut y = top + 12.0;
        for (indent, s, head) in &lines {
            if y > h {
                break;
            }
            if y + lh(*head) > BAR_H {
                let (size, col) = if *head { (LABEL, GOLD_FG) } else { (VALUE, FG) };
                let ty = y + if *head { 8.0 } else { 0.0 };
                if rtl {
                    c.text_right(x0 + cw - 20.0 - indent, ty, s, size, col);
                } else {
                    c.text_raw(x0 + 20.0 + indent, ty, s, size, col);
                }
            }
            y += lh(*head);
        }
        c.clip_rows(None);
    }
}
