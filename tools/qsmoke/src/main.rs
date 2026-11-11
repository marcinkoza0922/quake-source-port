//! qsmoke: load every map of every installed mod headless and report problems.
//!
//! Usage: qsmoke [-basedir <dir>] [-bin <engine>] [-j <jobs>] [-time <sim s>]
//!               [-timeout <wall s>] [-out <dir>] [mod[:map] ...]
//!
//! Each map runs in its own engine process (`-headless -autoquit`), so panics
//! and hangs are isolated. Console output is classified into issues; the run
//! writes `<out>/report.json` (per map) and `<out>/summary.txt` (issues grouped
//! by message, with counts and example maps). Exit code is 1 if any map had an
//! error-level issue.

use qcore::fs::Fs;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Level {
    Error,
    Warn,
}

struct Issue {
    level: Level,
    kind: &'static str,
    msg: String,
}

struct MapResult {
    game: String,
    map: String,
    status: String,
    secs: f64,
    issues: Vec<Issue>,
}

/// (substring, level, kind). First match wins; matched case-insensitively.
const RULES: &[(&str, Level, &str)] = &[
    ("panicked at", Level::Error, "panic"),
    ("host_error", Level::Error, "host_error"),
    ("server error", Level::Error, "qc_error"),
    ("object error", Level::Error, "qc_error"),
    ("couldn't spawn server", Level::Error, "spawn"),
    ("cannot find map", Level::Error, "spawn"),
    ("render error", Level::Error, "render"),
    ("bad server message", Level::Error, "protocol"),
    ("unknown command", Level::Warn, "unknown_cmd"),
    ("couldn't load model", Level::Warn, "missing_asset"),
    ("not found", Level::Warn, "missing_asset"),
    ("couldn't", Level::Warn, "misc"),
    ("bad ", Level::Warn, "misc"),
    ("warning", Level::Warn, "misc"),
];

fn classify(line: &str) -> Option<(Level, &'static str)> {
    let l = line.to_ascii_lowercase();
    RULES.iter().find(|r| l.contains(r.0)).map(|r| (r.1, r.2))
}

struct Opts {
    basedir: PathBuf,
    bin: PathBuf,
    jobs: usize,
    time: f64,
    timeout: f64,
    out: PathBuf,
    filters: Vec<String>,
}

fn parse_args() -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR")
            .map_or_else(|| PathBuf::from("vkQuake directory with maps"), PathBuf::from),
        bin: PathBuf::from("target/release/quake-source-port"),
        jobs: std::thread::available_parallelism().map_or(4, |n| n.get().min(8)),
        time: 5.0,
        timeout: 120.0,
        out: PathBuf::from("target/qsmoke"),
        filters: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = || it.next().unwrap_or_default();
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-bin" => o.bin = v().into(),
            "-j" => o.jobs = v().parse().unwrap_or(o.jobs).max(1),
            "-time" => o.time = v().parse().unwrap_or(o.time),
            "-timeout" => o.timeout = v().parse().unwrap_or(o.timeout),
            "-out" => o.out = v().into(),
            "-h" | "--help" => {
                eprintln!("{}", include_str!("main.rs").lines().take(11).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            _ => o.filters.push(a),
        }
    }
    o
}

/// Extra engine flags for mods that build on a mission pack.
fn base_flags(game: &str) -> &'static [&'static str] {
    match game {
        "hipnotic" => &["-hipnotic"],
        "rogue" => &["-rogue"],
        _ => &[],
    }
}

fn mount(basedir: &Path, game: &str) -> Fs {
    let mut fs = Fs::new(basedir);
    fs.add_game_dir("id1");
    for f in base_flags(game) {
        fs.add_game_dir(&f[1..]);
    }
    if game != "id1" {
        fs.add_game_dir(game);
    }
    fs
}

/// Maps owned by `game` (not inherited from id1 / the mission pack).
fn maps_of(basedir: &Path, game: &str) -> Vec<String> {
    let fs = mount(basedir, game);
    fs.list("maps/", ".bsp")
        .into_iter()
        .filter(|p| fs.origin(p).is_some_and(|o| o.game.eq_ignore_ascii_case(game)))
        .filter_map(|p| {
            let n = p.strip_prefix("maps/").unwrap_or(&p).strip_suffix(".bsp")?.to_string();
            // Skip brush-model helpers (b_*.bsp), nested paths, and maps
            // with no spawn point (prefabs, misc_model sources), which
            // other engines can't start either.
            let playable = fs
                .open(&p)
                .is_some_and(|d| d.windows(12).any(|w| w.eq_ignore_ascii_case(b"info_player_")));
            (!n.starts_with("b_") && !n.contains('/') && playable).then_some(n)
        })
        .collect()
}

fn games(basedir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(basedir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    out.sort();
    out
}

fn run_map(o: &Opts, game: &str, map: &str) -> MapResult {
    let start = Instant::now();
    let mut cmd = Command::new(&o.bin);
    cmd.arg("-basedir").arg(&o.basedir).args(base_flags(game));
    if game != "id1" {
        cmd.args(["-game", game]);
    }
    cmd.args(["-headless", "-nosound", "-width", "320", "-height", "240", "-autoquit"])
        .arg(o.time.to_string())
        .args(["+skill", "3", "+map", map])
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut res = MapResult { game: game.into(), map: map.into(), status: String::new(), secs: 0.0, issues: Vec::new() };
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            res.status = format!("spawn failed: {e}");
            return res;
        }
    };
    // Drain pipes on threads so a chatty child can't block.
    let drain = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut s);
            }
            String::from_utf8_lossy(&s).into_owned()
        })
    };
    let out_t = drain(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let err_t = drain(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let deadline = Duration::from_secs_f64(o.timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break None,
        }
    };
    res.secs = start.elapsed().as_secs_f64();
    let text = out_t.join().unwrap_or_default() + &err_t.join().unwrap_or_default();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((level, kind)) = classify(line) else { continue };
        let mut msg = line.trim().to_string();
        // QC errors: the message is on the following line.
        if kind == "qc_error"
            && let Some(next) = lines.next()
        {
            msg = format!("{msg} {}", next.trim());
        }
        if !res.issues.iter().any(|i| i.msg == msg) {
            res.issues.push(Issue { level, kind, msg });
        }
    }
    res.status = match status {
        None => {
            res.issues.push(Issue { level: Level::Error, kind: "timeout", msg: format!("timed out after {:.0}s", o.timeout) });
            "timeout".into()
        }
        Some(s) if s.success() => "ok".into(),
        Some(s) => {
            res.issues.push(Issue { level: Level::Error, kind: "crash", msg: format!("exit status {s}") });
            format!("{s}")
        }
    };
    res
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn write_reports(out: &Path, results: &[MapResult]) -> std::io::Result<()> {
    std::fs::create_dir_all(out)?;
    let mut j = String::from("[\n");
    for (i, r) in results.iter().enumerate() {
        let issues: Vec<String> = r
            .issues
            .iter()
            .map(|x| format!("{{\"level\":\"{:?}\",\"kind\":\"{}\",\"msg\":{}}}", x.level, x.kind, json_str(&x.msg)))
            .collect();
        j.push_str(&format!(
            "  {{\"game\":{},\"map\":{},\"status\":{},\"secs\":{:.2},\"issues\":[{}]}}{}\n",
            json_str(&r.game),
            json_str(&r.map),
            json_str(&r.status),
            r.secs,
            issues.join(","),
            if i + 1 < results.len() { "," } else { "" }
        ));
    }
    j.push_str("]\n");
    std::fs::write(out.join("report.json"), j)?;

    // Group by message across maps.
    let mut groups: BTreeMap<(Level, &str, &str), Vec<String>> = BTreeMap::new();
    for r in results {
        for x in &r.issues {
            groups.entry((x.level, x.kind, x.msg.as_str())).or_default().push(format!("{}/{}", r.game, r.map));
        }
    }
    let bad = results.iter().filter(|r| r.issues.iter().any(|x| x.level == Level::Error)).count();
    let mut s = format!("{} maps, {} with errors\n\n", results.len(), bad);
    for ((level, kind, msg), maps) in &groups {
        let ex: Vec<_> = maps.iter().take(4).cloned().collect();
        s.push_str(&format!(
            "{level:?} [{kind}] x{}: {msg}\n    {}{}\n",
            maps.len(),
            ex.join(" "),
            if maps.len() > 4 { " ..." } else { "" }
        ));
    }
    std::fs::write(out.join("summary.txt"), s)
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        eprintln!("qsmoke: no id1 in {}", o.basedir.display());
        std::process::exit(2);
    }
    if !o.bin.is_file() {
        eprintln!("qsmoke: engine binary {} missing (cargo build --release)", o.bin.display());
        std::process::exit(2);
    }
    let mut work: Vec<(String, String)> = Vec::new();
    for g in games(&o.basedir) {
        let want: Vec<&str> = o
            .filters
            .iter()
            .filter_map(|f| {
                let (fg, fm) = f.split_once(':').unwrap_or((f, ""));
                fg.eq_ignore_ascii_case(&g).then_some(fm)
            })
            .collect();
        if !o.filters.is_empty() && want.is_empty() {
            continue;
        }
        for m in maps_of(&o.basedir, &g) {
            if want.is_empty() || want.iter().any(|w| w.is_empty() || w.eq_ignore_ascii_case(&m)) {
                work.push((g.clone(), m));
            }
        }
    }
    let total = work.len();
    eprintln!("qsmoke: {total} maps, {} jobs", o.jobs);
    let queue = Mutex::new(work.into_iter().enumerate());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs {
            s.spawn(|| {
                loop {
                    let Some((i, (g, m))) = queue.lock().unwrap().next() else { break };
                    let r = run_map(&o, &g, &m);
                    let errs = r.issues.iter().filter(|x| x.level == Level::Error).count();
                    eprintln!(
                        "[{}/{total}] {g}/{m}: {} ({:.1}s){}",
                        i + 1,
                        r.status,
                        r.secs,
                        if errs > 0 { format!(" {errs} errors") } else { String::new() }
                    );
                    results.lock().unwrap().push(r);
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| (&a.game, &a.map).cmp(&(&b.game, &b.map)));
    if let Err(e) = write_reports(&o.out, &results) {
        eprintln!("qsmoke: writing report: {e}");
    }
    let bad = results.iter().filter(|r| r.issues.iter().any(|x| x.level == Level::Error)).count();
    eprintln!("qsmoke: {bad}/{} maps with errors; see {}/summary.txt", results.len(), o.out.display());
    std::process::exit(i32::from(bad > 0));
}
