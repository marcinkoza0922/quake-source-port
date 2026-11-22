//! `perfkit run`: the lean plan's runner (`docs/perf/PLAN.md`, step P1).
//!
//! Usage: perfkit run [-quick|-full] [-only a,b] [-res 540,2160] [-runs N]
//!                    [-output WxH] [-basedir DIR] [-timeout SECS] [-noattrib] [-list] [-dry]
//!
//! - `-quick` (default): the vanilla scenes plus the `-only` ones.
//!   `-full`: every runnable scene (or only the `-only` ones).
//! - Plays each scene's off / on / top runs ([`perfkit::plan`]) as windowed
//!   timedemos of our engine, 960×540 in a window and 3840×2160 in
//!   gamescope, all with a MangoHud frame log, in perfkit's sandbox. The
//!   same demo with the same pins is run once and shared by every scene that
//!   needs it, so the vanilla run of `combat-e1m3` is every combat scene's
//!   "off".
//! - Runs are interleaved: each round plays every run once, so drift spreads
//!   evenly. One discarded warm-up run per game and resolution fills the
//!   driver's shader cache first, and the first [`WARM_FRAMES`] frames of
//!   each run are left out of the frame statistics (PLAN.md: no warm-up in
//!   the numbers, hitches over 33 ms reported separately).
//! - After the rounds, one attribution run per demo run and resolution:
//!   the engine profiles the timedemo itself (`prof_timedemo`) for its GPU
//!   time per frame, without MangoHud. Its timing isn't used. `-noattrib`
//!   skips them.
//! - Output: `target/perf/runs/<commit>/report.{json,md}`, per-run console
//!   logs and MangoHud CSVs in `runs/` beside them.
//!
//! Live scenes ([`perfkit::live`]) run windowed too, with `-fixedstep`, and
//! are measured by the engine's own profiler (wall time per frame, host
//! frame times for p99 and hitches, GPU time per frame); MangoHud isn't
//! used for them. Scenes without a chosen setup are listed as blocked.
//!
//! It opens windows (the 4K ones fill the screen) for the whole run. Every
//! engine process is SIGKILLed by the runner, so no config is written, and
//! the user's configs are checked against a snapshot afterwards.

use crate::run_keys::{keys, select, Key, Src};
use crate::run_report;
use perfkit::engine::{abs, Engine};
use perfkit::guard::ConfigGuard;
use perfkit::live;
use perfkit::machine::Machine;
use perfkit::plan;
use perfkit::runner::{self, Display, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::scenes::Scene;
use perfkit::settings::Profile;
use perfkit::stats::Frames;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Frames left out at the start of every run: 2 game seconds at the demos'
/// ~62.5 frames per second (`perfkit::scenes`).
pub const WARM_FRAMES: usize = 125;
/// Frames slower than this count as hitches.
pub const HITCH_MS: f32 = 33.0;

pub struct Opts {
    pub full: bool,
    pub only: Vec<String>,
    pub res: Vec<Res>,
    pub runs: usize,
    pub output: (u32, u32),
    pub basedir: PathBuf,
    pub timeout: u64,
    pub list: bool,
    pub dry: bool,
    /// Skip the demo attribution runs (GPU time).
    pub noattrib: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Res {
    P540,
    K4,
}

impl Res {
    pub fn size(self) -> (u32, u32) {
        match self {
            Res::P540 => (960, 540),
            Res::K4 => (3840, 2160),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Res::P540 => "540p",
            Res::K4 => "4K",
        }
    }

    fn display(self, output: (u32, u32)) -> Display {
        match self {
            Res::P540 => Display::Window,
            Res::K4 => Display::Gamescope { out_w: output.0, out_h: output.1 },
        }
    }
}

fn die(m: &str) -> ! {
    eprintln!("perfkit run: {m}");
    std::process::exit(2)
}

pub fn parse_args(args: &[String]) -> Opts {
    let mut o = Opts {
        full: false,
        only: Vec::new(),
        res: vec![Res::P540, Res::K4],
        runs: 3,
        output: (2560, 1440),
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        timeout: 240,
        list: false,
        dry: false,
        noattrib: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut v = || it.next().cloned().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "-quick" => o.full = false,
            "-full" => o.full = true,
            "-only" => o.only = v().split(',').map(str::to_string).collect(),
            "-res" => {
                o.res = v()
                    .split(',')
                    .map(|r| match r {
                        "540" | "540p" => Res::P540,
                        "2160" | "4k" | "4K" => Res::K4,
                        _ => die(&format!("bad resolution {r} (540 or 2160)")),
                    })
                    .collect()
            }
            "-runs" => o.runs = v().parse::<usize>().unwrap_or_else(|_| die("bad -runs")).max(1),
            "-output" => {
                let s = v();
                o.output = s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))).unwrap_or_else(|| die("bad -output"))
            }
            "-basedir" => o.basedir = v().into(),
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| die("bad -timeout")),
            "-list" => o.list = true,
            "-dry" => o.dry = true,
            "-noattrib" => o.noattrib = true,
            _ => die(&format!("unknown option {a}")),
        }
    }
    for n in &o.only {
        if plan::find(n).is_none() {
            die(&format!("no scene {n} (perfkit run -list)"));
        }
    }
    o
}

/// One run's numbers.
#[derive(Clone, Debug)]
pub struct Sample {
    pub fps: f64,
    /// Mean frame time, ms.
    pub mean: f64,
    /// Frame statistics of the measured frames.
    pub frames: Option<Frames>,
    pub hitches: usize,
    /// Average GPU time per frame (live runs, from the engine's profiler).
    pub gpu_us: Option<f64>,
}

impl Sample {
    /// A timedemo: frame log without the first [`WARM_FRAMES`]; the mean
    /// from the log, else from the fps.
    pub fn of(fps: f64, frame_ms: Option<&[f32]>) -> Sample {
        let ms = frame_ms.map(|f| &f[WARM_FRAMES.min(f.len())..]);
        let frames = ms.and_then(Frames::of);
        let mean = frames.map_or(1000.0 / fps.max(1e-9), |f| f.mean);
        Sample { fps, mean, frames, hitches: ms.map_or(0, hitches), gpu_us: None }
    }

    /// A live run: the profiler covers only the measured window; the mean
    /// is wall time per frame.
    pub fn live(p: &live::Profile) -> Sample {
        let ms: Vec<f32> = p.frame_us.iter().map(|us| us / 1000.0).collect();
        let fps = p.fps();
        Sample { fps, mean: 1000.0 / fps.max(1e-9), frames: Frames::of(&ms), hitches: hitches(&ms), gpu_us: p.gpu_us }
    }

    pub fn mean_ms(&self) -> f64 {
        self.mean
    }
}

fn hitches(ms: &[f32]) -> usize {
    ms.iter().filter(|&&x| x > HITCH_MS).count()
}

/// Results of one key at one resolution.
#[derive(Default)]
pub struct Cell {
    pub samples: Vec<Sample>,
    pub errors: Vec<String>,
    /// GPU µs per frame from the demo's attribution run (live runs carry
    /// it in every sample).
    pub gpu_us: Option<f64>,
}

/// The demo file a run plays: the `.qsd` with FTE particle messages under
/// `ultra-fte` when there is one (as qbench does for ours).
fn demo_file(d: &Scene, profile: Profile, dir: &Path) -> (&'static str, PathBuf) {
    let qsd = dir.join(format!("{}.qsd", d.name));
    if profile == Profile::UltraFte && qsd.is_file() {
        ("full", qsd)
    } else {
        ("plain", dir.join(format!("{}.dem", d.name)))
    }
}

/// `vid_*` pins go on the command line (read before the window opens),
/// the rest into the run's cfg ([`perfkit::settings::split`]).
fn split(pins: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
    let (vid, rest): (Vec<_>, Vec<_>) = pins.iter().partition(|(k, _)| k.starts_with("vid_"));
    (vid.iter().flat_map(|(k, v)| [format!("+{k}"), v.to_string()]).collect(), rest.iter().map(|(k, v)| format!("{k} \"{v}\"")).collect())
}

/// Everything one game's runs share.
struct Ctx<'a> {
    o: &'a Opts,
    bin: &'a Path,
    sb: &'a Sandbox,
    out: &'a Path,
}

impl Ctx<'_> {
    /// One run of `k`; `demo` is the installed demo name for demo runs.
    /// `attrib`: a demo's attribution run, profiled by the engine
    /// (`prof_timedemo`) instead of logged by MangoHud; only its GPU time is
    /// used, since profiling costs a little.
    fn go(&self, k: &Key, demo: Option<&str>, res: Res, label: &str, attrib: bool) -> Result<Sample, String> {
        let (w, h) = res.size();
        let (mut args, mut cfg) = split(&k.pins);
        let dir = self.out.join("runs").join(format!("{}-{w}x{h}-{label}", k.label));
        let json = dir.join("profile.json");
        let _ = std::fs::remove_file(&json);
        let live = matches!(k.src, Src::Live(..));
        if let Src::Live(_, l) = k.src {
            if !l.realtime {
                args.extend(["-fixedstep".into(), live::STEP_HZ.to_string()]);
            }
        } else if attrib {
            cfg.push(format!("prof_timedemo \"{}\"", json.display()));
        }
        let spec = RunSpec {
            engine: Engine::Ours, bin: self.bin, base: &self.sb.dir, user_data: Some(&self.sb.userdata), game: k.src.game(),
            width: w, height: h, display: res.display(self.o.output), mangohud: !live && !attrib, cold: false, args, cfg,
            timedemo: demo.unwrap_or(""), timeout: Duration::from_secs(self.o.timeout), out: &dir,
        };
        let profile = || {
            let text = std::fs::read_to_string(&json).map_err(|e| format!("{}: {e}", json.display()))?;
            live::parse_profile(&text)
        };
        match k.src {
            Src::Demo(_) if attrib => runner::run(&spec).and_then(|r| {
                let p = profile()?;
                Ok(Sample { gpu_us: p.gpu_us, ..Sample::of(r.fps, None) })
            }),
            Src::Demo(_) => runner::run(&spec).map(|r| Sample::of(r.fps, r.frame_ms.as_deref())),
            Src::Live(_, l) => {
                runner::live(&spec, &live::script(&l, &json.display().to_string()), live::DONE)?;
                profile().map(|p| Sample::live(&p))
            }
        }
    }
}

pub fn run(args: &[String]) {
    let o = parse_args(args);
    if o.list {
        for s in plan::SCENES {
            let state = if s.runnable() { "ready" } else { "blocked" };
            let at = match s.kind {
                plan::Kind::Demo(d) => format!("demo {d}"),
                plan::Kind::Live(l) => format!("live {}/{}", l.game, l.map),
            };
            println!("{:<20} {:<12} {:<8} {:<26} {}", s.name, s.group.name(), state, at, s.stresses);
        }
        return;
    }
    let (scenes, skipped) = select(&o);
    let (keys, uses) = keys(&scenes);
    let total = keys.len() * o.res.len() * o.runs;
    eprintln!("perfkit run: {} scenes, {} distinct runs × {} res × {} rounds = {total} runs", scenes.len(), keys.len(), o.res.len(), o.runs);
    for (s, why) in &skipped {
        eprintln!("perfkit run:   skipped {}: {why}", s.name);
    }
    if o.dry {
        for k in &keys {
            let kind = if matches!(k.src, Src::Demo(_)) { "demo" } else { "live" };
            println!("{:<28} {kind} {:<20} {} pins", k.label, k.src.name(), k.pins.len());
        }
        return;
    }
    let demos = std::fs::canonicalize("target/perf/demos/latest").ok();
    if demos.is_none() && keys.iter().any(|k| matches!(k.src, Src::Demo(_))) {
        die("no target/perf/demos/latest: run `target/release/perfkit record -verify` first");
    }
    if !o.basedir.join("id1").is_dir() {
        die(&format!("no id1 in {}", o.basedir.display()));
    }
    let bin = Engine::Ours.locate(&o.basedir, Path::new("target/refengines")).unwrap_or_else(|| die("target/release/quake-source-port missing (cargo build --release)"));
    let machine = Machine::detect();
    for w in machine.warnings() {
        eprintln!("perfkit run: warning: {w}");
    }
    let commit = if machine.commit.is_empty() { "unknown".to_string() } else { machine.commit.clone() };
    // Created before `abs`, which only resolves paths that exist: the engine
    // runs from the sandbox, so relative log and profile paths fail.
    let rel = Path::new("target/perf/runs").join(&commit);
    let _ = std::fs::remove_dir_all(rel.join("runs"));
    std::fs::create_dir_all(&rel).unwrap_or_else(|e| die(&e.to_string()));
    let out = abs(&rel);
    eprintln!("perfkit run: windows open on the display for the whole run");

    let mut cells: Vec<Vec<Cell>> = keys.iter().map(|_| o.res.iter().map(|_| Cell::default()).collect()).collect();
    let mut changed = Vec::new();
    let mut games: Vec<&str> = keys.iter().map(|k| k.src.game()).collect();
    games.sort_unstable();
    games.dedup();
    let mut n = 0;
    for game in games {
        if !o.basedir.join(game).is_dir() {
            eprintln!("perfkit run: game {game} not installed, its scenes skipped");
            continue;
        }
        let guard = ConfigGuard::for_game(&o.basedir, game);
        let sb = Sandbox::build(&o.basedir, Path::new("target/perf"), game).unwrap_or_else(|e| die(&format!("sandbox: {e}")));
        // (key, installed demo name for demo runs)
        let mut ready: Vec<(usize, Option<String>)> = Vec::new();
        for i in (0..keys.len()).filter(|&i| keys[i].src.game() == game) {
            let Src::Demo(d) = keys[i].src else {
                ready.push((i, None));
                continue;
            };
            let (kind, src) = demo_file(d, keys[i].profile, demos.as_deref().unwrap_or(Path::new(".")));
            match sb.install_demo(game, &format!("{}-{kind}", d.name), &src) {
                Ok(name) => ready.push((i, Some(name))),
                Err(e) => cells[i].iter_mut().for_each(|c| c.errors.push(format!("demo {}: {e}", src.display()))),
            }
        }
        let ctx = Ctx { o: &o, bin: &bin, sb: &sb, out: &out };
        if let Some((i, demo)) = ready.first() {
            for &res in &o.res {
                eprintln!("perfkit run: warm-up {game} {}", res.name());
                if let Err(e) = ctx.go(&keys[*i], demo.as_deref(), res, "warmup", false) {
                    eprintln!("perfkit run:   warm-up failed: {e}");
                }
            }
        }
        for r in 0..o.runs {
            for (i, demo) in &ready {
                for (ri, &res) in o.res.iter().enumerate() {
                    n += 1;
                    eprint!("perfkit run: [{n}/{total}] {:<30} {:>4} ", keys[*i].label, res.name());
                    let cell = &mut cells[*i][ri];
                    match ctx.go(&keys[*i], demo.as_deref(), res, &format!("r{r}"), false) {
                        Ok(s) => {
                            eprintln!("{:>8.1} fps  {:>7.3} ms  {} hitches", s.fps, s.mean_ms(), s.hitches);
                            cell.samples.push(s);
                        }
                        Err(e) => {
                            eprintln!("failed: {e}");
                            cell.errors.push(format!("r{r}: {e}"));
                        }
                    }
                }
            }
        }
        if !o.noattrib {
            for (i, demo) in ready.iter().filter(|(_, d)| d.is_some()) {
                for (ri, &res) in o.res.iter().enumerate() {
                    eprint!("perfkit run: attribution {:<30} {:>4} ", keys[*i].label, res.name());
                    match ctx.go(&keys[*i], demo.as_deref(), res, "attrib", true) {
                        Ok(s) => {
                            eprintln!("GPU {} µs/frame", s.gpu_us.map_or("–".into(), |g| format!("{g:.0}")));
                            cells[*i][ri].gpu_us = s.gpu_us;
                        }
                        Err(e) => {
                            eprintln!("failed: {e}");
                            cells[*i][ri].errors.push(format!("attribution: {e}"));
                        }
                    }
                }
            }
        }
        let c = guard.changed();
        if !c.is_empty() {
            eprintln!("perfkit run: CONFIGS CHANGED, restored: {c:?}");
            guard.restore();
            changed.extend(c);
        }
    }
    let rep = run_report::Report { o: &o, machine: &machine, keys: &keys, uses: &uses, cells: &cells, skipped: &skipped, changed: &changed };
    let (json, md) = (rep.json(), rep.markdown());
    let _ = std::fs::write(out.join("report.json"), json);
    let _ = std::fs::write(out.join("report.md"), &md);
    let latest = out.with_file_name("latest");
    let _ = std::fs::remove_file(&latest);
    let _ = std::os::unix::fs::symlink(&commit, &latest);
    println!("{md}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_frames_and_hitches() {
        let mut ms = vec![100.0f32; WARM_FRAMES];
        ms.extend([1.0, 1.0, 40.0, 2.0]);
        let s = Sample::of(500.0, Some(&ms));
        assert_eq!((s.hitches, s.frames.unwrap().count), (1, 4));
        assert_eq!(s.mean_ms(), 11.0);
        assert_eq!(Sample::of(500.0, None).mean_ms(), 2.0);
        let p = live::Profile { frames: 4, wall_secs: 0.008, frame_us: vec![1500.0, 40000.0, 2000.0, 2000.0], gpu_us: Some(800.0) };
        let l = Sample::live(&p);
        assert_eq!((l.fps, l.mean, l.hitches, l.gpu_us), (500.0, 2.0, 1, Some(800.0)));
    }

    #[test]
    fn vid_pins_go_on_the_command_line() {
        let (a, c) = split(&[("vid_fsaamode", "0"), ("r_bloom", "1")]);
        assert_eq!(a, ["+vid_fsaamode", "0"]);
        assert_eq!(c, ["r_bloom \"1\""]);
    }
}
