//! First-run graphics preset pop-up: shown over the main menu the first time
//! a game starts with no user config (its own or inherited from the base
//! game). Each choice runs `graphics_preset <name>` (`host/presets.rs`);
//! Escape keeps the defaults. `menu_preset` opens it again.
use super::*;

/// Pop-up rows: title, two description lines, console name.
const CHOICES: [(&str, [&str; 2], &str); 3] = [
    ("Classic", ["Like the re-release: enhanced models,", "no added effects."], "classic"),
    ("Enhanced", ["Every enhancement at its highest quality.", "Needs a capable GPU."], "enhanced"),
    ("Low-End", ["Fastest. No anti-aliasing, original", "models, lowest effect quality."], "lowend"),
];

/// Pop-up box: x, y, w, h (menu units).
const BOX: (f32, f32, f32, f32) = (150.0, 72.0, 340.0, 214.0);
const CARD_X: f32 = BOX.0 + 16.0;
const CARD_W: f32 = BOX.2 - 32.0;
const CARD_Y: f32 = BOX.1 + 44.0;
const CARD_H: f32 = 46.0;
const CARD_PITCH: f32 = 52.0;

impl Menu {
    /// Shows the preset pop-up (Classic selected) over the current screen.
    pub fn open_preset(&mut self) {
        self.preset_pending = false;
        self.prompt = Some(Prompt::Preset { cursor: 0 });
    }

    /// Opens the pop-up once the main menu is up with nothing else asked.
    pub fn show_pending_preset(&mut self) {
        if self.preset_pending && self.screen == Screen::Main && self.prompt.is_none() {
            self.open_preset();
        }
    }

    fn preset_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        if !(CARD_X..CARD_X + CARD_W).contains(&x) || y < CARD_Y {
            return None;
        }
        let i = ((y - CARD_Y) / CARD_PITCH) as usize;
        (i < CHOICES.len() && y - CARD_Y - i as f32 * CARD_PITCH < CARD_H).then_some(i)
    }

    pub(super) fn preset_hover(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        let i = self.preset_at(pos)?;
        let Some(Prompt::Preset { cursor }) = self.prompt.as_mut() else { return None };
        (std::mem::replace(cursor, i) != i).then_some("misc/menu1.wav")
    }

    pub(super) fn preset_key(&mut self, key: i32, out: &mut Vec<Action>) {
        let Some(Prompt::Preset { mut cursor }) = self.prompt else { return };
        let n = CHOICES.len();
        let pick = match key {
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => {
                // keep the defaults; config.cfg is written on quit, so this
                // isn't asked again
                self.prompt = None;
                out.push(Action::Sound("misc/menu3.wav"));
                return;
            }
            K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN | K_TAB => {
                let up = matches!(key, K_UPARROW | K_DPAD_UP);
                cursor = if up { (cursor + n - 1) % n } else { (cursor + 1) % n };
                out.push(Action::Sound("misc/menu1.wav"));
                None
            }
            K_ENTER | K_KP_ENTER | K_ABUTTON => Some(cursor),
            K_MOUSE1 => self.mouse.and_then(|m| self.preset_at(m)),
            k if (b'1' as i32..=b'3' as i32).contains(&k) => Some((k - b'1' as i32) as usize),
            _ => None,
        };
        self.prompt = Some(Prompt::Preset { cursor });
        if let Some(i) = pick {
            self.prompt = None;
            out.push(Action::Sound("misc/menu2.wav"));
            out.push(Action::Command(format!("graphics_preset {}\n", CHOICES[i].2)));
        }
    }

    pub(super) fn draw_preset(&self, c: &mut Canvas, cursor: usize) {
        let (x, y, w, h) = BOX;
        c.fill(-c.ox, -c.oy, c.vw(), c.vh(), [0.0, 0.0, 0.0, 0.5]);
        c.round_box(x, y, w, h, 5.0, [0.06, 0.05, 0.04, 0.98], Some(BORDER));
        c.text_center(320.0, y + 10.0, "Choose a Graphics Preset", TXT, GOLD_FG);
        c.text_center(320.0, y + 24.0, "Everything can be changed later in Options.", 8.0, MUTED);
        for (i, (name, desc, _)) in CHOICES.iter().enumerate() {
            let top = CARD_Y + i as f32 * CARD_PITCH;
            let on = cursor == i;
            let fill = if on { HILITE } else { [0.0, 0.0, 0.0, 0.7] };
            c.round_box(CARD_X, top, CARD_W, CARD_H, 4.0, fill, Some(if on { GOLD_FG } else { BORDER }));
            c.text(CARD_X + 10.0, top + 7.0, name, 12.0, if on { GOLD_FG } else { FG });
            for (k, l) in desc.iter().enumerate() {
                c.text(CARD_X + 10.0, top + 23.0 + k as f32 * 10.0, l, 8.0, MUTED);
            }
        }
        if self.pad_ui {
            self.boxed_hints(c, 304.0, &[(K_ABUTTON, "Select"), (K_BBUTTON, "Keep Defaults")]);
        } else {
            c.text_center(320.0, y + h - 14.0, "Esc keeps the defaults", 8.0, MUTED);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(acts: &[Action]) -> Vec<String> {
        acts.iter().filter_map(|a| match a { Action::Command(c) => Some(c.clone()), _ => None }).collect()
    }

    fn popup() -> (Menu, Cvars) {
        let mut m = Menu::new();
        m.preset_pending = true;
        m.open(Screen::Main);
        m.open_preset();
        assert!(!m.preset_pending);
        (m, Cvars::default())
    }

    #[test]
    fn enter_picks_the_highlighted_preset() {
        let (mut m, mut cv) = popup();
        m.key(K_DOWNARROW, &mut cv, false);
        assert_eq!(commands(&m.key(K_ENTER, &mut cv, false)), ["graphics_preset enhanced\n"]);
        assert!(m.prompt.is_none());
        assert_eq!(m.screen, Screen::Main);
    }

    #[test]
    fn number_keys_and_clicks_pick_directly() {
        let (mut m, mut cv) = popup();
        assert_eq!(commands(&m.key(b'3' as i32, &mut cv, false)), ["graphics_preset lowend\n"]);
        let (mut m, mut cv) = popup();
        m.mouse_move((320.0, CARD_Y + CARD_H / 2.0), &mut cv);
        assert_eq!(commands(&m.key(K_MOUSE1, &mut cv, false)), ["graphics_preset classic\n"]);
    }

    #[test]
    fn escape_keeps_the_defaults() {
        let (mut m, mut cv) = popup();
        assert!(commands(&m.key(K_ESCAPE, &mut cv, false)).is_empty());
        assert!(m.prompt.is_none());
        assert_eq!(m.screen, Screen::Main);
    }
}
