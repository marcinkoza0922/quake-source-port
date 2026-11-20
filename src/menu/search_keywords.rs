//! Extra search words for the options search (`search.rs`): other names
//! players use for a setting, and the effects it controls. Only these words
//! and the visible names are searched, never tooltips, so common words stay
//! out of the results. English only; translated labels match as shown.

use super::*;

/// Words for a row, by its cvar (value rows), bound command (Controls) or
/// target page (sub-page rows).
pub(super) fn keywords(item: Item) -> &'static str {
    match item {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } | Item::Text { cvar, .. } => cvar_keywords(cvar),
        Item::Sub { page, .. } => page_keywords(page),
        Item::Bind { cmd, .. } => bind_keywords(cmd),
        Item::DisplayMode => "fullscreen windowed borderless window screen monitor",
        Item::Resolution => "res screen size pixels display monitor",
        Item::Apply => "display fullscreen resolution",
        Item::Language => "translation locale",
        Item::UserDataDir => "save folder path storage location config demos mods quaddicted",
        Item::Soundtrack => "music tracks cd nin quake64 playlist",
        Item::HudPreset => "interface layout status bar sbar modern classic custom saved preset",
        Item::ShaderPreset => "crt scanlines retroarch slang filter retro ntsc",
        Item::ResetBinds => "keys bindings defaults",
        Item::ResetShaderParams => "crt defaults",
        Item::Color { .. } => "colour team",
        // the HUD editor holds the elements that had their own rows
        Item::Run { run: Run::EditHud, .. } => {
            "layout interface move position health armor ammo weapons keys clock timer personal best kills secrets speedometer speed velocity ups fps \
             framerate performance cpu gpu memory messages captions subtitles text font strafe bunnyhop cgaz jump timing input sequence"
        }
        _ => "",
    }
}

/// Words for a page: used by its sub-page rows and, for the top-level
/// pages, by the page itself.
pub(super) fn page_keywords(p: Page) -> &'static str {
    match p {
        Page::Gameplay => "game general",
        Page::Controls => "keys keyboard bindings binds remap buttons",
        Page::Input => "keyboard mouse controller gamepad touch",
        Page::Graphics => "video display screen visuals",
        Page::Enhancements => "effects visuals modern graphics",
        Page::Audio => "sound volume",
        Page::Accessibility => "a11y assist motion sickness",
        Page::Hud => "interface status bar sbar overlay",
        Page::Demos => "replay recording",
        Page::Mouse => "sensitivity sens keyboard",
        Page::Controller => "gamepad joystick pad xbox playstation",
        Page::Gyro => "motion flick stick controller",
        Page::Touch => "touchscreen mobile phone tablet",
        Page::Autosave => "checkpoint",
        Page::WeaponPriority => "weapon switch autoswitch priority order rocket lightning",
        Page::Automap => "automap minimap map wireframe 3d radar compass",
        Page::AutoAim => "aim assist lock-on lockon",
        Page::GrenadeArc => "trajectory aim",
        Page::Crosshair => "reticle aim hitmarker",
        Page::Water => "liquid lava slime reflections",
        Page::Bloom => "light lighting glow bright",
        Page::RtLights => "light lighting shadows dlight dynamic",
        Page::Impacts => "particles effects decals",
        Page::Debris => "gibs physics",
        Page::Blood => "gore decals gibs",
        Page::ScreenShader => "crt scanlines retroarch slang filter retro post-processing",
        Page::HitSound | Page::KillSound => "hitsound killsound feedback",
        Page::Video => "capture record export ffmpeg",
        _ => "",
    }
}

fn bind_keywords(cmd: &str) -> &'static str {
    match cmd {
        "+attack" => "fire shoot",
        "+weaponwheel" => "weapons select",
        "impulse 10" | "impulse 12" => "weapon switch cycle",
        "+forward" | "+back" | "+moveleft" | "+moveright" => "walk movement wasd",
        "+jump" => "swim",
        "+speed" => "sprint walk",
        "+lookup" | "+lookdown" | "centerview" => "camera aim",
        "+showscores" => "scores stats",
        "automap" => "map minimap wireframe 3d",
        "restart" => "reset level retry speedrun",
        "screenshot" => "capture picture photo snap",
        "record_toggle" => "demo recording replay",
        "+lockon" => "aim assist autoaim",
        _ => "",
    }
}

fn cvar_keywords(cvar: &str) -> &'static str {
    match cvar {
        "fov" => "fov zoom",
        "cl_camera" => "third person 3rd thirdperson perspective view",
        "r_drawviewmodel" => "gun weapon hands",
        "cl_vwep" => "weapon models",
        "cl_bob" => "head bob motion sickness",
        "v_gunkick" => "recoil",
        "cl_alwaysrun" => "sprint walk",
        "cl_autoswitch" => "weapon switch pickup priority rocket thunderbolt",
        "cl_automap" | "cl_automap_view" | "cl_automap_display" | "cl_automap_rotate"
        | "cl_automap_reveal" | "cl_automap_zoom" | "cl_automap_alpha" | "cl_automap_markers" => "map minimap wireframe 3d radar",
        "cl_deathaction" => "die respawn",
        "sv_autosave" => "checkpoint",
        "sv_autosave_key" => "checkpoint key silver gold",
        "con_translate" => "language",
        "sensitivity" => "sensitivity sens",
        "m_invert" | "joy_invert" | "touch_invert" => "inverted flip y axis",
        "joy_sensitivity_yaw" | "joy_sensitivity_pitch" => "sensitivity sens gamepad controller",
        "aim_assist" | "aim_slowdown" | "aim_rotation" => "auto-aim autoaim magnetism",
        "joy_rumble" => "vibration haptics force feedback",
        "joy_glyphs" => "icons prompts xbox playstation",
        "gyro_enable" | "gyro_mode" => "motion",
        "gyro_yawsensitivity" | "gyro_pitchsensitivity" | "gyro_rollsensitivity" => "sensitivity sens",
        "joy_flick" | "joy_flick_time" => "flick stick",
        "vid_vsync" => "vertical sync tearing",
        "host_maxfps" => "framerate frame rate limit cap",
        "r_msaa" => "aa msaa antialiasing jaggies smoothing",
        "vid_fsaamode" => "ssaa antialiasing aa downsampling",
        "r_renderscale" => "resolution scaling performance",
        "r_texfilter" => "pixelated nearest linear smooth textures",
        "r_anisotropy" => "anisotropic af texture filtering",
        "r_softemu" => "palette software retro",
        "r_dither" => "banding noise",
        "gamma" => "gamma brightness dark",
        "contrast" => "brightness",
        "r_waterwarp" => "water distortion",
        "r_particles" => "effects",
        "r_dynamic" => "light lighting dlight",
        "r_coloredlight" => "light lighting lit colour",
        "r_enhancedmodels" => "remaster models",
        "r_fog" | "r_skyfog" => "haze mist",
        "r_lerpmodels" | "r_lerpmove" => "lerp smooth interpolation",
        "crosshair" | "crosshair_shape" | "crosshair_size" | "crosshair_outline" => "reticle",
        "crosshair_color" | "crosshair_targetcolor" | "crosshair_killcolor" => "reticle colour",
        "crosshair_ontarget" | "crosshair_onkill" => "reticle colour",
        "hud_hitmarkers" | "hitmarker_shape" | "hitmarker_size" => "hit feedback",
        "assist_grenadearc" => "trajectory aim",
        "assist_grenadearc_color" => "colour",
        "a11y_reducemotion" => "motion sickness bob shake",
        "v_flashscale" => "flash epilepsy photosensitivity",
        "r_colorblind" => "colorblind colourblind protanopia deuteranopia tritanopia",
        "in_toggleattack" | "in_togglerun" => "hold",
        "a11y_gamespeed" => "slow motion timescale difficulty",
        "assist_autoaim" => "aim assist lock-on lockon",
        "hud_preset" => "interface layout status bar sbar modern classic custom",
        "hud_damageindicator" => "damage direction",
        "cl_wheel_timescale" => "slow motion",
        "con_modern" => "terminal",
        "cl_captions" => "subtitles closed captions deaf",
        "volume" => "sfx effects audio loudness",
        "bgmvolume" => "soundtrack cd audio loudness",
        "menu_music" => "soundtrack",
        "snd_waterfx" => "underwater muffle lowpass filter liquid",
        "s_reverb" | "s_reverb_amount" => "echo room cavern hall acoustics",
        "s_hrtf" => "binaural 3d headphones spatial audio",
        "s_occlusion" => "walls doors muffle obstruction",
        "s_footsteps" | "s_footsteps_volume" => "walk run landing surface steps",
        "s_music_crossfade" => "fade transition soundtrack",
        "s_vol_ambient" | "s_vol_weapons" | "s_vol_monsters" | "s_vol_world" => "mixer volume mix balance",
        "s_hitfeedback" => "hitsound killsound feedback",
        "r_ssao" => "ssao shadows",
        "r_motionblur" | "r_motionblur_quality" | "r_motionblur_amount" | "r_speedblur" => "blur",
        "r_bloom" => "light lighting glow bright",
        "r_bloom_quality" | "r_bloom_intensity" | "r_bloom_size" | "r_bloom_bright" => "light glow",
        "r_shadow_realtime_dlight" => "light lighting shadows dlight",
        "r_shadow_realtime_dlight_shadows" | "r_shadow_quality" => "light lighting",
        "r_enhancedfx" | "r_enhancedparticles" | "r_particle_quality" | "r_impact_fx" => "particles effects",
        "r_impact_bullets" | "r_impact_nails" | "r_impact_scorches" => "decals bullet holes",
        "r_enhancedwater" | "r_water_quality" => "liquid lava slime",
        "r_watertranslucency" | "r_wateralpha" => "transparent transparency alpha see-through",
        "r_water_reflections" | "r_water_reflections_amount" => "mirror ray tracing rtx",
        "r_teleportwindows" | "r_portal_quality" => "portals",
        "r_debris" | "r_debris_quality" | "r_debris_gibs" | "r_debris_shrapnel" => "gibs physics",
        "r_blood" | "r_blood_quality" | "r_blood_amount" | "r_blood_size" | "r_blood_wounds" | "r_blood_gibs" => "gore decals",
        "cl_autodemo" | "cl_autodemo_slots" => "replay recording",
        "cl_autodemo_pb" => "pb personal best no-hit speedrun demo",
        "cl_demo_ghost" => "pb personal best no-hit speedrun race replay ghost",
        "capture_fps" | "capture_size" | "capture_codec" | "capture_quality" => "capture video recording",
        "capture_gpu" => "hardware nvenc",
        "capture_audio" => "sound",
        _ => "",
    }
}
