//! Every translatable text of the options pages, for the catalog test.

use super::*;

const ALL_PAGES: &[Page] = &[
    Page::Gameplay, Page::Controls, Page::Input, Page::Graphics, Page::Enhancements, Page::Audio, Page::AudioMix, Page::HitSound,
    Page::KillSound, Page::Accessibility, Page::Hud, Page::Crosshair, Page::Mouse, Page::Controller, Page::Gyro,
    Page::Touch, Page::Autosave, Page::WeaponPriority, Page::Automap, Page::AutoAim, Page::GrenadeArc, Page::Water, Page::Bloom, Page::RtLights,
    Page::Debris, Page::Blood, Page::Impacts, Page::ScreenShader, Page::Customize, Page::NewGameCustomize,
    Page::Modifiers, Page::Addons, Page::NewGameAddons, Page::PortalGun, Page::Boomerang, Page::Railgun, Page::Gel,
    Page::Washer, Page::Hookshot, Page::Cable, Page::Sticky, Page::Goggles, Page::ProjectileShotguns, Page::BulletTime,
    Page::Hitstop, Page::HitstopAdvanced, Page::Join, Page::Host, Page::Setup, Page::Demos, Page::Video, Page::Export, Page::RenameDemo,
];

/// Page names, row labels, choice values and tooltips.
pub(crate) fn menu_keys() -> Vec<&'static str> {
    let mut out = vec!["off", "on"];
    let items = ALL_PAGES.iter().flat_map(|&p| page_items(p).iter()).chain(CONTROLS_ALL.iter());
    for &p in ALL_PAGES {
        out.push(page_name(p));
    }
    for &item in items {
        match item {
            Item::Slider { label, .. } | Item::Toggle { label, .. } | Item::Text { label, .. } | Item::Color { label, .. } => {
                out.push(label)
            }
            Item::Run { label, .. } | Item::Bind { label, .. } | Item::Sub { label, .. } => out.push(label),
            Item::Choice { label, choices, .. } => {
                out.push(label);
                out.extend(choices.iter().map(|c| c.0));
            }
            Item::UserDataDir => out.push("User Data Directory"),
            Item::Soundtrack => out.push("Soundtrack"),
            Item::HudPreset => out.extend(["HUD Preset", "modern", "classic", "custom"]),
            Item::Heading(label) => out.push(label),
            _ => {}
        }
        out.extend(tooltip(item));
        out.extend(disabled_reason(item));
    }
    out.extend(MODES.iter().copied());
    out.extend([
        "Ping", "Protocol", "Players", "Unknown", "Empty", "Populated", "Open slots", "Full", "Deathmatch",
        "Mod", "Favorites",
    ]);
    out.extend(crate::hud::kind::lang_keys());
    out
}
