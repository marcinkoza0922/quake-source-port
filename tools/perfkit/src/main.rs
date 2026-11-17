//! perfkit: shared benchmark runner (see `src/lib.rs`), plus the R1
//! calibration of the performance plan (`docs/perf/README.md`).
//!
//! Usage: perfkit calibrate [-basedir DIR] [-game G] [-demo FILE] [-runs N]
//!                          [-engines ours,ironwail,vkquake]
//!                          [-res 960x540,1920x1080] [-output 2560x1440]
//!                          [-profile ultra|ultra-fte] [-timeout SECS]
//!                          [-out DIR]
//!        perfkit settings [ultra|ultra-fte]
//!        perfkit record [-only a,b] [-verify] ... (see `src/record_cmd.rs`)
//!        perfkit stills [-only view,view] ... (see `src/stills_cmd.rs`)
//!        perfkit partinfo [-samples N] ... (see `src/partinfo_cmd.rs`)
//!
//! `settings` prints Part 1's settings table (`perfkit::settings`) as
//! Markdown. `-profile` pins those settings in calibration runs; without it
//! each engine runs at its defaults (ours with an empty user config).
//!
//! For each engine and resolution it runs the same timedemo four ways: a
//! plain window, inside gamescope, and each of those with MangoHud logging.
//! The differences are what gamescope and MangoHud cost, which every later
//! Part 1/Part 2 number carries. Runs are interleaved (each configuration
//! once per round) so drift such as heat spreads evenly; one discarded
//! warm-up run per engine fills shader caches first.

mod partinfo_cmd;
mod record_cmd;
mod stills_cmd;
mod stills_html;

use perfkit::engine::{abs, Engine};
use perfkit::guard::ConfigGuard;
use perfkit::machine::Machine;
use perfkit::runner::{self, Display, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::settings::{self, Profile};
use perfkit::stats::{self, Frames};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Opts {
    basedir: PathBuf,
    game: String,
    demo: PathBuf,
    runs: usize,
    engines: Vec<Engine>,
    res: Vec<(u32, u32)>,
    output: (u32, u32),
    out: PathBuf,
    profile: Option<Profile>,
    /// Per-run limit in seconds (startup, load and demo).
    timeout: u64,
}

fn die(m: &str) -> ! {
    eprintln!("perfkit: {m}");
    std::process::exit(2)
}

fn parse_res(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn parse_args(args: &[String]) -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        game: "ad".into(),
        demo: "target/qbench/demos/ad_ad_sepulcher.dem".into(),
        runs: 5,
        engines: Engine::ALL.to_vec(),
        res: vec![(960, 540), (1920, 1080)],
        output: (2560, 1440),
        out: "target/perf/calibrate".into(),
        profile: None,
        timeout: 90,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut v = || it.next().cloned().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-game" => o.game = v(),
            "-demo" => o.demo = v().into(),
            "-runs" => o.runs = v().parse::<usize>().unwrap_or(5).max(1),
            "-engines" => o.engines = Engine::parse_list(&v()).unwrap_or_else(|e| die(&e)),
            "-res" => o.res = v().split(',').map(|r| parse_res(r).unwrap_or_else(|| die(&format!("bad resolution {r}")))).collect(),
            "-output" => o.output = parse_res(&v()).unwrap_or_else(|| die("bad -output")),
            "-out" => o.out = v().into(),
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| die("bad -timeout")),
            "-profile" => o.profile = Some(Profile::parse(&v()).unwrap_or_else(|| die("profile is ultra or ultra-fte"))),
            _ => die(&format!("unknown option {a}")),
        }
    }
    o
}

/// One cell of the calibration matrix.
#[derive(Clone, Copy, PartialEq)]
struct Config {
    engine: Engine,
    res: (u32, u32),
    display: Display,
    mangohud: bool,
}

impl Config {
    fn tag(&self) -> String {
        format!("{}-{}x{}-{}{}", self.engine.name(), self.res.0, self.res.1, self.display.name(), if self.mangohud { "-mangohud" } else { "" })
    }
}

#[derive(Default)]
struct Cell {
    fps: Vec<f64>,
    frames: Vec<Frames>,
    errors: Vec<String>,
}

fn calibrate(o: &Opts) {
    if !o.basedir.join("id1").is_dir() {
        die(&format!("no id1 in {}", o.basedir.display()));
    }
    if !o.demo.is_file() {
        die(&format!("no demo {} (record one with target/release/qbench first)", o.demo.display()));
    }
    let machine = Machine::detect();
    for w in machine.warnings() {
        eprintln!("perfkit: warning: {w}");
    }
    let root = PathBuf::from("target/perf");
    let sandbox = Sandbox::build(&o.basedir, &root, &o.game).unwrap_or_else(|e| die(&format!("sandbox: {e}")));
    let demo = sandbox.install_demo(&o.game, "calibrate", &o.demo).unwrap_or_else(|e| die(&format!("demo: {e}")));
    let guard = ConfigGuard::for_game(&o.basedir, &o.game);
    let refdir = root.parent().unwrap_or(&root).join("refengines");
    let mut bins = Vec::new();
    for &e in &o.engines {
        match e.locate(&o.basedir, &refdir) {
            Some(b) => bins.push((e, b)),
            None => eprintln!("perfkit: {} not found, skipped", e.name()),
        }
    }
    let gs = Display::Gamescope { out_w: o.output.0, out_h: o.output.1 };
    let mut configs = Vec::new();
    for &(engine, _) in &bins {
        for &res in &o.res {
            for display in [Display::Window, gs] {
                for mangohud in [false, true] {
                    configs.push(Config { engine, res, display, mangohud });
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(o.out.join("runs"));
    let run = |c: &Config, label: &str| {
        let bin = &bins.iter().find(|b| b.0 == c.engine).expect("located").1;
        let out = abs(&o.out).join("runs").join(format!("{}-{label}", c.tag()));
        let (args, cfg) = o.profile.map(|p| settings::split(p, c.engine)).unwrap_or_default();
        let spec = RunSpec {
            engine: c.engine, bin, base: &sandbox.dir, user_data: Some(&sandbox.userdata), game: &o.game,
            width: c.res.0, height: c.res.1, display: c.display, mangohud: c.mangohud, cold: false,
            args, cfg, timedemo: &demo, timeout: Duration::from_secs(o.timeout), out: &out,
        };
        runner::run(&spec)
    };
    for &(engine, _) in &bins {
        let warm = Config { engine, res: o.res[0], display: Display::Window, mangohud: false };
        eprintln!("perfkit: warm-up {}", engine.name());
        if let Err(e) = run(&warm, "warmup") {
            eprintln!("perfkit: warm-up {} failed: {e}", engine.name());
        }
    }
    let mut cells: Vec<Cell> = configs.iter().map(|_| Cell::default()).collect();
    for r in 0..o.runs {
        for (c, cell) in configs.iter().zip(&mut cells) {
            eprint!("perfkit: round {}/{} {:<44}", r + 1, o.runs, c.tag());
            match run(c, &format!("{r}")) {
                Ok(res) => {
                    eprintln!(" {:>8.1} fps", res.fps);
                    cell.fps.push(res.fps);
                    if let Some(f) = res.frame_ms.as_deref().and_then(Frames::of) {
                        cell.frames.push(f);
                    }
                }
                Err(e) => {
                    eprintln!(" failed: {e}");
                    cell.errors.push(e);
                }
            }
        }
    }
    let changed = guard.changed();
    if !changed.is_empty() {
        eprintln!("perfkit: CONFIGS CHANGED during the session, restored from the snapshot:");
        for p in guard.restore() {
            eprintln!("  {}", p.display());
        }
    }
    let (json, md) = report(o, &machine, &configs, &cells, &changed);
    std::fs::create_dir_all(&o.out).unwrap_or_else(|e| die(&e.to_string()));
    let _ = std::fs::write(o.out.join("report.json"), json);
    let _ = std::fs::write(o.out.join("report.md"), &md);
    println!("{md}");
}

fn med_frames(f: &[Frames], pick: fn(&Frames) -> f64) -> Option<f64> {
    stats::median(&f.iter().map(pick).collect::<Vec<_>>())
}

/// Frame-time cost of `b` over `a` in percent, from median fps.
fn pair_cost(a: Option<&Cell>, b: Option<&Cell>) -> Option<f64> {
    let (a, b) = a.zip(b)?;
    Some((stats::median(&a.fps)? / stats::median(&b.fps)? - 1.0) * 100.0)
}

fn report(o: &Opts, m: &Machine, configs: &[Config], cells: &[Cell], changed: &[PathBuf]) -> (String, String) {
    let mut md = String::new();
    let _ = writeln!(md, "# perfkit calibration\n\n{}\n", m.summary());
    let settings = o.profile.map_or("engine defaults".to_string(), |p| format!("profile `{}`", p.name()));
    let _ = writeln!(
        md, "Demo `{}`, game `{}`, {} runs per cell, gamescope output {}x{}, {settings}.\n",
        o.demo.display(), o.game, o.runs, o.output.0, o.output.1
    );
    let _ = writeln!(md, "| engine | resolution | display | MangoHud | median fps | min–max | p50 ms | p99 ms | max ms | errors |");
    let _ = writeln!(md, "|---|---|---|---|--:|--:|--:|--:|--:|--:|");
    let mut json = format!("{{\n  \"machine\": {},\n  \"demo\": {},\n  \"cells\": [\n", m.to_json(), perfkit::json_str(&o.demo.display().to_string()));
    let f1 = |x: Option<f64>| x.map_or("–".to_string(), |v| format!("{v:.1}"));
    let f3 = |x: Option<f64>| x.map_or("–".to_string(), |v| format!("{v:.3}"));
    let jn = |x: Option<f64>| x.map_or("null".to_string(), |v| format!("{v:.4}"));
    for (i, (c, cell)) in configs.iter().zip(cells).enumerate() {
        let (lo, hi) = stats::range(&cell.fps).map_or((None, None), |(a, b)| (Some(a), Some(b)));
        let (p50, p99, max) = (med_frames(&cell.frames, |f| f.p50), med_frames(&cell.frames, |f| f.p99), med_frames(&cell.frames, |f| f.max));
        let _ = writeln!(
            md, "| {} | {}x{} | {} | {} | {} | {}–{} | {} | {} | {} | {} |",
            c.engine.name(), c.res.0, c.res.1, c.display.name(), if c.mangohud { "on" } else { "off" },
            f1(stats::median(&cell.fps)), f1(lo), f1(hi), f3(p50), f3(p99), f3(max), cell.errors.len()
        );
        let _ = writeln!(
            json, "    {{\"engine\": \"{}\", \"width\": {}, \"height\": {}, \"display\": \"{}\", \"mangohud\": {}, \
             \"fps\": {:?}, \"fps_median\": {}, \"p50_ms\": {}, \"p99_ms\": {}, \"max_ms\": {}, \"errors\": [{}]}}{}",
            c.engine.name(), c.res.0, c.res.1, c.display.name(), c.mangohud, cell.fps, jn(stats::median(&cell.fps)),
            jn(p50), jn(p99), jn(max), cell.errors.iter().map(|e| perfkit::json_str(e)).collect::<Vec<_>>().join(", "),
            if i + 1 < configs.len() { "," } else { "" }
        );
    }
    json.push_str("  ],\n  \"configs_changed\": [");
    json.push_str(&changed.iter().map(|p| perfkit::json_str(&p.display().to_string())).collect::<Vec<_>>().join(", "));
    json.push_str("]\n}\n");

    let _ = writeln!(md, "\nOverheads (frame-time increase from median fps; + is slower):\n");
    let _ = writeln!(md, "| engine | resolution | gamescope vs window | MangoHud (window) | MangoHud (gamescope) | significant (3% + spread) |");
    let _ = writeln!(md, "|---|---|--:|--:|--:|---|");
    let find = |c: Config| configs.iter().position(|x| *x == c).map(|i| &cells[i]);
    let pc = |x: Option<f64>| x.map_or("–".to_string(), |v| format!("{v:+.1}%"));
    let mut seen = Vec::new();
    for c in configs {
        if seen.contains(&(c.engine, c.res)) {
            continue;
        }
        seen.push((c.engine, c.res));
        let gs = configs.iter().find(|x| matches!(x.display, Display::Gamescope { .. })).map(|x| x.display).unwrap_or(Display::Window);
        let at = |display, mangohud| find(Config { engine: c.engine, res: c.res, display, mangohud });
        let (w, wm, g, gm) = (at(Display::Window, false), at(Display::Window, true), at(gs, false), at(gs, true));
        let sig = match w.zip(g) {
            Some((a, b)) if stats::significant(&a.fps, &b.fps, 3.0) => "gamescope: yes",
            Some(_) => "gamescope: no",
            None => "–",
        };
        let _ = writeln!(
            md, "| {} | {}x{} | {} | {} | {} | {} |",
            c.engine.name(), c.res.0, c.res.1, pc(pair_cost(w, g)), pc(pair_cost(w, wm)), pc(pair_cost(g, gm)), sig
        );
    }
    if !changed.is_empty() {
        let _ = writeln!(md, "\n**Configs changed during the session (restored):** {}", changed.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    }
    let _ = writeln!(md, "\nPer-run console logs and MangoHud CSVs: `{}`.", Path::new(&o.out).join("runs").display());
    (json, md)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("calibrate") => calibrate(&parse_args(&args[1..])),
        Some("record") => record_cmd::run(&args[1..]),
        Some("stills") => stills_cmd::run(&args[1..]),
        Some("partinfo") => partinfo_cmd::run(&args[1..]),
        Some("settings") => {
            let p = args.get(1).map_or(Some(Profile::Ultra), |s| Profile::parse(s));
            print!("{}", settings::markdown(p.unwrap_or_else(|| die("profile is ultra or ultra-fte"))));
        }
        _ => die("usage: perfkit calibrate|settings|record|stills|partinfo [options] (see tools/perfkit/src/main.rs)"),
    }
}
