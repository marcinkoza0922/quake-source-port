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
    Item::Run { label: "Reset to Defaults", run: Run::ResetBulletTime },
];

/// Main menu → Add-ons: which toys the running game (or the next new
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
    toggle("Projectile Shotguns", "addon_projectile_shotguns"),
    Item::Sub { label: "Projectile Shotguns Settings", page: Page::ProjectileShotguns },
    toggle("Bullet Time", "addon_bullet_time"),
    Item::Sub { label: "Bullet Time Settings", page: Page::BulletTime },
    toggle("Goggles", "addon_goggles"),
    Item::Sub { label: "Goggles Settings", page: Page::Goggles },
];

/// New Game with Add-ons ticked: choose the toys, then start.
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
    toggle("Projectile Shotguns", "addon_projectile_shotguns"),
    Item::Sub { label: "Projectile Shotguns Settings", page: Page::ProjectileShotguns },
    toggle("Bullet Time", "addon_bullet_time"),
    Item::Sub { label: "Bullet Time Settings", page: Page::BulletTime },
    toggle("Goggles", "addon_goggles"),
    Item::Sub { label: "Goggles Settings", page: Page::Goggles },
    Item::Run { label: "Start New Game", run: Run::NewGame },
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
        for _ in 0..4 { menu.key(K_DOWNARROW, &mut cv, false); }
        menu.key(K_ENTER, &mut cv, false);
        assert_eq!(cv.value_of("addon_bullet_time_toggle"), 0.0);
        assert_eq!(cv.value_of("addon_bullet_time_meter"), 1.0);
        assert_eq!(cv.value_of("addon_bullet_time_duration"), 5.0);
        assert_eq!(cv.value_of("addon_bullet_time_regen"), 1.0);
    }
}
