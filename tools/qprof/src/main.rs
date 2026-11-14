//! qprof: automated CPU + GPU profiling suite for our engine.
//!
//! Runs every scenario in `scenarios.rs` (a pinned vanilla baseline, each
//! graphical enhancement at its most expensive setting, each add-on in
//! active use, and the all-at-once combinations) on each map, live: a real
//! game at skill 3 with god mode, all weapons, a slow spin and constant
//! fire, so monsters fight back, die, gib and bleed. The engine's
//! `prof_start`/`prof_stop` commands record per-frame CPU time per host
//! phase, the renderer's fence wait and GPU pass timestamps (host/prof.rs).
//!
//! Usage: qprof [-basedir <dir>] [-maps game:map,...] [-only a,b] [-group vanilla|enhancement|addon]
//!              [-secs S] [-warmup S] [-runs N] [-maxfps F] [-width W] [-height H]
//!              [-window] [-perf] [-shots S] [-list] [-out <dir>]
//!
//! Headless by default (offscreen, simulated time: every frame advances
//! 1/maxfps seconds, so each run does identical game work). Headless
//! frames pay an image readback in "ui+present", so compare headless with
//! headless only; `-window` measures the real swapchain path (opens
//! windows on the user's display; runs are SIGKILLed so config.cfg is never
//! written). `-perf` wraps each run in `perf record` and keeps the top
//! symbols (build with CARGO_PROFILE_RELEASE_DEBUG=line-tables-only and
//! needs kernel.perf_event_paranoid <= 1).
//!
//! Output: `<out>/<scenario>@<map>.json` per run, `<out>/report.json` and
//! `<out>/report.md` (tables, deltas against vanilla, hotspot ranking).
//! Default out: target/qprof/latest.

mod report;
mod scenarios;
mod ultimate;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Opts {
    pub basedir: PathBuf,
    pub maps: Vec<(String, String)>,
    pub only: Vec<String>,
    pub group: Option<String>,
    pub secs: f32,
    pub warmup: f32,
    pub runs: usize,
    pub maxfps: u32,
    pub width: u32,
    pub height: u32,
    pub window: bool,
    pub perf: bool,
    pub list: bool,
    /// Screenshots every N seconds of measurement (0 = none).
    pub shots: f32,
    pub out: PathBuf,
}

fn die(m: &str) -> ! {
    eprintln!("qprof: {m}");
    std::process::exit(2)
}

fn parse_args() -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("vkQuake directory with maps")),
        maps: vec![("id1".into(), "e1m1".into()), ("ad".into(), "ad_sepulcher".into())],
        only: Vec::new(),
        group: None,
        secs: 20.0,
        warmup: 4.0,
        runs: 1,
        maxfps: 250,
        width: 1280,
        height: 720,
        window: false,
        perf: false,
        list: false,
        shots: 0.0,
        out: PathBuf::from("target/qprof/latest"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-maps" => {
                o.maps = v().split(',').map(|m| match m.split_once(':') {
                    Some((g, m)) => (g.to_string(), m.to_string()),
                    None => ("id1".to_string(), m.to_string()),
                }).collect()
            }
            "-only" => o.only = v().split(',').map(str::to_string).collect(),
            "-group" => o.group = Some(v()),
            "-secs" => o.secs = v().parse().unwrap_or(o.secs),
            "-warmup" => o.warmup = v().parse().unwrap_or(o.warmup),
            "-runs" => o.runs = v().parse::<usize>().unwrap_or(1).max(1),
            "-maxfps" => o.maxfps = v().parse().unwrap_or(o.maxfps),
            "-width" => o.width = v().parse().unwrap_or(o.width),
            "-height" => o.height = v().parse().unwrap_or(o.height),
            "-out" => o.out = v().into(),
            "-window" => o.window = true,
            "-perf" => o.perf = true,
            "-list" => o.list = true,
            "-shots" => o.shots = v().parse().unwrap_or(0.0),
            _ => die(&format!("unknown option {a}")),
        }
    }
    o
}

fn abs(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Engine arguments for one run; the profile lands in `json`.
fn engine_args(o: &Opts, sc: &scenarios::Scenario, game: &str, map: &str, json: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec!["-basedir".into(), abs(&o.basedir).display().to_string(), "-game".into(), game.into()];
    a.push(if o.window { "-window".into() } else { "-headless".into() });
    a.extend(["-width".into(), o.width.to_string(), "-height".into(), o.height.to_string()]);
    let end = o.warmup + o.secs;
    a.extend(["-autoquit".into(), format!("{}", end + 3.0)]);
    let mut set = |k: &str, v: &str| a.push(format!("+{k} \"{v}\""));
    set("sv_autosave", "0");
    set("vid_vsync", "0");
    set("host_maxfps", &o.maxfps.to_string());
    set("skill", "3");
    set("deathmatch", "0");
    set("coop", "0");
    for (k, v) in scenarios::VANILLA.iter().chain(&sc.cvars) {
        set(k, v);
    }
    a.push(format!("+map {map}"));
    let mut script = scenarios::combat(o.secs, sc.own_fire);
    script.extend(sc.script.iter().cloned());
    for (t, cmd) in script {
        a.push(format!("+delay {:.2} {cmd}", o.warmup + t));
    }
    // The warm-up absorbs level load, shader compiles and the first fights.
    a.push(format!("+delay {:.2} prof_start", o.warmup));
    a.push(format!("+delay {end:.2} prof_stop {}", json.display()));
    if o.shots > 0.0 {
        let stem = json.with_extension("");
        let mut t = o.shots;
        while t < o.secs {
            a.push(format!("+delay {:.2} screenshot {}_{t:02.0}", o.warmup + t, stem.display()));
            t += o.shots;
        }
    }
    a
}

/// One run; returns the profile JSON text.
fn run(o: &Opts, bin: &Path, sc: &scenarios::Scenario, game: &str, map: &str, tag: &str) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let json = abs(&o.out).join(format!("{tag}.json"));
    let _ = std::fs::remove_file(&json);
    let args = engine_args(o, sc, game, map, &json);
    let log = std::fs::File::create(o.out.join(format!("{tag}.log"))).map_err(|e| e.to_string())?;
    let mut cmd = if o.perf {
        let mut c = Command::new("perf");
        c.args(["record", "-k", "CLOCK_MONOTONIC", "-e", "cycles:u", "-F", "2000", "-o"]).arg(o.out.join(format!("{tag}.perf.data"))).arg("--").arg(bin);
        c
    } else {
        Command::new(bin)
    };
    let mut child = cmd.args(&args).process_group(0)
        .stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log)
        .spawn().map_err(|e| format!("spawn: {e}"))?;
    // Simulated time runs faster or slower than wall time; allow plenty.
    let deadline = Instant::now() + Duration::from_secs_f32((o.warmup + o.secs) * 20.0 + 120.0);
    let res = loop {
        if json.is_file() && o.window {
            std::thread::sleep(Duration::from_millis(300));
            break Ok(());
        }
        if let Ok(Some(s)) = child.try_wait() {
            break if json.is_file() { Ok(()) } else { Err(format!("engine exited ({s}) without a profile")) };
        }
        if Instant::now() > deadline {
            break Err("timeout".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // SIGKILL windowed runs so config.cfg is never written (headless never
    // writes it); a headless run has already exited here.
    if o.window || res.is_err() {
        let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
    }
    let _ = child.wait();
    res?;
    let text = std::fs::read_to_string(&json).map_err(|e| e.to_string())?;
    if o.perf {
        perf_top(o, tag, &text);
    }
    Ok(text)
}

/// Writes `<tag>.perf.txt`: time per library, then the engine's top
/// self-time functions, limited to the measured window (`mono_start` ..
/// `mono_end` from the engine's profile, same clock as the samples).
fn perf_top(o: &Opts, tag: &str, json: &str) {
    let data = o.out.join(format!("{tag}.perf.data"));
    let clock = |k: &str| report::num_of(json, k).filter(|v| *v > 0.0);
    let window = match (clock("mono_start"), clock("mono_end")) {
        (Some(a), Some(b)) => format!("{a:.6},{b:.6}"),
        _ => String::new(),
    };
    let report = |extra: &[&str]| -> String {
        let mut c = Command::new("perf");
        c.args(["report", "--stdio", "--percent-limit", "0.3", "-i"]).arg(&data).args(extra);
        if !window.is_empty() {
            c.args(["--time", &window]);
        }
        c
            .stderr(Stdio::null()).output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                .take(40).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    };
    let txt = format!("== by library\n{}\n\n== engine functions (share of engine time)\n{}\n",
        report(&["--sort", "dso"]), report(&["--sort", "symbol", "--dsos", "quake-source-port"]));
    let _ = std::fs::write(o.out.join(format!("{tag}.perf.txt")), txt);
}

fn main() {
    let o = parse_args();
    let mut list = scenarios::all(o.secs);
    list.retain(|s| o.only.is_empty() || o.only.iter().any(|n| n == s.name));
    list.retain(|s| o.group.as_deref().is_none_or(|g| g == s.group.name() || s.name == "vanilla"));
    if o.list {
        for s in &list {
            println!("{:<18} {:<12} {}", s.name, s.group.name(), s.what);
        }
        return;
    }
    if !o.basedir.join("id1").is_dir() {
        die(&format!("no id1 in {}", o.basedir.display()));
    }
    let bin = abs(Path::new("target/release/quake-source-port"));
    if !bin.is_file() {
        die("target/release/quake-source-port missing (cargo build --release)");
    }
    std::fs::create_dir_all(&o.out).unwrap_or_else(|e| die(&e.to_string()));
    let maps: Vec<_> = o.maps.iter().filter(|(g, _)| {
        let ok = o.basedir.join(g).is_dir();
        if !ok { eprintln!("qprof: game {g} not installed, skipped"); }
        ok
    }).cloned().collect();
    let mut results = Vec::new();
    let total = list.len() * maps.len() * o.runs;
    let mut n = 0;
    for (game, map) in &maps {
        for sc in list.iter().filter(|s| s.map.is_none()) {
            let mut runs = Vec::new();
            for r in 0..o.runs {
                n += 1;
                let tag = if o.runs > 1 { format!("{}@{map}#{r}", sc.name) } else { format!("{}@{map}", sc.name) };
                eprint!("[{n}/{total}] {:<18} {map:<14} ", sc.name);
                match run(&o, &bin, sc, game, map, &tag) {
                    Ok(j) => match report::Run::parse(&j) {
                        Some(run) => {
                            eprintln!("{:7.1} fps  cpu {:6.0} us  gpu {:6.0} us", run.fps, run.frame_mean, run.gpu_total);
                            runs.push(run);
                        }
                        None => eprintln!("unreadable profile"),
                    },
                    Err(e) => eprintln!("FAILED: {e} (see {}/{tag}.log)", o.out.display()),
                }
            }
            if let Some(r) = report::Run::median(runs) {
                results.push(report::Result { scenario: sc.name, group: sc.group.name(), what: sc.what, map: map.clone(), run: r });
            }
        }
    }
    // Map-pinned scenarios (the ultimate test) run once on their own map.
    for sc in list.iter().filter(|s| s.map.is_some()) {
        let (game, map) = sc.map.unwrap_or_default();
        if !o.basedir.join(game).is_dir() {
            eprintln!("qprof: {} needs game {game}, skipped", sc.name);
            continue;
        }
        let mut runs = Vec::new();
        for r in 0..o.runs {
            let tag = if o.runs > 1 { format!("{}@{map}#{r}", sc.name) } else { format!("{}@{map}", sc.name) };
            eprint!("[{}] {:<18} {map:<14} ", sc.name, sc.group.name());
            match run(&o, &bin, sc, game, map, &tag).map(|j| report::Run::parse(&j)) {
                Ok(Some(run)) => {
                    eprintln!("{:7.1} fps  cpu {:6.0} us  gpu {:6.0} us", run.fps, run.frame_mean, run.gpu_total);
                    runs.push(run);
                }
                Ok(None) => eprintln!("unreadable profile"),
                Err(e) => eprintln!("FAILED: {e} (see {}/{tag}.log)", o.out.display()),
            }
        }
        if let Some(r) = report::Run::median(runs) {
            results.push(report::Result { scenario: sc.name, group: sc.group.name(), what: sc.what, map: map.to_string(), run: r });
        }
    }
    report::write(&o, &results);
}
