//! Save/Load cards: one column of saves, newest first, with a screenshot
//! thumbnail, stats and inventory; plus deleting a save from the list.
use super::*;

/// Thumbnail size on a card (16:9), right-aligned.
const THUMB: (f32, f32) = (96.0, 54.0);
/// Delete button on the selected card: size, offset from the card's top right.
const DEL: (f32, f32) = (44.0, 12.0);

/// A title cut to `max` characters, control characters made visible.
pub(super) fn clean_title(t: &str, max: usize) -> String {
    let mut s: String = t.chars().map(|ch| if ch.is_control() { '?' } else { ch }).collect();
    if s.chars().count() > max {
        s = s.chars().take(max - 1).collect::<String>() + "…";
    }
    s
}

/// Short label for what kind of save a card holds.
fn kind_label(sv: &SaveSlot) -> String {
    if sv.is_autosave() {
        format!("Autosave: {}", sv.info.reason.as_deref().unwrap_or("unknown trigger"))
    } else if sv.name.eq_ignore_ascii_case("quick") {
        "Quicksave".into()
    } else if sv.name.len() > 1 && sv.name.starts_with('s') && sv.name[1..].bytes().all(|b| b.is_ascii_digit()) {
        "Save".into()
    } else {
        sv.name.clone()
    }
}

impl Menu {
    pub(super) fn ask_delete(&mut self, sv: SaveSlot) {
        let title = format!("Delete \"{}\"?", clean_title(&sv.info.level, 40));
        let what = format!("{} - {}", kind_label(&sv), sv.info.when(qcore::time::SystemTime::now()));
        self.confirm(vec![title, what, "This cannot be undone.".into()], "Delete", format!("_deletesave \"{}\"\n", sv.name), false);
    }

    /// Delete button of the selected card: (x, y, w, h), if it shows.
    fn delete_button(&self) -> Option<(f32, f32, f32, f32)> {
        let i = self.load_cursor;
        self.save_at(i)?;
        let y = self.card_y(i) + CARD_H - DEL.1 - 5.0;
        let (top, bottom) = if i < self.card_sticky() { (CARD_Y0, CARD_Y0 + CARD_H) } else { (self.card_top(), CARD_BOTTOM) };
        (self.card_visible(i) && y >= top && y + DEL.1 <= bottom)
            .then_some((CARD_X + CARD_W - 12.0 - THUMB.0 - 8.0 - DEL.0, y, DEL.0, DEL.1))
    }

    pub(super) fn on_delete_button(&self, (x, y): (f32, f32)) -> bool {
        self.delete_button().is_some_and(|(bx, by, bw, bh)| (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y))
    }

    /// Save/load cards: one column, scrolled so the cursor is in view, with
    /// the next card cut off at the bottom to show the list goes on.
    pub(super) fn draw_cards(&self, c: &mut Canvas, pic: &mut PicFn) {
        let heading = if self.screen == Screen::Save { "Save Game" } else if self.autosaves { "Autosaves" } else { "Load Game" };
        c.text(CARD_X, 20.0, heading, 13.0, GOLD_FG);
        c.text(CARD_X + c.text_w(heading, 13.0) + 10.0, 23.0, "newest first", TXT, MUTED);
        let n = self.save_rows();
        let real = if self.screen == Screen::Save || self.autosaves { self.saves.len() } else { self.saves.len() + usize::from(self.latest.is_some()) };
        c.text_right(CARD_X + CARD_W, 23.0, &format!("{real} save{}", if real == 1 { "" } else { "s" }), TXT, MUTED);
        let small = 8.5;
        let sticky = self.card_sticky();
        if sticky > 0 {
            // divider between the pinned newest save and the scrolling rest
            let y = CARD_Y0 + CARD_H + 3.0;
            c.fill(CARD_X, y, CARD_W, 1.0, [0.55, 0.40, 0.20, 0.8]);
        }
        let now = qcore::time::SystemTime::now();
        for i in 0..n {
            if !self.card_visible(i) {
                continue;
            }
            let y = self.card_y(i);
            c.clip_rows(Some(if i < sticky { (CARD_Y0 - 1.0, CARD_Y0 + CARD_H + 1.0) } else { (self.card_top(), CARD_BOTTOM) }));
            let x = CARD_X;
            let selected = i == self.load_cursor;
            let bg = if selected { [0.30, 0.20, 0.09, 0.97] } else { [0.08, 0.07, 0.06, 0.94] };
            let edge = if selected { [0.85, 0.59, 0.22, 0.9] } else { [0.35, 0.27, 0.16, 0.7] };
            c.round_box(x, y, CARD_W, CARD_H, 4.0, bg, Some(edge));
            if selected {
                c.round_box(x + 3.0, y + 6.0, 2.5, CARD_H - 12.0, 1.25, [0.95, 0.66, 0.26, 1.0], None);
            }
            let (lx, rx) = (x + 14.0, x + CARD_W - 12.0);
            if self.is_browse_row(i) {
                c.text(lx, y + 9.0, "Browse Autosaves", 11.0, FG);
                c.text(lx, y + 27.0, "Every retained autosave, newest first", small, MUTED);
                c.shape(rx - 10.0, y + CARD_H / 2.0 - 5.0, 10.0, 10.0, crate::font::Shape::ArrowRight, GOLD_FG);
                continue;
            }
            let Some(slot) = self.save_at(i) else {
                let (label, sub) = if self.screen == Screen::Save { ("New save", "Select to save your game here") }
                    else if self.autosaves { ("No autosaves yet", "Autosaves are made on level entry, secrets and more (Options > Gameplay)") }
                    else { ("No saves yet", "Save from the Single Player menu, or press F6 to quicksave") };
                c.text(lx, y + 9.0, label, 11.0, if self.screen == Screen::Save { FG } else { MUTED });
                c.text(lx, y + 27.0, sub, small, MUTED);
                if self.screen == Screen::Save {
                    c.shape(rx - 10.0, y + CARD_H / 2.0 - 5.0, 10.0, 10.0, crate::font::Shape::ArrowRight, GOLD_FG);
                }
                continue;
            };
            let sv = &slot.info;
            // thumbnail on the right
            let (tx, ty) = (rx - THUMB.0, y + (CARD_H - THUMB.1) / 2.0);
            c.fill(tx, ty, THUMB.0, THUMB.1, [0.0, 0.0, 0.0, 0.6]);
            if let Some((id, ..)) = slot.thumb {
                let (px, py) = ((c.ox + tx) * c.scale, (c.oy + ty) * c.scale);
                c.push(qrender::Draw2D::Pic { x: px, y: py, w: THUMB.0 * c.scale, h: THUMB.1 * c.scale, pic: id, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0; 4] });
            } else {
                c.text_center(tx + THUMB.0 / 2.0, ty + THUMB.1 / 2.0 - 4.0, "No picture", small, MUTED);
            }
            c.frame(tx, ty, THUMB.0, THUMB.1, edge);
            let right = tx - 8.0;
            let mut kind = kind_label(slot);
            if i < sticky {
                kind = format!("Latest - {kind}");
            }
            let kind = clean_title(&kind, 40);
            c.text_right(right, y + 7.0, &kind, small, GOLD_FG);
            // title and map file, cut to the space left of the label
            let room = right - lx - c.text_w(&kind, small) - 12.0;
            let map = format!("  {}", sv.map);
            let mut title = clean_title(&sv.level, 60);
            let full = title.clone();
            while c.text_w(&title, 11.0) + c.text_w(&map, small) > room && title.pop().is_some() {}
            if title.len() < full.len() {
                title.push('…');
            }
            let tw = c.text(lx, y + 5.0, &title, 11.0, FG);
            if !sv.map.eq_ignore_ascii_case(&sv.level) {
                c.text(lx + tw, y + 7.0, &map, small, MUTED);
            }
            let line = format!("{}   {}   Time {}", sv.when(now), sv.skill_name(), sv.level_time());
            c.text(lx, y + 20.0, &line, small, MUTED);
            let hw = c.text(lx, y + 32.0, &format!("Health {}   Armor {}", sv.health, sv.armor), small, FG);
            // .qsa saves (customized or split-screen games) that other
            // ports cannot load
            let mp = match sv.players {
                (local, _) if local > 1 => format!("Split-screen, {local} players"),
                (_, slots) if slots > 1 => "Co-op".to_string(),
                _ => String::new(),
            };
            let tag = match (sv.addons.is_some(), mp.is_empty()) {
                (true, false) => format!("Customized, {mp}"),
                (true, true) => "Customized".to_string(),
                (false, _) => mp,
            };
            c.text(lx + hw + 12.0, y + 32.0, &tag, small, GOLD_FG);
            let stats = format!("Kills {}/{}   Secrets {}/{}", sv.kills.0, sv.kills.1, sv.secrets.0, sv.secrets.1);
            let sw = c.text(lx, y + 44.0, &stats, small, FG);
            // keys after the stats line, weapons between the text and the thumbnail
            let mut kx = lx + sw + 10.0;
            for icon in &self.save_inventory.keys(sv) {
                if pic(&icon.name).is_some() {
                    draw_inventory_icon(c, pic, kx, y + 42.0, 24.0, 12.0, icon);
                    kx += 26.0;
                } else {
                    kx += c.text(kx, y + 44.0, &icon.fallback, small, GOLD_FG) + 6.0;
                }
            }
            // everyone has the axe; it has no status-bar picture
            let icons: Vec<_> = self.save_inventory.weapons(sv).into_iter()
                .filter(|icon| pic(&icon.name).is_some() || !matches!(icon.name.as_str(), "inv_axe" | "inv_wrench"))
                .collect();
            let wx = x + 222.0;
            let cell = if icons.len() <= 10 { 22.0 } else { 16.0 };
            let columns = (((right - wx) / cell).floor() as usize).max(1);
            let mut missing = Vec::new();
            for (j, icon) in icons.iter().take(columns * 2).enumerate() {
                if pic(&icon.name).is_none() {
                    missing.push(icon.fallback.as_str());
                    continue;
                }
                draw_inventory_icon(c, pic, wx + (j % columns) as f32 * cell, y + 20.0 + (j / columns) as f32 * 14.0, cell - 2.0, 12.0, icon);
            }
            if !missing.is_empty() && !selected {
                c.text(wx, y + 46.0, &missing.join(", "), small, MUTED);
            }
        }
        c.clip = None;
        if let Some((bx, by, bw, bh)) = self.delete_button() {
            let hover = self.mouse.is_some_and(|m| self.on_delete_button(m));
            c.round_box(bx, by, bw, bh, 3.0, if hover { [0.55, 0.12, 0.08, 0.95] } else { [0.0, 0.0, 0.0, 0.6] }, Some(BORDER));
            c.text_center(bx + bw / 2.0, by + 2.0, "Delete", small, if hover { GOLD_FG } else { MUTED });
        }
        if !self.pad_ui && self.save_at(self.load_cursor).is_some() {
            c.text_right(CARD_X + CARD_W, CARD_BOTTOM + 8.0, "Del: delete save", TXT, MUTED);
        }
        if let Some((x, top, h, ty, th)) = self.card_scrollbar() {
            c.fill(x, top, 3.0, h, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, if self.drag.is_some() { 1.0 } else { 0.9 }]);
        }
    }
}
