//! Modal prompts over any menu screen: yes/no confirmations (overwrite or
//! delete a save, end the running game), the launch pop-up before a new
//! game (`launch.rs`) and the skill picker shown before starting a map from
//! Level Select or Map Packs.
use super::*;

pub(super) const SKILLS: [&str; 4] = [qcore::t!("Easy"), qcore::t!("Normal"), qcore::t!("Hard"), qcore::t!("Nightmare")];

#[derive(Clone, Debug)]
pub(super) enum Prompt {
    /// `cmd` runs on yes; `close` also closes the menu.
    Confirm { lines: Vec<String>, yes_label: &'static str, cmd: Action, close: bool, yes: bool },
    /// `cmd` runs after `skill <cursor>`; `warn` says the running game ends.
    Skill { cmd: String, cursor: usize, warn: bool },
    /// Play / Customize Experience / Disable All before a new game
    /// (`Menu::launch` holds the game).
    Launch { cursor: usize },
    /// First-run graphics preset pick (`preset.rs`).
    Preset { cursor: usize },
}

/// Box geometry (menu units): x, y, w, h.
const BOX: (f32, f32, f32, f32) = (170.0, 120.0, 300.0, 120.0);
/// Yes/No buttons: left edges, size, top.
const BTN_X: [f32; 2] = [220.0, 340.0];
const BTN: (f32, f32) = (80.0, 16.0);
const BTN_Y: f32 = BOX.1 + BOX.3 - 28.0;
/// Skill rows: first row top and pitch.
const SKILL_Y: f32 = BOX.1 + 36.0;
const SKILL_H: f32 = 14.0;

impl Menu {
    pub(super) fn confirm(&mut self, lines: Vec<String>, yes_label: &'static str, cmd: impl Into<Action>, close: bool) {
        self.prompt = Some(Prompt::Confirm { lines, yes_label, cmd: cmd.into(), close, yes: false });
    }

    pub(super) fn ask_skill(&mut self, cmd: String, cv: &Cvars) {
        let cursor = cv.value_of("skill").clamp(0.0, 3.0) as usize;
        self.prompt = Some(Prompt::Skill { cmd, cursor, warn: self.game_running });
    }

    /// Item under the pointer: a Yes/No button or a skill row.
    fn prompt_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        match self.prompt.as_ref()? {
            Prompt::Confirm { .. } => BTN_X.iter().position(|&bx| (bx..bx + BTN.0).contains(&x) && (BTN_Y..BTN_Y + BTN.1).contains(&y)),
            Prompt::Skill { .. } => {
                let inside = (BOX.0..BOX.0 + BOX.2).contains(&x) && y >= SKILL_Y;
                let row = ((y - SKILL_Y) / SKILL_H) as usize;
                (inside && row < SKILLS.len()).then_some(row)
            }
            Prompt::Launch { .. } | Prompt::Preset { .. } => None,
        }
    }

    pub(super) fn prompt_hover(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        let i = self.prompt_at(pos)?;
        let changed = match self.prompt.as_mut()? {
            Prompt::Confirm { yes, .. } => std::mem::replace(yes, i == 0) != (i == 0),
            Prompt::Skill { cursor, .. } => std::mem::replace(cursor, i) != i,
            Prompt::Launch { .. } | Prompt::Preset { .. } => false,
        };
        changed.then_some("misc/menu1.wav")
    }

    pub(super) fn prompt_key(&mut self, key: i32, out: &mut Vec<Action>) {
        if self.prompt.is_none() { return; }
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON);
        let click = key == K_MOUSE1;
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);
        let hit = if click { self.mouse.and_then(|m| self.prompt_at(m)) } else { None };
        let Some(p) = self.prompt.as_mut() else { return };
        let run;
        match p {
            Prompt::Confirm { yes, cmd, close, .. } => {
                if matches!(key, K_LEFTARROW | K_RIGHTARROW | K_DPAD_LEFT | K_DPAD_RIGHT) {
                    *yes = !*yes;
                    out.push(Action::Sound("misc/menu1.wav"));
                }
                let pick = if matches!(key, 0x79 | 0x59) { Some(true) }
                    else if matches!(key, 0x6e | 0x4e) || back { Some(false) }
                    else if enter { Some(*yes) }
                    else if click { hit.map(|i| i == 0) }
                    else { None };
                match pick {
                    Some(true) => run = (cmd.clone(), *close),
                    Some(false) => {
                        self.prompt = None;
                        out.push(Action::Sound("misc/menu3.wav"));
                        return;
                    }
                    None => return,
                }
            }
            Prompt::Skill { cmd, cursor, .. } => {
                let n = SKILLS.len();
                if matches!(key, K_UPARROW | K_DPAD_UP) { *cursor = (*cursor + n - 1) % n; out.push(Action::Sound("misc/menu1.wav")); }
                if matches!(key, K_DOWNARROW | K_DPAD_DOWN) { *cursor = (*cursor + 1) % n; out.push(Action::Sound("misc/menu1.wav")); }
                let shortcut = (b'1' as i32..=b'4' as i32).contains(&key).then(|| (key - b'1' as i32) as usize);
                if back {
                    self.prompt = None;
                    out.push(Action::Sound("misc/menu3.wav"));
                    return;
                }
                let pick = shortcut.or(if enter { Some(*cursor) } else { hit });
                let Some(s) = pick else { return };
                run = (Action::Command(format!("skill {s}\n{cmd}")), true);
            }
            Prompt::Launch { .. } | Prompt::Preset { .. } => return,
        }
        self.prompt = None;
        let (cmd, close) = run;
        out.push(Action::Sound("misc/menu2.wav"));
        if close {
            self.screen = Screen::None;
            out.push(Action::Close);
        }
        out.push(cmd);
    }

    pub(super) fn draw_prompt(&self, c: &mut Canvas, cv: &Cvars) {
        let Some(p) = &self.prompt else { return };
        if let Prompt::Launch { cursor } = *p {
            self.draw_launch(c, cv, cursor);
            return;
        }
        if let Prompt::Preset { cursor } = *p {
            self.draw_preset(c, cursor);
            return;
        }
        let (x, y, w, h) = BOX;
        c.fill(-c.ox, -c.oy, c.vw(), c.vh(), [0.0, 0.0, 0.0, 0.5]);
        c.round_box(x, y, w, h, 5.0, [0.06, 0.05, 0.04, 0.98], Some(BORDER));
        match p {
            Prompt::Confirm { lines, yes_label, yes, .. } => {
                for (i, l) in lines.iter().enumerate() {
                    c.text_center(320.0, y + 14.0 + i as f32 * 14.0, l, TXT, if i == 0 { GOLD_FG } else { FG });
                }
                for (k, (bx, label)) in BTN_X.iter().zip([*yes_label, "Cancel"]).enumerate() {
                    let on = (k == 0) == *yes;
                    c.round_box(*bx, BTN_Y, BTN.0, BTN.1, 4.0, if on { HILITE } else { [0.0, 0.0, 0.0, 0.7] }, Some(BORDER));
                    let key = (self.pad_ui && k == 1).then_some(K_BBUTTON);
                    self.glyphs.button_label(c, [*bx, BTN_Y, BTN.0, BTN.1], key, label, TXT, if on { GOLD_FG } else { MUTED });
                }
            }
            Prompt::Skill { cursor, warn, .. } => {
                c.text_center(320.0, y + 10.0, "Choose skill", TXT, GOLD_FG);
                if *warn {
                    c.text_center(320.0, y + 22.0, "The current game will end", TXT, MUTED);
                }
                for (i, s) in SKILLS.iter().enumerate() {
                    let ry = SKILL_Y + i as f32 * SKILL_H;
                    if i == *cursor {
                        c.round_box(x + 60.0, ry - 2.0, w - 120.0, SKILL_H - 1.0, 3.0, HILITE, None);
                    }
                    c.text_center(320.0, ry + 1.0, &format!("{}  {s}", i + 1), TXT, if i == *cursor { GOLD_FG } else { FG });
                }
            }
            Prompt::Launch { .. } | Prompt::Preset { .. } => {}
        }
    }
}
