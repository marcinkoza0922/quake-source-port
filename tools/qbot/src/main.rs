//! qbot: automated level-breaking test.
//!
//! Usage: qbot [-basedir <dir>] [-game <dir>|a,b,..|flagged|all] [-skill N] [-time <sim s>]
//!             [-maxwall <wall s>] [-j N] [-out <dir>] [-baseline <file>]
//!             [-save-baseline] [-v] [map ...]
//!
//! `-game flagged` runs every mod that has ever flagged an issue
//! (`tools/qbot/flagged.txt`; those not installed are skipped) against its
//! baseline. `-game all` runs every installed game dir (each with the mission-pack
//! base the mod menu would mount), all maps sharing the worker threads;
//! results go to `<out>/<game>/report.json` plus `<out>/summary.txt`, which
//! lists engine and QC errors first. Baselines stay per game. While it runs,
//! `<out>/progress.jsonl` gets a line per finished map and `<out>/running.txt`
//! lists the maps in progress; `-maxwall` stops a map after that many seconds.
//!
//! Each map runs on an in-process headless server at skill 3. The bot has god
//! mode, noclip and full knowledge of the edicts: it kills every monster,
//! finds every secret, sets off every trigger and button, picks up every key
//! and finally touches the exit, so the level's scripting all runs. Writes
//! `<out>/report.json`. Exits 1 if any map hits an engine error or does worse
//! than the baseline (kills, secrets, events, exit); `-save-baseline` merges
//! the current results into it.

mod bot;

use qcore::fs::Fs;
use std::fmt::Write as _;
use std::io::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct Opts {
    basedir: PathBuf,
    game: String,
    skill: i32,
    time: Option<f64>,
    max_wall: Option<f64>,
    jobs: usize,
    out: PathBuf,
    baseline: PathBuf,
    save_baseline: bool,
    verbose: bool,
    maps: Vec<String>,
}

const USAGE: &str = "qbot [-basedir D] [-game G|a,b|flagged|all] [-skill N] [-time S] [-maxwall S] [-j N] [-out DIR] [-baseline F] [-save-baseline] [-v] [map ...]";

fn parse_args() -> Opts {
    let basedir = std::env::var_os("QUAKE_BASEDIR").map_or_else(|| PathBuf::from("vkQuake directory with maps"), PathBuf::from);
    let mut o = Opts {
        basedir,
        game: "id1".into(),
        skill: 3,
        time: None,
        max_wall: None,
        jobs: std::thread::available_parallelism().map_or(4, |n| n.get()),
        out: PathBuf::from("target/qbot"),
        baseline: PathBuf::new(),
        save_baseline: false,
        verbose: false,
        maps: Vec::new(),
    };
    let mut baseline = None;
    let mut a = std::env::args().skip(1);
    while let Some(x) = a.next() {
        let mut val = || a.next().unwrap_or_else(|| panic!("{x} needs a value"));
        match x.as_str() {
            "-basedir" => o.basedir = val().into(),
            "-game" => o.game = val(),
            "-skill" => o.skill = val().parse().expect("-skill N"),
            "-time" => o.time = Some(val().parse().expect("-time S")),
            "-maxwall" => o.max_wall = Some(val().parse().expect("-maxwall S")),
            "-j" => o.jobs = val().parse::<usize>().expect("-j N").max(1),
            "-out" => o.out = val().into(),
            "-baseline" => baseline = Some(PathBuf::from(val())),
            "-save-baseline" => o.save_baseline = true,
            "-v" => o.verbose = true,
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                std::process::exit(0);
            }
            m => o.maps.push(m.to_string()),
        }
    }
    o.baseline = baseline.unwrap_or_default();
    o
}

/// Game dirs below id1 that `game` mounts, as the mod menu would (`qui::mods`:
/// the dir name or modinfo.txt `base`, which may be any game dir).
pub(crate) fn mod_dirs(basedir: &Path, game: &str) -> Vec<String> {
    match qui::mods::scan(basedir).into_iter().find(|m| m.dir.eq_ignore_ascii_case(game)) {
        Some(m) => m.game_dirs(),
        None if game.eq_ignore_ascii_case("id1") => Vec::new(),
        None => vec![game.to_string()],
    }
}

fn mount(basedir: &Path, game: &str) -> Fs {
    let mut fs = Fs::new(basedir);
    fs.add_game_dir("id1");
    for d in mod_dirs(basedir, game) {
        fs.add_game_dir(&d);
    }
    fs
}

/// Mods that have flagged an issue: `dir  reason` lines.
const FLAGGED: &str = include_str!("../flagged.txt");

fn flagged() -> impl Iterator<Item = &'static str> {
    FLAGGED.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_whitespace().next())
}

/// Game dirs for `-game`: one, a comma list, `flagged`, or `all` installed.
fn games_of(basedir: &Path, arg: &str) -> Vec<String> {
    if arg.eq_ignore_ascii_case("flagged") {
        return flagged().filter(|g| basedir.join(g).is_dir()).map(str::to_string).collect();
    }
    if arg.eq_ignore_ascii_case("all") {
        return qui::mods::scan(basedir).into_iter().map(|m| m.dir).collect();
    }
    arg.split(',').filter(|g| !g.is_empty()).map(str::to_string).collect()
}

/// Playable maps owned by `game` (those with a spawn point).
fn maps_of(basedir: &Path, game: &str) -> Vec<String> {
    let fs = mount(basedir, game);
    let mut v: Vec<String> = fs
        .list("maps/", ".bsp")
        .into_iter()
        .filter(|p| fs.origin(p).is_some_and(|o| o.game.eq_ignore_ascii_case(game)))
        .filter_map(|p| {
            let n = p.strip_prefix("maps/").unwrap_or(&p).strip_suffix(".bsp")?.to_string();
            let playable = fs.open(&p).is_some_and(|d| d.windows(17).any(|w| w.eq_ignore_ascii_case(b"info_player_start")));
            (!n.starts_with("b_") && !n.contains('/') && playable).then_some(n)
        })
        .collect();
    v.sort();
    v
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn report_json(r: &bot::Report) -> String {
    let list = |v: &[String]| v.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(", ");
    format!(
        "{{\"wall_capped\": {}, \"kills\": {}, \"total_monsters\": {}, \"secrets\": {}, \"total_secrets\": {}, \"triggers\": {}, \"total_triggers\": {}, \"buttons\": {}, \"total_buttons\": {}, \"pickups\": {}, \"total_pickups\": {}, \"has_exit\": {}, \"exited\": {}, \"sim_time\": {:.1}, \"wall_time\": {:.1}, \"failed\": [{}], \"errors\": [{}], \"unusual\": [{}]}}",
        r.wall_capped,
        r.kills,
        r.total_monsters,
        r.secrets,
        r.total_secrets,
        r.triggers,
        r.total_triggers,
        r.buttons,
        r.total_buttons,
        r.pickups,
        r.total_pickups,
        r.has_exit,
        r.exited,
        r.sim_time,
        r.wall_time,
        list(&r.failed),
        list(&r.errors),
        list(&r.unusual),
    )
}

fn run_one(o: &Opts, game: &str, map: &str) -> bot::Report {
    let opt = bot::Options { skill: o.skill, max_time: o.time, max_wall: o.max_wall, verbose: o.verbose };
    let fs = mount(&o.basedir, game);
    match catch_unwind(AssertUnwindSafe(|| bot::run(fs, map, &opt))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            bot::Report { errors: vec![format!("panic: {}", msg.unwrap_or_default())], ..Default::default() }
        }
    }
}

fn log_texts(r: &bot::Report) -> Vec<String> {
    r.unusual.iter().map(|l| bot::devlog::text(l).to_string()).collect()
}

/// Run every (game, map) on `o.jobs` threads, printing a line per map.
fn run_jobs(o: &Opts, maps: &[(String, String)]) -> Vec<(String, String, bot::Report)> {
    let next = Mutex::new(0usize);
    let results = Mutex::new(Vec::new());
    // Written as the sweep goes, so a long run can be watched (and a killed
    // one keeps what it finished): one JSON line per finished map, and the
    // maps in progress.
    let _ = std::fs::create_dir_all(&o.out);
    let progress = o.out.join("progress.jsonl");
    let _ = std::fs::write(&progress, "");
    let running: Mutex<Vec<(String, std::time::Instant)>> = Mutex::new(Vec::new());
    let show_running = |r: &[(String, std::time::Instant)]| {
        let body: String = r.iter().map(|(m, t)| format!("{m}  {:.0}s\n", t.elapsed().as_secs_f64())).collect();
        let _ = std::fs::write(o.out.join("running.txt"), body);
    };
    std::thread::scope(|s| {
        for _ in 0..o.jobs.min(maps.len()) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        *n += 1;
                        *n - 1
                    };
                    let Some((game, map)) = maps.get(i) else { break };
                    let label = if game == &o.game { map.clone() } else { format!("{game}/{map}") };
                    {
                        let mut r = running.lock().unwrap();
                        r.push((label.clone(), std::time::Instant::now()));
                        show_running(&r);
                    }
                    let r = run_one(o, game, map);
                    {
                        let mut run = running.lock().unwrap();
                        run.retain(|(m, _)| m != &label);
                        show_running(&run);
                    }
                    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&progress) {
                        let _ = writeln!(f, "{{\"game\": {}, \"map\": {}, \"report\": {}}}", json_str(game), json_str(map), report_json(&r));
                    }
                    eprintln!(
                        "{label:<16} kills {:>3}/{:<3} secrets {:>2}/{:<2} triggers {:>3}/{:<3} buttons {:>2}/{:<2} keys {}/{} failed {:>2}{}{}{} ({:.0}s sim, {:.1}s wall)",
                        r.kills,
                        r.total_monsters,
                        r.secrets,
                        r.total_secrets,
                        r.triggers,
                        r.total_triggers,
                        r.buttons,
                        r.total_buttons,
                        r.pickups,
                        r.total_pickups,
                        r.failed.len(),
                        if r.exited { " exit" } else if r.has_exit { " NO-EXIT" } else { "" },
                        if r.wall_capped { " WALL-CAP" } else { "" },
                        if r.errors.is_empty() { String::new() } else { format!(" ERRORS: {}", r.errors.join("; ")) },
                        r.sim_time,
                        r.wall_time
                    );
                    results.lock().unwrap().push((game.clone(), map.clone(), r));
                }
            });
        }
    });
    let mut v = results.into_inner().unwrap();
    v.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    v
}

/// Developer-log lines kept per map. A map that prints more has them left out
/// of its baseline (`null`), and new ones aren't looked for.
const MAX_LOG: usize = 400;

fn baseline_entry(kills: f64, secrets: f64, events: f64, exited: bool, errors: &[String], log: Option<&[String]>) -> String {
    let list = |v: &[String]| format!("[{}]", v.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(", "));
    let errors = if errors.is_empty() { String::new() } else { format!("\"errors\": {}, ", list(errors)) };
    let log = log.map_or("null".to_string(), list);
    format!("{{\"kills\": {kills}, \"secrets\": {secrets}, \"events\": {events}, \"exited\": {exited}, {errors}\"log\": {log}}}")
}

fn strings(e: &qui::json::Value, key: &str) -> Option<Vec<String>> {
    match e.get(key) {
        Some(qui::json::Value::Arr(a)) => Some(a.iter().filter_map(|x| if let qui::json::Value::Str(s) = x { Some(s.clone()) } else { None }).collect()),
        _ => None,
    }
}

/// The developer-log lines a baseline entry already knows about; `None` when
/// the map prints too many to keep.
fn baseline_log(e: &qui::json::Value) -> Option<Vec<String>> {
    strings(e, "log").or_else(|| e.get("log").is_none().then(Vec::new))
}

/// Errors a baseline entry knows (the mod's own QC or missing files).
fn baseline_errors(e: &qui::json::Value) -> Vec<String> {
    strings(e, "errors").unwrap_or_default()
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        eprintln!("qbot: no id1 in {}", o.basedir.display());
        std::process::exit(2);
    }
    let games = games_of(&o.basedir, &o.game);
    let multi = games.len() > 1;
    if multi && !o.maps.is_empty() {
        eprintln!("qbot: map names need a single -game");
        std::process::exit(2);
    }
    let mut work = Vec::new();
    for g in &games {
        let maps = if o.maps.is_empty() { maps_of(&o.basedir, g) } else { o.maps.clone() };
        work.extend(maps.into_iter().map(|m| (g.clone(), m)));
    }
    eprintln!("qbot: {} maps in {} game dir(s)", work.len(), games.len());
    let all = run_jobs(&o, &work);
    let mut any_regression = false;
    for g in &games {
        let results: Vec<(String, &bot::Report)> = all.iter().filter(|(gg, _, _)| gg == g).map(|(_, m, r)| (m.clone(), r)).collect();
        let out_dir = if multi { o.out.join(g) } else { o.out.clone() };
        let baseline = if o.baseline.as_os_str().is_empty() || multi { PathBuf::from(format!("tools/qbot/baseline-{g}.json")) } else { o.baseline.clone() };
        any_regression |= report_game(&o, g, multi, &out_dir, &baseline, &results);
    }
    if multi {
        write_summary(&o.out, &all);
    }
    if any_regression {
        std::process::exit(1);
    }
}

/// `<out>/summary.txt`: per game totals, then every engine/QC error and
/// unusual developer-log line, which is what a sweep over all packs is for.
fn write_summary(out: &Path, all: &[(String, String, bot::Report)]) {
    let mut s = String::new();
    let mut games: Vec<&str> = all.iter().map(|(g, _, _)| g.as_str()).collect();
    games.dedup();
    let errors: Vec<_> = all.iter().filter(|(_, _, r)| !r.errors.is_empty()).collect();
    let _ = writeln!(s, "{} maps in {} game dirs; {} maps with engine/QC errors\n", all.len(), games.len(), errors.len());
    let _ = writeln!(s, "{:<24} {:>5} {:>11} {:>9} {:>7} {:>6}", "game", "maps", "kills", "secrets", "exits", "errors");
    for g in &games {
        let rs: Vec<&bot::Report> = all.iter().filter(|(gg, _, _)| gg == g).map(|(_, _, r)| r).collect();
        let sum = |f: fn(&bot::Report) -> i32| rs.iter().map(|r| i64::from(f(r))).sum::<i64>();
        let _ = writeln!(
            s,
            "{g:<24} {:>5} {:>5}/{:<5} {:>4}/{:<4} {:>3}/{:<3} {:>6}",
            rs.len(),
            sum(|r| r.kills),
            sum(|r| r.total_monsters),
            sum(|r| r.secrets),
            sum(|r| r.total_secrets),
            rs.iter().filter(|r| r.exited).count(),
            rs.iter().filter(|r| r.has_exit).count(),
            rs.iter().filter(|r| !r.errors.is_empty()).count()
        );
    }
    let _ = writeln!(s, "\n## Engine / QC errors");
    for (g, m, r) in &errors {
        for e in &r.errors {
            let _ = writeln!(s, "{g}/{m}: {e}");
        }
    }
    let _ = writeln!(s, "\n## Stopped by -maxwall (results partial)");
    for (g, m, r) in all.iter().filter(|(_, _, r)| r.wall_capped) {
        let _ = writeln!(s, "{g}/{m}: {:.0}s wall, {:.0}s sim, kills {}/{}", r.wall_time, r.sim_time, r.kills, r.total_monsters);
    }
    let _ = writeln!(s, "\n## Unusual developer-log lines");
    for (g, m, r) in all {
        for l in &r.unusual {
            let _ = writeln!(s, "{g}/{m}: {l}");
        }
    }
    let _ = std::fs::create_dir_all(out);
    let p = out.join("summary.txt");
    if let Err(e) = std::fs::write(&p, s) {
        eprintln!("qbot: writing {}: {e}", p.display());
    } else {
        eprintln!("qbot: summary {}", p.display());
    }
}

/// Writes `<out_dir>/report.json` and traces, compares with (or saves) the
/// baseline; true when something regressed.
fn report_game(o: &Opts, game: &str, multi: bool, out_dir: &Path, baseline: &Path, results: &[(String, &bot::Report)]) -> bool {
    let mut j = String::from("{\n");
    for (i, (m, r)) in results.iter().enumerate() {
        let _ = writeln!(j, "  {}: {}{}", json_str(m), report_json(r), if i + 1 < results.len() { "," } else { "" });
    }
    j.push_str("}\n");
    let _ = std::fs::create_dir_all(out_dir);
    let out = out_dir.join("report.json");
    if let Err(e) = std::fs::write(&out, &j) {
        eprintln!("qbot: writing {}: {e}", out.display());
    }
    let traces = out_dir.join("trace");
    let _ = std::fs::create_dir_all(&traces);
    for (m, r) in results {
        let mut body = r.trace.join("\n");
        body.push('\n');
        if let Err(e) = std::fs::write(traces.join(format!("{m}.log")), body) {
            eprintln!("qbot: writing trace for {m}: {e}");
        }
    }

    let old = std::fs::read_to_string(baseline).ok().and_then(|t| qui::json::parse(&t));
    let known = |m: &str| old.as_ref().and_then(|b| b.get(m)).map(baseline_errors).unwrap_or_default();
    let mut regressions: Vec<String> = results
        .iter()
        .filter(|(m, r)| { let k = known(m); r.errors.iter().any(|e| !k.contains(e)) })
        .map(|(m, _)| format!("{m}: errors"))
        .collect();
    // how far a map stopped by -maxwall got depends on the machine
    let all = results;
    let results: Vec<&(String, &bot::Report)> = results.iter().filter(|(_, r)| !r.wall_capped).collect();
    if o.save_baseline {
        // Merge, so partial runs keep the other maps.
        let mut entries: Vec<(String, String)> = Vec::new();
        for (k, e) in old.as_ref().and_then(|b| b.as_obj()).unwrap_or_default() {
            if !results.iter().any(|(m, _)| m == k) {
                let n = |f: &str| e.get(f).and_then(|x| x.num()).unwrap_or(0.0);
                let exited = e.get("exited").and_then(|x| x.bool()).unwrap_or(false);
                entries.push((k.clone(), baseline_entry(n("kills"), n("secrets"), n("events"), exited, &baseline_errors(e), baseline_log(e).as_deref())));
            }
        }
        for (m, r) in &results {
            let log = log_texts(r);
            let log = (log.len() <= MAX_LOG).then_some(log.as_slice());
            entries.push((m.clone(), baseline_entry(r.kills as f64, r.secrets as f64, r.events() as f64, r.exited, &r.errors, log)));
        }
        entries.sort();
        let body: Vec<String> = entries.iter().map(|(k, v)| format!("  {}: {v}", json_str(k))).collect();
        let _ = std::fs::write(baseline, format!("{{\n{}\n}}\n", body.join(",\n")));
        eprintln!("qbot: baseline written to {}", baseline.display());
    } else if let Some(b) = old {
        for (m, r) in &results {
            let Some(e) = b.get(m) else { continue };
            let n = |f: &str| e.get(f).and_then(|x| x.num()).unwrap_or(0.0) as i64;
            let (kills, secrets, events) = (n("kills"), n("secrets"), n("events"));
            if (r.kills as i64) < kills || (r.secrets as i64) < secrets || (r.events() as i64) < events {
                regressions.push(format!(
                    "{m}: kills {} (was {kills}), secrets {} (was {secrets}), events {} (was {events})",
                    r.kills,
                    r.secrets,
                    r.events()
                ));
            }
            if e.get("exited").and_then(|x| x.bool()) == Some(true) && !r.exited {
                regressions.push(format!("{m}: no longer reaches the exit"));
            }
            let Some(known) = baseline_log(e) else { continue };
            for l in log_texts(r).into_iter().filter(|l| !known.contains(l)) {
                eprintln!("NEW-LOG {m}: {l}");
                regressions.push(format!("{m}: new developer log line"));
            }
        }
    }
    let failed: usize = all.iter().map(|(_, r)| r.failed.len()).sum();
    eprintln!("qbot: {} maps, {failed} goals failed, report {}", all.len(), out.display());
    for r in &regressions {
        eprintln!("REGRESSION {}{r}", if multi { format!("{game}/") } else { String::new() });
    }
    !regressions.is_empty()
}

#[cfg(test)]
mod tests;
