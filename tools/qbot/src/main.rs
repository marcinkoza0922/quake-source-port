//! qbot: automated level-breaking test.
//!
//! Usage: qbot [-basedir <dir>] [-game <dir>] [-skill N] [-time <sim s>] [-j N]
//!             [-out <dir>] [-baseline <file>] [-save-baseline] [-v] [map ...]
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
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct Opts {
    basedir: PathBuf,
    game: String,
    skill: i32,
    time: Option<f64>,
    jobs: usize,
    out: PathBuf,
    baseline: PathBuf,
    save_baseline: bool,
    verbose: bool,
    maps: Vec<String>,
}

const USAGE: &str = "qbot [-basedir D] [-game G] [-skill N] [-time S] [-j N] [-out DIR] [-baseline F] [-save-baseline] [-v] [map ...]";

fn parse_args() -> Opts {
    let basedir = std::env::var_os("QUAKE_BASEDIR").map_or_else(|| PathBuf::from("vkQuake directory with maps"), PathBuf::from);
    let mut o = Opts {
        basedir,
        game: "id1".into(),
        skill: 3,
        time: None,
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
    o.baseline = baseline.unwrap_or_else(|| PathBuf::from(format!("tools/qbot/baseline-{}.json", o.game)));
    o
}

fn mount(basedir: &Path, game: &str) -> Fs {
    let mut fs = Fs::new(basedir);
    fs.add_game_dir("id1");
    if game != "id1" {
        fs.add_game_dir(game);
    }
    fs
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
        "{{\"kills\": {}, \"total_monsters\": {}, \"secrets\": {}, \"total_secrets\": {}, \"triggers\": {}, \"total_triggers\": {}, \"buttons\": {}, \"total_buttons\": {}, \"pickups\": {}, \"total_pickups\": {}, \"has_exit\": {}, \"exited\": {}, \"sim_time\": {:.1}, \"wall_time\": {:.1}, \"failed\": [{}], \"errors\": [{}]}}",
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
    )
}

fn run_one(o: &Opts, map: &str) -> bot::Report {
    let opt = bot::Options { skill: o.skill, max_time: o.time, verbose: o.verbose };
    let fs = mount(&o.basedir, &o.game);
    match catch_unwind(AssertUnwindSafe(|| bot::run(fs, map, &opt))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            bot::Report { errors: vec![format!("panic: {}", msg.unwrap_or_default())], ..Default::default() }
        }
    }
}

/// Run every map on `o.jobs` threads, printing a line per map.
fn run_jobs(o: &Opts, maps: &[String]) -> Vec<(String, bot::Report)> {
    let next = Mutex::new(0usize);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.min(maps.len()) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        *n += 1;
                        *n - 1
                    };
                    let Some(map) = maps.get(i) else { break };
                    let r = run_one(o, map);
                    eprintln!(
                        "{map:<16} kills {:>3}/{:<3} secrets {:>2}/{:<2} triggers {:>3}/{:<3} buttons {:>2}/{:<2} keys {}/{} failed {:>2}{}{} ({:.0}s sim, {:.1}s wall)",
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
                        if r.errors.is_empty() { String::new() } else { format!(" ERRORS: {}", r.errors.join("; ")) },
                        r.sim_time,
                        r.wall_time
                    );
                    results.lock().unwrap().push((map.clone(), r));
                }
            });
        }
    });
    let mut v = results.into_inner().unwrap();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn baseline_entry(kills: f64, secrets: f64, events: f64, exited: bool) -> String {
    format!("{{\"kills\": {kills}, \"secrets\": {secrets}, \"events\": {events}, \"exited\": {exited}}}")
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        eprintln!("qbot: no id1 in {}", o.basedir.display());
        std::process::exit(2);
    }
    let maps = if o.maps.is_empty() { maps_of(&o.basedir, &o.game) } else { o.maps.clone() };
    let results = run_jobs(&o, &maps);

    let mut j = String::from("{\n");
    for (i, (m, r)) in results.iter().enumerate() {
        let _ = writeln!(j, "  {}: {}{}", json_str(m), report_json(r), if i + 1 < results.len() { "," } else { "" });
    }
    j.push_str("}\n");
    let _ = std::fs::create_dir_all(&o.out);
    let out = o.out.join("report.json");
    if let Err(e) = std::fs::write(&out, &j) {
        eprintln!("qbot: writing {}: {e}", out.display());
    }

    let mut regressions: Vec<String> = results.iter().filter(|(_, r)| !r.errors.is_empty()).map(|(m, _)| format!("{m}: errors")).collect();
    let old = std::fs::read_to_string(&o.baseline).ok().and_then(|t| qui::json::parse(&t));
    if o.save_baseline {
        // Merge, so partial runs keep the other maps.
        let mut entries: Vec<(String, String)> = Vec::new();
        for (k, e) in old.as_ref().and_then(|b| b.as_obj()).unwrap_or_default() {
            if !results.iter().any(|(m, _)| m == k) {
                let n = |f: &str| e.get(f).and_then(|x| x.num()).unwrap_or(0.0);
                let exited = e.get("exited").and_then(|x| x.bool()).unwrap_or(false);
                entries.push((k.clone(), baseline_entry(n("kills"), n("secrets"), n("events"), exited)));
            }
        }
        for (m, r) in &results {
            entries.push((m.clone(), baseline_entry(r.kills as f64, r.secrets as f64, r.events() as f64, r.exited)));
        }
        entries.sort();
        let body: Vec<String> = entries.iter().map(|(k, v)| format!("  {}: {v}", json_str(k))).collect();
        let _ = std::fs::write(&o.baseline, format!("{{\n{}\n}}\n", body.join(",\n")));
        eprintln!("qbot: baseline written to {}", o.baseline.display());
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
        }
    }
    let failed: usize = results.iter().map(|(_, r)| r.failed.len()).sum();
    eprintln!("qbot: {} maps, {failed} goals failed, report {}", results.len(), out.display());
    if !regressions.is_empty() {
        for r in &regressions {
            eprintln!("REGRESSION {r}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        std::env::var_os("QUAKE_BASEDIR")
            .map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"), PathBuf::from)
    }

    #[test]
    fn report_json_escapes_strings() {
        let r = bot::Report { failed: vec!["Button custom\"button\n".into()], ..Default::default() };
        let j = qui::json::parse(&report_json(&r)).unwrap();
        let Some(qui::json::Value::Arr(f)) = j.get("failed") else { panic!("expected failed array") };
        assert_eq!(f[0], qui::json::Value::Str("Button custom\"button\n".into()));
    }

    /// e1m1 and e1m3 at skill 3: everything killed, found and triggered, and
    /// the exit taken.
    #[test]
    fn breaks_episode_one_maps() {
        let base = base();
        let fs = mount(&base, "id1");
        if fs.open("progs.dat").is_none() {
            eprintln!("no game data; skipping");
            return;
        }
        let opt = bot::Options { skill: 3, max_time: None, verbose: false };
        for map in ["e1m1", "e1m3"] {
            let r = bot::run(mount(&base, "id1"), map, &opt);
            assert!(r.errors.is_empty(), "{map}: {:?}", r.errors);
            assert_eq!(r.kills, r.total_monsters, "{map}: kills; failed {:?}", r.failed);
            assert_eq!(r.secrets, r.total_secrets, "{map}: secrets; failed {:?}", r.failed);
            assert!(r.exited, "{map}: never reached the exit");
        }
    }
}
