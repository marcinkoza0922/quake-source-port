//! Touch dialogs: every modal question of the menus (confirmations, the
//! skill picker, the New Game pop-up, the first-run graphics preset, the
//! auto-aim and rebinding questions, the death screen and Quit) as one box
//! with large buttons, over whatever screen is below. Text comes from the
//! classic dialogs and a button runs the same key handler its classic
//! counterpart does, so both behave alike.

use super::*;

/// Which question is open (in the order the classic key handling checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::menu) enum Ask {
    Bind,
    AutoAim,
    Confirm,
    Skill,
    Launch,
    Preset,
    Death,
    Quit,
}

/// One button: label and an optional second line.
pub(super) struct Button {
    label: String,
    note: Option<String>,
    look: Look,
}

pub(super) struct Dialog {
    title: String,
    lines: Vec<String>,
    pub(super) buttons: Vec<Button>,
    /// Buttons side by side (yes/no) rather than stacked.
    row: bool,
    /// Notes under the buttons.
    foot: Vec<String>,
}

const W: f32 = 480.0;
const PAD: f32 = 18.0;
const TITLE: f32 = 21.0;
const BTN_H: f32 = 52.0;
const BTN_NOTE_H: f32 = 64.0;
const GAP: f32 = 10.0;

fn button(label: &str, look: Look) -> Button {
    Button { label: qcore::lang::tr(label).into_owned(), note: None, look }
}

/// Lines of `s` that fit `max_w` at `size`, measured conservatively
/// without a canvas, so drawing and hit testing agree.
fn wrap(s: &str, max_w: f32, size: f32) -> Vec<String> {
    let est = |t: &str| t.chars().count() as f32 * size * 0.56;
    s.lines().flat_map(|l| if est(l) <= max_w { vec![l.to_string()] } else { qcore::lang::wrap(l, max_w, est) }).collect()
}

impl Menu {
    pub(in crate::menu) fn tv_ask(&self) -> Option<Ask> {
        if self.bind_pending.is_some() {
            return Some(Ask::Bind);
        }
        if self.autoaim_prompt {
            return Some(Ask::AutoAim);
        }
        match self.prompt {
            Some(Prompt::Confirm { .. }) => return Some(Ask::Confirm),
            Some(Prompt::Skill { .. }) => return Some(Ask::Skill),
            Some(Prompt::Launch { .. }) => return Some(Ask::Launch),
            Some(Prompt::Preset { .. }) => return Some(Ask::Preset),
            None => {}
        }
        match self.screen {
            Screen::Death => Some(Ask::Death),
            Screen::Quit => Some(Ask::Quit),
            _ => None,
        }
    }

    /// A touch dialog is up (it takes all pointer input).
    pub(in crate::menu) fn tv_dialog_open(&self) -> bool {
        self.touch_ui && !self.tv_classic_modal() && self.tv_ask().is_some()
    }

    pub(super) fn tv_dialog(&self, ask: Ask, cv: &Cvars) -> Dialog {
        let tr = |s: &str| qcore::lang::tr(s).into_owned();
        let yes_no = |title: String, lines: Vec<String>| Dialog {
            title,
            lines,
            buttons: vec![button(qcore::t!("Yes"), Look::Primary), button(qcore::t!("No"), Look::Plain)],
            row: true,
            foot: Vec::new(),
        };
        match ask {
            Ask::Bind => {
                let (key, _) = self.bind_pending.unwrap_or((0, ""));
                let old = self.bindings.get(key as usize).and_then(Option::as_deref).unwrap_or("");
                let title = format!("{} {}", keynum_to_string(key), tr(qcore::t!("is already assigned")));
                yes_no(title, vec![old.chars().take(65).collect(), tr("Replace this binding?")])
            }
            Ask::AutoAim => {
                let keys = self.autoaim_conflicts.iter().map(|&k| keynum_to_string(k)).collect::<Vec<_>>().join(" / ");
                yes_no(qcore::tr!("{} already assigned", keys), vec![tr("Open Controls to remap Lock-on?")])
            }
            Ask::Quit => yes_no(tr("Really quit?"), Vec::new()),
            Ask::Confirm => {
                let Some(Prompt::Confirm { lines, yes_label, .. }) = &self.prompt else { unreachable!() };
                Dialog {
                    title: lines.first().map(|l| tr(l)).unwrap_or_default(),
                    lines: lines.iter().skip(1).map(|l| tr(l)).collect(),
                    buttons: vec![button(yes_label, Look::Primary), button(qcore::t!("Cancel"), Look::Plain)],
                    row: true,
                    foot: Vec::new(),
                }
            }
            Ask::Skill => {
                let Some(Prompt::Skill { cursor, warn, .. }) = self.prompt else { unreachable!() };
                Dialog {
                    title: tr("Choose skill"),
                    lines: if warn { vec![tr("The current game will end")] } else { Vec::new() },
                    buttons: prompt::SKILLS
                        .iter()
                        .enumerate()
                        .map(|(i, s)| button(s, if i == cursor { Look::Primary } else { Look::Plain }))
                        .collect(),
                    row: false,
                    foot: Vec::new(),
                }
            }
            Ask::Launch => {
                let custom = launch::customized(cv);
                let mut buttons = vec![
                    Button {
                        label: tr("Play"),
                        note: custom.then(|| tr("CUSTOM")),
                        look: Look::Primary,
                    },
                    button(qcore::t!("Customize Experience"), Look::Plain),
                ];
                if custom {
                    buttons.push(button(qcore::t!("Disable All Modifications"), Look::Plain));
                }
                buttons.push(button(qcore::t!("Cancel"), Look::Plain));
                let foot = if custom {
                    vec![customize::custom_summary(cv)]
                } else {
                    vec![tr("Standard rules, no add-ons."), tr("Saves load in other source ports too.")]
                };
                Dialog {
                    title: tr("New Game"),
                    lines: self.launch.as_ref().and_then(|l| l.title.as_deref()).map(|t| clean_title(t, 44)).into_iter().collect(),
                    buttons,
                    row: false,
                    foot,
                }
            }
            Ask::Preset => Dialog {
                title: tr("Choose a Graphics Preset"),
                lines: vec![tr("Everything can be changed later in Options.")],
                buttons: preset::CHOICES
                    .iter()
                    .map(|(name, desc, _)| Button {
                        label: tr(name),
                        note: Some(desc.iter().map(|d| tr(d)).collect::<Vec<_>>().join(" ")),
                        look: Look::Plain,
                    })
                    .chain(std::iter::once(button(qcore::t!("Keep Defaults"), Look::Dim)))
                    .collect(),
                row: false,
                foot: Vec::new(),
            },
            Ask::Death => Dialog {
                title: tr("You died"),
                lines: Vec::new(),
                buttons: navigation::DEATH_LABELS
                    .iter()
                    .take(self.death_rows())
                    .enumerate()
                    .map(|(i, l)| button(l, if i == 0 { Look::Primary } else { Look::Plain }))
                    .collect(),
                row: false,
                foot: Vec::new(),
            },
        }
    }

    /// Box, title and line tops, button rectangles and the foot's top.
    pub(super) fn tv_dialog_geom(&self, d: &Dialog) -> (Rect, Vec<Rect>, f32) {
        let (w, h) = self.tv_size();
        let bw = W.min(w - 2.0 * MARGIN);
        let inner = bw - 2.0 * PAD;
        let text_h = wrap(&d.title, inner, TITLE).len() as f32 * (TITLE + 6.0)
            + d.lines.iter().map(|l| wrap(l, inner, VALUE).len()).sum::<usize>() as f32 * (VALUE + 6.0);
        let foot_h = d.foot.iter().map(|l| wrap(l, inner, SMALL).len()).sum::<usize>() as f32 * (SMALL + 5.0);
        let heights: Vec<f32> = d.buttons.iter().map(|b| if b.note.is_some() { BTN_NOTE_H } else { BTN_H }).collect();
        let btns_h = if d.row { BTN_H } else { heights.iter().sum::<f32>() + GAP * (heights.len().saturating_sub(1)) as f32 };
        let mut bh = PAD + text_h + 8.0 + btns_h + PAD + if foot_h > 0.0 { foot_h + 4.0 } else { 0.0 };
        // a dialog taller than the screen squeezes its buttons
        let squeeze = if bh > h - 8.0 { ((h - 8.0 - (bh - btns_h)) / btns_h).clamp(0.6, 1.0) } else { 1.0 };
        bh = bh.min(h - 8.0);
        let b = [(w - bw) / 2.0, ((h - bh) / 2.0).max(4.0), bw, bh];
        let mut y = b[1] + PAD + text_h + 8.0;
        let rects = if d.row {
            let n = d.buttons.len().max(1) as f32;
            let each = (inner - GAP * (n - 1.0)) / n;
            (0..d.buttons.len()).map(|k| [b[0] + PAD + k as f32 * (each + GAP), y, each, BTN_H * squeeze]).collect()
        } else {
            heights
                .iter()
                .map(|&hh| {
                    let r = [b[0] + PAD, y, inner, hh * squeeze];
                    y += (hh + GAP) * squeeze;
                    r
                })
                .collect()
        };
        let foot_y = if d.row { b[1] + PAD + text_h + 8.0 + BTN_H * squeeze + 10.0 } else { y + 2.0 };
        (b, rects, foot_y)
    }

    pub(super) fn tv_draw_dialog(&self, c: &mut Canvas, cv: &Cvars) {
        let Some(ask) = self.tv_ask() else { return };
        let d = self.tv_dialog(ask, cv);
        let (b, rects, foot_y) = self.tv_dialog_geom(&d);
        let (w, h) = self.tv_size();
        c.fill(-c.ox, -c.oy, w + 2.0 * c.ox + 1.0, h + 2.0 * c.oy + 1.0, [0.0, 0.0, 0.0, 0.6]);
        c.round_box(b[0], b[1], b[2], b[3], 10.0, DIALOG_BG, Some(BORDER));
        let inner = b[2] - 2.0 * PAD;
        let cx = b[0] + b[2] / 2.0;
        let mut y = b[1] + PAD;
        for l in wrap(&d.title, inner, TITLE) {
            let tw = c.text_w_raw(&l, TITLE).min(inner);
            clip_raw(c, cx - tw / 2.0, y, &l, TITLE, inner, GOLD_FG);
            y += TITLE + 6.0;
        }
        for line in &d.lines {
            for l in wrap(line, inner, VALUE) {
                let tw = c.text_w_raw(&l, VALUE).min(inner);
                clip_raw(c, cx - tw / 2.0, y, &l, VALUE, inner, FG);
                y += VALUE + 6.0;
            }
        }
        for (bt, r) in d.buttons.iter().zip(&rects) {
            let look = if self.tv_pressed(*r) { Look::Pressed } else { bt.look };
            match &bt.note {
                None => tv_button(c, *r, &bt.label, look),
                Some(note) => {
                    tv_button(c, *r, "", look);
                    let col = if look == Look::Plain { FG } else { GOLD_FG };
                    fit_left(c, r[0] + 14.0, r[1] + 9.0, &bt.label, LABEL, r[2] - 28.0, col);
                    clip_raw(c, r[0] + 14.0, r[1] + 13.0 + LABEL, note, SMALL, r[2] - 28.0, MUTED);
                }
            }
        }
        let mut y = foot_y;
        for line in &d.foot {
            for l in wrap(line, inner, SMALL) {
                let tw = c.text_w_raw(&l, SMALL).min(inner);
                clip_raw(c, cx - tw / 2.0, y, &l, SMALL, inner, MUTED);
                y += SMALL + 5.0;
            }
        }
    }

    /// A tap with a dialog up: a button picks, outside the box cancels.
    pub(in crate::menu) fn tv_dialog_click(&mut self, cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) {
        let (Some(ask), Some(pos)) = (self.tv_ask(), self.mouse) else { return };
        let d = self.tv_dialog(ask, cv);
        let (b, rects, _) = self.tv_dialog_geom(&d);
        let pick = rects.iter().position(|r| inside(*r, pos));
        if pick.is_none() && inside(b, pos) {
            return;
        }
        let cancel = pick.is_none() || pick == Some(d.buttons.len() - 1) && matches!(ask, Ask::Launch | Ask::Preset);
        match ask {
            Ask::Bind => {
                let key = if pick == Some(0) { b'y' as i32 } else { K_ESCAPE };
                self.binding_key(key, out);
            }
            Ask::AutoAim => {
                if pick.is_none() {
                    return;
                }
                self.autoaim_yes = pick == Some(0);
                self.autoaim_prompt_key(K_ENTER, cv, out);
            }
            Ask::Confirm => {
                if let Some(Prompt::Confirm { yes, .. }) = self.prompt.as_mut() {
                    *yes = pick == Some(0);
                }
                self.prompt_key(if pick == Some(0) { K_ENTER } else { K_ESCAPE }, out);
            }
            Ask::Skill => {
                if let (Some(k), Some(Prompt::Skill { cursor, .. })) = (pick, self.prompt.as_mut()) {
                    *cursor = k;
                }
                self.prompt_key(if pick.is_some() { K_ENTER } else { K_ESCAPE }, out);
            }
            Ask::Launch => {
                if cancel {
                    self.launch_key(K_ESCAPE, cv, out);
                } else if let Some(k) = pick {
                    self.prompt = Some(Prompt::Launch { cursor: k });
                    self.launch_key(K_ENTER, cv, out);
                }
            }
            Ask::Preset => {
                if cancel {
                    self.preset_key(K_ESCAPE, out);
                } else if let Some(k) = pick {
                    self.prompt = Some(Prompt::Preset { cursor: k });
                    self.preset_key(K_ENTER, out);
                }
            }
            Ask::Death => {
                if let Some(k) = pick {
                    self.death_cursor = k;
                    out.extend(self.key(K_ENTER, cv, server_active));
                }
            }
            Ask::Quit => {
                self.quit_cursor = usize::from(pick != Some(0));
                out.extend(self.key(K_ENTER, cv, server_active));
            }
        }
    }
}
