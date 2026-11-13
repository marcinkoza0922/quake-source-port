use super::*;

impl Menu {
    pub(super) fn autoaim_defaults(&mut self, out: &mut Vec<Action>) {
        self.autoaim_conflicts.clear();
        for key in [K_MOUSE2, K_LTRIGGER] {
            // Preserve a user's existing lock binding for each device kind.
            if keys_for(&self.bindings, "+lockon").iter().any(|&k| is_pad_key(k) == is_pad_key(key)) {
                continue;
            }
            match self.bindings[key as usize].as_deref() {
                None | Some("") => {
                    self.bindings[key as usize] = Some("+lockon".into());
                    out.push(Action::Command(bind_cmd(key, "+lockon")));
                }
                Some(_) => self.autoaim_conflicts.push(key),
            }
        }
    }

    pub(super) fn autoaim_prompt_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) {
        if matches!(key, K_LEFTARROW | K_RIGHTARROW | K_DPAD_LEFT | K_DPAD_RIGHT) {
            self.autoaim_yes = !self.autoaim_yes;
        }
        if key == K_MOUSE1 && let Some((x, y)) = self.mouse {
            if !(215.0..425.0).contains(&x) || !(214.0..248.0).contains(&y) { return; }
            self.autoaim_yes = x < 320.0;
        }
        let yes = matches!(key, 0x79 | 0x59)
            || (matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1) && self.autoaim_yes);
        let no = matches!(key, 0x6e | 0x4e | K_ESCAPE | K_BBUTTON | K_MOUSE2)
            || (matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1) && !self.autoaim_yes);
        if yes || no {
            self.autoaim_prompt = false;
            self.autoaim_conflicts.clear();
            if yes {
                self.sub_return = None;
                self.screen = Screen::OptionsPage(Page::Controls);
                self.opt_cursor = 1;
                self.opt_scroll.set(0);
            } else {
                // Resume the Back action that opened the prompt.
                out.extend(self.key(K_ESCAPE, cv, false));
            }
        }
    }

    pub(super) fn draw_autoaim_prompt(&self, c: &mut Canvas) {
        c.fill(90.0, 160.0, 460.0, 92.0, [0.04, 0.04, 0.04, 1.0]);
        c.frame(90.0, 160.0, 460.0, 92.0, BORDER);
        let keys = self.autoaim_conflicts.iter().map(|&k| keynum_to_string(k)).collect::<Vec<_>>().join(" / ");
        c.text_center(320.0, 172.0, &format!("{keys} already assigned"), TXT, FG);
        c.text_center(320.0, 194.0, "Open Controls to remap Lock-on?", TXT, FG);
        c.text_center(270.0, 226.0, "Yes", TXT, if self.autoaim_yes { GOLD_FG } else { MUTED });
        c.text_center(370.0, 226.0, "No", TXT, if self.autoaim_yes { MUTED } else { GOLD_FG });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_conflicts_and_back_opens_controls_on_yes() {
        let mut m = Menu::new();
        let mut cv = Cvars::new();
        cv.register("assist_autoaim", "0", 0);
        m.bindings[K_MOUSE2 as usize] = Some("+jump".into());
        let mut out = Vec::new();
        m.adjust(toggle("Auto-aim", "assist_autoaim"), 1, true, &mut cv, &mut out);
        assert_eq!(m.bindings[K_MOUSE2 as usize].as_deref(), Some("+jump"));
        assert_eq!(m.bindings[K_LTRIGGER as usize].as_deref(), Some("+lockon"));
        m.screen = Screen::OptionsPage(Page::Assists);
        m.key(K_ESCAPE, &mut cv, false);
        assert!(m.autoaim_prompt);
        m.key(b'y' as i32, &mut cv, false);
        assert_eq!(m.screen, Screen::OptionsPage(Page::Controls));
        assert!(matches!(m.page_item(Page::Controls, m.opt_cursor), Item::Bind { cmd: "+lockon", .. }));
    }

    #[test]
    fn declining_conflict_returns_without_changing_binding() {
        let mut m = Menu::new();
        let mut cv = Cvars::new();
        cv.register("assist_autoaim", "1", 0);
        m.bindings[K_MOUSE2 as usize] = Some("+jump".into());
        m.autoaim_conflicts.push(K_MOUSE2);
        m.screen = Screen::OptionsPage(Page::Assists);
        m.key(K_ESCAPE, &mut cv, false);
        m.key(b'n' as i32, &mut cv, false);
        assert!(!m.autoaim_prompt);
        assert_ne!(m.screen, Screen::OptionsPage(Page::Assists));
        assert_eq!(m.bindings[K_MOUSE2 as usize].as_deref(), Some("+jump"));
    }
}
