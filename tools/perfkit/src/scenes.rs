//! Part 1's demo set (`docs/perf/VANILLA.md`, Scenes) and its recording.
//!
//! Every demo is recorded by our engine, headless, in the sandbox (so with
//! `XDG_DATA_HOME` there and `+sv_autosave 0`: the user's saves and configs
//! are never touched), at a fixed 250 Hz step. The server still ticks at
//! 72 Hz, so a demo holds about 62.5 frames per game second; the lengths
//! below give ~15000 frames for spins and ~7500 for combat, so even ironwail
//! at 540p (~4000 fps) plays for several seconds.
//!
//! `sv_fteparticles` (and the client's `r_fteparticles`) is pinned while
//! recording because the recording server decides what AD shows: it
//! advertises FTE particles only while `sv_fteparticles` is on, and AD drops
//! its `misc_gtkmodel` decorations (1703 on ad_sepulcher) and its point
//! particles without them. Our engine then writes a plain
//! `.dem` every engine plays and, when the conversion dropped FTE/DP
//! particle messages, keeps the original as `.qsd` for ours and vkQuake.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    /// God mode, a slow 30°/s spin at the spawn point (qbench's demo).
    Spin,
    /// Skill 3, god mode, all weapons, a 40°/s spin firing nonstop:
    /// rockets, then super nailgun, then super shotgun (qprof's combat).
    Combat,
}

#[derive(Clone, Copy, Debug)]
pub struct Scene {
    pub name: &'static str,
    pub game: &'static str,
    pub map: &'static str,
    pub script: Script,
    /// Game seconds recorded.
    pub secs: u32,
    /// `r_fteparticles` while recording.
    pub fte: bool,
}

const fn spin(name: &'static str, game: &'static str, map: &'static str, fte: bool) -> Scene {
    Scene { name, game, map, script: Script::Spin, secs: 240, fte }
}

pub const SCENES: &[Scene] = &[
    spin("spin-e1m1", "id1", "e1m1", false),
    spin("spin-e2m2", "id1", "e2m2", false),
    spin("spin-mg1", "mg1", "start", false),
    spin("spin-sepulcher", "ad", "ad_sepulcher", false),
    spin("spin-sepulcher-fte", "ad", "ad_sepulcher", true),
    spin("spin-tears", "ad", "ad_tears", false),
    Scene { name: "combat-e1m3", game: "id1", map: "e1m3", script: Script::Combat, secs: 120, fte: false },
    // The FTE suite's combat (V6): AD's effectinfo explosions, blood and
    // trails; the most monsters near a spawn of any AD map, no sky rain.
    Scene { name: "combat-grendel-fte", game: "ad", map: "ad_grendel", script: Script::Combat, secs: 120, fte: true },
];

pub fn find(name: &str) -> Option<&'static Scene> {
    SCENES.iter().find(|s| s.name == name)
}

/// The scene's console commands after `+record`, as `+cmd` arguments.
pub fn script_args(s: &Scene) -> Vec<String> {
    // Commands right after `+record` run before the player spawns: a bare
    // `+god` was ignored (AD's combat recording died to its own rockets).
    let mut v: Vec<String> = vec!["+delay 0.5 god".into()];
    match s.script {
        Script::Spin => v.extend(["+cl_yawspeed 30".into(), "+\"+right\"".into()]),
        Script::Combat => {
            // `impulse 9` before the spawn is lost too, and QuakeC sees one
            // impulse per frame, so the rocket launcher waited until the
            // 20 s refill and the 40 s switch: 40 s of shotgun.
            v.extend(["+delay 0.5 impulse 9", "+cl_yawspeed 40", "+\"+right\"", "+delay 1 impulse 7", "+\"+attack\""].map(String::from));
            let third = s.secs / 3;
            v.push(format!("+delay {third} impulse 5"));
            v.push(format!("+delay {} impulse 3", 2 * third));
            // Refill ammo now and then so the fight never goes quiet.
            for t in (20..s.secs).step_by(20) {
                v.push(format!("+delay {t} impulse 9"));
            }
        }
    }
    v.push(format!("+delay {} stop", s.secs));
    v
}

/// The recording command line (without the binary).
pub fn record_args(s: &Scene, base: &Path) -> Vec<String> {
    let mut v: Vec<String> = vec!["-basedir".into(), base.display().to_string()];
    v.extend(["-headless", "-game", s.game, "-width", "320", "-height", "240", "-fixedstep", "250"].map(String::from));
    v.extend(["-autoquit".into(), (s.secs + 10).to_string()]);
    let fte = if s.fte { "1" } else { "0" };
    v.extend(["+sv_autosave", "0", "+sv_fteparticles", fte, "+r_fteparticles", fte].map(String::from));
    if s.script == Script::Combat {
        v.extend(["+skill".into(), "3".into()]);
    }
    v.push(format!("+record perfkit_{} {}", s.name, s.map));
    v.extend(script_args(s));
    v
}

/// The files one recording produced.
#[derive(Clone, Debug)]
pub struct Recorded {
    /// Plain protocol demo every engine plays.
    pub dem: PathBuf,
    /// The original with FTE/DP particle messages, when the conversion
    /// dropped some.
    pub qsd: Option<PathBuf>,
}

/// Records `s` with our engine into `out/<name>.dem` (and `.qsd`).
pub fn record(s: &Scene, ours: &Path, sb: &crate::sandbox::Sandbox, out: &Path) -> Result<Recorded, String> {
    let user = sb.userdata.join("quake-source-port").join(s.game.to_ascii_lowercase());
    let stem = format!("perfkit_{}", s.name);
    for ext in ["dem", "qsd"] {
        let _ = std::fs::remove_file(user.join(format!("{stem}.{ext}")));
    }
    let log = Command::new(ours)
        .args(record_args(s, &sb.dir))
        .env("XDG_DATA_HOME", &sb.userdata)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("spawn {}: {e}", ours.display()))?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let console = [log.stdout, log.stderr].concat();
    let _ = std::fs::write(out.join(format!("{}.record.txt", s.name)), &console);
    let src = user.join(format!("{stem}.dem"));
    if !src.is_file() {
        return Err(format!("{}: no demo written ({}; see {}.record.txt)", s.name, log.status, s.name));
    }
    let dem = out.join(format!("{}.dem", s.name));
    std::fs::rename(&src, &dem).or_else(|_| std::fs::copy(&src, &dem).map(|_| ())).map_err(|e| e.to_string())?;
    let qsd_src = user.join(format!("{stem}.qsd"));
    let qsd = if qsd_src.is_file() {
        let q = out.join(format!("{}.qsd", s.name));
        std::fs::rename(&qsd_src, &q).or_else(|_| std::fs::copy(&qsd_src, &q).map(|_| ())).map_err(|e| e.to_string())?;
        Some(q)
    } else {
        let _ = std::fs::remove_file(out.join(format!("{}.qsd", s.name)));
        None
    };
    Ok(Recorded { dem, qsd })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_names_are_unique_and_found() {
        for (i, s) in SCENES.iter().enumerate() {
            assert!(SCENES[i + 1..].iter().all(|o| o.name != s.name), "{}", s.name);
            assert_eq!(find(s.name).map(|f| f.map), Some(s.map));
        }
    }

    #[test]
    fn record_command_is_isolated_and_stops() {
        let s = find("combat-e1m3").unwrap();
        let a = record_args(s, Path::new("/sb/base")).join(" ");
        assert!(a.contains("-headless") && a.contains("+sv_autosave 0") && a.contains("-fixedstep 250"), "{a}");
        assert!(a.contains("+skill 3 +record perfkit_combat-e1m3 e1m3 +delay 0.5 god"), "{a}");
        assert!(a.ends_with("+delay 120 stop"), "{a}");
        assert!(a.contains("+delay 40 impulse 5") && a.contains("+delay 80 impulse 3"));
        assert!(a.contains("+delay 0.5 impulse 9") && a.contains("+delay 1 impulse 7"), "{a}");
        let g = record_args(find("combat-grendel-fte").unwrap(), Path::new("/sb")).join(" ");
        assert!(g.contains("+sv_fteparticles 1") && g.contains("+skill 3 +record perfkit_combat-grendel-fte ad_grendel"), "{g}");
        let fte = record_args(find("spin-sepulcher-fte").unwrap(), Path::new("/sb")).join(" ");
        assert!(fte.contains("+sv_fteparticles 1 +r_fteparticles 1") && !fte.contains("+skill"));
        assert!(a.contains("+sv_fteparticles 0"));
    }
}
