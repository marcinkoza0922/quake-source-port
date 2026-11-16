//! Customize: the Single Player "Customize" row, the hub pages (gameplay
//! modifiers and add-ons) and the locks that keep a vanilla game vanilla.
//! Whether a game is customized is fixed when it starts (`custom_enable` →
//! `Core::custom`, mirrored in `custom_active`): only a customized game
//! applies modifiers and add-ons, and only it saves as `.qsa`. Inside a
//! customized game every modifier and add-on stays editable.

use super::*;

pub(super) const DAMAGE_SCALE: &[(&str, f32)] = &[
    ("25%", 0.25), ("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("200%", 2.0), ("300%", 3.0),
];
pub(super) const AMMO_MULT: &[(&str, f32)] =
    &[("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("150%", 1.5), ("200%", 2.0), ("300%", 3.0), ("400%", 4.0)];
pub(super) const HEALTH_MULT: &[(&str, f32)] = &[("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("150%", 1.5), ("200%", 2.0)];
pub(super) const JUMPS: &[(&str, f32)] = &[("single", 1.0), ("double", 2.0), ("triple", 3.0)];
pub(super) const HORDE: &[(&str, f32)] = &[("off", 0.0), ("2x monsters", 1.0), ("3x monsters", 2.0), ("4x monsters", 3.0)];

/// Customize → Gameplay Modifiers: harder rules first, then easier ones.
pub(super) const MODIFIERS: &[Item] = &[
    Item::Choice { label: "Damage Taken", cvar: "modifier_damagetaken", choices: DAMAGE_SCALE },
    Item::Choice { label: "Enemy Health", cvar: "modifier_enemyhealth", choices: DAMAGE_SCALE },
    Item::Choice { label: "Ammo Pickups", cvar: "modifier_ammo", choices: AMMO_MULT },
    Item::Choice { label: "Health Pickups", cvar: "modifier_health", choices: HEALTH_MULT },
    Item::Choice { label: "Extra Monsters", cvar: "modifier_horde", choices: HORDE },
    toggle("Pistol Start", "modifier_pistolstart"),
    toggle("No Manual Saves", "modifier_nosaves"),
    toggle("Infinite Ammo", "modifier_infiniteammo"),
    toggle("Health Regeneration", "modifier_regen"),
    toggle("No Fall Damage", "modifier_nofalldamage"),
    toggle("No Drowning", "modifier_nodrown"),
    toggle("No Self Damage", "modifier_noselfdamage"),
    Item::Choice { label: "Jumps", cvar: "modifier_jumps", choices: JUMPS },
    Item::Run { label: "Reset to Defaults", run: Run::ResetModifiers },
];

/// Main menu → Customize, while a customized game runs.
pub(super) const CUSTOMIZE: &[Item] = &[
    Item::Sub { label: "Gameplay Modifiers", page: Page::Modifiers },
    Item::Sub { label: "Add-ons", page: Page::Addons },
];

/// New Game with Customize ticked: set up the rules, then start.
pub(super) const NEW_GAME_CUSTOMIZE: &[Item] = &[
    Item::Sub { label: "Gameplay Modifiers", page: Page::Modifiers },
    Item::Sub { label: "Add-ons", page: Page::NewGameAddons },
    Item::Run { label: "Start New Game", run: Run::NewGame },
];

/// Single Player row of the Customize checkbox (below Level Select).
pub(super) const SP_CUSTOMIZE_ROW: usize = 4;
/// Single Player rows: New Game, Load, Save, Level Select, Customize.
pub(super) const SP_ITEMS: usize = 5;

/// Whether this cvar selects a toy.
pub(super) fn is_toy_cvar(cvar: &str) -> bool {
    qserver::addons::Toy::ALL.iter().any(|t| t.cvar() == cvar) || matches!(cvar, "addon_dodge" | "addon_goggles" | "addon_projectile_shotguns" | "addon_bullet_time" | "addon_hitstop")
}

/// A per-game setting: a toy or a gameplay modifier.
pub(super) fn is_game_cvar(cvar: &str) -> bool {
    is_toy_cvar(cvar) || qserver::modifiers::CVARS.iter().any(|c| c.0 == cvar)
}

impl Menu {
    /// The running game is vanilla, so its modifiers and add-ons cannot be
    /// changed (fixed when a game starts). Not while setting up a new game.
    pub(super) fn toys_locked(&self, cv: &Cvars) -> bool {
        self.game_running && cv.value_of("custom_active") == 0.0 && !self.new_game_setup
    }

    /// A per-game row that the running game doesn't allow changing.
    pub(super) fn game_locked(&self, item: Item, cv: &Cvars) -> bool {
        match item {
            Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } | Item::Slider { cvar, .. } => is_game_cvar(cvar) && self.toys_locked(cv),
            _ => false,
        }
    }

    /// Flips the Single Player "Customize" checkbox (the next new game).
    pub(super) fn toggle_custom_box(cv: &mut Cvars, out: &mut Vec<Action>) {
        let on = cv.value_of("custom_enable") != 0.0;
        cv.set("custom_enable", if on { "0" } else { "1" });
        out.push(Action::Sound("misc/menu3.wav"));
    }

    /// New Game pressed: straight in, or via the Customize hub when ticked.
    pub(super) fn new_game(&mut self, cv: &Cvars, out: &mut Vec<Action>) {
        if cv.value_of("custom_enable") != 0.0 {
            self.open_customize(true);
        } else {
            self.start_new_game(out);
        }
    }

    /// Opens the Customize hub: for the next new game, or the running one.
    pub(super) fn open_customize(&mut self, new_game: bool) {
        self.new_game_setup = new_game;
        self.opt_cursor = 0;
        self.sub_return = None;
        self.screen = Screen::OptionsPage(if new_game { Page::NewGameCustomize } else { Page::Customize });
    }

    pub(super) fn start_new_game(&mut self, out: &mut Vec<Action>) {
        let cmd = "maxplayers 1\ndeathmatch 0\ncoop 0\nmap start\n".to_string();
        if self.game_running {
            // vanilla asks before dropping the running game
            self.confirm(vec!["Start a new game?".into(), "Unsaved progress will be lost.".into()], "New game", cmd, true);
            return;
        }
        self.screen = Screen::None;
        out.push(Action::Close);
        out.push(Action::Command(cmd));
    }

    /// Main menu rows: the classic five, Map Packs, and Customize while a
    /// customized game runs.
    pub(super) fn main_items(&self) -> usize {
        MAIN_ITEMS + usize::from(self.custom_running)
    }

    /// The Customize row below Level Select (classic-offset space).
    pub(super) fn draw_custom_row(&self, c: &mut Canvas, cv: &Cvars) {
        let (y, w) = (32.0 + SP_CUSTOMIZE_ROW as f32 * 20.0 + 2.0, 112.0);
        let on = cv.value_of("custom_enable") != 0.0;
        let focused = self.sp_cursor == SP_CUSTOMIZE_ROW;
        c.fill(72.0, y, w, 16.0, [0.0, 0.0, 0.0, 0.55]);
        c.frame(72.0, y, w, 16.0, if focused { GOLD_FG } else { BORDER });
        // checkbox + label in the classic gold font, like Level Select
        let (bx, by) = (84.0, y + 3.5);
        c.round_box(bx, by, 9.0, 9.0, 1.5, [0.0, 0.0, 0.0, 0.6], Some(if on || focused { GOLD_FG } else { BORDER }));
        if on {
            c.round_box(bx + 2.0, by + 2.0, 5.0, 5.0, 1.0, GOLD_FG, None);
        }
        gold(c, bx + 17.0, y + 4.0, "Customize");
        if focused {
            let lines = [
                "Pick gameplay modifiers (easier or harder rules) and",
                "add-ons for the next new game. Customized games save",
                "as .qsa files, which other source ports can't load.",
            ];
            let w = lines.iter().map(|s| c.text_w(s, TXT)).fold(0.0, f32::max);
            let (tx, ty) = (160.0 - w / 2.0, 32.0 + 6.0 * 20.0 + 12.0);
            c.round_box(tx - 8.0, ty - 4.0, w + 16.0, lines.len() as f32 * 11.0 + 6.0, 4.0, [0.0, 0.0, 0.0, 0.85], Some(BORDER));
            for (i, s) in lines.iter().enumerate() {
                c.text(tx, ty + i as f32 * 11.0, s, TXT, FG);
            }
        }
    }

    /// Notes under the Customize pages' rows; returns the lines' height.
    pub(super) fn draw_custom_notes(&self, c: &mut Canvas, cv: &Cvars, page: Page, rows: usize) {
        let y = PAGE_ROW0 + rows as f32 * ROW_H + 30.0;
        let summary = custom_summary(cv);
        let lines: Vec<(String, [f32; 4])> = match page {
            Page::NewGameCustomize => vec![
                (summary, FG),
                ("Saves of this game are .qsa files: they need this source port.".into(), GOLD_FG),
            ],
            Page::Customize => vec![
                (summary, FG),
                ("Changes apply now and are saved with this game (.qsa).".into(), MUTED),
            ],
            Page::Modifiers if self.toys_locked(cv) => vec![
                ("This game started without Customize, so its rules are fixed.".into(), GOLD_FG),
                ("Tick Customize on the Single Player menu, then start a new game.".into(), GOLD_FG),
            ],
            Page::Modifiers if self.new_game_setup || !self.game_running => {
                vec![("These rules apply to the next customized game.".into(), MUTED)]
            }
            Page::Modifiers => vec![("Changes apply now and are saved with this game (.qsa).".into(), MUTED)],
            _ => return,
        };
        for (i, (s, col)) in lines.iter().enumerate() {
            c.text_center(320.0, y + i as f32 * 11.0, s, TXT, *col);
        }
    }
}

/// "3 modifiers changed; add-ons: Portal Gun, Railgun" for the hub pages.
pub(super) fn custom_summary(cv: &Cvars) -> String {
    let n = qserver::modifiers::changed(cv).count();
    let mods = match n {
        0 => "Standard rules".to_string(),
        1 => "1 modifier changed".to_string(),
        n => format!("{n} modifiers changed"),
    };
    let on: Vec<&str> = page_items(Page::NewGameAddons)
        .iter()
        .filter_map(|i| match *i {
            Item::Toggle { label, cvar } if is_toy_cvar(cvar) && cv.value_of(cvar) != 0.0 => Some(label),
            _ => None,
        })
        .collect();
    let addons = match on.len() {
        0 => "no add-ons".to_string(),
        1..=2 => format!("add-ons: {}", on.join(", ")),
        k => format!("{k} add-ons"),
    };
    format!("{mods}; {addons}")
}
