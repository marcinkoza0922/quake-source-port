//! Binding capture and explicit confirmation before replacing an occupied key.
use super::*;

impl Menu {
    pub(super) fn request_binding(&mut self, key: i32, cmd: &'static str, out: &mut Vec<Action>) {
        if self.bindings.get(key as usize).and_then(Option::as_deref).is_some_and(|old| !old.is_empty() && old != cmd) {
            self.bind_pending = Some((key, cmd));
        } else {
            self.apply_binding(key, cmd, out);
        }
    }

    fn apply_binding(&mut self, key: i32, cmd: &'static str, out: &mut Vec<Action>) {
        let same: Vec<_> = keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == is_pad_key(key)).collect();
        if same.len() >= 2 && !same.contains(&key) {
            for k in same {
                out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
                self.bindings[k as usize] = None;
            }
        }
        out.push(Action::Command(bind_cmd(key, cmd)));
        if let Some(b) = self.bindings.get_mut(key as usize) {
            *b = Some(cmd.into());
        }
        out.push(Action::Sound("misc/menu1.wav"));
    }

    pub(super) fn binding_key(&mut self, key: i32, out: &mut Vec<Action>) -> bool {
        if let Some((taken, cmd)) = self.bind_pending {
            let mouse = if key == K_MOUSE1 { self.mouse.filter(|&(x, y)| (180.0..460.0).contains(&x) && (214.0..239.0).contains(&y)) } else { None };
            let yes = matches!(key, 0x79 | 0x59 | K_ABUTTON) || mouse.is_some_and(|(x, _)| x < 320.0);
            let no = matches!(key, 0x6e | 0x4e | K_ESCAPE | K_BBUTTON | K_MOUSE2 | K_ENTER | K_KP_ENTER) || mouse.is_some_and(|(x, _)| x >= 320.0);
            if yes || no {
                self.bind_pending = None;
                if yes {
                    self.apply_binding(taken, cmd, out);
                }
            }
            return true;
        }
        if let Some(cmd) = self.addon_capture.take() {
            if key != K_ESCAPE {
                self.request_binding(key, cmd, out);
            }
            return true;
        }
        false
    }

    pub(super) fn clear_addon_binding(&mut self, cmd: &str, pad: bool, out: &mut Vec<Action>) {
        for k in keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == pad) {
            out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
            self.bindings[k as usize] = None;
        }
    }

    /// Preserve the other device's bindings while the active column flashes.
    pub(super) fn draw_binding_columns(&self, c: &mut Canvas, cmd: &str, y: f32,
        columns: [(f32, f32); 2], capturing: bool, blink: bool) {
        let keys = keys_for(&self.bindings, cmd);
        for (pad, (mut x, edge)) in [false, true].into_iter().zip(columns) {
            if capturing && pad == self.pad_ui {
                if blink {
                    c.text(x, y + 2.0, if pad { "Press button..." } else { "Press key..." }, 8.0, GOLD_FG);
                }
                continue;
            }
            let start = x;
            for key in keys.iter().copied().filter(|&key| is_pad_key(key) == pad) {
                if x + crate::glyphs::width(c, key, self.glyphs.family, crate::glyphs::H) < edge {
                    x += self.glyphs.key(c, x, y, key) + 3.0;
                }
            }
            if x == start {
                c.text(x, y + 1.0, "---", TXT, MUTED);
            }
        }
    }

    pub(super) fn draw_binding_prompt(&self, c: &mut Canvas) {
        let Some((key, _)) = self.bind_pending else { return };
        c.menu_layout();
        c.fill(110.0, 152.0, 420.0, 94.0, [0.04, 0.04, 0.04, 1.0]);
        c.frame(110.0, 152.0, 420.0, 94.0, BORDER);
        let label = "is already assigned";
        let w = crate::glyphs::width(c, key, self.glyphs.family, crate::glyphs::HINT_H)
            + 5.0 + c.text_w(label, TXT);
        let x = 320.0 - w / 2.0;
        let kw = self.glyphs.key_h(c, x, 159.0, key, crate::glyphs::HINT_H);
        c.text(x + kw + 5.0, 162.0, label, TXT, GOLD_FG);
        let old = self.bindings.get(key as usize).and_then(Option::as_deref).unwrap_or("");
        let old: String = old.chars().take(65).collect();
        c.text_center(320.0, 179.0, &old, TXT, FG);
        c.text_center(320.0, 196.0, "Replace this binding?", TXT, FG);
        let (yes, no) = if self.pad_ui { (K_ABUTTON, K_BBUTTON) } else { (b'y' as i32, K_ENTER) };
        self.glyphs.hints_centered(c, 250.0, 217.0, &[(yes, "Yes")]);
        self.glyphs.hints_centered(c, 390.0, 217.0, &[(no, "No")]);
    }
}

#[cfg(test)]
mod tests;
