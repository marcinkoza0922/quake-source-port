use super::*;

pub(super) const PAGES: [(&str, Page); 7] = [
    ("Gameplay", Page::Gameplay),
    ("HUD", Page::Hud),
    ("Controls", Page::Controls),
    ("Input", Page::Input),
    ("Graphics", Page::Graphics),
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
    /// Opens a nested options page.
    Sub { label: &'static str, page: Page },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Run {
    Connect,
    Search,
    Start,
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

pub(super) const CONTROLS: &[Item] = &[
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
];

pub(super) const PLAYERS: &[(&str, f32)] = &[("2", 2.0), ("3", 3.0), ("4", 4.0), ("6", 6.0), ("8", 8.0), ("12", 12.0), ("16", 16.0)];
pub(super) const GAMETYPE: &[(&str, f32)] = &[("deathmatch", 0.0), ("cooperative", 1.0)];
pub(super) const TEAMPLAY: &[(&str, f32)] = &[("off", 0.0), ("no team damage", 1.0), ("team damage", 2.0)];
pub(super) const SKILL: &[(&str, f32)] = &[("easy", 0.0), ("normal", 1.0), ("hard", 2.0), ("nightmare", 3.0)];
pub(super) const FRAGS: &[(&str, f32)] =
    &[("none", 0.0), ("10", 10.0), ("20", 20.0), ("30", 30.0), ("50", 50.0), ("75", 75.0), ("100", 100.0)];
pub(super) const MINUTES: &[(&str, f32)] =
    &[("none", 0.0), ("5", 5.0), ("10", 10.0), ("15", 15.0), ("20", 20.0), ("30", 30.0), ("45", 45.0), ("60", 60.0)];

pub(super) const HOST: &[Item] = &[
    Item::Choice { label: "Max Players", cvar: "_mp_maxplayers", choices: PLAYERS },
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
    slider("Field of View", "fov", 60.0, 130.0, 5.0),
    toggle("Always Run", "cl_alwaysrun"),
    slider("View Bob", "cl_bob", 0.0, 0.02, 0.002),
    toggle("Gun Kick", "v_gunkick"),
    toggle("Show Weapon", "r_drawviewmodel"),
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
    Item::Choice { label: "Ambient Occlusion", cvar: "r_ssao", choices: SSAO },
    toggle("Motion Blur", "r_motionblur"),
    slider("Motion Blur Amount", "r_motionblur_amount", 0.1, 2.0, 0.1),
    toggle("Enhanced Water", "r_enhancedwater"),
    Item::Sub { label: "Water Settings", page: Page::Water },
    toggle("Ragdolls & Debris", "r_debris"),
    Item::Sub { label: "Debris Settings", page: Page::Debris },
    toggle("Interpolate Animation", "r_lerpmodels"),
    toggle("Interpolate Movement", "r_lerpmove"),
];

/// Reached from Graphics; unavailable while `r_enhancedwater` is off.
pub(super) const WATER: &[Item] = &[
    slider("Water Opacity", "r_wateralpha", 0.0, 1.0, 0.1),
    toggle("Ripples", "r_waterripples"),
    slider("Ripple Intensity", "r_waterripples_amount", 0.1, 2.0, 0.1),
    toggle("Splashes", "r_watersplash"),
    toggle("Caustics", "r_watercaustics"),
    slider("Caustic Intensity", "r_watercaustics_amount", 0.1, 2.0, 0.1),
];

pub(super) const RAGDOLL_START: &[(&str, f32)] = &[("after animation", 0.0), ("at death", 1.0)];

/// Reached from Graphics; unavailable while `r_debris` is off.
pub(super) const DEBRIS: &[Item] = &[
    toggle("Ragdolls", "r_debris_ragdolls"),
    Item::Choice { label: "Ragdoll Start", cvar: "r_debris_ragdollstart", choices: RAGDOLL_START },
    slider("Max Ragdolls", "r_debris_maxragdolls", 8.0, 64.0, 8.0),
    toggle("Gibs", "r_debris_gibs"),
    toggle("Shrapnel", "r_debris_shrapnel"),
    toggle("Hit Doors & Lifts", "r_debris_movers"),
    slider("Push Strength", "r_debris_force", 0.0, 2.0, 0.25),
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

pub(super) const AUDIO: &[Item] = &[slider("Sound Volume", "volume", 0.0, 1.0, 0.1), slider("Music Volume", "bgmvolume", 0.0, 1.0, 0.1)];
