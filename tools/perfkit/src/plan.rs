//! The lean performance plan's scene list (`docs/perf/PLAN.md`, step P0).
//!
//! Every scene the plan measures is one entry in [`SCENES`]: where it plays
//! (a `perfkit record` demo or a scripted live game), which cvars turn its
//! feature on at the default and at the top tier, and what it stresses.
//! The "off" run of a scene is the same scene with only the [`baseline`]
//! pins, so a feature's cost is on minus off. Adding a feature to the plan
//! means adding one entry here; the tests check that every cvar a scene sets
//! exists in the engine and is pinned by the baseline, so nothing can be on
//! or off by accident (ADDITIONS.md found `r_enhancedfx` unpinned).

use crate::live::{at, every, Cmd, Live};
use crate::settings::{self, Profile};

pub type Cvars = &'static [(&'static str, &'static str)];

/// Which part of the plan a scene belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Vanilla,
    Enhancement,
    Audio,
    Addon,
    All,
}

impl Group {
    pub fn name(self) -> &'static str {
        match self {
            Group::Vanilla => "vanilla",
            Group::Enhancement => "enhancement",
            Group::Audio => "audio",
            Group::Addon => "addon",
            Group::All => "all",
        }
    }
}

/// Where a scene plays.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A timedemo of a `perfkit record` demo ([`crate::scenes::SCENES`]).
    Demo(&'static str),
    /// A scripted live game ([`crate::live`]).
    Live(Live),
}

#[derive(Clone, Copy, Debug)]
pub struct PlanScene {
    pub name: &'static str,
    pub group: Group,
    pub kind: Kind,
    /// The feature on at its default tier (empty for vanilla scenes).
    pub on: Cvars,
    /// Extra cvars on top of `on` for the top tier (empty: no separate top run).
    pub top: Cvars,
    pub stresses: &'static str,
    /// Why the scene can't run yet, if it can't (besides being live).
    pub blocked: Option<&'static str>,
}

impl PlanScene {
    /// The demo it plays, if it is a demo scene.
    pub fn demo(&self) -> Option<&'static crate::scenes::Scene> {
        match self.kind {
            Kind::Demo(d) => crate::scenes::find(d),
            Kind::Live(_) => None,
        }
    }

    pub fn game(&self) -> &'static str {
        match self.kind {
            Kind::Demo(d) => crate::scenes::find(d).map_or("id1", |s| s.game),
            Kind::Live(l) => l.game,
        }
    }

    /// The settings profile its baseline uses: FTE particles for FTE demos.
    pub fn profile(&self) -> Profile {
        match self.demo() {
            Some(s) if s.fte => Profile::UltraFte,
            _ => Profile::Ultra,
        }
    }

    /// Runnable by `perfkit run` today.
    pub fn runnable(&self) -> bool {
        self.blocked.is_none()
    }
}

/// Everything the plan's features touch, off or at the default tier, on
/// top of Part 1's "vanilla ultra" pins ([`settings::ULTRA`]).
pub const PLAN_OFF: Cvars = &[
    ("r_bloom", "0"), ("r_bloom_quality", "1"),
    ("r_shadow_realtime_dlight", "0"), ("r_shadow_realtime_dlight_shadows", "1"), ("r_shadow_quality", "1"),
    ("r_ssao", "0"), ("r_motionblur", "0"), ("r_motionblur_quality", "1"), ("r_speedblur", "0"),
    ("r_enhancedwater", "0"), ("r_water_quality", "1"), ("r_water_reflections", "0"),
    ("r_watertranslucency", "0"), ("r_wateralpha", "1"), ("r_waterripples", "0"), ("r_watersplash", "0"),
    ("r_watercaustics", "0"), ("r_water_refraction", "0"), ("r_water_droplets", "0"),
    ("r_enhancedfx", "0"), ("r_enhancedparticles", "0"),
    ("r_impact_fx", "0"), ("r_impact_bullets", "0"), ("r_impact_nails", "0"), ("r_impact_scorches", "0"),
    ("r_shotgun_tracers", "0"), ("r_blood", "0"), ("r_blood_quality", "1"),
    ("r_debris", "0"), ("r_debris_quality", "1"),
    ("r_teleportwindows", "0"), ("r_portal_quality", "1"),
    ("r_hdr_lightmaps", "0"), ("r_deluxemapping", "0"), ("r_skywind", "0"),
    ("s_reverb", "0"), ("s_hrtf", "0"), ("s_occlusion", "0"), ("snd_waterfx", "0"),
    ("custom_enable", "0"), ("addon_hitstop", "0"),
    ("addon_portalgun", "0"), ("addon_portal_view", "1"), ("addon_portal_max", "4"), ("addon_portal_depth", "3"),
    ("addon_gel", "0"), ("addon_gel_quality", "1"), ("addon_washer", "0"), ("addon_washer_quality", "1"),
    ("addon_cable", "0"), ("addon_goggles", "0"), ("addon_goggles_quality", "1"),
];

/// The "off" pins of a profile for our engine, in order: Part 1's pins,
/// then [`PLAN_OFF`] (a later value wins when the cfg runs).
pub fn baseline(p: Profile) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<_> = settings::rows(p).flat_map(|r| r.ours.iter().copied()).collect();
    v.extend_from_slice(PLAN_OFF);
    v
}

const WATER_ON: Cvars = &[
    ("r_enhancedwater", "1"), ("r_watertranslucency", "1"), ("r_wateralpha", "0.5"),
    ("r_waterripples", "1"), ("r_watersplash", "1"), ("r_watercaustics", "1"),
];
const FX_BLOOD: Cvars = &[("r_enhancedfx", "1"), ("r_blood", "1")];
const FX_IMPACTS: Cvars = &[
    ("r_enhancedfx", "1"), ("r_impact_fx", "1"), ("r_impact_bullets", "1"), ("r_impact_nails", "1"), ("r_impact_scorches", "1"),
];
const FX_PARTICLES: Cvars = &[("r_enhancedfx", "1"), ("r_enhancedparticles", "1")];

const fn demo(name: &'static str, demo: &'static str, group: Group, on: Cvars, top: Cvars, stresses: &'static str) -> PlanScene {
    PlanScene { name, group, kind: Kind::Demo(demo), on, top, stresses, blocked: None }
}

const fn live(name: &'static str, l: Live, group: Group, on: Cvars, top: Cvars, stresses: &'static str) -> PlanScene {
    PlanScene { name, group, kind: Kind::Live(l), on, top, stresses, blocked: None }
}

/// A live scene whose camera and setup aren't chosen yet.
const fn todo(name: &'static str, game: &'static str, map: &'static str, group: Group, on: Cvars, stresses: &'static str, why: &'static str) -> PlanScene {
    let l = Live::cam(game, map, [0.0; 5]);
    PlanScene { name, group, kind: Kind::Live(l), on, top: &[], stresses, blocked: Some(why) }
}

/// e2m2's big pool (`tools/qdata.py liquids e2m2`): under the surface, and
/// above it looking along the water.
const E2M2_UNDER: [f32; 5] = [796.0, 900.0, -110.0, 0.0, 90.0];
const E2M2_ABOVE: [f32; 5] = [796.0, 900.0, -40.0, 10.0, 90.0];
const E2M1_POOL: [f32; 5] = [576.0, 280.0, -90.0, 20.0, 90.0];
/// The audio fight: e1m3's spawn at skill 3, rockets fired nonstop while
/// turning, so explosions, monster sounds and echoes keep coming. Real time.
const FIGHT: &[Cmd] = &[
    at(2.5, "impulse 9"), at(2.7, "impulse 7"), at(3.0, "cl_yawspeed 40"), at(3.0, "+right"), at(3.2, "+attack"),
    every(8.0, 5.0, "impulse 9"),
];
const E1M3_FIGHT: Live = Live { fight: true, realtime: true, ..Live::player("id1", "e1m3", [-736.0, -1592.0, 88.0, 0.0, 270.0], FIGHT) };
/// Under e3m5's deep pool (`qdata.py liquids e3m5`), firing nails.
const UNDERWATER_FIGHT: &[Cmd] = &[at(2.5, "impulse 9"), at(2.7, "impulse 5"), at(3.0, "cl_yawspeed 40"), at(3.0, "+right"), at(3.2, "+attack")];
const E3M5_UNDER: Live =
    Live { fight: true, realtime: true, ..Live::player("id1", "e3m5", [440.0, -1938.0, -450.0, 0.0, 90.0], UNDERWATER_FIGHT) };

/// The e1m1 start corridor, facing down it (portal-gun skill).
const E1M1_CORRIDOR: [f32; 5] = [480.0, 120.0, 40.0, 10.0, 90.0];
/// A facing portal pair down the corridor: a recursion tunnel.
const PORTALS: &[Cmd] = &[at(3.0, "portal_set 0 480 80 48 0 1 0 0 0 1"), at(3.0, "portal_set 1 480 330 48 0 -1 0 0 0 1")];
/// Both gels in turn while turning slowly, so coats spread over new
/// surfaces and rebuild all through the measurement.
const GEL: &[Cmd] = &[
    at(3.0, "toy 5"), at(3.0, "cl_yawspeed 15"), at(3.0, "+right"),
    every(3.2, 2.0, "+attack"), every(4.0, 2.0, "-attack"), every(4.2, 2.0, "+attack2"), every(5.0, 2.0, "-attack2"),
];
const WASHER: &[Cmd] = &[at(3.0, "toy 6"), at(3.0, "cl_yawspeed 15"), at(3.0, "+right"), at(3.2, "+attack")];
/// Socket highlight, plug attempts and the bolt when plugged (as qprof).
const CABLE: &[Cmd] = &[at(3.0, "toy 7"), every(3.2, 1.5, "+attack"), every(3.35, 1.5, "-attack"), every(3.95, 1.5, "+attack2"), every(4.1, 1.5, "-attack2")];
/// ad_sepulcher's spawn.
const SEPULCHER_SPAWN: [f32; 5] = [-2560.0, 2464.0, -8.0, 0.0, 270.0];
const GOGGLES_NV: &[Cmd] = &[at(3.0, "goggles nv")];

/// Every enhancement at its default tier, the add-ons and the audio
/// features: the everything-on scene's "on" (its "off" is the same 4-player
/// fight with all of it off).
const EVERYTHING: Cvars = &[
    ("r_bloom", "1"), ("r_shadow_realtime_dlight", "1"), ("r_ssao", "1"), ("r_motionblur", "1"), ("r_speedblur", "1"),
    ("r_enhancedwater", "1"), ("r_watertranslucency", "1"), ("r_wateralpha", "0.5"), ("r_waterripples", "1"),
    ("r_watersplash", "1"), ("r_watercaustics", "1"), ("r_water_reflections", "1"), ("r_water_refraction", "1"),
    ("r_water_droplets", "1"), ("r_enhancedfx", "1"), ("r_enhancedparticles", "1"), ("r_blood", "1"),
    ("r_impact_fx", "1"), ("r_impact_bullets", "1"), ("r_impact_nails", "1"), ("r_impact_scorches", "1"),
    ("r_debris", "1"), ("r_teleportwindows", "1"), ("r_skywind", "1"),
    ("s_reverb", "1"), ("s_hrtf", "1"), ("s_occlusion", "1"), ("snd_waterfx", "1"),
    ("custom_enable", "1"), ("addon_portalgun", "1"), ("addon_gel", "1"), ("addon_washer", "1"), ("addon_cable", "1"),
    ("addon_goggles", "1"),
];
/// Four players at ad_sepulcher's spawn, monsters awake. Player 1: portal
/// gun fired both ways while turning, night vision. Player 2: both gels.
/// Player 3: the washer held. Player 4: the cable pulsed. Guests get god
/// mode once they have joined; everyone turns so toys hit new surfaces.
const ALL_SPLIT4_SETUP: &[Cmd] = &[
    at(3.0, "splitcmd 2 god"), at(3.0, "splitcmd 3 god"), at(3.0, "splitcmd 4 god"),
    at(3.0, "cl_yawspeed 15"), at(3.0, "+right"),
    at(3.0, "splitbtn 2 +right"), at(3.0, "splitbtn 3 +left"), at(3.0, "splitbtn 4 +left"),
    at(3.0, "goggles nv"),
    at(3.1, "toy 0"), every(3.2, 1.5, "+attack"), every(3.35, 1.5, "-attack"), every(3.95, 1.5, "+attack2"), every(4.1, 1.5, "-attack2"),
    at(3.1, "splittoy 2 5"), every(3.2, 2.0, "splitbtn 2 +attack"), every(4.0, 2.0, "splitbtn 2 -attack"),
    every(4.2, 2.0, "splitbtn 2 +attack2"), every(5.0, 2.0, "splitbtn 2 -attack2"),
    at(3.1, "splittoy 3 6"), at(3.2, "splitbtn 3 +attack"),
    at(3.1, "splittoy 4 7"), every(3.2, 1.5, "splitbtn 4 +attack"), every(3.35, 1.5, "splitbtn 4 -attack"),
    every(3.95, 1.5, "splitbtn 4 +attack2"), every(4.1, 1.5, "splitbtn 4 -attack2"),
];
const ALL_SPLIT4: Live = Live { players: 4, fight: true, ..Live::player("ad", "ad_sepulcher", SEPULCHER_SPAWN, ALL_SPLIT4_SETUP) };
const GOGGLES_THERMAL: &[Cmd] = &[at(3.0, "goggles thermal")];

const NONE: Cvars = &[];

pub const SCENES: &[PlanScene] = &[
    // 1. Vanilla baseline.
    demo("combat-e1m3", "combat-e1m3", Group::Vanilla, NONE, NONE, "typical gameplay: fighting, weapons, monsters"),
    demo("spin-e1m1", "spin-e1m1", Group::Vanilla, NONE, NONE, "typical id1 map, light load"),
    demo("spin-sepulcher", "spin-sepulcher", Group::Vanilla, NONE, NONE, "big AD map, many entities"),
    demo("spin-sepulcher-fte", "spin-sepulcher-fte", Group::Vanilla, NONE, NONE, "FTE particle rain"),
    // 2. Enhancements.
    demo("bloom", "spin-sepulcher", Group::Enhancement, &[("r_bloom", "1")], NONE, "full-screen passes (4K)"),
    demo(
        "rt-dlights", "combat-e1m3", Group::Enhancement, &[("r_shadow_realtime_dlight", "1")], &[("r_shadow_quality", "2")],
        "shadow passes per light",
    ),
    // e2m1's open pool from its edge (`qdata.py liquids e2m1`): the surface
    // fills the lower half and mirrors the room.
    live(
        "water-planar", Live::cam("id1", "e2m1", E2M1_POOL), Group::Enhancement,
        &[("r_enhancedwater", "1"), ("r_water_reflections", "1")], &[("r_water_quality", "2")], "extra scene pass",
    ),
    live(
        "water-rt", Live::cam("id1", "e2m1", E2M1_POOL), Group::Enhancement,
        &[("r_enhancedwater", "1"), ("r_water_reflections", "2")], &[("r_water_quality", "2")], "ray queries, BLAS/TLAS updates",
    ),
    live(
        "water-rest", Live::cam("id1", "e2m1", E2M1_POOL), Group::Enhancement, WATER_ON,
        &[("r_water_quality", "2")], "translucency, ripples, splashes, caustics",
    ),
    live(
        "water-refraction", Live::cam("id1", "e2m2", E2M2_UNDER), Group::Enhancement,
        &[("r_enhancedwater", "1"), ("r_water_refraction", "1")], NONE, "full-screen post (4K), camera under water",
    ),
    // Under water for 0.5 s every 3 s: the 2 s of droplets cover most frames.
    live(
        "water-droplets", Live { alt: Some((E2M2_UNDER, 3.0, 0.5)), ..Live::cam("id1", "e2m2", E2M2_ABOVE) }, Group::Enhancement,
        &[("r_enhancedwater", "1"), ("r_water_droplets", "1")], NONE, "full-screen post (4K), surfacing every 3 s",
    ),
    demo("ssao", "spin-sepulcher", Group::Enhancement, &[("r_ssao", "1")], &[("r_ssao", "3")], "4K"),
    demo("motionblur", "combat-e1m3", Group::Enhancement, &[("r_motionblur", "1"), ("r_speedblur", "1")], NONE, "full-screen pass"),
    demo("blood", "combat-e1m3", Group::Enhancement, FX_BLOOD, &[("r_blood_quality", "2")], "particle counts, CPU"),
    demo("impacts", "combat-e1m3", Group::Enhancement, FX_IMPACTS, &[("r_impact_fx", "4"), ("r_shotgun_tracers", "1")], "decals, CPU"),
    demo("particles", "combat-e1m3", Group::Enhancement, FX_PARTICLES, NONE, "particle counts, CPU"),
    demo("debris", "combat-e1m3", Group::Enhancement, &[("r_debris", "1")], &[("r_debris_quality", "2")], "CPU physics"),
    // The "normal" skill teleporter (`qinspect portals`), from 110 units.
    live(
        "teleporter-windows", Live::cam("id1", "start", [544.0, 1274.0, 56.0, 0.0, 90.0]), Group::Enhancement,
        &[("r_teleportwindows", "1")], NONE, "destination view",
    ),
    todo(
        "hdr-deluxe", "id1", "?", Group::Enhancement, &[("r_hdr_lightmaps", "1"), ("r_deluxemapping", "1")], "lightmap upload, shading",
        "no installed map has .lit2 / LIGHTING_E5BGR9 or .dlit data",
    ),
    // Spawn, looking up at the sky with the hub tower in view.
    live(
        "skywind", Live::cam("qbj3", "start", [0.0, -1648.0, -920.0, -40.0, 90.0]), Group::Enhancement,
        &[("r_skywind", "1")], NONE, "sky (mak_cloudysky4_wind.cfg)",
    ),
    // Audio: real time; frame-time cost until the mixer counters exist (P2).
    live("reverb", E1M3_FIGHT, Group::Audio, &[("s_reverb", "1")], NONE, "mixer, acoustic probes"),
    live("hrtf", E1M3_FIGHT, Group::Audio, &[("s_hrtf", "1")], NONE, "mixer"),
    live("occlusion", E1M3_FIGHT, Group::Audio, &[("s_occlusion", "1")], NONE, "acoustic probes"),
    live("underwater-sound", E3M5_UNDER, Group::Audio, &[("snd_waterfx", "1")], NONE, "mixer filter"),
    // Add-ons: customized game, live.
    live(
        "portalgun", Live::player("id1", "e1m1", [470.0, 160.0, 40.0, 0.0, 80.0], PORTALS), Group::Addon,
        &[("custom_enable", "1"), ("addon_portalgun", "1")], &[("addon_portal_depth", "6"), ("r_portal_quality", "2")],
        "portal views: 2 facing portals, recursion 3 (top 6)",
    ),
    live(
        "gel", Live::player("id1", "e1m1", [480.0, 120.0, 40.0, 25.0, 90.0], GEL), Group::Addon,
        &[("custom_enable", "1"), ("addon_gel", "1")], NONE, "coat rebuild spikes (p99, max)",
    ),
    live(
        "washer", Live::player("id1", "e1m1", E1M1_CORRIDOR, WASHER), Group::Addon,
        &[("custom_enable", "1"), ("addon_washer", "1")], NONE, "jet and spray",
    ),
    live(
        "cable", Live::player("id1", "e1m1", E1M1_CORRIDOR, CABLE), Group::Addon,
        &[("custom_enable", "1"), ("addon_cable", "1")], NONE, "socket highlight, plug, bolt",
    ),
    live(
        "goggles-nv", Live::player("ad", "ad_sepulcher", SEPULCHER_SPAWN, GOGGLES_NV), Group::Addon,
        &[("custom_enable", "1"), ("addon_goggles", "1")], &[("addon_goggles_quality", "2")], "night-vision post",
    ),
    live(
        "goggles-thermal", Live::player("ad", "ad_sepulcher", SEPULCHER_SPAWN, GOGGLES_THERMAL), Group::Addon,
        &[("custom_enable", "1"), ("addon_goggles", "1")], &[("addon_goggles_quality", "2")], "thermal post",
    ),
    // 3. Everything on.
    live(
        "all-split4", ALL_SPLIT4, Group::All, EVERYTHING, NONE,
        "every feature, 4 split-screen players fighting, toys in use",
    ),
];

pub fn find(name: &str) -> Option<&'static PlanScene> {
    SCENES.iter().find(|s| s.name == name)
}

/// The cvars of one variant of a scene, as `name "value"` cfg lines after
/// the baseline: `top` adds the top tier on top of `on`.
pub fn cfg_lines(s: &PlanScene, v: Variant) -> Vec<String> {
    pins(s, v).iter().map(|(k, v)| format!("{k} \"{v}\"")).collect()
}

/// The pins of one variant, in order (a later value wins).
pub fn pins(s: &PlanScene, v: Variant) -> Vec<(&'static str, &'static str)> {
    let mut pins = baseline(s.profile());
    if v != Variant::Off {
        pins.extend_from_slice(s.on);
    }
    if v == Variant::Top {
        pins.extend_from_slice(s.top);
    }
    pins
}

/// Which run of a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variant {
    Off,
    On,
    Top,
}

impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Variant::Off => "off",
            Variant::On => "on",
            Variant::Top => "top",
        }
    }

    /// The variants a scene runs: vanilla scenes only `off`.
    pub fn of(s: &PlanScene) -> Vec<Variant> {
        match (s.on.is_empty(), s.top.is_empty()) {
            (true, _) => vec![Variant::Off],
            (false, true) => vec![Variant::Off, Variant::On],
            (false, false) => vec![Variant::Off, Variant::On, Variant::Top],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn names_unique_and_demos_exist() {
        for n in ["water-refraction", "water-droplets", "teleporter-windows", "skywind", "hdr-deluxe", "all-split4", "portalgun", "gel", "goggles-thermal"] {
            assert!(find(n).is_some(), "{n} missing");
        }
        for (i, s) in SCENES.iter().enumerate() {
            assert!(SCENES[i + 1..].iter().all(|o| o.name != s.name), "{}", s.name);
            if let Kind::Demo(d) = s.kind {
                assert!(crate::scenes::find(d).is_some(), "{}: no demo {d}", s.name);
            }
            assert_eq!(s.group == Group::Vanilla, s.on.is_empty(), "{}", s.name);
        }
    }

    /// Every cvar a scene sets is pinned by the baseline, so the off run
    /// has it at a known value whatever the engine default.
    #[test]
    fn every_feature_cvar_is_pinned() {
        for s in SCENES {
            let base = baseline(s.profile());
            for (k, _) in s.on.iter().chain(s.top) {
                assert!(base.iter().any(|(b, _)| b == k), "{}: {k} not pinned by the baseline", s.name);
            }
        }
    }

    fn rust_sources(dir: &Path, out: &mut String) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_sources(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            }
        }
    }

    /// Every pinned or scene cvar is registered by the engine (a typo
    /// would silently measure nothing).
    #[test]
    fn every_cvar_exists_in_the_engine() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut src = String::new();
        rust_sources(&root.join("src"), &mut src);
        rust_sources(&root.join("crates"), &mut src);
        for p in Profile::ALL {
            for (k, _) in baseline(p).iter().chain(SCENES.iter().flat_map(|s| s.on.iter().chain(s.top))) {
                assert!(src.contains(&format!("(\"{k}\", \"")), "cvar {k} is not registered by the engine");
            }
        }
    }

    #[test]
    fn variants_and_cfg_order() {
        let b = find("rt-dlights").unwrap();
        assert_eq!(Variant::of(b), [Variant::Off, Variant::On, Variant::Top]);
        assert_eq!(Variant::of(find("spin-e1m1").unwrap()), [Variant::Off]);
        let off = cfg_lines(b, Variant::Off);
        let top = cfg_lines(b, Variant::Top);
        let last = |v: &[String], k: &str| v.iter().rposition(|l| l.starts_with(&format!("{k} "))).map(|i| v[i].clone());
        assert_eq!(last(&off, "r_shadow_realtime_dlight").as_deref(), Some("r_shadow_realtime_dlight \"0\""));
        assert_eq!(last(&top, "r_shadow_realtime_dlight").as_deref(), Some("r_shadow_realtime_dlight \"1\""));
        assert_eq!(last(&top, "r_shadow_quality").as_deref(), Some("r_shadow_quality \"2\""));
        assert_eq!(Variant::of(find("bloom").unwrap()), [Variant::Off, Variant::On]);
        assert_eq!(find("spin-sepulcher-fte").unwrap().profile(), Profile::UltraFte);
        assert!(!find("hdr-deluxe").unwrap().runnable() && b.runnable() && find("water-droplets").unwrap().runnable());
    }
}
