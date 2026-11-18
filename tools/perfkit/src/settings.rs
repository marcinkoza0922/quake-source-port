//! Part 1's "vanilla ultra" settings in each engine's cvar names
//! (`docs/perf/VANILLA.md`, Settings): 8x MSAA, no supersampling, 16x
//! anisotropic filtering, remaster md5 models, everything else the classic
//! look. Every value is pinned on the command line, defaults included, so a
//! changed default in any engine can't move the numbers.
//!
//! Two profiles differ only in particles:
//! - [`Profile::Ultra`]: classic particles everywhere (the main suite: all
//!   three engines draw the same thing);
//! - [`Profile::UltraFte`]: FTE scripted particles in ours and vkQuake (the
//!   particle suite); ironwail has none and keeps classic particles.
//!
//! [`split`] divides an engine's pins between the command line and a cfg
//! the runner `exec`s. `perfkit settings` prints the table as Markdown.

use crate::engine::Engine;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Ultra,
    UltraFte,
}

impl Profile {
    pub const ALL: [Profile; 2] = [Profile::Ultra, Profile::UltraFte];

    pub fn name(self) -> &'static str {
        match self {
            Profile::Ultra => "ultra",
            Profile::UltraFte => "ultra-fte",
        }
    }

    pub fn parse(s: &str) -> Option<Profile> {
        Profile::ALL.into_iter().find(|p| p.name() == s)
    }
}

type Cvars = &'static [(&'static str, &'static str)];

/// One setting: what it is, and the cvars that set it in each engine.
pub struct Row {
    pub what: &'static str,
    pub ours: Cvars,
    pub ironwail: Cvars,
    pub vkquake: Cvars,
}

impl Row {
    pub fn cvars(&self, e: Engine) -> Cvars {
        match e {
            Engine::Ours => self.ours,
            Engine::Ironwail => self.ironwail,
            Engine::VkQuake => self.vkquake,
        }
    }
}

const fn row(what: &'static str, ours: Cvars, ironwail: Cvars, vkquake: Cvars) -> Row {
    Row { what, ours, ironwail, vkquake }
}

const LERP: Cvars = &[("r_lerpmodels", "1"), ("r_lerpmove", "1")];
const GAMMA: Cvars = &[("gamma", "1"), ("contrast", "1")];

/// The settings every Part 1 run shares.
pub const ULTRA: &[Row] = &[
    row("8x MSAA", &[("r_msaa", "8")], &[("vid_fsaa", "8")], &[("vid_fsaa", "8")]),
    row("no supersampling", &[("vid_fsaamode", "0")], &[("vid_fsaamode", "0")], &[("vid_fsaamode", "0")]),
    row("16x anisotropic", &[("r_anisotropy", "16")], &[("gl_texture_anisotropy", "16")], &[("vid_anisotropic", "1")]),
    row("classic texture filter", &[("r_texfilter", "0")], &[("gl_texturemode", "GL_NEAREST_MIPMAP_LINEAR")], &[("vid_filter", "1")]),
    // vkQuake has no render scale: always 1.
    row("render scale 1", &[("r_renderscale", "1")], &[("r_scale", "1")], &[]),
    row("remaster md5 models", &[("r_enhancedmodels", "1")], &[("r_enhancedmodels", "1")], &[("r_allow_replacement_md5models", "1")]),
    row("model/move lerping", LERP, LERP, LERP),
    row("dynamic lights", &[("r_dynamic", "1")], &[("r_dynamic", "1")], &[("r_dynamic", "1")]),
    row("coloured light (.lit)", &[("r_coloredlight", "1")], &[], &[]),
    row("fog", &[("r_fog", "1"), ("r_skyfog", "0.5")], &[("r_skyfog", "0.5")], &[("r_skyfog", "0.5")]),
    row("gamma / contrast", GAMMA, GAMMA, GAMMA),
    row("vkQuake extras off", &[], &[], &[("r_rtshadows", "0"), ("vid_palettize", "0")]),
    row(
        "our enhancements and add-ons off",
        &[
            ("r_ssao", "0"), ("r_motionblur", "0"), ("r_speedblur", "0"), ("r_enhancedwater", "0"),
            ("r_teleportwindows", "0"), ("r_enhancedfx", "0"), ("r_enhancedparticles", "0"),
            ("r_impact_fx", "0"), ("r_impact_bullets", "0"), ("r_impact_nails", "0"),
            ("r_impact_scorches", "0"), ("r_shotgun_tracers", "0"), ("r_blood", "0"), ("r_debris", "0"),
            ("r_softemu", "0"), ("r_dither", "0"), ("r_colorblind", "0"), ("custom_enable", "0"), ("addon_hitstop", "0"),
        ],
        &[],
        &[],
    ),
];

/// Classic particles in every engine (round dots off: square, the
/// references' default). Particle Quality stays at our default; classic
/// particles aren't soft, so it changes nothing here.
pub const CLASSIC_PARTICLES: &[Row] = &[row(
    "classic particles",
    &[("r_fteparticles", "0"), ("r_particle_quality", "1"), ("r_particles", "2")],
    &[("r_particles", "2")],
    &[("r_fteparticles", "0"), ("r_particles", "2")],
)];

/// FTE scripted particles in ours and vkQuake with the classic effect set,
/// soft particles off (not part of the base game; the benchmarked vkQuake
/// 1.34.1 AppImage has none, only newer vkQuake source does); ironwail
/// classic. Ours turns soft particles off only with Particle Quality 0
/// (`r_softparticles` is a no-op since 2026-11-16), which also halves
/// scripted particle density, so `r_part_density 2` restores it (V8;
/// `perfkit partinfo` checks the counts against vkQuake).
pub const FTE_PARTICLES: &[Row] = &[row(
    "FTE particles (ironwail: classic)",
    &[
        ("r_fteparticles", "1"), ("r_particledesc", "classic"), ("r_particle_quality", "0"), ("r_part_density", "2"),
        ("r_quadparticles", "1"), ("r_particles", "2"),
    ],
    &[("r_particles", "2")],
    &[("r_fteparticles", "1"), ("r_particledesc", "classic"), ("r_quadparticles", "1"), ("r_particles", "2")],
)];

/// The rows of a profile.
pub fn rows(p: Profile) -> impl Iterator<Item = &'static Row> {
    let particles = match p {
        Profile::Ultra => CLASSIC_PARTICLES,
        Profile::UltraFte => FTE_PARTICLES,
    };
    ULTRA.iter().chain(particles)
}

/// The engine's pins split by where they must go: `vid_*` cvars as
/// `+name value` command-line pairs, because the references read video
/// cvars from the command line before creating the window
/// (CFG_ReadCvarOverrides); everything else as cfg lines for `exec`,
/// because ironwail cuts off a long command line (it stopped at
/// `+r_sca` and never reached `+timedemo`).
pub fn split(p: Profile, e: Engine) -> (Vec<String>, Vec<String>) {
    let (vid, rest): (Vec<_>, Vec<_>) = rows(p).flat_map(|r| r.cvars(e)).partition(|(k, _)| k.starts_with("vid_"));
    let args = vid.iter().flat_map(|&&(k, v)| [format!("+{k}"), v.to_string()]).collect();
    let cfg = rest.iter().map(|&&(k, v)| format!("{k} \"{v}\"")).collect();
    (args, cfg)
}

/// The table as Markdown (`perfkit settings`).
pub fn markdown(p: Profile) -> String {
    let cell = |c: Cvars, other_engine_has: bool| {
        if c.is_empty() {
            return if other_engine_has { "–".to_string() } else { String::new() };
        }
        c.iter().map(|(k, v)| format!("`{k} {v}`")).collect::<Vec<_>>().join(", ")
    };
    let mut s = String::from("| Setting | ours | ironwail | vkQuake |\n|---|---|---|---|\n");
    for r in rows(p) {
        let any = !r.ours.is_empty();
        s.push_str(&format!("| {} | {} | {} | {} |\n", r.what, cell(r.ours, false), cell(r.ironwail, any), cell(r.vkquake, any)));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Every `.rs`/`.c` file under `dirs`, concatenated.
    fn sources(dirs: &[PathBuf], ext: &str) -> String {
        let mut out = String::new();
        let mut stack: Vec<PathBuf> = dirs.to_vec();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == ext) {
                    out.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
                }
            }
        }
        out
    }

    fn missing(e: Engine, src: &str) -> Vec<&'static str> {
        let mut m: Vec<&str> = Profile::ALL
            .into_iter()
            .flat_map(rows)
            .flat_map(|r| r.cvars(e))
            .map(|&(k, _)| k)
            .filter(|k| !src.contains(&format!("\"{k}\"")))
            .collect();
        m.dedup();
        m
    }

    #[test]
    fn our_cvars_exist() {
        let src = sources(&[repo().join("src"), repo().join("crates")], "rs");
        assert!(src.len() > 100_000, "engine sources not found");
        assert_eq!(missing(Engine::Ours, &src), Vec::<&str>::new());
    }

    #[test]
    fn reference_cvars_exist() {
        for (e, dir) in [(Engine::Ironwail, "ironwail/Quake"), (Engine::VkQuake, "vkQuake/Quake")] {
            let d = repo().join(dir);
            if !d.is_dir() {
                eprintln!("skipped: {} not checked out", d.display());
                continue;
            }
            assert_eq!(missing(e, &sources(&[d], "c")), Vec::<&str>::new(), "{}", e.name());
        }
    }

    /// Cvars ours registers only so old and other engines' configs load,
    /// read nowhere (the `COMPAT_CVARS` run that starts at "Folded into
    /// r_particle_quality" in `src/host/cvars.rs`): a pin on one of them
    /// is silently ignored, as `r_softparticles` was (V8).
    #[test]
    fn our_pins_are_not_retired_cvars() {
        let src = std::fs::read_to_string(repo().join("src/host/cvars.rs")).expect("src/host/cvars.rs");
        let at = src.find("// Folded into r_particle_quality").expect("retired-cvar comment in cvars.rs");
        let retired: Vec<&str> = src[at..]
            .lines()
            .skip(1)
            .take_while(|l| !l.trim_start().starts_with("//"))
            .filter_map(|l| l.trim().strip_prefix("(\"")?.split('"').next())
            .collect();
        assert!(retired.contains(&"r_softparticles"), "{retired:?}");
        let pinned: Vec<&str> = Profile::ALL
            .into_iter()
            .flat_map(rows)
            .flat_map(|r| r.ours)
            .map(|&(k, _)| k)
            .filter(|k| retired.contains(k))
            .collect();
        assert_eq!(pinned, Vec::<&str>::new());
    }

    #[test]
    fn fte_profile_turns_soft_particles_off_at_full_density() {
        let (_, cfg) = split(Profile::UltraFte, Engine::Ours);
        for l in ["r_particle_quality \"0\"", "r_part_density \"2\""] {
            assert!(cfg.contains(&l.to_string()), "{cfg:?}");
        }
        // one pin per cvar, so the cfg order can't decide a value
        for p in Profile::ALL {
            let mut keys: Vec<&str> = rows(p).flat_map(|r| r.ours).map(|&(k, _)| k).collect();
            let n = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), n, "{}", p.name());
        }
    }

    #[test]
    fn split_puts_video_cvars_on_the_command_line() {
        for e in Engine::ALL {
            let (a, cfg) = split(Profile::Ultra, e);
            assert_eq!(a.len() % 2, 0);
            assert!(a.iter().step_by(2).all(|k| k.starts_with("+vid_")), "{a:?}");
            assert!(cfg.iter().all(|l| !l.starts_with("vid_")));
            assert!(a.len() < 12, "the command line must stay short: {a:?}");
        }
        let (a, cfg) = split(Profile::Ultra, Engine::Ironwail);
        assert_eq!(a, ["+vid_fsaa", "8", "+vid_fsaamode", "0"]);
        assert!(cfg.contains(&"gl_texturemode \"GL_NEAREST_MIPMAP_LINEAR\"".to_string()));
        assert!(split(Profile::Ultra, Engine::Ours).1.contains(&"r_fteparticles \"0\"".to_string()));
        assert!(split(Profile::UltraFte, Engine::VkQuake).1.contains(&"r_fteparticles \"1\"".to_string()));
        assert_eq!(split(Profile::Ultra, Engine::Ironwail), split(Profile::UltraFte, Engine::Ironwail));
        assert!(markdown(Profile::Ultra).contains("| 8x MSAA | `r_msaa 8` | `vid_fsaa 8` | `vid_fsaa 8` |"));
    }
}
