//! Add-ons in the menus: the New Game checkbox, the add-on picker shown when
//! it is ticked, and the locks that keep a game started without add-ons
//! from gaining them later.

use super::*;

/// Clickable checkbox + label beside New Game: (x, y, w, h), classic menu units.
const NG_BOX: (f32, f32, f32, f32) = (224.0, 31.0, 84.0, 14.0);

/// Whether this cvar selects a toy.
pub(super) fn is_toy_cvar(cvar: &str) -> bool {
    qserver::addons::Toy::ALL.iter().any(|t| t.cvar() == cvar)
}

impl Menu {
    /// The running game was started without add-ons, so its toys cannot be
    /// changed (they are fixed when a game starts).
    pub(super) fn toys_locked(&self, cv: &Cvars) -> bool {
        self.game_running && cv.value_of("addons_active") == 0.0
    }

    /// Flips the New Game "Add-ons" checkbox.
    pub(super) fn toggle_addons_box(cv: &mut Cvars, out: &mut Vec<Action>) {
        let on = cv.value_of("addons_enable") != 0.0;
        cv.set("addons_enable", if on { "0" } else { "1" });
        out.push(Action::Sound("misc/menu3.wav"));
    }

    /// New Game pressed: straight in, or via the picker when add-ons are ticked.
    pub(super) fn new_game(&mut self, cv: &Cvars, out: &mut Vec<Action>) {
        if cv.value_of("addons_enable") != 0.0 {
            self.opt_cursor = 0;
            self.screen = Screen::OptionsPage(Page::NewGameAddons);
        } else {
            self.start_new_game(out);
        }
    }

    pub(super) fn start_new_game(&mut self, out: &mut Vec<Action>) {
        self.screen = Screen::None;
        out.push(Action::Close);
        out.push(Action::Command("maxplayers 1\ndeathmatch 0\ncoop 0\nmap start\n".into()));
    }

    /// Pointer over the New Game checkbox (`pos` in screen menu units).
    pub(super) fn on_new_game_box(&self, (x, y): (f32, f32)) -> bool {
        let (bx, by, bw, bh) = NG_BOX;
        self.screen == Screen::SinglePlayer
            && (CLASSIC.0 + bx..CLASSIC.0 + bx + bw).contains(&x)
            && (CLASSIC.1 + by..CLASSIC.1 + by + bh).contains(&y)
    }

    /// Checkbox beside the New Game row (drawn in classic-offset space).
    pub(super) fn draw_new_game_box(&self, c: &mut Canvas, cv: &Cvars) {
        let (bx, by, ..) = NG_BOX;
        let on = cv.value_of("addons_enable") != 0.0;
        let hot = self.sp_cursor == 0;
        c.round_box(bx, by + 3.0, 9.0, 9.0, 1.5, [0.0, 0.0, 0.0, 0.6], Some(BORDER));
        if on {
            c.round_box(bx + 2.0, by + 5.0, 5.0, 5.0, 1.0, GOLD_FG, None);
        }
        c.text(bx + 14.0, by + 3.0, "Add-ons", TXT, if on || hot { GOLD_FG } else { MUTED });
    }

    /// Explanatory lines under an add-on page's rows.
    pub(super) fn draw_addon_notes(&self, c: &mut Canvas, cv: &Cvars, page: Page, rows: usize) {
        let y = PAGE_ROW0 + rows as f32 * ROW_H + 34.0;
        let lines: &[(&str, [f32; 4])] = match page {
            Page::NewGameAddons => &[
                ("Pick the add-ons for this game, then Start New Game.", FG),
                ("Warning: saves made with add-ons enabled (.qsa files) do not", GOLD_FG),
                ("work in other source ports, and can't be loaded without them.", GOLD_FG),
                ("You can change which add-ons are on at any time during the game.", MUTED),
            ],
            Page::Addons if self.toys_locked(cv) => &[
                ("This game was started without add-ons, so they can't be turned on", GOLD_FG),
                ("for it. Tick Add-ons beside New Game to start one that has them.", GOLD_FG),
            ],
            Page::Addons if self.game_running => &[
                ("Changes apply to this game and are saved with it (.qsa).", MUTED),
                ("Saves of this game will not work in other source ports.", MUTED),
            ],
            Page::Addons => &[
                ("Add-ons are chosen when a game starts: tick Add-ons beside", MUTED),
                ("New Game. Pick what a new game starts with here.", MUTED),
            ],
            _ => return,
        };
        for (i, (s, col)) in lines.iter().enumerate() {
            c.text_center(320.0, y + i as f32 * 11.0, s, TXT, *col);
        }
    }
}
