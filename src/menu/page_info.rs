use super::*;

/// Help line shown under an options page for the selected item.
pub(super) fn tooltip(item: Item) -> Option<&'static str> {
    let cvar = match item {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } => cvar,
        Item::ShaderPreset => return Some(qcore::t!("RetroArch slang shader on the final image: CRT, scanlines, NTSC, sharpening; add your own in <game>/shaders")),
        Item::ResetShaderParams => return Some(qcore::t!("Puts every setting of the chosen shader back to the preset's defaults")),
        Item::ShaderParam(_) => return Some(qcore::t!("A setting of the chosen shader; arrows step it, Enter is not needed")),
        Item::Language => return Some(qcore::t!("Language of the menus, messages and the 2021 re-release's game text; System follows your computer's language")),
        Item::Text { cvar: "_export_name", .. } => return Some(qcore::t!("Name of the video file; the extension follows the video format, and a file of the same name is replaced")),
        Item::Text { cvar: "_demo_rename", .. } => return Some(qcore::t!("New name of the demo; if another demo has it, a number is added")),
        _ => return None,
    };
    Some(match cvar {
        "cl_autodemo" => "Records every level you play to the Automatic list in Main Menu > Demos; keep the ones you want, the oldest are replaced",
        "cl_autodemo_slots" => "How many automatic demos are kept before the oldest is deleted; kept demos never count",
        "cl_demobar" => "Shows the playback bar (time, speed, timeline) after a control key, always, or never",
        "cl_demo_endpause" => "A demo you play stops on its last frame instead of returning to the menu, so you can rewind",
        "capture_fps" => "Frames per second of recorded video; the game runs at exactly this rate while recording",
        "capture_codec" => "Video format; needs ffmpeg with that encoder, else frames are saved as PNG images",
        "capture_quality" => "High keeps the most detail; small file saves disk space",
        "capture_size" => "Video size; frames are scaled from the window size (bars keep the shape), so a larger size adds no detail",
        "capture_gpu" => "Encodes on the graphics card (NVIDIA NVENC, AMD or Intel through VA-API, AMF or Quick Sync, Apple VideoToolbox) when ffmpeg can use it, leaving the processor free; otherwise the processor encodes",
        "capture_audio" => "Records game sound and music into the video",
        "con_translate" => "Also translates the engine's console messages; off keeps them in English",
        "gyro_enable" => "Turn the controller's motion sensor into a look input; needs a pad with a gyroscope",
        "joy_flick" => "2D aiming turns the view continuously with the right stick; flick stick snaps the view to where you push the stick, then turning the stick rotates it",
        "gyro_mode" => "When gyro looking applies: always, off while +gyroaction is held, or only while it is held",
        "gyro_yawsensitivity" => "How far the view turns left and right when you turn the controller (yaw axis)",
        "gyro_pitchsensitivity" => "How far the view tilts up and down when you tilt the controller (pitch axis)",
        "gyro_rollsensitivity" => "Turns the view when you roll the controller sideways; 0 disables it, negative reverses it",
        "gyro_noise_thresh" => "Ignores gyro rotation slower than this many degrees per second, to hide hand tremor",
        "joy_flick_time" => "Seconds the flick stick snap takes; lower is faster",
        "touch_enable" => "On-screen stick and buttons: auto shows them after a touch and hides them when another device is used",
        "touch_sensitivity" => "How far the view turns when you drag on the right half of the screen",
        "touch_invert" => "Dragging up looks down instead of up",
        "touch_size" => "Size of the on-screen buttons and move stick",
        "touch_opacity" => "How visible the on-screen controls are",
        "cl_wheel_timescale" => "Slows the game down while the weapon wheel is open",
        "r_texfilter" => "Nearest keeps crisp classic pixels; linear smooths textures",
        "r_anisotropy" => "Keeps textures sharp on floors and walls seen at steep angles",
        "r_dynamic" => "Lights from rockets, muzzle flashes and explosions brighten nearby walls",
        "r_enhancedmodels" => "Uses the 2021 re-release's smoother MD5 models (and MD3 replacements from mods) instead of the classic ones. Applies on the next map load",
        "r_coloredlight" => "Uses the map's coloured light (.lit file or BSPX lump); off gives vanilla white light. Applies on the next map load",
        "r_fog" => "Shows the distance fog that maps and mods set; off always gives clear view",
        "r_skyfog" => "How strongly fog covers the sky when fog is on",
        "r_lerpmodels" => "Blends between animation frames of monsters and weapons",
        "r_lerpmove" => "Smooths the stepped movement of monsters",
        "r_softemu" => "Emulates the original 8-bit palette of software rendering",
        "vid_fsaamode" => "Shades every anti-aliasing sample; needs Anti-aliasing on",
        "r_renderscale" => "3D resolution relative to the window; the UI stays sharp",
        "r_dither" => "Adds fine noise to hide color banding",
        "r_ssao" => "Darkens corners and contact areas where geometry meets; higher quality is sharper",
        "r_motionblur" => "Blurs fast-moving monsters, projectiles and doors (not camera turns)",
        "r_motionblur_quality" => "Low skips the background smear halo and uses fewer blur samples; high keeps the full halo and smoother streaks",
        "r_speedblur" => "Blurs the screen edges during fast falls and launches; keeps the centre clear. Reduce Motion disables it",
        "r_motionblur_amount" => "Length of the motion blur streaks",
        "addon_portal_view" => "Draw the view out of the linked portal inside each portal; off shows flat discs (fastest)",
        "addon_portal_max" => "Max portals per screen with a live view; each one redraws the scene",
        "addon_portal_depth" => "Max nested portal reflections to draw; each level redraws the scene",
        "addon_portal_dist" => "Portals farther away than this are drawn as flat discs",
        "addon_boomerang" => "Hold fire and sweep the crosshair over monsters to queue them, release to throw: it stuns each in turn and flies back; alt-fire clears the queue",
        "addon_sticky_damage" => "Explosion damage for each remotely detonated sticky bomb",
        "addon_boomerang_stun" => "Seconds a monster hit by the boomerang stays stunned",
        "addon_boomerang_speed" => "How fast the boomerang flies (units per second)",
        "addon_sticky" => "Lobs sticky bombs (rockets ammo) that cling to walls, doors and monsters; alt-fire detonates them all",
        "addon_railgun" => "A Quake III style railgun: a slug that pierces every enemy on its line, using 2 cells a shot and refiring every 1.5 s; select it through its assigned weapon slot (Add-ons)",
        "addon_rail_damage" => "Damage the railgun deals to each enemy it passes through (Quake III: 100)",
        "addon_rail_color" => "Colour of the railgun's spiral trail; split-screen players each get the next colour along",
        "addon_gel" => "A Portal-style gel sprayer: fire sprays blue Repulsion gel (land, jump or run into it to bounce), alt-fire orange Propulsion gel (run on it to go much faster). Pairs well with the Powerwasher to clean gel up",
        "addon_gel_quality" => "Coat atlas resolution, rebuild rate and shader detail: low builds smaller coats less often with fast shading; high uses full detail",
        "addon_washer" => "A water cannon: fire shoots a straight jet with limited reach, alt-fire a wide fan. Pushes enemies back, turns projectiles aside and washes away gel and blood. Recommended with the Gel Sprayer",
        "addon_washer_push" => "How hard the Powerwasher's water pushes enemies",
        "addon_washer_quality" => "Water jet segments, droplet count and glossy liquid shading: low uses simpler translucent streams; high uses smooth glossy jets",
        "addon_hookshot" => "A Zelda-style hookshot: fire sends a chain hook out; it latches onto hookable surfaces or enemies and reels you in; fire again to let go",
        "addon_hook_range" => "How far the hook flies before it retracts",
        "addon_hook_wood" => "The hook latches onto wooden surfaces (texture names like wood, crate, plank)",
        "addon_hook_metal" => "The hook latches onto metal surfaces (metal, tech, computer panels, copper)",
        "addon_hook_stone" => "The hook latches onto stone, brick, rock and any surface not recognised as wood or metal",
        "addon_hook_enemies" => "Hooking an enemy pulls you to it",
        "addon_cable" => "A male-to-male cable: plug one end into electronics with alt-fire and the other end shoots lightning, but you are tethered to the socket",
        "addon_cable_length" => "How far the cable reaches from its socket, measured along the cable",
        "addon_cable_metal" => "Any metal surface takes the plug, not just electronics",
        "addon_cable_damage" => "Lightning damage every tenth of a second (the stock lightning gun does 30)",
        "addon_goggles" => "Night-vision and thermal goggles, cycle normal, night vision and thermal with Change Goggles (Add-ons), or use the Equipment Wheel",
        "addon_goggles_quality" => "Bloom glow around bright lights and heat sources; medium uses 4 taps, high uses 8, low disables halo taps",
        "addon_goggles_intensity" => "Night vision: how much dark areas are brightened; thermal: how strongly heat stands out",
        "addon_goggles_noise" => "Sensor grain, scanlines and flicker of the goggle image",
        "addon_hitstop" => "Freeze the game briefly when you damage an enemy; single player with add-ons only",
        "addon_hitstop_weapons" => "Which weapons trigger the freeze; melee covers the axe and any mod weapon used at close range without ammo",
        "addon_hitstop_duration" => "How long a hit freezes the game, in milliseconds",
        "addon_hitstop_kill_bonus" => "Multiplier on the freeze when the hit kills the enemy; 1 means no extra",
        "addon_hitstop_shake" => "Small camera jolt on each freeze; off with Reduce Motion",
        "addon_hitstop_targets" => "Freeze for every enemy, or only for enemies that were tough before the hit",
        "addon_hitstop_tough" => "Health an enemy must have had before the hit to count as tough",
        "addon_hitstop_scale_damage" => "Extra freeze milliseconds for every 10 damage dealt, so heavy hits stop longer",
        "addon_hitstop_min_damage" => "Hits dealing less damage than this do not freeze the game; the default 10 skips nails and shotgun pellets",
        "addon_hitstop_cooldown" => "Minimum gap between freezes, so shotgun pellets and rapid fire count once; kills ignore it",
        "addon_hitstop_max" => "No single freeze lasts longer than this, however much damage or kill bonus adds",
        "addon_portalgun" => "Two-portal gun: fire and alt-fire place linked portals on flat walls; assign a weapon slot in Add-ons",
        "r_portal_quality" => "Resolution and detail inside portal and teleporter views; lower tiers trim nested effects",
        "r_waterwarp" => "How the view distorts underwater: Classic ripples the whole screen like software Quake, GLQuake slowly stretches and squashes the view",
        "r_particles" => "Look of the classic particles: GLQuake draws soft round dots, Classic draws square pixels like software Quake",
        "r_enhancedfx" => "Unlocks the FX Settings page: enhanced particles, impact effects and blood splatter",
        "r_enhancedparticles" => "Replaces the classic dot particles with glowing sparks, smoke and blood; mods with their own particle effects keep them",
        "r_particle_quality" => "Low: hard particle edges, half the scripted particles and fewer hit sparks. Medium: soft edges. High: soft edges checked per MSAA sample, most hit sparks",
        "r_debris" => "Gibs tumble and barrels throw shrapnel; shots and blasts push them (cosmetic only)",
        "r_debris_quality" => "Simulation detail, step rate, fade-out lifetimes and caps for gibs and shrapnel; low also skips mover collision",
        "r_debris_gibs" => "Gibs and heads tumble and bounce",
        "r_debris_shrapnel" => "Exploding boxes throw metal shards",
        "r_impact_fx" => "Sparks, smoke, chips, splinters and splashes fly off whatever your shots hit",
        "r_impact_bullets" => "Shotgun hits leave small surface marks (cosmetic only)",
        "r_impact_nails" => "Nails stick to wood and bodies; hard surfaces deflect them to the floor",
        "r_impact_scorches" => "Explosions scorch nearby solid surfaces",
        "r_impact_lifetime" => "Seconds impact marks and stuck nails stay before fading out; independent of Particle Quality",
        "r_impact_limit" => "Oldest marks and nails are recycled at this shared limit",
        "r_blood" => "Blood stains surfaces and bodies; cosmetic, independent of particle packs",
        "r_blood_quality" => "Caps mark count, lifetime, airborne droplets and wound/gib painting; low skips enemy wounds and gib trails",
        "r_blood_amount" => "Number of splatter rays per blood impact",
        "r_blood_lifetime" => "Marks fade out before expiring; oldest marks are recycled at the limit",
        "r_blood_wounds" => "Local red wounds follow enemy animation",
        "r_blood_gibs" => "Moving gibs leave blood on surfaces they touch",
        "r_debris_movers" => "Debris lands on and is carried or shoved by doors, lifts and platforms",
        "r_debris_force" => "How hard shots and explosions push debris around",
        "r_enhancedwater" => "Turns on the extra liquid effects; Water Settings controls each one",
        "r_watertranslucency" => "See through enhanced water; enables Water Opacity",
        "r_wateralpha" => "Surface opacity; requires Enhanced Water and Translucent Water",
        "r_waterripples_amount" => "Strength of the surface swell and rings",
        "r_watercaustics_amount" => "Brightness of the underwater light patterns",
        "r_waterripples" => "Liquid surfaces swell and ring out where things move through them",
        "r_watersplash" => "Droplets spray up when things enter or leave water, slime or lava",
        "r_watercaustics" => "Rippling light patterns on surfaces under water and slime",
        "r_water_quality" => "Detail for caustics, ripples and splashes: low uses smaller budgets and shorter view distance; high uses larger budgets and farther view distance",
        "assist_grenadearc" => "Show where grenades and other arcing shots will fly, bounce and land; local play only, not in deathmatch",
        "assist_grenadearc_style" => "Draw the predicted grenade path as a solid or dashed line",
        "assist_grenadearc_color" => "Color of the grenade arc",
        "assist_grenadearc_alpha" => "Opacity of the grenade arc",
        "assist_autoaim" => "Snap and track enemies in local play, including split-screen; unavailable online",
        "assist_autoaim_mode" => "Hold: keep Lock-on held. Toggle: press Auto-aim Toggle to enable automatic acquisition. Click look stick to switch targets",
        "assist_autoaim_indicator" => "Show corner ticks at screen centre while locked, even with Crosshair off",
        "assist_autoaim_reacquire" => "Automatically acquire another enemy while holding Lock-on after losing a target",
        "assist_autoaim_cone" => "How close an enemy must be to the crosshair for automatic acquisition in toggle mode (degrees)",
        "assist_autoaim_range" => "Maximum distance for acquiring and keeping an enemy locked",
        "aim_slowdown" => "Slows stick look while the crosshair is over an enemy",
        "aim_rotation" => "Turns the view to follow a moving enemy you are aiming at",
        "a11y_reducemotion" => "Turns off view bob, strafe tilt and the camera kick when hit or firing",
        "cl_camera" => "First person, a third-person camera centered behind the player, or one over the right or left shoulder; third person hides viewmodels",
        "r_drawviewmodel" => "Place every weapon at the center, right or left of the view, or hide all viewmodels",
        "cl_vwep" => "Player bodies hold the weapon in use instead of the generic gun",
        "v_flashscale" => "Strength of the full-screen flashes for damage, pickups and powerups",
        "hud_damageindicator" => "Red screen edges show where damage came from: top is front, bottom is behind; Screen Flashes controls their strength",
        "r_shader_scope" => "3D view keeps the HUD and menus sharp; whole screen runs them through the shader too",
        "r_shader_scale" => "Caps shader input at output size so 200% render scale keeps 3D sharpness without 4x shader cost; low scales stay low-res",
        "r_colorblind" => "Shifts colors so they stay distinct for the chosen color blindness",
        "hud_hitmarkers" => "Flash the selected marker on confirmed hits; colors are shared with the crosshair",
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
        "menu_music" => "Plays the Quake theme in the main menu when no game is running",
        "cl_captions" => "Shows text for important sounds: doors, alerted enemies, secrets, pickups",
        "scr_textscale" => "Size of on-screen messages and captions",
        "scr_textbg" => "Dark backdrop behind on-screen messages for readability",
        "scr_centertime" => "Seconds a center-screen message stays up",
        "con_notifytime" => "Seconds messages at the top of the screen stay up",
        "in_toggleattack" => "Press attack once to keep firing, again to stop",
        "in_togglerun" => "Press run once to switch between running and walking",
        "modifier_damagetaken" => "Scales all damage you take; above 100% is harder",
        "modifier_enemyhealth" => "Monsters take proportionally less damage at higher values; above 100% is harder",
        "modifier_ammo" => "Multiplies ammo from pickups (up to the normal carry limit); below 100% is harder",
        "modifier_health" => "Multiplies health from pickups (up to 100, or the item's own limit); below 100% is harder",
        "modifier_horde" => "Spawns extra copies of every monster around the original when a level starts",
        "modifier_pistolstart" => "Every level starts with only the shotgun and axe, as if started fresh",
        "modifier_nosaves" => "Only autosaves: quicksaves and the Save menu are off",
        "modifier_infiniteammo" => "Firing never uses up ammo",
        "modifier_nofalldamage" => "Long falls don't hurt",
        "modifier_nodrown" => "Staying underwater doesn't hurt",
        "modifier_noselfdamage" => "Your own rockets and grenades don't hurt you; rocket jumps still work",
        "modifier_regen" => "Slowly heals up to 100 health after 5 seconds without being hit",
        "modifier_jumps" => "Jump again in mid-air",
        "scr_hudstyle" => "Classic: the original status bar; modern: compact elements at the screen edges",
        "scr_centerpos" => "Where center-screen messages appear",
        "con_notifypos" => "Where pickup and event messages appear",
        "con_modern" => "Modern terminal console with autocomplete popup, history search, ANSI colors, bash shell and piping; off uses the classic Quake console",
        "hud_face" => "Shows the player face next to the health number",
        "hud_health" | "hud_armor" | "hud_ammo" | "hud_ammolist" | "hud_weapons" | "hud_items" => {
            "Immersive: appears for a few seconds only when its value changes"
        }
        "hud_health_pos" | "hud_armor_pos" | "hud_ammo_pos" | "hud_ammolist_pos" | "hud_weapons_pos" | "hud_items_pos" => {
            "Screen edge for this element; elements on the same spot stack"
        }
        "a11y_gamespeed" => "Slows the whole game down; single player only. Saves and demos stay compatible with other ports",
        _ => return None,
    })
}

/// Add-on binds are listed on the Controls page: while the running game has
/// add-ons, or (no game running) when New Game would start with them. Set
/// whenever the menu handles a key or draws.
pub(super) static ADDON_BINDS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn sync_addon_binds(cv: &Cvars, game_running: bool) {
    let on = if game_running { cv.value_of("custom_active") != 0.0 } else { super::launch::customized(cv) };
    ADDON_BINDS.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub(super) fn page_items(p: Page) -> &'static [Item] {
    match p {
        Page::Gameplay => GAMEPLAY,
        Page::Controls if ADDON_BINDS.load(std::sync::atomic::Ordering::Relaxed) => CONTROLS_ALL,
        Page::Controls => CONTROLS,
        Page::Customize => CUSTOMIZE,
        Page::NewGameCustomize => NEW_GAME_CUSTOMIZE,
        Page::Modifiers => MODIFIERS,
        Page::Addons => ADDONS,
        Page::NewGameAddons => NEW_GAME_ADDONS,
        Page::PortalGun => PORTAL_GUN,
        Page::Boomerang => BOOMERANG,
        Page::Railgun => RAILGUN,
        Page::Gel => GEL,
        Page::Washer => WASHER,
        Page::Hookshot => HOOKSHOT,
        Page::Cable => CABLE,
        Page::Sticky => STICKY,
        Page::Goggles => GOGGLES,
        Page::ProjectileShotguns => super::addon_settings::SHOTGUNS,
        Page::BulletTime => super::addon_settings::BULLET_TIME,
        Page::Hitstop => super::addon_settings::HITSTOP,
        Page::HitstopAdvanced => super::addon_settings::HITSTOP_ADVANCED,
        Page::Input => INPUT,
        Page::Graphics => GRAPHICS,
        Page::Enhancements => ENHANCEMENTS,
        Page::Impacts => IMPACTS,
        Page::Audio => AUDIO,
        Page::Demos => super::demos::DEMOS,
        Page::Video => super::demos::VIDEO,
        Page::Export => super::demos::EXPORT,
        Page::RenameDemo => super::demos::RENAME,
        Page::HitSound => HIT_SOUND,
        Page::KillSound => KILL_SOUND,
        Page::Accessibility => ACCESSIBILITY,
        Page::Hud => HUD,
        Page::Crosshair => CROSSHAIR,
        Page::HudLayout => HUD_LAYOUT,
        Page::Messages => MESSAGES,
        Page::Water => WATER,
        Page::Debris => DEBRIS,
        Page::Blood => BLOOD,
        Page::ScreenShader => SCREEN_SHADER,
        Page::Mouse => MOUSE,
        Page::Controller => CONTROLLER,
        Page::Gyro => GYRO,
        Page::Touch => TOUCH,
        Page::Autosave => AUTOSAVE,
        Page::AutoAim => AUTOAIM,
        Page::GrenadeArc => GRENADE_ARC,
        Page::Join => JOIN,
        Page::Host => HOST,
        Page::Setup => SETUP,
    }
}

pub(super) fn page_name(p: Page) -> &'static str {
    match p {
        Page::Crosshair => "Crosshair",
        Page::HitSound => "Hit Sound",
        Page::KillSound => "Kill Sound",
        Page::Join => "Join Game",
        Page::Host => "Host Game",
        Page::Setup => "Player Setup",
        Page::Mouse => "Mouse & Keyboard",
        Page::Controller => "Controller",
        Page::Gyro => "Gyro & Flick Stick",
        Page::Touch => "Touchscreen",
        Page::Autosave => "Autosave",
        Page::Video => "Video Recording",
        Page::Export => "Export Video",
        Page::RenameDemo => "Rename Demo",
        Page::Customize => "Customize This Game",
        Page::NewGameCustomize => "New Game: Customize",
        Page::Modifiers => "Gameplay Modifiers",
        Page::AutoAim => "Auto-aim Settings",
        Page::GrenadeArc => "Grenade Arc Settings",
        Page::HudLayout => "Modern HUD Layout",
        Page::Messages => "Messages & Captions",
        Page::Water => "Water Settings",
        Page::Impacts => "FX Settings",
        Page::Debris => "Debris Settings",
        Page::Blood => "Blood Splatter Settings",
        Page::ScreenShader => "Screen Shader",
        Page::Addons => "Add-ons",
        Page::NewGameAddons => "New Game: Add-ons",
        Page::PortalGun => "Portal Gun",
        Page::Boomerang => "Boomerang",
        Page::Railgun => "Railgun",
        Page::Gel => "Gel Sprayer",
        Page::Washer => "Powerwasher",
        Page::Hookshot => "Hookshot",
        Page::Cable => "Male-to-Male Cable",
        Page::Sticky => "Stickybomb Launcher",
        Page::Goggles => "Goggles",
        Page::ProjectileShotguns => "Projectile Shotguns",
        Page::BulletTime => "Bullet Time",
        Page::Hitstop => "Hitstop",
        Page::HitstopAdvanced => "Hitstop Advanced",
        _ => PAGES.iter().find(|x| x.1 == p).map_or("", |x| x.0),
    }
}

/// The page an `Item::Sub` page returns to, with that row's index.
pub(super) fn parent_page(p: Page) -> Option<(Page, usize)> {
    [Page::Customize, Page::NewGameCustomize, Page::Gameplay, Page::Input, Page::Controller, Page::Hud, Page::Crosshair, Page::Accessibility, Page::Enhancements, Page::Impacts, Page::Addons, Page::NewGameAddons, Page::Audio, Page::Demos].into_iter().find_map(|parent| {
        page_items(parent).iter().position(|i| matches!(i, Item::Sub { page, .. } if *page == p)).map(|i| (parent, i))
    })
}

/// Why a dimmed row is switched off, shown instead of its usual tooltip.
pub(super) fn disabled_reason(item: Item) -> Option<&'static str> {
    match item {
        Item::Choice { cvar: "r_waterwarp", .. } => {
            Some("Disabled while Enhanced Water is on: its own underwater effects replace the classic and GLQuake view warps")
        }
        Item::Run { run: Run::ExportClip, .. } => Some("No clip highlighted: while the demo plays, drag across the timeline, or press I and O (LT and RT on a controller)"),
        Item::Choice { cvar: "r_particles", .. } => {
            Some("Disabled while Enhanced Particles is on: its particles replace the GLQuake and Classic styles")
        }
        _ => None,
    }
}

/// Sub-pages that are switched off by their parent setting.
pub(super) fn sub_disabled(page: Page, cv: &Cvars) -> bool {
    (page == Page::AutoAim && cv.value_of("assist_autoaim") == 0.0)
        || (page == Page::GrenadeArc && cv.value_of("assist_grenadearc") == 0.0)
        || (page == Page::Autosave && cv.value_of("sv_autosave") == 0.0)
        || (page == Page::HudLayout && cv.value_of("scr_hudstyle") == 0.0)
        || (page == Page::Water && cv.value_of("r_enhancedwater") == 0.0)
        || (page == Page::Impacts && cv.value_of("r_enhancedfx") == 0.0)
        || (page == Page::Blood && cv.value_of("r_blood") == 0.0)
        || (page == Page::Debris && cv.value_of("r_debris") == 0.0)
        || (page == Page::Goggles && cv.value_of("addon_goggles") == 0.0)
        || (matches!(page, Page::Hitstop | Page::HitstopAdvanced) && cv.value_of("addon_hitstop") == 0.0)
}

pub(super) fn is_net_page(p: Page) -> bool {
    matches!(p, Page::Join | Page::Host | Page::Setup)
}

impl Menu {
    /// Shared by keyboard adjustment, pointer dragging and drawing.
    pub(super) fn item_disabled(&self, item: Item, cv: &Cvars) -> bool {
        self.game_locked(item, cv)
            || (matches!(item, Item::Run { run: Run::ExportClip, .. }) && cv.value_of("_export_hasclip") == 0.0)
            || (matches!(item, Item::Slider { cvar: "r_motionblur_amount", .. } | Item::Choice { cvar: "r_motionblur_quality", .. })
            && cv.value_of("r_motionblur") == 0.0)
            || (matches!(item, Item::Slider { cvar: "r_waterripples_amount", .. }) && cv.value_of("r_waterripples") == 0.0)
            || (matches!(item, Item::Slider { cvar: "r_watercaustics_amount", .. }) && cv.value_of("r_watercaustics") == 0.0)
            || (matches!(
                item,
                Item::Choice { cvar: "r_particle_quality" | "r_impact_fx", .. }
                    | Item::Toggle { cvar: "r_impact_bullets" | "r_impact_nails" | "r_impact_scorches", .. }
                    | Item::Slider { cvar: "r_impact_lifetime" | "r_impact_limit", .. }
            ) && cv.value_of("r_enhancedparticles") == 0.0)
            || (matches!(item, Item::Slider { cvar: "r_wateralpha", .. })
            && (cv.value_of("r_enhancedwater") == 0.0 || cv.value_of("r_watertranslucency") == 0.0))
            || (matches!(item, Item::Choice { cvar: "r_waterwarp", .. }) && cv.value_of("r_enhancedwater") != 0.0)
            || (matches!(item, Item::Choice { cvar: "r_particles", .. }) && cv.value_of("r_enhancedparticles") != 0.0)
            || (matches!(item, Item::Choice { cvar: "r_drawviewmodel", .. }) && cv.value_of("cl_camera") != 0.0)
            || (matches!(item, Item::Choice { cvar: "r_portal_quality", .. })
                && match self.screen {
                    Screen::OptionsPage(Page::Enhancements) => cv.value_of("r_teleportwindows") == 0.0,
                    Screen::OptionsPage(Page::PortalGun) => cv.value_of("addon_portal_view") == 0.0,
                    _ => false,
                })
            || (matches!(
                item,
                Item::Choice { cvar: "addon_portal_max" | "addon_portal_depth", .. }
                    | Item::Slider { cvar: "addon_portal_dist", .. }
            ) && cv.value_of("addon_portal_view") == 0.0)
    }
}
