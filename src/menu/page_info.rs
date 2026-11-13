use super::*;

/// Help line shown under an options page for the selected item.
pub(super) fn tooltip(item: Item) -> Option<&'static str> {
    let cvar = match item {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } => cvar,
        Item::ShaderPreset => "RetroArch slang shader on the final image: CRT, scanlines, NTSC, sharpening; add your own in <game>/shaders",
        Item::ResetShaderParams => "Puts every setting of the chosen shader back to the preset's defaults",
        Item::ShaderParam(_) => "A setting of the chosen shader; arrows step it, Enter is not needed",
        _ => return None,
    };
    Some(match cvar {
        "touch_enable" => "On-screen stick and buttons: auto shows them after a touch and hides them when another device is used",
        "touch_sensitivity" => "How far the view turns when you drag on the right half of the screen",
        "touch_invert" => "Dragging up looks down instead of up",
        "touch_size" => "Size of the on-screen buttons and move stick",
        "touch_opacity" => "How visible the on-screen controls are",
        "cl_wheel_timescale" => "Slows the game down while the weapon wheel is open",
        "r_texfilter" => "Nearest keeps crisp classic pixels; linear smooths textures",
        "r_anisotropy" => "Keeps textures sharp on floors and walls seen at steep angles",
        "r_dynamic" => "Lights from rockets, muzzle flashes and explosions brighten nearby walls",
        "r_coloredlight" => "Uses the map's coloured light (.lit file or BSPX lump); off gives vanilla white light. Applies on the next map load",
        "r_fog" => "Shows the distance fog that maps and mods set; off always gives clear view",
        "r_skyfog" => "How strongly fog covers the sky when fog is on",
        "r_lerpmodels" => "Blends between animation frames of monsters and weapons",
        "r_lerpmove" => "Smooths the stepped movement of monsters",
        "r_softemu" => "Emulates the original 8-bit palette of software rendering",
        "vid_fsaamode" => "Shades every anti-aliasing sample; needs Anti-aliasing on",
        "r_renderscale" => "3D resolution relative to the window; the UI stays sharp",
        "r_dither" => "Adds fine noise to hide color banding",
        "r_ssao" => "Darkens corners and contact areas where geometry meets",
        "r_motionblur" => "Blurs fast-moving monsters, projectiles and doors (not camera turns)",
        "r_motionblur_amount" => "Length of the motion blur streaks",
        "addon_portal_view" => "Draw the view out of the linked portal inside each portal; off shows flat discs (fastest)",
        "addon_portal_max" => "How many portals per screen show a live view; each one redraws the scene, so lower this on slow machines or in split-screen",
        "addon_portal_depth" => "How deep portals seen through other portals keep showing a live view (a hall of mirrors between facing portals); each level redraws the scene",
        "addon_portal_dist" => "Portals farther away than this are drawn as flat discs",
        "addon_hookshot" => "A Zelda-style hookshot: fire sends a chain hook out; it latches onto hookable surfaces or enemies and reels you in; fire again to let go",
        "addon_hook_range" => "How far the hook flies before it retracts",
        "addon_hook_wood" => "The hook latches onto wooden surfaces (texture names like wood, crate, plank)",
        "addon_hook_metal" => "The hook latches onto metal surfaces (metal, tech, computer panels, copper)",
        "addon_hook_stone" => "The hook latches onto stone, brick, rock and any surface not recognised as wood or metal",
        "addon_hook_enemies" => "Hooking an enemy pulls you to it",
        "addon_portalgun" => "A two-portal gun: fire and alt-fire place linked portals on flat walls; pick it with the Toy Slot bind (Controls)",
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
        "assist_autoaim" => "Snap and track enemies in local play, including split-screen; unavailable online",
        "assist_autoaim_mode" => "Hold: keep Lock-on held. Toggle: press Auto-aim Toggle to enable automatic acquisition. Click look stick to switch targets",
        "assist_autoaim_indicator" => "Show corner ticks at screen centre while locked, even with Crosshair off",
        "assist_autoaim_reacquire" => "Automatically acquire another enemy while holding Lock-on after losing a target",
        "assist_autoaim_cone" => "How close an enemy must be to the crosshair for automatic acquisition in toggle mode (degrees)",
        "assist_autoaim_range" => "Maximum distance for acquiring and keeping an enemy locked",
        "aim_slowdown" => "Slows stick look while the crosshair is over an enemy",
        "aim_rotation" => "Turns the view to follow a moving enemy you are aiming at",
        "a11y_reducemotion" => "Turns off view bob, strafe tilt and the camera kick when hit or firing",
        "r_drawviewmodel" => "Place every weapon at the center, right or left of the view, or hide all viewmodels",
        "v_flashscale" => "Strength of the full-screen flashes for damage, pickups and powerups",
        "r_shader_scope" => "3D view keeps the HUD and menus sharp; whole screen runs them through the shader too",
        "r_colorblind" => "Shifts colors so they stay distinct for the chosen color blindness",
        "hud_hitmarkers" => "Flash a marker on confirmed hits; kills use a different color, and the shape follows your crosshair",
        "s_hitfeedback" => "Play your chosen hit or kill cue when you damage an enemy",
        "s_hit_length" | "s_kill_length" => "Stretch or shorten the sound while keeping its pitch",
        "s_hit_speed" | "s_kill_speed" => "Playback speed: faster sounds shorter and higher",
        "s_hit_pitch" | "s_kill_pitch" => "Raise or lower the tone independently of sound length",
        "s_hit_bass" | "s_kill_bass" => "Boost or cut low frequencies in decibels",
        "s_hit_treble" | "s_kill_treble" => "Boost or cut high frequencies in decibels",
        "s_hit_drive" | "s_kill_drive" => "Add saturation and distortion",
        "s_hit_echo" | "s_kill_echo" => "Add delayed repeats after the cue",
        "s_hit_reverb" | "s_kill_reverb" => "Add a ringing room tail after the cue",
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

/// Add-on binds are listed on the Controls page: while the running game has
/// add-ons, or (no game running) when New Game would start with them. Set
/// whenever the menu handles a key or draws.
pub(super) static ADDON_BINDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn sync_addon_binds(cv: &Cvars, game_running: bool) {
    let on = cv.value_of(if game_running { "addons_active" } else { "addons_enable" }) != 0.0;
    ADDON_BINDS.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(super) fn page_items(p: Page) -> &'static [Item] {
    match p {
        Page::Gameplay => GAMEPLAY,
        Page::Controls if ADDON_BINDS.load(std::sync::atomic::Ordering::Relaxed) => CONTROLS_ALL,
        Page::Controls => CONTROLS,
        Page::Addons => ADDONS,
        Page::NewGameAddons => NEW_GAME_ADDONS,
        Page::PortalGun => PORTAL_GUN,
        Page::Hookshot => HOOKSHOT,
        Page::Input => INPUT,
        Page::Graphics => GRAPHICS,
        Page::Enhancements => ENHANCEMENTS,
        Page::Impacts => IMPACTS,
        Page::Audio => AUDIO,
        Page::HitSound => HIT_SOUND,
        Page::KillSound => KILL_SOUND,
        Page::Accessibility => ACCESSIBILITY,
        Page::Hud => HUD,
        Page::HudLayout => HUD_LAYOUT,
        Page::Messages => MESSAGES,
        Page::Water => WATER,
        Page::Debris => DEBRIS,
        Page::Blood => BLOOD,
        Page::ScreenShader => SCREEN_SHADER,
        Page::Mouse => MOUSE,
        Page::Controller => CONTROLLER,
        Page::Touch => TOUCH,
        Page::Autosave => AUTOSAVE,
        Page::Assists => ASSISTS,
        Page::AutoAim => AUTOAIM,
        Page::Join => JOIN,
        Page::Host => HOST,
        Page::Setup => SETUP,
    }
}

pub(super) fn page_name(p: Page) -> &'static str {
    match p {
        Page::HitSound => "Hit Sound",
        Page::KillSound => "Kill Sound",
        Page::Join => "Join Game",
        Page::Host => "Host Game",
        Page::Setup => "Player Setup",
        Page::Mouse => "Mouse & Keyboard",
        Page::Controller => "Controller",
        Page::Touch => "Touchscreen",
        Page::Autosave => "Autosave",
        Page::Assists => "Assists",
        Page::AutoAim => "Auto-aim Settings",
        Page::HudLayout => "Modern HUD Layout",
        Page::Messages => "Messages & Captions",
        Page::Water => "Water Settings",
        Page::Impacts => "Impact Effects",
        Page::Debris => "Debris Settings",
        Page::Blood => "Blood Splatter Settings",
        Page::ScreenShader => "Screen Shader",
        Page::Addons => "Add-ons",
        Page::NewGameAddons => "New Game: Add-ons",
        Page::PortalGun => "Portal Gun",
        Page::Hookshot => "Hookshot",
        _ => PAGES.iter().find(|x| x.1 == p).map_or("", |x| x.0),
    }
}

/// The page an `Item::Sub` page returns to, with that row's index.
pub(super) fn parent_page(p: Page) -> Option<(Page, usize)> {
    [Page::Assists, Page::Gameplay, Page::Input, Page::Hud, Page::Accessibility, Page::Enhancements, Page::Addons, Page::NewGameAddons, Page::Audio].into_iter().find_map(|parent| {
        page_items(parent).iter().position(|i| matches!(i, Item::Sub { page, .. } if *page == p)).map(|i| (parent, i))
    })
}

/// Sub-pages that are switched off by their parent setting.
pub(super) fn sub_disabled(page: Page, cv: &Cvars) -> bool {
    (page == Page::AutoAim && cv.value_of("assist_autoaim") == 0.0)
        || (page == Page::Autosave && cv.value_of("sv_autosave") == 0.0)
        || (page == Page::HudLayout && cv.value_of("scr_hudstyle") == 0.0)
        || (page == Page::Water && cv.value_of("r_enhancedwater") == 0.0)
        || (page == Page::Blood && cv.value_of("r_blood") == 0.0)
        || (page == Page::Debris && cv.value_of("r_debris") == 0.0)
}

pub(super) fn is_net_page(p: Page) -> bool {
    matches!(p, Page::Join | Page::Host | Page::Setup)
}
