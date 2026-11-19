use super::*;

impl Menu {
    /// Whether the on-screen keyboard is open.
    #[cfg(test)]
    pub(super) fn osk_open(&self) -> bool {
        self.osk.is_some()
    }

    pub(super) fn osk_key(&mut self, key: i32, mut o: Osk, cv: &mut Cvars, out: &mut Vec<Action>) {
        // a text row, the Options hub's search field, or a mod/map filter box
        #[derive(Clone, Copy)]
        enum Target {
            Search,
            Mods(modmenu::Page),
            Text,
        }
        let target = match self.screen {
            Screen::Options => Target::Search,
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => Target::Mods(self.mods_page()),
            Screen::OptionsPage(p) if matches!(self.page_item(p, self.opt_cursor), Item::Text { .. }) => Target::Text,
            _ => {
                self.osk = None;
                return;
            }
        };
        let backspace = |m: &mut Self, cv: &mut Cvars| match target {
            Target::Search => m.search_backspace(),
            Target::Mods(p) => m.mods.filter_pop(p),
            Target::Text => m.text_delete(false, cv),
        };
        let n = OSK_ACTIONS.len();
        match key {
            K_UPARROW | K_DPAD_UP => o.row = (o.row + 4) % 5,
            K_DOWNARROW | K_DPAD_DOWN => o.row = (o.row + 1) % 5,
            K_LEFTARROW | K_DPAD_LEFT if o.row == 4 => o.col = ((o.action() + n - 1) % n * 10).div_ceil(n),
            K_RIGHTARROW | K_DPAD_RIGHT if o.row == 4 => o.col = ((o.action() + 1) % n * 10).div_ceil(n),
            K_LEFTARROW | K_DPAD_LEFT => o.col = (o.col + 9) % 10,
            K_RIGHTARROW | K_DPAD_RIGHT => o.col = (o.col + 1) % 10,
            K_XBUTTON | K_BACKSPACE => backspace(self, cv),
            K_YBUTTON => o.shift = !o.shift,
            K_BBUTTON | K_ESCAPE | K_START => {
                self.osk = None;
                out.push(Action::Sound("misc/menu3.wav"));
                return;
            }
            K_ABUTTON | K_ENTER | K_KP_ENTER => {
                if o.row < 4 {
                    let ch = OSK_ROWS[o.row].as_bytes()[o.col] as char;
                    self.osk_char(if o.shift { ch.to_ascii_uppercase() } else { ch }, cv);
                } else {
                    match o.action() {
                        0 => o.shift = !o.shift,
                        1 => self.osk_char(' ', cv),
                        2 => backspace(self, cv),
                        _ => {
                            self.osk = None;
                            out.push(Action::Sound("misc/menu2.wav"));
                            return;
                        }
                    }
                }
            }
            _ => return,
        }
        out.push(Action::Sound("misc/menu1.wav"));
        self.osk = Some(o);
    }

    pub(super) fn draw_osk(&self, c: &mut Canvas, o: Osk) {
        let (kw, kh) = OSK_KEY;
        let x0 = 320.0 - kw * 5.0;
        c.fill(x0 - 6.0, OSK_Y - 6.0, kw * 10.0 + 12.0, kh * 5.0 + 12.0, [0.05, 0.04, 0.03, 0.95]);
        c.frame(x0 - 6.0, OSK_Y - 6.0, kw * 10.0 + 12.0, kh * 5.0 + 12.0, BORDER);
        for (r, row) in OSK_ROWS.iter().enumerate() {
            for (k, ch) in row.bytes().enumerate() {
                let (x, y) = (x0 + k as f32 * kw, OSK_Y + r as f32 * kh);
                if o.row == r && o.col == k {
                    c.fill(x, y, kw, kh, HILITE);
                }
                c.frame(x + 1.0, y + 1.0, kw - 2.0, kh - 2.0, [0.35, 0.27, 0.16, 0.9]);
                let ch = if o.shift { ch.to_ascii_uppercase() } else { ch };
                c.text_center(x + kw / 2.0, crate::glyphs::center_text_y(y, kh, TXT), &(ch as char).to_string(), TXT, FG);
            }
        }
        let aw = kw * 10.0 / OSK_ACTIONS.len() as f32;
        let y = OSK_Y + 4.0 * kh;
        const ACTION_KEYS: [Option<i32>; 4] = [Some(K_YBUTTON), None, Some(K_XBUTTON), Some(K_BBUTTON)];
        for (i, label) in OSK_ACTIONS.iter().enumerate() {
            let x = x0 + i as f32 * aw;
            if o.row == 4 && o.action() == i {
                c.fill(x, y, aw, kh, HILITE);
            }
            c.frame(x + 1.0, y + 1.0, aw - 2.0, kh - 2.0, [0.35, 0.27, 0.16, 0.9]);
            let col = if i == 0 && o.shift { GOLD_FG } else { FG };
            let key = self.pad_ui.then_some(ACTION_KEYS[i]).flatten();
            self.glyphs.button_label(c, [x, y, aw, kh], key, label, TXT, col);
        }
    }
}
