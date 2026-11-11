//! qbot: automated level traversal test (M7).
//!
//! Usage: qbot [-basedir <dir>] [-game <dir>] [-skill N] [-time <sim s>] [-j N]
//!             [-out <dir>] [-baseline <file>] [-save-baseline] [-routes <dir>]
//!             [-no-routes] [-fresh] [-v] [map ...]
//!
//! Each map runs on an in-process headless server at skill 3 with god mode.
//! The bot paths to secrets, monsters, buttons and keys over a nav graph built
//! from the world hull, shoots what it sees and noclips to a goal when it gets
//! stuck (counted in the report). A run that finishes its level by game means
//! only is cached as a route (`<routes>/<map>.qbr`, default
//! `target/qbot/routes/<game>`) and replayed next time. Writes
//! `<out>/report.json`. With
//! `-baseline`, exits 1 if any map has errors or fewer kills/secrets than the
//! baseline; `-save-baseline` writes the current results as the new baseline.

mod bot;
mod nav;
mod route;

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
    baseline: Option<PathBuf>,
    save_baseline: bool,
    verbose: bool,
    routes: Option<PathBuf>,
    fresh: bool,
    maps: Vec<String>,
}

fn parse_args() -> Opts {
    let basedir = std::env::var_os("QUAKE_BASEDIR").map_or_else(|| PathBuf::from("vkQuake directory with maps"), PathBuf::from);
    let mut o = Opts {
        basedir,
        game: "id1".into(),
        skill: 3,
        time: None,
        jobs: std::thread::available_parallelism().map_or(4, |n| n.get()),
        out: PathBuf::from("target/qbot"),
        baseline: None,
        save_baseline: false,
        verbose: false,
        routes: None,
        fresh: false,
        maps: Vec::new(),
    };
    let mut no_routes = false;
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
            "-baseline" => o.baseline = Some(val().into()),
            "-save-baseline" => o.save_baseline = true,
            "-v" => o.verbose = true,
            "-routes" => o.routes = Some(val().into()),
            "-no-routes" => no_routes = true,
            "-fresh" => o.fresh = true,
            "-h" | "--help" => {
                eprintln!("{}", USAGE);
                std::process::exit(0);
            }
            m => o.maps.push(m.to_string()),
        }
    }
    if no_routes {
        o.routes = None;
    } else if o.routes.is_none() {
        o.routes = Some(PathBuf::from(format!("target/qbot/routes/{}", o.game)));
    }
    if o.baseline.is_none() {
        o.baseline = Some(PathBuf::from(format!("tools/qbot/baseline-{}.json", o.game)));
    }
    o
}

const USAGE: &str = "qbot [-basedir D] [-game G] [-skill N] [-time S] [-j N] [-out DIR] [-baseline F] [-save-baseline] [-routes DIR] [-no-routes] [-fresh] [-v] [map ...]";

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
    let errs: Vec<String> = r.errors.iter().map(|e| json_str(e)).collect();
    format!(
        "{{\"kills\": {}, \"total_monsters\": {}, \"secrets\": {}, \"total_secrets\": {}, \"noclip_uses\": {}, \"goals_failed\": {}, \"triggers_missed\": {}, \"nav_nodes\": {}, \"has_exit\": {}, \"exited\": {}, \"stranded\": {}, \"route\": {}, \"sim_time\": {:.1}, \"wall_time\": {:.1}, \"errors\": [{}]}}",
        r.kills,
        r.total_monsters,
        r.secrets,
        r.total_secrets,
        r.noclip_uses,
        r.goals_failed,
        r.triggers_missed,
        r.nav_nodes,
        r.has_exit,
        r.exited,
        r.stranded,
        json_str(&r.route),
        r.sim_time,
        r.wall_time,
        errs.join(", ")
    )
}

fn run_one(o: &Opts, map: &str) -> bot::Report {
    let opt = bot::Options { skill: o.skill, max_time: o.time, verbose: o.verbose, routes: o.routes.clone(), fresh: o.fresh };
    let fs = mount(&o.basedir, &o.game);
    match catch_unwind(AssertUnwindSafe(|| bot::run(fs, map, &opt))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            bot::Report { errors: vec![format!("panic: {}", msg.unwrap_or_default())], ..Default::default() }
        }
    }
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        eprintln!("qbot: no id1 in {}", o.basedir.display());
        std::process::exit(2);
    }
    let maps = if o.maps.is_empty() { maps_of(&o.basedir, &o.game) } else { o.maps.clone() };
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<(String, bot::Report)>> = Mutex::new(Vec::new());
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
                    let r = run_one(&o, map);
                    eprintln!(
                        "{:<16} kills {:>3}/{:<3} secrets {:>2}/{:<2} noclip {:>2} failed {:>2}{}{}{}{} ({:.0}s sim, {:.1}s wall)",
                        map,
                        r.kills,
                        r.total_monsters,
                        r.secrets,
                        r.total_secrets,
                        r.noclip_uses,
                        r.goals_failed,
                        if r.exited { " exit" } else if r.has_exit { " NO-EXIT" } else { "" },
                        if r.stranded { " STRANDED" } else { "" },
                        if r.route.is_empty() { String::new() } else { format!(" route {}", r.route) },
                        if r.errors.is_empty() { String::new() } else { format!(" ERRORS: {}", r.errors.join("; ")) },
                        r.sim_time,
                        r.wall_time
                    );
                    results.lock().unwrap().push((map.clone(), r));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));

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

    let mut regressions = Vec::new();
    for (m, r) in &results {
        if !r.errors.is_empty() {
            regressions.push(format!("{m}: errors"));
        }
    }
    let base = o.baseline.as_ref().unwrap();
    let entry = |kills: f64, secrets: f64, exited: bool, noclips: f64| {
        format!("{{\"kills\": {kills}, \"secrets\": {secrets}, \"exited\": {exited}, \"noclips\": {noclips}}}")
    };
    let mut soft = Vec::new();
    if o.save_baseline {
        // Merge into the existing baseline so partial runs keep other maps.
        let mut entries: Vec<(String, String)> = Vec::new();
        if let Some(old) = std::fs::read_to_string(base).ok().and_then(|t| qui::json::parse(&t)) {
            for (k, v) in old.as_obj().unwrap_or_default() {
                if !results.iter().any(|(m, _)| m == k) {
                    let n = |f: &str| v.get(f).and_then(|x| x.num()).unwrap_or(0.0);
                    let exited = v.get("exited").and_then(|x| x.bool()).unwrap_or(false);
                    entries.push((k.clone(), entry(n("kills"), n("secrets"), exited, n("noclips"))));
                }
            }
        }
        for (m, r) in &results {
            entries.push((m.clone(), entry(r.kills as f64, r.secrets as f64, r.exited, r.noclip_uses as f64)));
        }
        entries.sort();
        let body: Vec<String> = entries.iter().map(|(k, v)| format!("  {}: {v}", json_str(k))).collect();
        let _ = std::fs::write(base, format!("{{\n{}\n}}\n", body.join(",\n")));
        eprintln!("qbot: baseline written to {}", base.display());
    } else if let Some(b) = std::fs::read_to_string(base).ok().and_then(|t| qui::json::parse(&t)) {
        for (m, r) in &results {
            let Some(e) = b.get(m) else { continue };
            let n = |f: &str| e.get(f).and_then(|x| x.num()).unwrap_or(0.0);
            let (kills, secrets) = (n("kills") as i32, n("secrets") as i32);
            if r.kills < kills || r.secrets < secrets {
                regressions.push(format!("{m}: kills {} (was {kills}), secrets {} (was {secrets})", r.kills, r.secrets));
            }
            if e.get("exited").and_then(|x| x.bool()) == Some(true) && !r.exited {
                regressions.push(format!("{m}: no longer reaches the exit"));
            }
            // More noclips is a soft failure: reported, not fatal.
            if let Some(nc) = e.get("noclips").and_then(|x| x.num())
                && r.noclip_uses as f64 > nc
            {
                soft.push(format!("{m}: {} noclips (was {nc})", r.noclip_uses));
            }
        }
    }
    for s in &soft {
        eprintln!("SOFT {s}");
    }
    eprintln!("qbot: {} maps, report {}", results.len(), out.display());
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

    /// e1m1 at skill 3: the bot should kill nearly every monster, find most
    /// secrets and leave through the exit.
    #[test]
    fn e1m1_traversal() {
        let base = std::env::var_os("QUAKE_BASEDIR").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"),
            PathBuf::from,
        );
        if !base.join("id1").is_dir() {
            eprintln!("no game data; skipping");
            return;
        }
        let opt = bot::Options { skill: 3, max_time: None, verbose: false, routes: None, fresh: false };
        let r = bot::run(mount(&base, "id1"), "e1m1", &opt);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.secrets + 1 >= r.total_secrets, "secrets {}/{}", r.secrets, r.total_secrets);
        assert!(r.kills * 10 >= r.total_monsters * 9, "kills {}/{}", r.kills, r.total_monsters);
        assert!(r.exited, "never reached the exit");
    }
}
