//! qbench: Part 1 of the performance plan (`docs/perf/VANILLA.md`), our
//! engine against ironwail and vkQuake at "vanilla ultra" settings.
//!
//! Usage: qbench [-suite main|fte|all] [-only scene,...] [-engines ours,...]
//!               [-res 540,4k] [-runs N] [-mhruns N] [-nocold]
//!               [-output 2560x1440] [-timeout SECS] [-basedir DIR]
//!
//! Plays the demo set recorded by `perfkit record` (target/perf/demos/latest)
//! through the perfkit runner:
//! - `main`: six scenes, profile `ultra` (classic particles), all engines;
//! - `fte`: `spin-sepulcher-fte` and `combat-grendel-fte` (ours and vkQuake
//!   play the `.qsd` with AD's FTE particles, ironwail the plain `.dem`),
//!   profile `ultra-fte`.
//!
//! Resolutions: 960×540 in a plain window (CPU-bound) and 3840×2160 inside
//! gamescope scaled to `-output` (GPU-bound). Per scene and engine: a
//! cold-start run with the driver's shader cache off (MangoHud, worst
//! frames), one warm-up, then `-runs` fps runs without MangoHud and
//! `-mhruns` with it for frame-time percentiles, interleaved in rounds.
//! Results: target/qbench/report.{json,md}, per-run logs in target/qbench/runs.

mod report;

use perfkit::engine::{abs, Engine};
use perfkit::guard::ConfigGuard;
use perfkit::machine::Machine;
use perfkit::runner::{self, Display, RunResult, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::scenes::{self, Scene};
use perfkit::settings::{self, Profile};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Opts {
    basedir: PathBuf,
    suites: Vec<&'static str>,
    only: Vec<String>,
    engines: Vec<Engine>,
    res: Vec<Res>,
    runs: usize,
    mhruns: usize,
    cold: bool,
    output: (u32, u32),
    timeout: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
    fn display(self, output: (u32, u32)) -> Display {
        match self {
            Res::P540 => Display::Window,
            Res::K4 => Display::Gamescope { out_w: output.0, out_h: output.1 },
        }
    }
}

fn die(m: &str) -> ! {
    eprintln!("qbench: {m}");
    std::process::exit(2)
}

fn parse_args() -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        suites: vec!["main", "fte"],
        only: Vec::new(),
        engines: Engine::ALL.to_vec(),
        res: vec![Res::P540, Res::K4],
        runs: 3,
        mhruns: 1,
        cold: true,
        output: (2560, 1440),
        timeout: 180,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-suite" => {
                o.suites = match v().as_str() {
                    "main" => vec!["main"],
                    "fte" => vec!["fte"],
                    "all" => vec!["main", "fte"],
                    s => die(&format!("unknown suite {s}")),
                }
            }
            "-only" => o.only = v().split(',').map(String::from).collect(),
            "-engines" => o.engines = Engine::parse_list(&v()).unwrap_or_else(|e| die(&e)),
            "-res" => {
                o.res = v()
                    .split(',')
                    .map(|r| match r {
                        "540" | "540p" => Res::P540,
                        "4k" | "2160" | "2160p" => Res::K4,
                        _ => die(&format!("unknown resolution {r} (540 or 4k)")),
                    })
                    .collect()
            }
            "-runs" => o.runs = v().parse().unwrap_or_else(|_| die("bad -runs")),
            "-mhruns" => o.mhruns = v().parse().unwrap_or_else(|_| die("bad -mhruns")),
            "-nocold" => o.cold = false,
            "-output" => {
                let s = v();
                o.output = s
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .unwrap_or_else(|| die("bad -output"))
            }
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| die("bad -timeout")),
            _ => die(&format!("unknown option {a}")),
        }
    }
    o
}

/// One scene played with one profile by some engines.
pub struct Case {
    pub suite: &'static str,
    pub scene: &'static Scene,
    pub profile: Profile,
    pub engines: Vec<Engine>,
}

fn cases(o: &Opts) -> Vec<Case> {
    let scene = |n| scenes::find(n).expect("scene in catalogue");
    let mut v = Vec::new();
    if o.suites.contains(&"main") {
        for n in ["spin-e1m1", "spin-e2m2", "spin-mg1", "spin-sepulcher", "spin-tears", "combat-e1m3"] {
            v.push(Case { suite: "main", scene: scene(n), profile: Profile::Ultra, engines: Engine::ALL.to_vec() });
        }
    }
    if o.suites.contains(&"fte") {
        v.push(Case { suite: "fte", scene: scene("spin-sepulcher-fte"), profile: Profile::UltraFte, engines: Engine::ALL.to_vec() });
        // Not combat-e1m3: `r_particledesc classic` gives id1 no FTE
        // effects, so it only repeated the main-suite cell (V3/V6).
        v.push(Case { suite: "fte", scene: scene("combat-grendel-fte"), profile: Profile::UltraFte, engines: Engine::ALL.to_vec() });
    }
    v.retain(|c| o.only.is_empty() || o.only.iter().any(|n| n == c.scene.name));
    for c in &mut v {
        c.engines.retain(|e| o.engines.contains(e));
    }
    v
}

/// Measurements of one (case, engine, resolution).
#[derive(Default)]
pub struct Cell {
    pub file: &'static str,
    pub frames: u32,
    pub fps: Vec<f64>,
    pub frame_ms: Vec<Vec<f32>>,
    pub cold: Option<Vec<f32>>,
    pub errors: Vec<String>,
}

/// Key: (case index, engine, resolution).
pub type Cells = BTreeMap<(usize, &'static str, Res), Cell>;

/// The demo an engine plays for a case: the `.qsd` (with FTE particle
/// messages) for ours and vkQuake under `ultra-fte` when there is one.
fn demo_file(case: &Case, e: Engine, dir: &Path) -> (&'static str, PathBuf) {
    let qsd = dir.join(format!("{}.qsd", case.scene.name));
    if case.profile == Profile::UltraFte && e != Engine::Ironwail && qsd.is_file() {
        ("full", qsd)
    } else {
        ("plain", dir.join(format!("{}.dem", case.scene.name)))
    }
}

struct Ctx<'a> {
    o: &'a Opts,
    out: PathBuf,
    refdir: PathBuf,
}

/// How one run is made.
#[derive(Clone, Copy)]
struct How<'a> {
    res: Res,
    mangohud: bool,
    cold: bool,
    label: &'a str,
}

impl Ctx<'_> {
    fn run(&self, case: &Case, e: Engine, sb: &Sandbox, demo: &str, how: How) -> Result<RunResult, String> {
        let bin = e.locate(&self.o.basedir, &self.refdir).ok_or("engine not found")?;
        let (args, cfg) = settings::split(case.profile, e);
        let (w, h) = how.res.size();
        let tag = format!("{}-{}-{}-{}x{}-{}", case.scene.name, case.profile.name(), e.name(), w, h, how.label);
        let out = self.out.join("runs").join(tag);
        let spec = RunSpec {
            engine: e, bin: &bin, base: &sb.dir, user_data: Some(&sb.userdata), game: case.scene.game, width: w, height: h,
            display: how.res.display(self.o.output), mangohud: how.mangohud, cold: how.cold, args, cfg, timedemo: demo,
            timeout: Duration::from_secs(self.o.timeout), out: &out,
        };
        runner::run(&spec)
    }
}

fn main() {
    let o = parse_args();
    let demos = std::fs::canonicalize("target/perf/demos/latest").unwrap_or_else(|_| die("no target/perf/demos/latest: run `target/release/perfkit record -verify` first"));
    let machine = Machine::detect();
    for w in machine.warnings() {
        eprintln!("qbench: warning: {w}");
    }
    let cases = cases(&o);
    let runs_per = o.res.len() * (o.runs + o.mhruns) + if o.cold { 2 } else { 1 };
    let total: usize = cases.iter().map(|c| c.engines.len() * runs_per).sum();
    eprintln!("qbench: {} cases, {total} runs; windows open on the display for the whole run", cases.len());
    let ctx = Ctx { o: &o, out: abs(Path::new("target/qbench")), refdir: abs(Path::new("target")).join("refengines") };
    let _ = std::fs::remove_dir_all(ctx.out.join("runs"));
    let root = PathBuf::from("target/perf");
    let mut cells: Cells = BTreeMap::new();
    let mut changed = Vec::new();
    for (ci, case) in cases.iter().enumerate() {
        let guard = ConfigGuard::for_game(&o.basedir, case.scene.game);
        let sb = Sandbox::build(&o.basedir, &root, case.scene.game).unwrap_or_else(|e| die(&format!("sandbox: {e}")));
        eprintln!("qbench: {} [{}] {}", case.scene.name, case.profile.name(), case.engines.iter().map(|e| e.name()).collect::<Vec<_>>().join(","));
        let mut installed = BTreeMap::new();
        for &e in &case.engines {
            let (file, src) = demo_file(case, e, &demos);
            let name = format!("{}-{file}", case.scene.name);
            match sb.install_demo(case.scene.game, &name, &src) {
                Ok(d) => {
                    installed.insert(e.name(), d);
                }
                Err(err) => eprintln!("qbench:   {} demo {}: {err}", e.name(), src.display()),
            }
            for &r in &o.res {
                cells.entry((ci, e.name(), r)).or_default().file = file;
            }
        }
        let go = |e: Engine, how: How, cells: &mut Cells| {
            let Some(demo) = installed.get(e.name()) else { return };
            let res = ctx.run(case, e, &sb, demo, how);
            let cell = cells.get_mut(&(ci, e.name(), how.res)).expect("cell");
            let label = how.label;
            match res {
                Ok(run) if label == "warmup" => cell.frames = run.frames,
                Ok(run) if how.cold => cell.cold = run.frame_ms,
                Ok(run) => {
                    cell.frames = run.frames;
                    match run.frame_ms {
                        Some(ms) => cell.frame_ms.push(ms),
                        None => cell.fps.push(run.fps),
                    }
                }
                Err(err) => {
                    eprintln!("qbench:   {} {label} failed: {err}", e.name());
                    cell.errors.push(format!("{label}: {err}"));
                }
            }
        };
        for &e in &case.engines {
            if o.cold {
                go(e, How { res: Res::P540, mangohud: true, cold: true, label: "cold" }, &mut cells);
            }
            go(e, How { res: Res::P540, mangohud: false, cold: false, label: "warmup" }, &mut cells);
        }
        for r in 0..o.runs.max(o.mhruns) {
            for &res in &o.res {
                for &e in &case.engines {
                    if r < o.runs {
                        go(e, How { res, mangohud: false, cold: false, label: &format!("fps{r}") }, &mut cells);
                    }
                    if r < o.mhruns {
                        go(e, How { res, mangohud: true, cold: false, label: &format!("mh{r}") }, &mut cells);
                    }
                }
            }
        }
        for (k, c) in cells.range((ci, "", Res::P540)..(ci + 1, "", Res::P540)) {
            if let Some(m) = perfkit::stats::median(&c.fps) {
                eprintln!("qbench:   {:<9} {:>4}  {m:>8.1} fps", k.1, if k.2 == Res::P540 { "540p" } else { "4k" });
            }
        }
        let c = guard.changed();
        if !c.is_empty() {
            eprintln!("qbench: CONFIGS CHANGED, restored: {c:?}");
            guard.restore();
            changed.extend(c);
        }
    }
    let demo_set = demos.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (json, md) = report::write(&o, &machine, &demo_set, &cases, &cells, &changed);
    let _ = std::fs::create_dir_all(&ctx.out);
    let _ = std::fs::write(ctx.out.join("report.json"), json);
    let _ = std::fs::write(ctx.out.join("report.md"), &md);
    println!("{md}");
}
