//! `perfkit record`: records Part 1's demo set (`perfkit::scenes`) into
//! `target/perf/demos/<commit>/` with a `manifest.json`, points
//! `target/perf/demos/latest` at it, and with `-verify` plays every demo once
//! in each engine (windowed 960×540, the scene's settings profile) to check
//! it plays to the end with the same frame count everywhere.
//!
//! Usage: perfkit record [-basedir DIR] [-only a,b] [-verify]
//!                       [-engines ours,ironwail,vkquake] [-timeout SECS]

use perfkit::engine::{abs, Engine};
use perfkit::guard::ConfigGuard;
use perfkit::json_str;
use perfkit::machine::Machine;
use perfkit::runner::{self, Display, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::scenes::{self, Recorded, Scene};
use perfkit::settings::{self, Profile};
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Opts {
    basedir: PathBuf,
    only: Vec<String>,
    verify: bool,
    engines: Vec<Engine>,
    timeout: u64,
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        only: Vec::new(),
        verify: false,
        engines: Engine::ALL.to_vec(),
        timeout: 120,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut v = || it.next().cloned().unwrap_or_else(|| crate::die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-only" => o.only = v().split(',').map(String::from).collect(),
            "-verify" => o.verify = true,
            "-engines" => o.engines = Engine::parse_list(&v()).unwrap_or_else(|e| crate::die(&e)),
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| crate::die("bad -timeout")),
            _ => crate::die(&format!("unknown option {a}")),
        }
    }
    for n in &o.only {
        if scenes::find(n).is_none() {
            crate::die(&format!("unknown scene {n}"));
        }
    }
    o
}

/// One verification playback.
struct Played {
    engine: Engine,
    file: &'static str,
    frames: Option<u32>,
    fps: Option<f64>,
    error: Option<String>,
}

pub fn run(args: &[String]) {
    let o = parse(args);
    let root = PathBuf::from("target/perf");
    let ours = Engine::Ours.locate(&o.basedir, &root).unwrap_or_else(|| crate::die("target/release/quake-source-port missing"));
    let machine = Machine::detect();
    let commit = if machine.commit.is_empty() { "unknown".to_string() } else { machine.commit.clone() };
    let out = abs(&root.join("demos")).join(&commit);
    let picked: Vec<&Scene> = scenes::SCENES.iter().filter(|s| o.only.is_empty() || o.only.iter().any(|n| n == s.name)).collect();
    let mut rows = Vec::new();
    for s in picked {
        let guard = ConfigGuard::for_game(&o.basedir, s.game);
        let sb = Sandbox::build(&o.basedir, &root, s.game).unwrap_or_else(|e| crate::die(&format!("sandbox: {e}")));
        eprint!("perfkit: recording {:<20} ({} s of {} {})", s.name, s.secs, s.game, s.map);
        let rec = scenes::record(s, &ours, &sb, &out);
        match &rec {
            Ok(r) => eprintln!(" -> {}{}", r.dem.display(), if r.qsd.is_some() { " + .qsd" } else { "" }),
            Err(e) => eprintln!(" failed: {e}"),
        }
        let played = match (&rec, o.verify) {
            (Ok(r), true) => verify(&o, s, r, &sb, &root, &out),
            _ => Vec::new(),
        };
        let changed = guard.changed();
        if !changed.is_empty() {
            eprintln!("perfkit: CONFIGS CHANGED, restoring: {changed:?}");
            guard.restore();
        }
        rows.push((s, rec, played));
    }
    write_manifest(&out, &machine, &rows);
    let latest = root.join("demos").join("latest");
    let _ = std::fs::remove_file(&latest);
    let _ = std::os::unix::fs::symlink(&commit, &latest);
    println!("demos: {} (target/perf/demos/latest)", out.display());
    let bad = rows.iter().any(|(_, r, p)| r.is_err() || p.iter().any(|x| x.error.is_some()));
    if bad {
        std::process::exit(1);
    }
}

/// Plays the scene's demo(s) once in each engine; frame counts must agree
/// within one frame (ours counts the first frame, the references don't).
fn verify(o: &Opts, s: &Scene, r: &Recorded, sb: &Sandbox, root: &Path, out: &Path) -> Vec<Played> {
    let refdir = abs(root).parent().map(|p| p.join("refengines")).unwrap_or_default();
    let profile = if s.fte { Profile::UltraFte } else { Profile::Ultra };
    let mut res: Vec<Played> = Vec::new();
    for &e in &o.engines {
        // FTE scenes: ours and vkQuake play the original with its particle
        // messages; ironwail can only play the plain conversion.
        let (file, src): (&'static str, &Path) = match (&r.qsd, e) {
            (Some(q), Engine::Ours | Engine::VkQuake) => ("qsd", q),
            _ => ("dem", &r.dem),
        };
        let mut p = Played { engine: e, file, frames: None, fps: None, error: None };
        let Some(bin) = e.locate(&o.basedir, &refdir) else {
            p.error = Some("engine not found".into());
            res.push(p);
            continue;
        };
        // Not "-dem"/"-qsd": vkQuake strips a trailing "qsd" from demo
        // names ("x-qsd" became "x-.dem").
        let name = format!("{}-{}", s.name, if file == "qsd" { "full" } else { "plain" });
        let demo = match sb.install_demo(s.game, &name, src) {
            Ok(d) => d,
            Err(err) => {
                p.error = Some(err.to_string());
                res.push(p);
                continue;
            }
        };
        let (args, cfg) = settings::split(profile, e);
        let run_out = out.join("verify").join(format!("{}-{}", s.name, e.name()));
        let spec = RunSpec {
            engine: e, bin: &bin, base: &sb.dir, user_data: Some(&sb.userdata), game: s.game, width: 960, height: 540,
            display: Display::Window, mangohud: false, cold: false, args, cfg, timedemo: &demo,
            timeout: Duration::from_secs(o.timeout), out: &run_out,
        };
        match runner::run(&spec) {
            Ok(r) => {
                p.frames = Some(r.frames);
                p.fps = Some(r.fps);
            }
            Err(err) => p.error = Some(err),
        }
        eprintln!(
            "perfkit:   {:<9} {} {}",
            e.name(), file,
            p.error.as_deref().map_or_else(|| format!("{} frames, {:.1} fps", p.frames.unwrap_or(0), p.fps.unwrap_or(0.0)), |e| format!("FAILED: {e}"))
        );
        res.push(p);
    }
    let counts: Vec<u32> = res.iter().filter_map(|p| p.frames).collect();
    if let (Some(lo), Some(hi)) = (counts.iter().min(), counts.iter().max())
        && hi - lo > 1
    {
        for p in res.iter_mut().filter(|p| p.frames.is_some()) {
            p.error.get_or_insert(format!("frame counts differ between engines ({lo}..{hi})"));
        }
        eprintln!("perfkit:   frame counts differ between engines: {counts:?}");
    }
    res
}

/// Writes `<scene>.json` for each recorded scene, then `manifest.json` from
/// every scene file in `out` (catalogue order), so `-only` runs update
/// their scenes without dropping the others.
fn write_manifest(out: &Path, m: &Machine, rows: &[(&Scene, Result<Recorded, String>, Vec<Played>)]) {
    for (s, rec, played) in rows {
        let _ = std::fs::write(out.join(format!("{}.json", s.name)), scene_json(s, rec, played));
    }
    let items: Vec<String> = scenes::SCENES
        .iter()
        .filter_map(|s| std::fs::read_to_string(out.join(format!("{}.json", s.name))).ok())
        .map(|j| format!("    {}", j.trim()))
        .collect();
    let text = format!("{{\n  \"machine\": {},\n  \"scenes\": [\n{}\n  ]\n}}\n", m.to_json(), items.join(",\n"));
    let _ = std::fs::write(out.join("manifest.json"), text);
}

fn scene_json(s: &Scene, rec: &Result<Recorded, String>, played: &[Played]) -> String {
    let opt = |x: Option<String>| x.unwrap_or_else(|| "null".into());
    {
        let file = |p: &Option<PathBuf>| opt(p.as_ref().map(|p| json_str(&p.file_name().unwrap_or_default().to_string_lossy())));
        let size = |p: &Option<PathBuf>| opt(p.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len().to_string()));
        let (dem, qsd) = match rec {
            Ok(r) => (Some(r.dem.clone()), r.qsd.clone()),
            Err(_) => (None, None),
        };
        let verify: Vec<String> = played
            .iter()
            .map(|p| {
                format!(
                    "{{\"engine\": \"{}\", \"file\": \"{}\", \"frames\": {}, \"fps\": {}, \"error\": {}}}",
                    p.engine.name(), p.file, opt(p.frames.map(|f| f.to_string())), opt(p.fps.map(|f| format!("{f:.1}"))),
                    opt(p.error.as_deref().map(json_str))
                )
            })
            .collect();
        format!(
            "{{\"name\": \"{}\", \"game\": \"{}\", \"map\": \"{}\", \"script\": \"{:?}\", \"secs\": {}, \"fte\": {}, \
             \"dem\": {}, \"dem_bytes\": {}, \"qsd\": {}, \"qsd_bytes\": {}, \"error\": {}, \"verify\": [{}]}}\n",
            s.name, s.game, s.map, s.script, s.secs, s.fte, file(&dem), size(&dem), file(&qsd), size(&qsd),
            opt(rec.as_ref().err().map(|e| json_str(e))), verify.join(", ")
        )
    }
}
