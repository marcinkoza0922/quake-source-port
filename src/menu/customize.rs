//! Customize: the hub pages (gameplay modifiers and add-ons, reached from
//! the launch pop-up's Customize Experience or, in a customized game, the
//! main menu) and the locks that keep a vanilla game vanilla.
//! Whether a game is customized is fixed when it starts (`custom_enable`,
//! set by Play → `Core::custom`, mirrored in `custom_active`): only a customized game
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

/// New game → Customize Experience: set up the rules, then play (Back
/// returns to the launch pop-up).
pub(super) const NEW_GAME_CUSTOMIZE: &[Item] = &[
    Item::Sub { label: "Gameplay Modifiers", page: Page::Modifiers },
    Item::Sub { label: "Add-ons", page: Page::NewGameAddons },
    Item::Run { label: "Play", run: Run::NewGame },
];

/// Single Player rows: New Game, Load, Save, Level Select.
pub(super) const SP_ITEMS: usize = 4;

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

    /// Opens the Customize hub: for the next new game, or the running one.
    pub(super) fn open_customize(&mut self, new_game: bool) {
        self.new_game_setup = new_game;
        self.opt_cursor = 0;
        self.sub_return = None;
        self.screen = Screen::OptionsPage(if new_game { Page::NewGameCustomize } else { Page::Customize });
    }

    /// Main menu rows: the classic five, Map Packs, Demos, and Customize while a
    /// customized game runs.
    pub(super) fn main_items(&self) -> usize {
        MAIN_ITEMS + usize::from(self.custom_running) + usize::from(self.touch_shortcut)
    }

    /// Notes under the Customize pages' rows; returns the lines' height.
    pub(super) fn draw_custom_notes(&self, c: &mut Canvas, cv: &Cvars, page: Page) {
        let y = self.page_bottom(page) + 30.0;
        let summary = custom_summary(cv);
        let lines: Vec<(String, [f32; 4])> = match page {
            Page::NewGameCustomize if super::launch::customized(cv) => vec![
                (summary, FG),
                (qcore::tr!("Saves of this game are .qsa files: they need this source port."), GOLD_FG),
            ],
            Page::NewGameCustomize => vec![
                (summary, FG),
                (qcore::tr!("Nothing changed: this plays as standard Quake."), MUTED),
            ],
            Page::Customize => vec![
                (summary, FG),
                (qcore::tr!("Changes apply now and are saved with this game (.qsa)."), MUTED),
            ],
            Page::Modifiers if self.toys_locked(cv) => vec![
                (qcore::tr!("This game started without Customize, so its rules are fixed."), GOLD_FG),
                (qcore::tr!("Start a new game and choose Customize Experience."), GOLD_FG),
            ],
            Page::Modifiers if self.new_game_setup || !self.game_running => {
                vec![(qcore::tr!("These rules apply to the next customized game."), MUTED)]
            }
            Page::Modifiers => vec![(qcore::tr!("Changes apply now and are saved with this game (.qsa)."), MUTED)],
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
        0 => qcore::tr!("Standard rules"),
        n => qcore::tr!("Modifiers changed: {}", n),
    };
    let on = super::launch::addons_on(cv);
    let addons = match on.len() {
        0 => qcore::tr!("no add-ons"),
        1..=2 => qcore::tr!("add-ons: {}", on.iter().map(|a| qcore::lang::tr(a)).collect::<Vec<_>>().join(", ")),
        k => qcore::tr!("add-ons: {}", k),
    };
    format!("{mods}; {addons}")
}
