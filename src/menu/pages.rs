use super::*;

pub(super) const PAGES: [(&str, Page); 9] = [
    ("Gameplay", Page::Gameplay),
    ("HUD", Page::Hud),
    ("Controls", Page::Controls),
    ("Input", Page::Input),
    ("Graphics", Page::Graphics),
    ("Enhancements", Page::Enhancements),
    ("Audio", Page::Audio),
    ("Accessibility", Page::Accessibility),
    ("Demos", Page::Demos),
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
    /// `language`: System (auto) or one of the re-release's languages.
    Language,
    /// User data directory (opens the in-game directory explorer).
    UserDataDir,
    /// Soundtrack pack (`s_soundtrack`) from `Menu::soundtracks`.
    Soundtrack,
    /// Built-in and user-saved HUD presets (`hud_preset`, `hud_preset_name`).
    HudPreset,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Run {
    Connect,
    BrowseServers,
    Start,
    /// Begin a new single player game from the add-on picker.
    NewGame,
    PreviewHit,
    PreviewKill,
    ResetShotguns,
    ResetBulletTime,
    ResetHitstop,
    /// Every gameplay modifier back to its default (standard rules).
    ResetModifiers,
    /// Export dialog: the highlighted clip (dimmed without one), or all.
    ExportClip,
    ExportWhole,
    /// Rename dialog: renames the demo (an auto-demo is kept).
    RenameDemo,
    /// HUD → Edit HUD: the HUD editor (`crate::hud::editor`).
    EditHud,
}

pub use crate::net::browser::ServerEntry;

pub(super) const JOIN: &[Item] = &[
    Item::Text { label: "Address", cvar: "_mp_address", max: 40 },
    Item::Run { label: "Connect", run: Run::Connect },
    Item::Run { label: "Server Browser", run: Run::BrowseServers },
];

pub(super) const fn bind(label: &'static str, cmd: &'static str) -> Item {
    Item::Bind { label, cmd }
}

/// Controls rows; add-on binds appear above Reset when add-ons are active.
macro_rules! controls_list {
    ($($extra:expr,)*) => {
        &[
            bind("Attack", "+attack"),
            bind("Weapon Wheel", "+weaponwheel"),
            bind("Next Weapon", "impulse 10"),
            bind("Previous Weapon", "impulse 12"),
            bind("Move Forward", "+forward"),
            bind("Move Back", "+back"),
            bind("Strafe Left", "+moveleft"),
            bind("Strafe Right", "+moveright"),
            bind("Jump / Swim Up", "+jump"),
            bind("Swim Down", "+movedown"),
            bind("Run", "+speed"),
            bind("Turn Left", "+left"),
            bind("Turn Right", "+right"),
            bind("Look Up", "+lookup"),
            bind("Look Down", "+lookdown"),
            bind("Center View", "centerview"),
            bind("Automap", "automap"),
            bind("Scoreboard", "+showscores"),
            bind("Quick Save", "echo Quicksaving...; wait; save quick"),
            bind("Quick Load", "echo Quickloading...; wait; load quick"),
            bind("Quick Reset", "restart"),
            bind("Screenshot", "screenshot"),
            bind("Record Demo", "record_toggle"),
            $($extra,)*
            Item::ResetBinds,
        ]
    };
}
pub(super) const CONTROLS: &[Item] = controls_list!();
pub(super) const CONTROLS_ALL: &[Item] = controls_list!(
    bind("Alt Fire (Add-ons)", "+attack2"),
    bind("Equipment Wheel (Add-ons)", "+equipwheel"),
    bind("Dodge / Dive (Add-ons)", "+dodge"),
    bind("Bullet Time (Add-ons)", "+bullettime"),
);

/// Reached from the Add-ons pages: how portals look and perform.
pub(super) const PORTAL_GUN: &[Item] = &[
    toggle("See Through Portals", "addon_portal_view"),
    Item::Choice { label: "Portal View Quality", cvar: "r_portal_quality", choices: PORTAL_QUALITY },
    Item::Choice { label: "Live Portal Views", cvar: "addon_portal_max", choices: PORTAL_MAX },
    slider("Live View Distance", "addon_portal_dist", 512.0, 8192.0, 512.0),
    Item::Choice { label: "Portal Recursion", cvar: "addon_portal_depth", choices: PORTAL_DEPTH },
];
/// Reached from the Add-ons pages: boomerang stun time and flight speed.
pub(super) const BOOMERANG: &[Item] = &[
    slider("Stun Duration", "addon_boomerang_stun", 1.0, 10.0, 0.5),
    slider("Flight Speed", "addon_boomerang_speed", 300.0, 1500.0, 100.0),
];
/// Reached from the Add-ons pages: powerwasher push strength and jet quality.
pub(super) const WASHER_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("high", 1.0)];
pub(super) const WASHER: &[Item] = &[
    slider("Push", "addon_washer_push", 0.25, 3.0, 0.25),
    Item::Choice { label: "Jet Quality", cvar: "addon_washer_quality", choices: WASHER_QUALITY },
];
/// Reached from the Add-ons pages: gel coat resolution, rebuild cadence and shader detail.
pub(super) const GEL_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("high", 1.0)];
pub(super) const GEL: &[Item] = &[Item::Choice { label: "Gel Quality", cvar: "addon_gel_quality", choices: GEL_QUALITY }];
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
/// Reached from the Add-ons pages: cable length, sockets and bolt damage.
pub(super) const CABLE: &[Item] = &[
    slider("Cable Length", "addon_cable_length", 128.0, 2048.0, 64.0),
    toggle("Plug Into Metal", "addon_cable_metal"),
    slider("Bolt Damage", "addon_cable_damage", 5.0, 100.0, 5.0),
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

pub(super) const WEAPON_SWITCH: &[(&str, f32)] = &[("Never", 0.0), ("Better weapon", 1.0)];
pub(super) const AUTOMAP_VIEW: &[(&str, f32)] = &[("3D wireframe", 0.0), ("Top-down", 1.0)];
pub(super) const AUTOMAP_DISPLAY: &[(&str, f32)] = &[("Framed panel", 0.0), ("Center overlay", 1.0)];
pub(super) const AUTOMAP_REVEAL: &[(&str, f32)] = &[("Explored only", 0.0), ("Full map", 1.0)];

pub(super) const GAMEPLAY: &[Item] = &[
    Item::Language,
    toggle("Translate Console", "con_translate"),
    slider("Field of View", "fov", 60.0, 130.0, 5.0),
    Item::Choice { label: "Camera", cvar: "cl_camera", choices: &[("First person", 0.0), ("Third person", 1.0), ("Shoulder right", 2.0), ("Shoulder left", 3.0)] },
    Item::Choice { label: "Viewmodels", cvar: "r_drawviewmodel", choices: &[("Center", 1.0), ("Right", 2.0), ("Left", 3.0), ("Off", 0.0)] },
    toggle("Visible Weapons", "cl_vwep"),
    slider("View Bob", "cl_bob", 0.0, 0.02, 0.002),
    toggle("Gun Kick", "v_gunkick"),
    toggle("Always Run", "cl_alwaysrun"),
    Item::Choice { label: "Weapon Auto-Switch", cvar: "cl_autoswitch", choices: WEAPON_SWITCH },
    Item::Sub { label: "Weapon Priority", page: Page::WeaponPriority },
    toggle("Automap", "cl_automap"),
    Item::Sub { label: "Automap Settings", page: Page::Automap },
    Item::Choice { label: "On Death", cvar: "cl_deathaction", choices: &[("Ask", 0.0), ("Most recent save", 1.0), ("Select save", 2.0), ("Restart level", 3.0)] },
    toggle("Autosave", "sv_autosave"),
    Item::Sub { label: "Autosave Settings", page: Page::Autosave },
    Item::UserDataDir,
];

pub(super) const AUTOMAP: &[Item] = &[
    Item::Choice { label: "View Mode", cvar: "cl_automap_view", choices: AUTOMAP_VIEW },
    Item::Choice { label: "Display", cvar: "cl_automap_display", choices: AUTOMAP_DISPLAY },
    toggle("Rotate with Player", "cl_automap_rotate"),
    Item::Choice { label: "Reveal", cvar: "cl_automap_reveal", choices: AUTOMAP_REVEAL },
    slider("Zoom", "cl_automap_zoom", 0.5, 3.0, 0.25),
    slider("Opacity", "cl_automap_alpha", 0.2, 1.0, 0.1),
    toggle("Show Markers", "cl_automap_markers"),
];

pub(super) const GRENADE_ARC: &[Item] = &[
    Item::Choice { label: "Style", cvar: "assist_grenadearc_style", choices: &[("solid", 0.0), ("dashed", 1.0)] },
    Item::Choice { label: "Color", cvar: "assist_grenadearc_color", choices: XHAIR_COLOR },
    slider("Opacity", "assist_grenadearc_alpha", 0.1, 1.0, 0.1),
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
    Item::Choice { label: "Autosave Timer", cvar: "sv_autosave_interval", choices: MINUTES },
    toggle("Save on Level Entry", "sv_autosave_entry"),
    toggle("Save on Secret Found", "sv_autosave_secret"),
    toggle("Save on Picking Up Key", "sv_autosave_key"),
    toggle("Save on All Secrets Found", "sv_autosave_secrets"),
    toggle("Save on All Enemies Killed", "sv_autosave_monsters"),
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
    Item::Sub { label: "Gyro", page: Page::Gyro },
    slider("Rumble", "joy_rumble", 0.0, 1.0, 0.25),
    Item::Choice { label: "Button Glyphs", cvar: "joy_glyphs", choices: GLYPHS },
];

pub(super) const GYRO: &[Item] = &[
    toggle("Gyro Aiming", "gyro_enable"),
    Item::Choice { label: "Aim Style", cvar: "joy_flick", choices: AIM_STYLE },
    Item::Choice { label: "Gyro Active", cvar: "gyro_mode", choices: GYRO_MODE },
    slider("Yaw Sensitivity", "gyro_yawsensitivity", 0.0, 10.0, 0.25),
    slider("Pitch Sensitivity", "gyro_pitchsensitivity", 0.0, 10.0, 0.25),
    slider("Roll Sensitivity", "gyro_rollsensitivity", -10.0, 10.0, 0.25),
    slider("Noise Filter", "gyro_noise_thresh", 0.0, 5.0, 0.25),
    slider("Flick Time", "joy_flick_time", 0.05, 0.5, 0.025),
];

pub(super) const AIM_STYLE: &[(&str, f32)] = &[("2D aiming", 0.0), ("flick stick", 1.0)];
pub(super) const GYRO_MODE: &[(&str, f32)] = &[("always", 0.0), ("off while held", 1.0), ("only while held", 2.0)];

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
pub(super) const SSAO: &[(&str, f32)] = &[("off", 0.0), ("low", 1.0), ("medium", 2.0), ("high", 3.0)];
pub(super) const PORTAL_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("medium", 1.0), ("high", 2.0)];
pub(super) const WATER_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("high", 1.0)];
pub(super) const MOTIONBLUR_QUALITY: &[(&str, f32)] = &[("low", 0.0), ("high", 1.0)];
pub(super) const MODES: [&str; 3] = ["windowed", "fullscreen", "borderless"];

pub(super) const WATERWARP: &[(&str, f32)] = &[("off", 0.0), ("Classic", 1.0), ("GLQuake", 2.0)];
/// vkQuake numbering: 1 is GLQuake, 2 is Classic.
pub(super) const PARTICLES: &[(&str, f32)] = &[("off", 0.0), ("GLQuake", 1.0), ("Classic", 2.0)];

const GRAPHICS_ROWS: &[Item] = &[
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
    Item::Choice { label: "Underwater FX", cvar: "r_waterwarp", choices: WATERWARP },
    Item::Choice { label: "Particles", cvar: "r_particles", choices: PARTICLES },
    toggle("Dynamic Lights", "r_dynamic"),
    toggle("Colored Lighting", "r_coloredlight"),
    toggle("HDR Lightmaps", "r_hdr_lightmaps"),
    toggle("Directional Lighting", "r_deluxemapping"),
    slider("Light Specular", "r_deluxemapping_specular", 0.0, 1.0, 0.05),
    toggle("Enhanced Models", "r_enhancedmodels"),
    toggle("Fog", "r_fog"),
    slider("Sky Fog", "r_skyfog", 0.0, 1.0, 0.1),
    toggle("Interpolate Animation", "r_lerpmodels"),
    toggle("Interpolate Movement", "r_lerpmove"),
];

pub(super) const GRAPHICS: &[Item] = if cfg!(target_os = "android") {
    // Display Mode, Resolution and their Apply button are desktop controls.
    GRAPHICS_ROWS.split_at(3).1
} else {
    GRAPHICS_ROWS
};

pub(super) const COLORBLIND: &[(&str, f32)] =
    &[("off", 0.0), ("protanopia", 1.0), ("deuteranopia", 2.0), ("tritanopia", 3.0)];
pub(super) const XHAIR_SIZE: &[(&str, f32)] = &[("small", 0.75), ("normal", 1.0), ("large", 1.5), ("huge", 2.5)];
pub(super) const XHAIR_SHAPE: &[(&str, f32)] =
    &[("cross", 0.0), ("gap cross", 1.0), ("dot", 2.0), ("cross + dot", 3.0), ("T", 4.0), ("box", 5.0)];
pub(super) const HITMARKER_SHAPE: &[(&str, f32)] = &[
    ("diagonal streaks", 0.0), ("short diagonals", 1.0), ("diamond", 2.0),
    ("double diagonals", 3.0), ("double chevrons", 4.0), ("broken square", 5.0),
];
pub(super) const XHAIR_COLOR: &[(&str, f32)] =
    &[("grey", 6.0), ("white", 0.0), ("yellow", 1.0), ("green", 2.0), ("cyan", 3.0), ("magenta", 4.0), ("red", 5.0)];
pub(super) const CROSSHAIR: &[Item] = &[
    toggle("Crosshair", "crosshair"),
    Item::Choice { label: "Crosshair Shape", cvar: "crosshair_shape", choices: XHAIR_SHAPE },
    Item::Choice { label: "Crosshair Size", cvar: "crosshair_size", choices: XHAIR_SIZE },
    toggle("Crosshair Outline", "crosshair_outline"),
    Item::Choice { label: "Normal Color", cvar: "crosshair_color", choices: XHAIR_COLOR },
    toggle("Change Color on Target", "crosshair_ontarget"),
    Item::Choice { label: "On Target Color", cvar: "crosshair_targetcolor", choices: XHAIR_COLOR },
    toggle("Change Color on Kill", "crosshair_onkill"),
    Item::Choice { label: "On Kill Color", cvar: "crosshair_killcolor", choices: XHAIR_COLOR },
    toggle("Hit Markers", "hud_hitmarkers"),
    Item::Choice { label: "Hitmarker Type", cvar: "hitmarker_shape", choices: HITMARKER_SHAPE },
    Item::Choice { label: "Hitmarker Size", cvar: "hitmarker_size", choices: XHAIR_SIZE },
    toggle("Grenade Arc", "assist_grenadearc"),
    Item::Sub { label: "Grenade Arc Settings", page: Page::GrenadeArc },
];
pub(super) const GAME_SPEED: &[(&str, f32)] = &[("100%", 1.0), ("90%", 0.9), ("75%", 0.75), ("60%", 0.6), ("50%", 0.5)];

pub(super) const ACCESSIBILITY: &[Item] = &[
    toggle("Reduce Motion", "a11y_reducemotion"),
    slider("Screen Flashes", "v_flashscale", 0.0, 1.0, 0.1),
    Item::Choice { label: "Color Filter", cvar: "r_colorblind", choices: COLORBLIND },
    toggle("Sound Captions", "cl_captions"),
    toggle("Toggle Attack", "in_toggleattack"),
    toggle("Toggle Run", "in_togglerun"),
    Item::Choice { label: "Game Speed", cvar: "a11y_gamespeed", choices: GAME_SPEED },
    toggle("Auto-aim", "assist_autoaim"),
    Item::Sub { label: "Auto-aim Settings", page: Page::AutoAim },
];

pub(super) const HUD: &[Item] = &[
    Item::HudPreset,
    Item::Run { label: "Edit HUD", run: Run::EditHud },
    Item::Sub { label: "Crosshair", page: Page::Crosshair },
    toggle("Directional Damage", "hud_damageindicator"),
    Item::Choice { label: "Weapon Wheel Slowdown", cvar: "cl_wheel_timescale", choices: WHEEL_TIME },
    toggle("Modern Console", "con_modern"),
];

pub(super) const AUDIO: &[Item] = &[
    slider("Sound Volume", "volume", 0.0, 1.0, 0.1),
    slider("Music Volume", "bgmvolume", 0.0, 1.0, 0.1),
    Item::Sub { label: "Volume Mix", page: Page::AudioMix },
    Item::Soundtrack,
    slider("Music Crossfade", "s_music_crossfade", 0.0, 3.0, 0.25),
    toggle("Main Menu Music", "menu_music"),
    toggle("Underwater Muffling", "snd_waterfx"),
    toggle("Environmental Reverb", "s_reverb"),
    slider("Reverb Amount", "s_reverb_amount", 0.25, 2.0, 0.25),
    toggle("3D Binaural Audio", "s_hrtf"),
    toggle("Sound Occlusion", "s_occlusion"),
    toggle("Footsteps", "s_footsteps"),
    slider("Footstep Volume", "s_footsteps_volume", 0.1, 1.5, 0.1),
    toggle("Hit & Kill Sounds", "s_hitfeedback"),
    Item::Sub { label: "Hit Sound", page: Page::HitSound },
    Item::Sub { label: "Kill Sound", page: Page::KillSound },
];

pub(super) const AUDIO_MIX: &[Item] = &[
    slider("Ambient", "s_vol_ambient", 0.0, 1.0, 0.1),
    slider("Weapons & Player", "s_vol_weapons", 0.0, 1.0, 0.1),
    slider("Monsters", "s_vol_monsters", 0.0, 1.0, 0.1),
    slider("World & Pickups", "s_vol_world", 0.0, 1.0, 0.1),
];

/// Independent of the debris toggle and particle scripts.
pub(super) const BLOOD: &[Item] = &[
    Item::Choice { label: "Blood Quality", cvar: "r_blood_quality", choices: DEBRIS_QUALITY },
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
    Item::Choice { label: "Input Scale", cvar: "r_shader_scale", choices: SHADER_SCALE },
    Item::ResetShaderParams,
];
pub(super) const SHADER_SCOPE: &[(&str, f32)] = &[("3D view", 0.0), ("whole screen", 1.0)];
pub(super) const SHADER_SCALE: &[(&str, f32)] = &[("fit output", 0.0), ("3D resolution", 1.0)];

/// Reached from the Add-ons pages: night-vision / thermal goggle look.
pub(super) const GOGGLES: &[Item] = &[
    Item::Choice { label: "Goggle Glow Quality", cvar: "addon_goggles_quality", choices: DEBRIS_QUALITY },
    slider("Goggle Intensity", "addon_goggles_intensity", 0.25, 3.0, 0.25),
    slider("Goggle Noise", "addon_goggles_noise", 0.0, 2.0, 0.25),
];

/// Stickybomb explosion balance; action mapping lives in the browser.
pub(super) const STICKY: &[Item] = &[slider("Explosion Damage", "addon_sticky_damage", 25.0, 300.0, 25.0)];
