use super::*;

pub(super) const PAGES: [(&str, Page); 8] = [
    ("Gameplay", Page::Gameplay),
    ("HUD", Page::Hud),
    ("Controls", Page::Controls),
    ("Input", Page::Input),
    ("Graphics", Page::Graphics),
    ("Enhancements", Page::Enhancements),
    ("Audio", Page::Audio),
    ("Accessibility", Page::Accessibility),
];

/// One options row.
#[derive(Clone, Copy)]
pub(super) enum Item {
    /// cvar slider; `invert` puts larger values on the left (gamma).
    Slider { label: &'static str, cvar: &'static str, min: f32, max: f32, step: f32, invert: bool },
    Toggle { label: &'static str, cvar: &'static str },
    /// Fixed list of (text, value).
    Choice { label: &'static str, cvar: &'static str, choices: &'static [(&'static str, f32)] },
    /// Pending display mode / resolution, committed by `Apply`.
    DisplayMode,
    Resolution,
    Apply,
    /// Editable text cvar.
    Text { label: &'static str, cvar: &'static str, max: usize },
    /// Shirt (`high`) or pants nibble of `_cl_color`.
    Color { label: &'static str, high: bool },
    /// Map for a hosted game (`_mp_map`), from the host's map list.
    Map,
    Run { label: &'static str, run: Run },
    /// A found LAN server (index into `Menu::servers`).
    Server(usize),
    /// Rebindable action: keyboard/mouse and gamepad keys bound to `cmd`.
    Bind { label: &'static str, cmd: &'static str },
    /// Restore the default bindings.
    ResetBinds,
    /// Slang preset (`r_shader_preset`) from `Menu::shader_presets`.
    ShaderPreset,
    /// k-th parameter of the loaded slang preset.
    ShaderParam(usize),
    /// Clears `r_shader_params` (the preset's own defaults).
    ResetShaderParams,
    /// Opens a nested options page.
    Sub { label: &'static str, page: Page },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Run {
    Connect,
    Search,
    Start,
    /// Begin a new single player game from the add-on picker.
    NewGame,
    PreviewHit,
    PreviewKill,
}

/// A server answering the LAN search.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerEntry {
    pub address: String,
    pub hostname: String,
    pub map: String,
    pub players: u8,
    pub max_players: u8,
}

pub(super) const JOIN: &[Item] = &[
    Item::Text { label: "Address", cvar: "_mp_address", max: 40 },
    Item::Run { label: "Connect", run: Run::Connect },
    Item::Run { label: "Search LAN", run: Run::Search },
];

pub(super) const fn bind(label: &'static str, cmd: &'static str) -> Item {
    Item::Bind { label, cmd }
}

/// Controls rows; the last two (add-on binds) show only with add-ons on.
pub(super) const CONTROLS_ALL: &[Item] = &[
    bind("Attack", "+attack"),
    bind("Jump / Swim Up", "+jump"),
    bind("Weapon Wheel", "+weaponwheel"),
    bind("Next Weapon", "impulse 10"),
    bind("Previous Weapon", "impulse 12"),
    bind("Move Forward", "+forward"),
    bind("Move Back", "+back"),
    bind("Strafe Left", "+moveleft"),
    bind("Strafe Right", "+moveright"),
    bind("Swim Down", "+movedown"),
    bind("Run", "+speed"),
    bind("Turn Left", "+left"),
    bind("Turn Right", "+right"),
    bind("Look Up", "+lookup"),
    bind("Look Down", "+lookdown"),
    bind("Center View", "centerview"),
    bind("Scoreboard", "+showscores"),
    bind("Quick Save", "echo Quicksaving...; wait; save quick"),
    bind("Quick Load", "echo Quickloading...; wait; load quick"),
    Item::ResetBinds,
    bind("Alt Fire (Add-ons)", "+attack2"),
    bind("Toy Slot (Add-ons)", "+toy"),
    bind("Equipment Wheel (Add-ons)", "+equipwheel"),
];
pub(super) const CONTROLS: &[Item] = CONTROLS_ALL.split_at(CONTROLS_ALL.len() - 3).0;

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
    toggle("Goggles", "addon_goggles"),
    Item::Sub { label: "Goggles Settings", page: Page::Goggles },
    Item::Run { label: "Start New Game", run: Run::NewGame },
];

/// Reached from the Add-ons pages: how portals look and perform.
pub(super) const PORTAL_GUN: &[Item] = &[
    toggle("See Through Portals", "addon_portal_view"),
    Item::Choice { label: "Live Portal Views", cvar: "addon_portal_max", choices: PORTAL_MAX },
    slider("Live View Distance", "addon_portal_dist", 512.0, 8192.0, 512.0),
    Item::Choice { label: "Portal Recursion", cvar: "addon_portal_depth", choices: PORTAL_DEPTH },
];
/// Reached from the Add-ons pages: boomerang stun time and flight speed.
pub(super) const BOOMERANG: &[Item] = &[
    slider("Stun Duration", "addon_boomerang_stun", 1.0, 10.0, 0.5),
    slider("Flight Speed", "addon_boomerang_speed", 300.0, 1500.0, 100.0),
];
/// Reached from the Add-ons pages: railgun damage and trail colour.
pub(super) const RAILGUN: &[Item] = &[
    slider("Damage", "addon_rail_damage", 25.0, 300.0, 25.0),
    Item::Choice { label: "Trail Colour", cvar: "addon_rail_color", choices: RAIL_COLOR },
];
pub(super) const RAIL_COLOR: &[(&str, f32)] =
    &[("Blue", 0.0), ("Green", 1.0), ("Red", 2.0), ("Yellow", 3.0), ("Purple", 4.0), ("White", 5.0)];
/// Reached from the Add-ons pages: hookshot range and what it latches onto.
pub(super) const HOOKSHOT: &[Item] = &[
    slider("Hookshot Range", "addon_hook_range", 256.0, 4096.0, 128.0),
    toggle("Hook Wood", "addon_hook_wood"),
    toggle("Hook Metal", "addon_hook_metal"),
    toggle("Hook Stone", "addon_hook_stone"),
    toggle("Hook Enemies", "addon_hook_enemies"),
];
pub(super) const PORTAL_DEPTH: &[(&str, f32)] =
    &[("Off", 0.0), ("1", 1.0), ("2", 2.0), ("3", 3.0), ("4", 4.0), ("6", 6.0)];
pub(super) const PORTAL_MAX: &[(&str, f32)] = &[("1", 1.0), ("2", 2.0), ("3", 3.0), ("4", 4.0)];

pub(super) const PLAYERS: &[(&str, f32)] = &[("2", 2.0), ("3", 3.0), ("4", 4.0), ("6", 6.0), ("8", 8.0), ("12", 12.0), ("16", 16.0)];
pub(super) const LOCAL_PLAYERS: &[(&str, f32)] =
    &[("1", 1.0), ("2 split-screen", 2.0), ("3 split-screen", 3.0), ("4 split-screen", 4.0), ("5 split-screen", 5.0), ("6 split-screen", 6.0)];
pub(super) const GAMETYPE: &[(&str, f32)] = &[("deathmatch", 0.0), ("cooperative", 1.0)];
pub(super) const TEAMPLAY: &[(&str, f32)] = &[("off", 0.0), ("no team damage", 1.0), ("team damage", 2.0)];
pub(super) const SKILL: &[(&str, f32)] = &[("easy", 0.0), ("normal", 1.0), ("hard", 2.0), ("nightmare", 3.0)];
pub(super) const FRAGS: &[(&str, f32)] =
    &[("none", 0.0), ("10", 10.0), ("20", 20.0), ("30", 30.0), ("50", 50.0), ("75", 75.0), ("100", 100.0)];
pub(super) const MINUTES: &[(&str, f32)] =
    &[("none", 0.0), ("5", 5.0), ("10", 10.0), ("15", 15.0), ("20", 20.0), ("30", 30.0), ("45", 45.0), ("60", 60.0)];

pub(super) const HOST: &[Item] = &[
    Item::Choice { label: "Max Players", cvar: "_mp_maxplayers", choices: PLAYERS },
    Item::Choice { label: "Local Players", cvar: "_mp_localplayers", choices: LOCAL_PLAYERS },
    Item::Choice { label: "Game Type", cvar: "_mp_coop", choices: GAMETYPE },
    Item::Choice { label: "Teamplay", cvar: "_mp_teamplay", choices: TEAMPLAY },
    Item::Choice { label: "Skill", cvar: "_mp_skill", choices: SKILL },
    Item::Choice { label: "Frag Limit", cvar: "_mp_fraglimit", choices: FRAGS },
    Item::Choice { label: "Time Limit", cvar: "_mp_timelimit", choices: MINUTES },
    Item::Map,
    Item::Run { label: "Start Game", run: Run::Start },
];

pub(super) const SETUP: &[Item] = &[
    Item::Text { label: "Name", cvar: "_cl_name", max: 15 },
    Item::Color { label: "Shirt Color", high: true },
    Item::Color { label: "Pants Color", high: false },
];

pub(super) const fn slider(label: &'static str, cvar: &'static str, min: f32, max: f32, step: f32) -> Item {
    Item::Slider { label, cvar, min, max, step, invert: false }
}
pub(super) const fn toggle(label: &'static str, cvar: &'static str) -> Item {
    Item::Toggle { label, cvar }
}

pub(super) const GAMEPLAY: &[Item] = &[
    Item::Choice { label: "On Death", cvar: "cl_deathaction", choices: &[("Ask", 0.0), ("Most recent save", 1.0), ("Select save", 2.0), ("Restart level", 3.0)] },
    slider("Field of View", "fov", 60.0, 130.0, 5.0),
    toggle("Always Run", "cl_alwaysrun"),
    slider("View Bob", "cl_bob", 0.0, 0.02, 0.002),
    toggle("Gun Kick", "v_gunkick"),
    Item::Choice { label: "Viewmodels", cvar: "r_drawviewmodel", choices: &[("Center", 1.0), ("Right", 2.0), ("Left", 3.0), ("Off", 0.0)] },
    toggle("Level Select", "ui_levelselect"),
    toggle("Autosave", "sv_autosave"),
    Item::Sub { label: "Autosave Settings", page: Page::Autosave },
    Item::Sub { label: "Assists", page: Page::Assists },
];

pub(super) const PERCENT: &[(&str, f32)] = &[
    ("25%", 0.25), ("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("200%", 2.0),
];
pub(super) const AMMO_MULT: &[(&str, f32)] = &[("100%", 1.0), ("150%", 1.5), ("200%", 2.0), ("300%", 3.0)];
pub(super) const JUMPS: &[(&str, f32)] = &[("single", 1.0), ("double", 2.0), ("triple", 3.0)];

pub(super) const ASSISTS: &[Item] = &[
    toggle("Auto-aim", "assist_autoaim"),
    Item::Sub { label: "Auto-aim Settings", page: Page::AutoAim },
    Item::Choice { label: "Damage Taken", cvar: "assist_damagetaken", choices: PERCENT },
    Item::Choice { label: "Enemy Health", cvar: "assist_enemyhealth", choices: PERCENT },
    Item::Choice { label: "Ammo Pickups", cvar: "assist_ammo", choices: AMMO_MULT },
    toggle("Infinite Ammo", "assist_infiniteammo"),
    toggle("No Fall Damage", "assist_nofalldamage"),
    toggle("No Drowning", "assist_nodrown"),
    toggle("No Self Damage", "assist_noselfdamage"),
    toggle("Health Regeneration", "assist_regen"),
    Item::Choice { label: "Jumps", cvar: "assist_jumps", choices: JUMPS },
];

pub(super) const AUTOAIM: &[Item] = &[
    Item::Choice { label: "Mode", cvar: "assist_autoaim_mode", choices: &[("Hold lock-on", 0.0), ("Toggle auto-aim", 1.0)] },
    toggle("Lock Indicator", "assist_autoaim_indicator"),
    toggle("Reacquire While Held", "assist_autoaim_reacquire"),
    slider("Acquisition Cone", "assist_autoaim_cone", 1.0, 30.0, 1.0),
    slider("Maximum Range", "assist_autoaim_range", 128.0, 4096.0, 128.0),
];

pub(super) const AUTOSAVE: &[Item] = &[
    slider("Autosaves to Keep", "sv_autosave_keep", 1.0, 10.0, 1.0),
    toggle("Save on Level Entry", "sv_autosave_entry"),
    Item::Choice { label: "Autosave Timer", cvar: "sv_autosave_interval", choices: MINUTES },
    toggle("Save on Secret Found", "sv_autosave_secret"),
    toggle("Save on All Enemies Killed", "sv_autosave_monsters"),
    toggle("Save on All Secrets Found", "sv_autosave_secrets"),
];

pub(super) const INPUT: &[Item] = &[
    Item::Sub { label: "Mouse & Keyboard", page: Page::Mouse },
    Item::Sub { label: "Controller", page: Page::Controller },
    Item::Sub { label: "Touchscreen", page: Page::Touch },
];

pub(super) const MOUSE: &[Item] = &[
    slider("Mouse Speed", "sensitivity", 1.0, 11.0, 0.5),
    toggle("Invert Mouse", "m_invert"),
];

pub(super) const CONTROLLER: &[Item] = &[
    slider("Stick Look Yaw", "joy_sensitivity_yaw", 60.0, 480.0, 20.0),
    slider("Stick Look Pitch", "joy_sensitivity_pitch", 40.0, 300.0, 10.0),
    toggle("Invert Stick", "joy_invert"),
    toggle("Aim Assist", "aim_assist"),
    slider("Aim Slowdown", "aim_slowdown", 0.0, 1.0, 0.1),
    slider("Aim Rotation", "aim_rotation", 0.0, 1.0, 0.1),
    toggle("Gyro", "gyro_enable"),
    slider("Rumble", "joy_rumble", 0.0, 1.0, 0.25),
    Item::Choice { label: "Button Glyphs", cvar: "joy_glyphs", choices: GLYPHS },
];

pub(super) const TOUCH: &[Item] = &[
    Item::Choice { label: "Touch Controls", cvar: "touch_enable", choices: TOUCH_MODE },
    slider("Look Speed", "touch_sensitivity", 0.25, 3.0, 0.25),
    toggle("Invert Look", "touch_invert"),
    slider("Button Size", "touch_size", 0.5, 2.0, 0.25),
    slider("Opacity", "touch_opacity", 0.1, 1.0, 0.1),
];

pub(super) const TOUCH_MODE: &[(&str, f32)] = &[("off", 0.0), ("auto", 1.0), ("always", 2.0)];
pub(super) const GLYPHS: &[(&str, f32)] = &[("auto", 0.0), ("xbox", 1.0), ("playstation", 2.0), ("nintendo", 3.0)];
pub(super) const WHEEL_TIME: &[(&str, f32)] = &[("off", 1.0), ("half speed", 0.5), ("quarter speed", 0.25), ("10% speed", 0.1)];

pub(super) const MSAA: &[(&str, f32)] = &[("off", 1.0), ("2x", 2.0), ("4x", 4.0), ("8x", 8.0)];
pub(super) const SCALE: &[(&str, f32)] = &[("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("200%", 2.0)];
pub(super) const FILTER: &[(&str, f32)] = &[("nearest", 0.0), ("linear", 1.0)];
pub(super) const ANISO: &[(&str, f32)] = &[("off", 1.0), ("2x", 2.0), ("4x", 4.0), ("8x", 8.0), ("16x", 16.0)];
pub(super) const SOFTEMU: &[(&str, f32)] = &[("off", 0.0), ("fine", 1.0), ("coarse", 2.0), ("banded", 3.0)];
pub(super) const SSAO: &[(&str, f32)] = &[("off", 0.0), ("low", 0.5), ("medium", 1.0), ("high", 2.0)];
pub(super) const MODES: [&str; 3] = ["windowed", "fullscreen", "borderless"];

pub(super) const GRAPHICS: &[Item] = &[
    Item::DisplayMode,
    Item::Resolution,
    Item::Apply,
    toggle("VSync", "vid_vsync"),
    slider("Max FPS", "host_maxfps", 60.0, 500.0, 10.0),
    Item::Choice { label: "Anti-aliasing", cvar: "r_msaa", choices: MSAA },
    toggle("Supersampling", "vid_fsaamode"),
    Item::Choice { label: "Render Scale", cvar: "r_renderscale", choices: SCALE },
    Item::Choice { label: "Texture Filter", cvar: "r_texfilter", choices: FILTER },
    Item::Choice { label: "Anisotropic", cvar: "r_anisotropy", choices: ANISO },
    Item::Choice { label: "8-bit Color", cvar: "r_softemu", choices: SOFTEMU },
    toggle("Dithering", "r_dither"),
    Item::Slider { label: "Brightness", cvar: "gamma", min: 0.5, max: 1.5, step: 0.05, invert: true },
    slider("Contrast", "contrast", 1.0, 2.0, 0.1),
    toggle("Dynamic Lights", "r_dynamic"),
    toggle("Colored Lighting", "r_coloredlight"),
    toggle("Fog", "r_fog"),
    slider("Sky Fog", "r_skyfog", 0.0, 1.0, 0.1),
    toggle("Interpolate Animation", "r_lerpmodels"),
    toggle("Interpolate Movement", "r_lerpmove"),
];

pub(super) const ENHANCEMENTS: &[Item] = &[
    Item::Choice { label: "Ambient Occlusion", cvar: "r_ssao", choices: SSAO },
    toggle("Motion Blur", "r_motionblur"),
    slider("Motion Blur Amount", "r_motionblur_amount", 0.1, 2.0, 0.1),
    toggle("Enhanced Water", "r_enhancedwater"),
    toggle("See-through Teleporters", "r_teleportwindows"),
    Item::Sub { label: "Water Settings", page: Page::Water },
    Item::Sub { label: "Screen Shader", page: Page::ScreenShader },
    Item::Sub { label: "Impact Effects", page: Page::Impacts },
    toggle("Enhanced Blood Splatter", "r_blood"),
    Item::Sub { label: "Blood Splatter Settings", page: Page::Blood },
    toggle("Ragdolls & Debris", "r_debris"),
    Item::Sub { label: "Debris Settings", page: Page::Debris },
];

pub(super) const IMPACTS: &[Item] = &[
    toggle("Shotgun Bullet Decals", "r_impact_bullets"),
    toggle("Physical Nail Impacts", "r_impact_nails"),
    toggle("Explosion Scorch Marks", "r_impact_scorches"),
    slider("Impact Lifetime", "r_impact_lifetime", 5.0, 600.0, 5.0),
    slider("Maximum Impacts", "r_impact_limit", 32.0, 2048.0, 32.0),
];

/// Reached from Enhancements; unavailable while `r_enhancedwater` is off.
pub(super) const WATER: &[Item] = &[
    slider("Water Opacity", "r_wateralpha", 0.0, 1.0, 0.1),
    toggle("Ripples", "r_waterripples"),
    slider("Ripple Intensity", "r_waterripples_amount", 0.1, 2.0, 0.1),
    toggle("Splashes", "r_watersplash"),
    toggle("Caustics", "r_watercaustics"),
    slider("Caustic Intensity", "r_watercaustics_amount", 0.1, 2.0, 0.1),
];

pub(super) const RAGDOLL_START: &[(&str, f32)] = &[("after animation", 0.0), ("at death", 1.0), ("mixed", 2.0)];

/// Reached from Enhancements; unavailable while `r_debris` is off.
pub(super) const DEBRIS: &[Item] = &[
    toggle("Ragdolls", "r_debris_ragdolls"),
    Item::Choice { label: "Ragdoll Start", cvar: "r_debris_ragdollstart", choices: RAGDOLL_START },
    slider("Max Ragdolls", "r_debris_maxragdolls", 8.0, 64.0, 8.0),
    toggle("Gibs", "r_debris_gibs"),
    toggle("Shrapnel", "r_debris_shrapnel"),
    toggle("Hit Doors & Lifts", "r_debris_movers"),
    slider("Push Strength", "r_debris_force", 0.0, 6.0, 0.25),
    toggle("Kill Lift", "r_debris_killlift"),
];

pub(super) const COLORBLIND: &[(&str, f32)] =
    &[("off", 0.0), ("protanopia", 1.0), ("deuteranopia", 2.0), ("tritanopia", 3.0)];
pub(super) const XHAIR_SIZE: &[(&str, f32)] = &[("small", 0.75), ("normal", 1.0), ("large", 1.5), ("huge", 2.5)];
pub(super) const XHAIR_SHAPE: &[(&str, f32)] =
    &[("cross", 0.0), ("gap cross", 1.0), ("dot", 2.0), ("cross + dot", 3.0), ("T", 4.0), ("box", 5.0)];
pub(super) const XHAIR_COLOR: &[(&str, f32)] =
    &[("white", 0.0), ("yellow", 1.0), ("green", 2.0), ("cyan", 3.0), ("magenta", 4.0), ("red", 5.0)];
pub(super) const TEXT_SIZE: &[(&str, f32)] = &[("normal", 1.0), ("large", 1.5), ("larger", 2.0), ("largest", 3.0)];
pub(super) const GAME_SPEED: &[(&str, f32)] = &[("100%", 1.0), ("90%", 0.9), ("75%", 0.75), ("60%", 0.6), ("50%", 0.5)];

pub(super) const ACCESSIBILITY: &[Item] = &[
    toggle("Reduce Motion", "a11y_reducemotion"),
    slider("Screen Flashes", "v_flashscale", 0.0, 1.0, 0.1),
    Item::Choice { label: "Color Filter", cvar: "r_colorblind", choices: COLORBLIND },
    Item::Sub { label: "Messages & Captions", page: Page::Messages },
    toggle("Toggle Attack", "in_toggleattack"),
    toggle("Toggle Run", "in_togglerun"),
    Item::Choice { label: "Game Speed", cvar: "a11y_gamespeed", choices: GAME_SPEED },
    Item::Sub { label: "Assists", page: Page::Assists },
];

pub(super) const HUD_MODE: &[(&str, f32)] = &[("off", 0.0), ("on", 1.0), ("immersive", 2.0)];
pub(super) const HUD_POS: &[(&str, f32)] = &[
    ("bottom left", 0.0),
    ("bottom center", 1.0),
    ("bottom right", 2.0),
    ("top left", 3.0),
    ("top center", 4.0),
    ("top right", 5.0),
    ("middle left", 6.0),
    ("middle right", 7.0),
];
pub(super) const HUD_STYLE: &[(&str, f32)] = &[("classic", 0.0), ("modern", 1.0)];
pub(super) const CENTER_POS: &[(&str, f32)] = &[("upper", 0.0), ("middle", 1.0), ("lower", 2.0)];
pub(super) const NOTIFY_POS: &[(&str, f32)] = &[("top left", 0.0), ("top center", 1.0), ("bottom left", 2.0)];

pub(super) const HUD: &[Item] = &[
    Item::Choice { label: "HUD Style", cvar: "scr_hudstyle", choices: HUD_STYLE },
    Item::Sub { label: "Modern HUD Layout", page: Page::HudLayout },
    toggle("Crosshair", "crosshair"),
    Item::Choice { label: "Crosshair Shape", cvar: "crosshair_shape", choices: XHAIR_SHAPE },
    Item::Choice { label: "Crosshair Size", cvar: "crosshair_size", choices: XHAIR_SIZE },
    Item::Choice { label: "Crosshair Color", cvar: "crosshair_color", choices: XHAIR_COLOR },
    toggle("Crosshair Outline", "crosshair_outline"),
    toggle("Hit Markers", "hud_hitmarkers"),
    Item::Choice { label: "Weapon Wheel Slowdown", cvar: "cl_wheel_timescale", choices: WHEEL_TIME },
    Item::Sub { label: "Messages & Captions", page: Page::Messages },
    toggle("Show FPS", "scr_showfps"),
];

pub(super) const MESSAGES: &[Item] = &[
    Item::Choice { label: "Center Message Position", cvar: "scr_centerpos", choices: CENTER_POS },
    slider("Center Message Time", "scr_centertime", 1.0, 10.0, 1.0),
    Item::Choice { label: "Notify Message Position", cvar: "con_notifypos", choices: NOTIFY_POS },
    slider("Notify Message Time", "con_notifytime", 1.0, 10.0, 1.0),
    Item::Choice { label: "Message Text Size", cvar: "scr_textscale", choices: TEXT_SIZE },
    toggle("Message Background", "scr_textbg"),
    toggle("Sound Captions", "cl_captions"),
];

pub(super) const fn hud_mode(label: &'static str, cvar: &'static str) -> Item {
    Item::Choice { label, cvar, choices: HUD_MODE }
}
pub(super) const fn hud_pos(label: &'static str, cvar: &'static str) -> Item {
    Item::Choice { label, cvar, choices: HUD_POS }
}

pub(super) const HUD_LAYOUT: &[Item] = &[
    hud_mode("Health", "hud_health"),
    hud_pos("Health Position", "hud_health_pos"),
    toggle("Show Face", "hud_face"),
    hud_mode("Armor", "hud_armor"),
    hud_pos("Armor Position", "hud_armor_pos"),
    hud_mode("Current Ammo", "hud_ammo"),
    hud_pos("Current Ammo Position", "hud_ammo_pos"),
    hud_mode("Ammo Counts", "hud_ammolist"),
    hud_pos("Ammo Counts Position", "hud_ammolist_pos"),
    hud_mode("Weapons", "hud_weapons"),
    hud_pos("Weapons Position", "hud_weapons_pos"),
    hud_mode("Keys & Powerups", "hud_items"),
    hud_pos("Keys & Powerups Position", "hud_items_pos"),
];

pub(super) const AUDIO: &[Item] = &[
    slider("Sound Volume", "volume", 0.0, 1.0, 0.1),
    slider("Music Volume", "bgmvolume", 0.0, 1.0, 0.1),
    toggle("Hit & Kill Sounds", "s_hitfeedback"),
    Item::Sub { label: "Hit Sound", page: Page::HitSound },
    Item::Sub { label: "Kill Sound", page: Page::KillSound },
];

/// Independent of the debris toggle and particle scripts.
pub(super) const BLOOD: &[Item] = &[
    slider("Splatter Amount", "r_blood_amount", 1.0, 12.0, 1.0),
    slider("Splatter Size", "r_blood_size", 0.25, 3.0, 0.25),
    slider("Lifetime (seconds)", "r_blood_lifetime", 5.0, 600.0, 5.0),
    slider("Maximum Marks", "r_blood_maxmarks", 32.0, 2048.0, 32.0),
    toggle("Enemy Wounds", "r_blood_wounds"),
    toggle("Gib Surface Painting", "r_blood_gibs"),
];

/// Screen Shader page; the preset's parameter sliders follow these rows.
pub(super) const SCREEN_SHADER: &[Item] = &[
    Item::ShaderPreset,
    Item::Choice { label: "Apply To", cvar: "r_shader_scope", choices: SHADER_SCOPE },
    Item::ResetShaderParams,
];
pub(super) const SHADER_SCOPE: &[(&str, f32)] = &[("3D view", 0.0), ("whole screen", 1.0)];

/// Reached from the Add-ons pages: night-vision / thermal goggle look.
pub(super) const GOGGLES: &[Item] = &[
    slider("Goggle Intensity", "addon_goggles_intensity", 0.25, 3.0, 0.25),
    slider("Goggle Noise", "addon_goggles_noise", 0.0, 2.0, 0.25),
];
