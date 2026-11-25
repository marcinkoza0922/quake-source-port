//! Gameplay add-on parameters and explicit default reset actions.
use super::*;

pub(super) const SHOTGUNS: &[Item] = &[
    slider("Projectile Speed (units/s)", "addon_shotgun_speed", 100.0, 30000.0, 100.0),
    Item::Run { label: "Reset to Defaults", run: Run::ResetShotguns },
];
pub(super) const BULLET_TIME: &[Item] = &[
    Item::Choice { label: "Activation", cvar: "addon_bullet_time_toggle", choices: &[("Hold", 0.0), ("Toggle", 1.0)] },
    toggle("Use Bullet Time Meter", "addon_bullet_time_meter"),
    slider("Meter Duration (real seconds)", "addon_bullet_time_duration", 1.0, 60.0, 1.0),
    slider("Regen (seconds per second)", "addon_bullet_time_regen", 0.1, 10.0, 0.1),
    Item::Run { label: "Adjust HUD", run: Run::AdjustHud("addon_bullet_time") },
    Item::Run { label: "Reset to Defaults", run: Run::ResetBulletTime },
];

pub(super) const HITSTOP: &[Item] = &[
    Item::Choice { label: "Weapons", cvar: "addon_hitstop_weapons", choices: &[("All", 0.0), ("Melee only", 1.0), ("Ranged only", 2.0), ("Explosives only", 3.0)] },
    slider("Freeze Duration (ms)", "addon_hitstop_duration", 10.0, 250.0, 10.0),
    slider("Kill Bonus (x)", "addon_hitstop_kill_bonus", 1.0, 4.0, 0.25),
    slider("Screen Shake", "addon_hitstop_shake", 0.0, 1.0, 0.1),
    Item::Sub { label: "Advanced Settings", page: Page::HitstopAdvanced },
    Item::Run { label: "Reset to Defaults", run: Run::ResetHitstop },
];
pub(super) const HITSTOP_ADVANCED: &[Item] = &[
    Item::Choice { label: "Targets", cvar: "addon_hitstop_targets", choices: &[("All enemies", 0.0), ("Tough enemies only", 1.0)] },
    slider("Tough Enemy Health", "addon_hitstop_tough", 10.0, 1000.0, 10.0),
    slider("Extra Per 10 Damage (ms)", "addon_hitstop_scale_damage", 0.0, 50.0, 1.0),
    slider("Minimum Damage", "addon_hitstop_min_damage", 0.0, 100.0, 1.0),
    slider("Cooldown (ms)", "addon_hitstop_cooldown", 0.0, 1000.0, 10.0),
    slider("Longest Freeze (ms)", "addon_hitstop_max", 50.0, 1000.0, 10.0),
    Item::Run { label: "Reset to Defaults", run: Run::ResetHitstop },
];

/// Reached from the Add-ons pages: akimbo pistols.
pub(super) const AKIMBO: &[Item] = &[
    slider("Damage", "addon_akimbo_damage", 5.0, 100.0, 1.0),
    slider("Spread", "addon_akimbo_spread", 0.0, 10.0, 0.5),
    slider("Magazine Size", "addon_akimbo_mag", 2.0, 40.0, 1.0),
    slider("Reload Time", "addon_akimbo_reload", 0.4, 4.0, 0.1),
    toggle("Reload When Empty", "addon_akimbo_autoreload"),
    toggle("Swap Pistols", "addon_akimbo_swap"),
    Item::Run { label: "Adjust HUD", run: Run::AdjustHud("addon_akimbo") },
];
/// Reached from the Add-ons pages: assault rifle.
pub(super) const RIFLE: &[Item] = &[
    slider("Damage", "addon_rifle_damage", 5.0, 100.0, 1.0),
    slider("Magazine Size", "addon_rifle_mag", 10.0, 100.0, 5.0),
    slider("Reload Time", "addon_rifle_reload", 0.5, 5.0, 0.1),
    toggle("Reload When Empty", "addon_rifle_autoreload"),
    Item::Choice { label: "Aim Button", cvar: "addon_rifle_aim", choices: &[("Hold", 0.0), ("Toggle", 1.0)] },
    slider("Sight Zoom", "addon_rifle_zoom", 1.0, 2.5, 0.1),
    Item::Choice { label: "Dot Colour", cvar: "addon_rifle_dot", choices: &[("Red", 0.0), ("Green", 1.0), ("Yellow", 2.0)] },
    slider("Recoil", "addon_rifle_recoil", 0.0, 2.0, 0.1),
    Item::Run { label: "Adjust HUD", run: Run::AdjustHud("addon_rifle") },
];

/// Customize → Add-ons: which toys the running customized game (or the next new
/// game) has. Locked while the running game was started without add-ons.
pub(super) const ADDONS: &[Item] = &[
    toggle("Portal Gun", "addon_portalgun"),
    Item::Sub { label: "Portal Gun Settings", page: Page::PortalGun },
    toggle("Boomerang", "addon_boomerang"),
    Item::Sub { label: "Boomerang Settings", page: Page::Boomerang },
    toggle("Stickybomb Launcher", "addon_sticky"),
    toggle("Hookshot", "addon_hookshot"),
    Item::Sub { label: "Hookshot Settings", page: Page::Hookshot },
    toggle("Railgun", "addon_railgun"),
    Item::Sub { label: "Railgun Settings", page: Page::Railgun },
    toggle("Goggles", "addon_goggles"),
    Item::Sub { label: "Goggles Settings", page: Page::Goggles },
    toggle("Dodge / Dive", "addon_dodge"),
    toggle("Bullet Time", "addon_bullet_time"),
    Item::Sub { label: "Bullet Time Settings", page: Page::BulletTime },
    toggle("Projectile Shotguns", "addon_projectile_shotguns"),
    Item::Sub { label: "Projectile Shotguns Settings", page: Page::ProjectileShotguns },
    toggle("Hitstop", "addon_hitstop"),
    Item::Sub { label: "Hitstop Settings", page: Page::Hitstop },
];

/// New game → Customize Experience → Add-ons: choose the toys, then play.
pub(super) const NEW_GAME_ADDONS: &[Item] = &[
    toggle("Portal Gun", "addon_portalgun"),
    Item::Sub { label: "Portal Gun Settings", page: Page::PortalGun },
    toggle("Boomerang", "addon_boomerang"),
    Item::Sub { label: "Boomerang Settings", page: Page::Boomerang },
    toggle("Stickybomb Launcher", "addon_sticky"),
    toggle("Hookshot", "addon_hookshot"),
    Item::Sub { label: "Hookshot Settings", page: Page::Hookshot },
    toggle("Railgun", "addon_railgun"),
    Item::Sub { label: "Railgun Settings", page: Page::Railgun },
    toggle("Goggles", "addon_goggles"),
    Item::Sub { label: "Goggles Settings", page: Page::Goggles },
    toggle("Dodge / Dive", "addon_dodge"),
    toggle("Bullet Time", "addon_bullet_time"),
    Item::Sub { label: "Bullet Time Settings", page: Page::BulletTime },
    toggle("Projectile Shotguns", "addon_projectile_shotguns"),
    Item::Sub { label: "Projectile Shotguns Settings", page: Page::ProjectileShotguns },
    toggle("Hitstop", "addon_hitstop"),
    Item::Sub { label: "Hitstop Settings", page: Page::Hitstop },
    Item::Run { label: "Play", run: Run::NewGame },
];


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_restores_parameters_without_changing_addon_selection() {
        let mut cv = Cvars::new();
        for &(name, default, flags) in qserver::addons::CVARS { cv.register(name, default, flags); }
        cv.set("addon_shotgun_speed", "300");
        cv.set("addon_projectile_shotguns", "0");
        let mut menu = Menu::new();
        menu.open(Screen::OptionsPage(Page::ProjectileShotguns));
        menu.key(K_DOWNARROW, &mut cv, false);
        menu.key(K_ENTER, &mut cv, false);
        assert_eq!(cv.value_of("addon_shotgun_speed"), 10000.0);
        assert_eq!(cv.value_of("addon_projectile_shotguns"), 0.0);
        cv.set("addon_bullet_time_toggle", "1");
        cv.set("addon_bullet_time_meter", "0");
        cv.set("addon_bullet_time_duration", "20");
        cv.set("addon_bullet_time_regen", "3");
        menu.open(Screen::OptionsPage(Page::BulletTime));
        menu.opt_cursor = 0;
        for _ in 0..5 { menu.key(K_DOWNARROW, &mut cv, false); }
        menu.key(K_ENTER, &mut cv, false);
        assert_eq!(cv.value_of("addon_bullet_time_toggle"), 0.0);
        assert_eq!(cv.value_of("addon_bullet_time_meter"), 1.0);
        assert_eq!(cv.value_of("addon_bullet_time_duration"), 5.0);
        assert_eq!(cv.value_of("addon_bullet_time_regen"), 1.0);
    }
}
