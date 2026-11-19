//! The launch pop-up shown before a new game starts (Single Player → New
//! Game, or a map picked in Level Select / Map Packs): a big Play button
//! (tagged "Custom" while modifiers or add-ons are on), Customize Experience
//! (the New Game Customize hub, which returns here) and a shortcut that turns
//! every modification off (Del / Backspace, gamepad X, or a click).
//!
//! Play fixes whether the game is customized (`custom_enable`, read by `map`):
//! it is when any modifier differs from its default or any add-on is on.
use super::*;

/// A new game waiting on the launch pop-up.
#[derive(Clone, Debug)]
pub(super) struct Launch {
    /// Command that starts the game.
    pub cmd: String,
    /// Ask for the skill before starting (Level Select, Map Packs).
    pub skill: bool,
    /// Screen the pop-up sits over (and returns to after Customize).
    pub from: Screen,
    /// Second heading line: the map's title, when known.
    pub title: Option<String>,
}

/// The New Game command (the start map picks the skill in-world).
pub(super) const NEW_GAME_CMD: &str = "maxplayers 1\ndeathmatch 0\ncoop 0\nmap start\n";

/// The map a Level Select / Map Packs command starts (`game "dir" "map"`).
pub(super) fn map_of(cmd: &str) -> Option<&str> {
    cmd.trim().split('"').nth(3).filter(|m| !m.is_empty())
}

/// Pop-up box: x, y, w, h (menu units).
const BOX: (f32, f32, f32, f32) = (180.0, 96.0, 280.0, 140.0);
const BTN_X: f32 = BOX.0 + 20.0;
const BTN_W: f32 = BOX.2 - 40.0;
/// Play, Customize Experience, Disable All Modifications: top and height.
const BTNS: [(f32, f32); 3] = [(BOX.1 + 38.0, 34.0), (BOX.1 + 78.0, 18.0), (BOX.1 + 102.0, 16.0)];

/// Every add-on the New Game browser can switch on (name, cvar).
fn toy_toggles() -> impl Iterator<Item = (&'static str, &'static str)> {
    super::addon_browser::toggles()
}

/// Names of the add-ons that are on.
pub(super) fn addons_on(cv: &Cvars) -> Vec<&'static str> {
    toy_toggles().filter(|t| cv.value_of(t.1) != 0.0).map(|t| t.0).collect()
}

/// The next new game would be customized: a modifier changed or an add-on on.
pub(super) fn customized(cv: &Cvars) -> bool {
    // unregistered cvars (menu tests) read 0, not their default
    qserver::modifiers::changed(cv).any(|(n, _)| cv.by_name(n).is_some()) || toy_toggles().any(|t| cv.value_of(t.1) != 0.0)
}

/// Standard rules and no add-ons (per-add-on settings like slots are kept).
pub(super) fn disable_all(cv: &mut Cvars) {
    for &(name, default, _) in qserver::modifiers::CVARS {
        cv.set(name, default);
    }
    for (_, cvar) in toy_toggles() {
        cv.set(cvar, "0");
    }
}

impl Menu {
    /// Shows the launch pop-up over the current screen.
    pub(super) fn open_launch(&mut self, cmd: String, skill: bool, title: Option<String>) {
        self.launch = Some(Launch { cmd, skill, from: self.screen, title });
        self.prompt = Some(Prompt::Launch { cursor: 0 });
    }

    /// `menu_newgame`: Single Player with the New Game pop-up open.
    pub fn open_new_game(&mut self) {
        self.open(Screen::SinglePlayer);
        self.sp_cursor = 0;
        self.open_launch(NEW_GAME_CMD.into(), false, None);
    }

    /// Back from the New Game Customize hub: the pop-up again, on Customize.
    pub(super) fn back_to_launch(&mut self) {
        self.new_game_setup = false;
        match &self.launch {
            Some(l) => {
                self.screen = l.from;
                self.prompt = Some(Prompt::Launch { cursor: 1 });
            }
            None => self.screen = Screen::SinglePlayer,
        }
    }

    /// Play: fixes vanilla or customized, then starts (asking for the skill
    /// or confirming the end of a running game when needed).
    pub(super) fn launch_play(&mut self, cv: &mut Cvars, out: &mut Vec<Action>) {
        let l = self.launch.take().unwrap_or_else(|| Launch { cmd: NEW_GAME_CMD.into(), skill: false, from: Screen::SinglePlayer, title: None });
        cv.set("custom_enable", if customized(cv) { "1" } else { "0" });
        self.new_game_setup = false;
        self.prompt = None;
        self.screen = l.from;
        if l.skill {
            self.ask_skill(l.cmd, cv);
        } else if self.game_running {
            // vanilla asks before dropping the running game
            self.confirm(vec![qcore::tr!("Start a new game?"), qcore::tr!("Unsaved progress will be lost.")], qcore::t!("New game"), l.cmd, true);
        } else {
            self.screen = Screen::None;
            out.push(Action::Close);
            out.push(Action::Command(l.cmd));
        }
    }

    /// Buttons shown: Disable All only while something is modified.
    fn launch_buttons(cv: &Cvars) -> usize {
        if customized(cv) { 3 } else { 2 }
    }

    pub(super) fn launch_at(&self, (x, y): (f32, f32), cv: &Cvars) -> Option<usize> {
        if !(BTN_X..BTN_X + BTN_W).contains(&x) {
            return None;
        }
        BTNS[..Self::launch_buttons(cv)].iter().position(|&(top, h)| (top..top + h).contains(&y))
    }

    pub(super) fn launch_hover(&mut self, pos: (f32, f32), cv: &Cvars) -> Option<&'static str> {
        let i = self.launch_at(pos, cv)?;
        let Some(Prompt::Launch { cursor }) = self.prompt.as_mut() else { return None };
        (std::mem::replace(cursor, i) != i).then_some("misc/menu1.wav")
    }

    pub(super) fn launch_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) {
        let Some(Prompt::Launch { cursor }) = self.prompt else { return };
        let n = Self::launch_buttons(cv);
        let mut cursor = cursor.min(n - 1);
        let pick = match key {
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => {
                self.prompt = None;
                self.launch = None;
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
            K_MOUSE1 => self.mouse.and_then(|m| self.launch_at(m, cv)),
            K_DEL | K_BACKSPACE | K_XBUTTON => Some(2),
            _ => None,
        };
        self.prompt = Some(Prompt::Launch { cursor });
        match pick {
            Some(0) => {
                out.push(Action::Sound("misc/menu2.wav"));
                self.launch_play(cv, out);
            }
            Some(1) => {
                out.push(Action::Sound("misc/menu2.wav"));
                self.prompt = None;
                self.open_customize(true);
            }
            Some(2) if customized(cv) => {
                disable_all(cv);
                self.prompt = Some(Prompt::Launch { cursor: 0 });
                out.push(Action::Sound("misc/menu3.wav"));
            }
            _ => {}
        }
    }

    pub(super) fn draw_launch(&self, c: &mut Canvas, cv: &Cvars, cursor: usize) {
        let (x, y, w, h) = BOX;
        let custom = customized(cv);
        let launch = self.launch.as_ref();
        c.fill(-c.ox, -c.oy, c.vw(), c.vh(), [0.0, 0.0, 0.0, 0.5]);
        c.round_box(x, y, w, h, 5.0, [0.06, 0.05, 0.04, 0.98], Some(BORDER));
        c.text_center(320.0, y + 10.0, "New Game", TXT, GOLD_FG);
        if let Some(t) = launch.and_then(|l| l.title.as_deref()) {
            c.text_center(320.0, y + 22.0, &clean_title(t, 44), TXT, FG);
        }
        let button = |c: &mut Canvas, i: usize, big: bool| {
            let (top, bh) = BTNS[i];
            let on = cursor == i;
            let fill = if on { HILITE } else if big { [0.25, 0.16, 0.06, 0.6] } else { [0.0, 0.0, 0.0, 0.7] };
            c.round_box(BTN_X, top, BTN_W, bh, 4.0, fill, Some(if on { GOLD_FG } else { BORDER }));
        };
        // Play: the big default button, tagged while the game would be customized
        let (top, bh) = BTNS[0];
        button(c, 0, true);
        c.text_center(320.0, top + (bh - 16.0) / 2.0, "Play", 16.0, if cursor == 0 { GOLD_FG } else { FG });
        if custom {
            let (tw, th) = (c.text_w("CUSTOM", 7.0) + 10.0, 11.0);
            let (tx, ty) = (BTN_X + BTN_W - tw - 8.0, top + (bh - th) / 2.0);
            c.round_box(tx, ty, tw, th, 3.0, GOLD_FG, None);
            c.text(tx + 5.0, ty + 2.0, "CUSTOM", 7.0, [0.08, 0.05, 0.02, 1.0]);
        }
        let (top, bh) = BTNS[1];
        button(c, 1, false);
        c.text_center(320.0, top + (bh - TXT) / 2.0, "Customize Experience", TXT, if cursor == 1 { GOLD_FG } else { FG });
        if custom {
            let (top, _) = BTNS[2];
            button(c, 2, false);
            let key = if self.pad_ui { K_XBUTTON } else { K_DEL };
            self.glyphs.hints_centered(c, 320.0, top + 1.5, &[(key, qcore::t!("Disable All Modifications"))]);
            c.text_center(320.0, BOX.1 + 128.0, &customize::custom_summary(cv), 8.0, MUTED);
        } else {
            c.text_center(320.0, BOX.1 + 106.0, "Standard rules, no add-ons.", 8.0, MUTED);
            c.text_center(320.0, BOX.1 + 118.0, "Saves load in other source ports too.", 8.0, MUTED);
        }
        if self.pad_ui {
            self.boxed_hints(c, 344.0, &[(K_ABUTTON, "Select"), (K_BBUTTON, "Cancel")]);
        }
    }
}

#[cfg(test)]
mod tests;
