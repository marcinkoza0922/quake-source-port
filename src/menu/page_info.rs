use super::*;

/// Help line shown under an options page for the selected item.
pub(super) fn tooltip(item: Item) -> Option<&'static str> {
    let cvar = match item {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } => cvar,
        _ => return None,
    };
    Some(match cvar {
        "cl_wheel_timescale" => "Slows the game down while the weapon wheel is open",
        "r_texfilter" => "Nearest keeps crisp classic pixels; linear smooths textures",
        "r_anisotropy" => "Keeps textures sharp on floors and walls seen at steep angles",
        "r_lerpmodels" => "Blends between animation frames of monsters and weapons",
        "r_lerpmove" => "Smooths the stepped movement of monsters",
        "r_softemu" => "Emulates the original 8-bit palette of software rendering",
        "vid_fsaamode" => "Shades every anti-aliasing sample; needs Anti-aliasing on",
        "r_renderscale" => "3D resolution relative to the window; the UI stays sharp",
        "r_dither" => "Adds fine noise to hide color banding",
        "r_ssao" => "Darkens corners and contact areas where geometry meets",
        "r_motionblur" => "Blurs fast-moving monsters, projectiles and doors (not camera turns)",
        "r_motionblur_amount" => "Length of the motion blur streaks",
        "r_debris" => "Corpses go limp as ragdolls, gibs tumble and barrels throw shrapnel; shots and blasts push them (cosmetic only)",
        "r_debris_ragdolls" => "Corpses go limp and fall as ragdolls",
        "r_debris_ragdollstart" => "After animation keeps the death animations; at death goes limp the moment a monster dies; mixed goes limp on a hard hit, and the weaker the killing hit the likelier the animation plays",
        "r_debris_maxragdolls" => "How many ragdolls can lie around at once",
        "r_debris_gibs" => "Gibs and heads tumble and bounce; blasts can gib ragdolls",
        "r_debris_shrapnel" => "Exploding boxes throw metal shards",
        "r_impact_bullets" => "Shotgun hits leave small surface marks (cosmetic only)",
        "r_impact_nails" => "Nails stick to wood and bodies; hard surfaces deflect them to the floor",
        "r_impact_scorches" => "Explosions scorch nearby solid surfaces",
        "r_impact_lifetime" => "Impact marks and nails fade before expiring",
        "r_impact_limit" => "Oldest marks and nails are recycled at this shared limit",
        "r_blood" => "Blood stains surfaces and bodies; cosmetic, independent of ragdolls and particle packs",
        "r_blood_amount" => "Number of splatter rays per blood impact",
        "r_blood_lifetime" => "Marks fade out before expiring; oldest marks are recycled at the limit",
        "r_blood_wounds" => "Local red wounds follow enemy animation and ragdoll deformation",
        "r_blood_gibs" => "Moving gibs leave blood on surfaces they touch",
        "r_debris_movers" => "Debris lands on and is carried or shoved by doors, lifts and platforms",
        "r_debris_killlift" => "With ragdolls starting at death, a killing shot also lifts the body off its feet",
        "r_debris_force" => "How hard shots and explosions push debris around",
        "r_enhancedwater" => "Turns on the extra liquid effects; Water Settings controls each one",
        "r_wateralpha" => "How see-through water surfaces are",
        "r_waterripples_amount" => "Strength of the surface swell and rings",
        "r_watercaustics_amount" => "Brightness of the underwater light patterns",
        "r_waterripples" => "Liquid surfaces swell and ring out where things move through them",
        "r_watersplash" => "Droplets spray up when things enter or leave water, slime or lava",
        "r_watercaustics" => "Rippling light patterns on surfaces under water and slime",
        "aim_slowdown" => "Slows stick look while the crosshair is over an enemy",
        "aim_rotation" => "Turns the view to follow a moving enemy you are aiming at",
        "a11y_reducemotion" => "Turns off view bob, strafe tilt and the camera kick when hit or firing",
        "v_flashscale" => "Strength of the full-screen flashes for damage, pickups and powerups",
        "r_colorblind" => "Shifts colors so they stay distinct for the chosen color blindness",
        "crosshair_outline" => "Dark border keeps the crosshair visible on any background",
        "cl_captions" => "Shows text for important sounds: doors, alerted enemies, secrets, pickups",
        "scr_textscale" => "Size of on-screen messages and captions",
        "scr_textbg" => "Dark backdrop behind on-screen messages for readability",
        "scr_centertime" => "Seconds a center-screen message stays up",
        "con_notifytime" => "Seconds messages at the top of the screen stay up",
        "in_toggleattack" => "Press attack once to keep firing, again to stop",
        "in_togglerun" => "Press run once to switch between running and walking",
        "assist_damagetaken" => "Scales all damage you take; not in deathmatch",
        "assist_enemyhealth" => "Monsters take proportionally less damage at higher values",
        "assist_ammo" => "Multiplies ammo from pickups (up to the normal carry limit)",
        "assist_infiniteammo" => "Firing never uses up ammo",
        "assist_noselfdamage" => "Your own rockets and grenades don't hurt you; rocket jumps still work",
        "assist_regen" => "Slowly heals up to 100 health after 5 seconds without being hit",
        "assist_jumps" => "Jump again in mid-air",
        "scr_hudstyle" => "Classic: the original status bar; modern: compact elements at the screen edges",
        "scr_centerpos" => "Where center-screen messages appear",
        "con_notifypos" => "Where pickup and event messages appear",
        "hud_face" => "Shows the player face next to the health number",
        "hud_health" | "hud_armor" | "hud_ammo" | "hud_ammolist" | "hud_weapons" | "hud_items" => {
            "Immersive: appears for a few seconds only when its value changes"
        }
        "hud_health_pos" | "hud_armor_pos" | "hud_ammo_pos" | "hud_ammolist_pos" | "hud_weapons_pos" | "hud_items_pos" => {
            "Screen edge for this element; elements on the same spot stack"
        }
        "a11y_gamespeed" => "Slows the whole game down; single player only",
        _ => return None,
    })
}

pub(super) fn page_items(p: Page) -> &'static [Item] {
    match p {
        Page::Gameplay => GAMEPLAY,
        Page::Controls => CONTROLS,
        Page::Input => INPUT,
        Page::Graphics => GRAPHICS,
        Page::Enhancements => ENHANCEMENTS,
        Page::Impacts => IMPACTS,
        Page::Audio => AUDIO,
        Page::Accessibility => ACCESSIBILITY,
        Page::Hud => HUD,
        Page::HudLayout => HUD_LAYOUT,
        Page::Messages => MESSAGES,
        Page::Water => WATER,
        Page::Debris => DEBRIS,
        Page::Blood => BLOOD,
        Page::Mouse => MOUSE,
        Page::Controller => CONTROLLER,
        Page::Autosave => AUTOSAVE,
        Page::Assists => ASSISTS,
        Page::Join => JOIN,
        Page::Host => HOST,
        Page::Setup => SETUP,
    }
}

pub(super) fn page_name(p: Page) -> &'static str {
    match p {
        Page::Join => "Join Game",
        Page::Host => "Host Game",
        Page::Setup => "Player Setup",
        Page::Mouse => "Mouse & Keyboard",
        Page::Controller => "Controller",
        Page::Autosave => "Autosave",
        Page::Assists => "Assists",
        Page::HudLayout => "Modern HUD Layout",
        Page::Messages => "Messages & Captions",
        Page::Water => "Water Settings",
        Page::Impacts => "Impact Effects",
        Page::Debris => "Debris Settings",
        Page::Blood => "Blood Splatter Settings",
        _ => PAGES.iter().find(|x| x.1 == p).map_or("", |x| x.0),
    }
}

/// The page an `Item::Sub` page returns to, with that row's index.
pub(super) fn parent_page(p: Page) -> Option<(Page, usize)> {
    [Page::Gameplay, Page::Input, Page::Hud, Page::Accessibility, Page::Enhancements].into_iter().find_map(|parent| {
        page_items(parent).iter().position(|i| matches!(i, Item::Sub { page, .. } if *page == p)).map(|i| (parent, i))
    })
}

/// Sub-pages that are switched off by their parent setting.
pub(super) fn sub_disabled(page: Page, cv: &Cvars) -> bool {
    (page == Page::Autosave && cv.value_of("sv_autosave") == 0.0)
        || (page == Page::HudLayout && cv.value_of("scr_hudstyle") == 0.0)
        || (page == Page::Water && cv.value_of("r_enhancedwater") == 0.0)
        || (page == Page::Blood && cv.value_of("r_blood") == 0.0)
        || (page == Page::Debris && cv.value_of("r_debris") == 0.0)
}

pub(super) fn is_net_page(p: Page) -> bool {
    matches!(p, Page::Join | Page::Host | Page::Setup)
}
