//! The scenario catalogue: a pinned "vanilla" baseline, every graphical
//! enhancement at its most expensive setting (each alone, then all at
//! once), and every add-on in active use (each alone, then all at once).
//!
//! Each scenario is the vanilla cvars plus its own overrides, and a script
//! of timed console commands that run after the warm-up (times in seconds
//! of simulated time from the start of measurement).

/// What a scenario stresses; used to group the report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Vanilla,
    Enhancement,
    Addon,
    Ultimate,
}

impl Group {
    pub fn name(self) -> &'static str {
        match self {
            Group::Vanilla => "vanilla",
            Group::Enhancement => "enhancement",
            Group::Addon => "addon",
            Group::Ultimate => "ultimate",
        }
    }
}

pub struct Scenario {
    pub name: &'static str,
    pub group: Group,
    pub what: &'static str,
    /// cvar overrides on top of [`VANILLA`].
    pub cvars: Vec<(&'static str, &'static str)>,
    /// Runs only on this (game, map), instead of every `-maps` entry.
    pub map: Option<(&'static str, &'static str)>,
    /// (seconds after warm-up, command), on top of [`combat`]'s script.
    pub script: Vec<(f32, String)>,
    /// Replace the default weapon rotation (toys hold their own fire).
    pub own_fire: bool,
}

/// Classic look, every enhancement off. Pinned so the user's config.cfg
/// never leaks into a measurement.
pub const VANILLA: &[(&str, &str)] = &[
    ("r_msaa", "1"), ("vid_fsaamode", "0"), ("r_renderscale", "1"), ("r_texfilter", "0"),
    ("r_anisotropy", "1"), ("r_softemu", "0"), ("r_dither", "0"), ("gamma", "1"), ("contrast", "1"),
    ("r_dynamic", "1"), ("r_coloredlight", "0"), ("r_fog", "1"), ("r_skyfog", "0.5"),
    ("r_lerpmodels", "1"), ("r_lerpmove", "1"),
    ("r_ssao", "0"), ("r_motionblur", "0"), ("r_speedblur", "0"), ("a11y_reducemotion", "0"),
    ("r_enhancedwater", "0"), ("r_teleportwindows", "0"),
    ("r_impact_fx", "0"), ("r_impact_bullets", "0"), ("r_impact_nails", "0"),
    ("r_shotgun_tracers", "0"), ("r_impact_scorches", "0"),
    ("r_blood", "0"), ("r_debris", "0"), ("r_shader_preset", ""), ("r_colorblind", "0"),
    ("addons_enable", "0"),
];

const WATER: &[(&str, &str)] = &[
    ("r_enhancedwater", "1"), ("r_watertranslucency", "1"), ("r_wateralpha", "0.3"),
    ("r_waterripples", "1"), ("r_waterripples_amount", "2"), ("r_watersplash", "1"),
    ("r_watercaustics", "1"), ("r_watercaustics_amount", "2"),
];
const IMPACTS: &[(&str, &str)] = &[
    ("r_impact_fx", "4"), ("r_impact_bullets", "1"), ("r_impact_nails", "1"),
    ("r_shotgun_tracers", "1"), ("r_impact_scorches", "1"),
    ("r_impact_lifetime", "600"), ("r_impact_limit", "2048"),
];
const BLOOD: &[(&str, &str)] = &[
    ("r_blood", "1"), ("r_blood_amount", "12"), ("r_blood_size", "3"), ("r_blood_lifetime", "600"),
    ("r_blood_maxmarks", "2048"), ("r_blood_wounds", "1"), ("r_blood_gibs", "1"),
];
const DEBRIS: &[(&str, &str)] = &[
    ("r_debris", "1"), ("r_debris_ragdolls", "1"), ("r_debris_ragdollstart", "1"),
    ("r_debris_maxragdolls", "64"), ("r_debris_gibs", "1"), ("r_debris_shrapnel", "1"),
    ("r_debris_movers", "1"), ("r_debris_force", "6"), ("r_debris_killlift", "1"),
];
const MSAA: &[(&str, &str)] = &[("r_msaa", "8")];
const SSAA: &[(&str, &str)] = &[("r_msaa", "8"), ("vid_fsaamode", "1")];
const SCALE: &[(&str, &str)] = &[("r_renderscale", "2")];
const ANISO: &[(&str, &str)] = &[("r_texfilter", "1"), ("r_anisotropy", "16")];
const SOFTEMU: &[(&str, &str)] = &[("r_softemu", "3"), ("r_dither", "1")];
const COLOR: &[(&str, &str)] = &[("r_coloredlight", "1"), ("gamma", "0.8"), ("contrast", "1.4")];
const SSAO: &[(&str, &str)] = &[("r_ssao", "2")];
const BLUR: &[(&str, &str)] = &[("r_motionblur", "1"), ("r_motionblur_amount", "2"), ("r_speedblur", "1")];
const TELE: &[(&str, &str)] = &[("r_teleportwindows", "1")];
const SHADER: &[(&str, &str)] = &[("r_shader_preset", "crt/crt-guest-advanced-hd"), ("r_shader_scope", "1")];
const COLORBLIND: &[(&str, &str)] = &[("r_colorblind", "2")];

/// (name, description, cvars) of every enhancement stress.
type Cvars = &'static [(&'static str, &'static str)];
const ENHANCEMENTS: &[(&str, &str, Cvars)] = &[
    ("msaa8", "8x MSAA", MSAA),
    ("supersample", "8x MSAA + supersampling", SSAA),
    ("renderscale200", "200% render scale", SCALE),
    ("aniso16", "linear filtering, 16x anisotropic", ANISO),
    ("softemu", "8-bit colour emulation (banded) + dithering", SOFTEMU),
    ("coloredlight", "coloured lighting, gamma/contrast", COLOR),
    ("ssao", "ambient occlusion: high", SSAO),
    ("motionblur", "motion + speed blur at 2.0", BLUR),
    ("water", "enhanced water: translucency, ripples, splashes, caustics at max", WATER),
    ("teleportwindows", "see-through teleporters", TELE),
    ("impacts", "impact fx spectacle, decals, tracers, scorches, 2048 x 600 s", IMPACTS),
    ("blood", "blood splatter max: x12, size 3, 2048 marks, wounds, gib painting", BLOOD),
    ("debris", "ragdolls at death, 64 ragdolls, gibs, shrapnel, movers, force 6", DEBRIS),
    ("screenshader", "slang CRT preset (crt-guest-advanced-hd), whole screen", SHADER),
    ("colorblind", "colour-blind filter", COLORBLIND),
];

pub(crate) const ADDON_ON: &[(&str, &str)] = &[
    ("addons_enable", "1"), ("addon_projectile_shotguns", "1"), ("addon_shotgun_speed", "10000"),
    ("addon_dodge", "1"), ("addon_bullet_time", "1"), ("addon_bullet_time_toggle", "0"),
    ("addon_bullet_time_duration", "5"), ("addon_portalgun", "1"), ("addon_sticky", "1"),
    ("addon_boomerang", "1"), ("addon_railgun", "1"), ("addon_hookshot", "1"), ("addon_gel", "1"),
    ("addon_washer", "1"), ("addon_goggles", "1"),
    ("addon_portal_view", "1"), ("addon_portal_max", "4"), ("addon_portal_depth", "6"),
    ("addon_portal_dist", "8192"),
];

/// Repeats `on`/`off` command pairs every `period` seconds over `secs`.
pub(crate) fn pulse(secs: f32, period: f32, hold: f32, on: &str, off: &str) -> Vec<(f32, String)> {
    let mut v = Vec::new();
    let mut t = 0.2;
    while t < secs {
        v.push((t, on.to_string()));
        v.push((t + hold, off.to_string()));
        t += period;
    }
    v
}

/// Holds a toy's primary fire for the whole run.
fn hold_toy(toy: usize) -> Vec<(f32, String)> {
    vec![(0.0, format!("toy {toy}")), (0.1, "+attack".into())]
}

/// Alternates primary and secondary fire of a toy.
fn alt_toy(toy: usize, secs: f32, period: f32) -> Vec<(f32, String)> {
    let mut v = vec![(0.0, format!("toy {toy}"))];
    v.extend(pulse(secs, period, 0.15, "+attack", "-attack"));
    v.extend(pulse(secs, period, 0.15, "+attack2", "-attack2").into_iter().map(|(t, c)| (t + period / 2.0, c)));
    v
}

/// The default script: god mode, all weapons, slow spin, constant fire
/// rotating rockets → super nailgun → super shotgun so explosions, dynamic
/// lights, gibs, nail and bullet impacts all happen.
pub fn combat(secs: f32, own_fire: bool) -> Vec<(f32, String)> {
    let mut v = vec![
        (0.0, "god".to_string()), (0.0, "impulse 9".into()), (0.0, "cl_yawspeed 40".into()),
        (0.0, "+right".into()),
    ];
    if !own_fire {
        v.push((0.0, "impulse 7".into()));
        v.push((0.1, "+attack".into()));
        v.push((secs / 3.0, "impulse 5".into()));
        v.push((2.0 * secs / 3.0, "impulse 3".into()));
    }
    v
}

pub fn all(secs: f32) -> Vec<Scenario> {
    let mut out = vec![Scenario {
        name: "vanilla", group: Group::Vanilla, what: "classic renderer, every enhancement and add-on off",
        cvars: vec![], map: None, script: vec![], own_fire: false,
    }];
    let mut every = Vec::new();
    for (name, what, cv) in ENHANCEMENTS {
        out.push(Scenario {
            name, group: Group::Enhancement, what, cvars: cv.to_vec(), map: None, script: vec![], own_fire: false,
        });
        // supersample implies msaa8; colorblind and softemu stay out of the
        // combined run (they replace rather than add work).
        if !matches!(*name, "msaa8" | "colorblind" | "softemu") {
            every.extend_from_slice(cv);
        }
    }
    out.push(Scenario {
        name: "all-enhancements", group: Group::Enhancement,
        what: "every enhancement above at once (except colour-blind and 8-bit emulation)",
        cvars: every, map: None, script: vec![], own_fire: false,
    });

    let addon = |name, what, script: Vec<(f32, String)>, own_fire| Scenario {
        name, group: Group::Addon, what, cvars: ADDON_ON.to_vec(), map: None, script, own_fire,
    };
    out.push(addon("addons-idle", "add-ons enabled, stock weapons (projectile shotguns)", vec![], false));
    out.push(addon("portalgun", "portal gun: place both portals repeatedly, 4 live views, recursion 6", alt_toy(0, secs, 1.5), true));
    out.push(addon("boomerang", "boomerang thrown continuously", hold_toy(1), true));
    out.push(addon("sticky", "stickybombs: launch and detonate", alt_toy(2, secs, 1.0), true));
    out.push(addon("hookshot", "hookshot fired continuously", hold_toy(3), true));
    out.push(addon("railgun", "railgun fired continuously", hold_toy(4), true));
    out.push(addon("gel", "gel sprayer: both gels", alt_toy(5, secs, 1.0), true));
    out.push(addon("washer", "powerwasher held", hold_toy(6), true));
    out.push(addon("goggles-nv", "night-vision goggles on", vec![(0.0, "goggles nv".into())], false));
    out.push(addon("goggles-thermal", "thermal goggles on", vec![(0.0, "goggles thermal".into())], false));
    out.push(addon("bullettime", "bullet time pulsed (5 s on, 1 s off)", pulse(secs, 6.0, 5.0, "+bullettime", "-bullettime"), false));
    out.push(addon("dodge", "dodge every 0.5 s while strafing", {
        let mut v = vec![(0.0, "+moveleft".to_string())];
        v.extend(pulse(secs, 0.5, 0.1, "+dodge", "-dodge"));
        v
    }, false));
    // Everything: cycle the toys, goggles on, bullet time pulsed.
    let mut all = vec![(0.0, "goggles nv".to_string())];
    all.extend(pulse(secs, 6.0, 5.0, "+bullettime", "-bullettime"));
    all.extend(pulse(secs, 0.6, 0.3, "+attack", "-attack"));
    all.extend(pulse(secs, 1.8, 0.2, "+attack2", "-attack2"));
    let mut t = 0.0;
    let mut k = 0;
    while t < secs {
        all.push((t, format!("toy {}", k % 7)));
        t += 2.0;
        k += 1;
    }
    out.push(addon("all-addons", "every toy in rotation, goggles, bullet time", all, true));

    let every = out.iter().find(|s| s.name == "all-enhancements").map(|s| s.cvars.clone()).unwrap_or_default();
    out.extend(crate::ultimate::scenarios(secs, every));
    out
}
