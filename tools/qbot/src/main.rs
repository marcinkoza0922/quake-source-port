//! qbot: automated level traversal test (M7).
//!
//! Usage: qbot [-basedir <dir>] [-game <dir>] [-skill N] [-time <sim s>] [-j N]
//!             [-out <dir>] [-baseline <file>] [-save-baseline] [-routes <dir>]
//!             [-no-routes] [-hints DIR] [-no-hints] [-fresh] [-variants N] [-v] [map ...]
//!
//! Each map runs on an in-process headless server at skill 3 with god mode.
//! The bot paths to secrets, monsters, buttons and keys over a nav graph built
//! from the world hull, shoots what it sees and noclips to a goal when it gets
//! stuck (counted in the report). A run that finishes its level by game means
//! only is cached as a route (`<routes>/<map>.qbr`, default
//! `target/qbot/routes/<game>`) and replayed next time. Writes
//! `<out>/report.json`. With `-variants N`, maps the plain bot doesn't finish
//! cleanly are rerun with seeds 1..N-1 (other goal orders, routes and lift
//! patience) and the best run is reported and cached. With
//! `-baseline`, exits 1 if any map has errors or fewer kills/secrets than the
//! baseline; `-save-baseline` writes the current results as the new baseline.

mod bot;
mod hints;
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
    hints: Option<PathBuf>,
    fresh: bool,
    variants: u32,
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
        hints: None,
        fresh: false,
        variants: 1,
        maps: Vec::new(),
    };
    let mut no_routes = false;
    let mut no_hints = false;
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
            "-hints" => o.hints = Some(val().into()),
            "-no-hints" => no_hints = true,
            "-fresh" => o.fresh = true,
            "-variants" => o.variants = val().parse::<u32>().expect("-variants N").max(1),
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
    if no_hints {
        o.hints = None;
    } else if o.hints.is_none() {
        o.hints = Some(PathBuf::from(format!("tools/qbot/hints/{}", o.game)));
    }
    if o.baseline.is_none() {
        o.baseline = Some(PathBuf::from(format!("tools/qbot/baseline-{}.json", o.game)));
    }
    o
}

const USAGE: &str = "qbot [-basedir D] [-game G] [-skill N] [-time S] [-j N] [-out DIR] [-baseline F] [-save-baseline] [-routes DIR] [-no-routes] [-hints DIR] [-no-hints] [-fresh] [-variants N] [-v] [map ...]";

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
        "{{\"kills\": {}, \"total_monsters\": {}, \"secrets\": {}, \"total_secrets\": {}, \"noclip_uses\": {}, \"goals_failed\": {}, \"triggers_missed\": {}, \"monster_noclips\": {}, \"nav_nodes\": {}, \"has_exit\": {}, \"exited\": {}, \"stranded\": {}, \"route\": {}, \"seed\": {}, \"variants\": {}, \"clean_variants\": {}, \"sim_time\": {:.1}, \"wall_time\": {:.1}, \"errors\": [{}]}}",
        r.kills,
        r.total_monsters,
        r.secrets,
        r.total_secrets,
        r.noclip_uses,
        r.goals_failed,
        r.triggers_missed,
        r.monster_noclips,
        r.nav_nodes,
        r.has_exit,
        r.exited,
        r.stranded,
        json_str(&r.route),
        r.seed,
        r.variants,
        r.clean_variants,
        r.sim_time,
        r.wall_time,
        errs.join(", ")
    )
}

fn run_one(o: &Opts, map: &str, seed: u64) -> bot::Report {
    let opt = bot::Options { skill: o.skill, max_time: o.time, verbose: o.verbose, routes: o.routes.clone(), fresh: o.fresh, seed, hints: o.hints.clone() };
    let fs = mount(&o.basedir, &o.game);
    match catch_unwind(AssertUnwindSafe(|| bot::run(fs, map, &opt))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            bot::Report { errors: vec![format!("panic: {}", msg.unwrap_or_default())], ..Default::default() }
        }
    }
}

/// The plain bot's numbers for a map, which the baseline holds.
struct Plain {
    kills: i32,
    secrets: i32,
    exited: bool,
    noclips: u32,
}

/// Run (map, seed) jobs on `o.jobs` threads, printing each result if `print`.
fn run_jobs(o: &Opts, jobs: Vec<(String, u64)>, print: bool) -> Vec<(String, bot::Report)> {
    let next = Mutex::new(0usize);
    let results: Mutex<Vec<(String, bot::Report)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.min(jobs.len()) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        *n += 1;
                        *n - 1
                    };
                    let Some((map, seed)) = jobs.get(i) else { break };
                    let r = run_one(o, map, *seed);
                    if print {
                        let name = if *seed == 0 { map.clone() } else { format!("{map}#{seed}") };
                        eprintln!(
                            "{:<16} kills {:>3}/{:<3} secrets {:>2}/{:<2} noclip {:>2} ({:>2} monster) failed {:>2}{}{}{}{} ({:.0}s sim, {:.1}s wall)",
                            name,
                            r.kills,
                            r.total_monsters,
                            r.secrets,
                            r.total_secrets,
                            r.noclip_uses,
                            r.monster_noclips,
                            r.goals_failed,
                            if r.exited { " exit" } else if r.has_exit { " NO-EXIT" } else { "" },
                            if r.stranded { " STRANDED" } else { "" },
                            if r.route.is_empty() { String::new() } else { format!(" route {}", r.route) },
                            if r.errors.is_empty() { String::new() } else { format!(" ERRORS: {}", r.errors.join("; ")) },
                            r.sim_time,
                            r.wall_time
                        );
                    }
                    results.lock().unwrap().push((map.clone(), r));
                }
            });
        }
    });
    results.into_inner().unwrap()
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        eprintln!("qbot: no id1 in {}", o.basedir.display());
        std::process::exit(2);
    }
    let maps = if o.maps.is_empty() { maps_of(&o.basedir, &o.game) } else { o.maps.clone() };
    // Phase 1: the plain bot (seed 0) on every map; it replays cached routes.
    let first = run_jobs(&o, maps.iter().map(|m| (m.clone(), 0)).collect(), true);
    let mut by_map: std::collections::BTreeMap<String, Vec<bot::Report>> = std::collections::BTreeMap::new();
    for (m, r) in first {
        by_map.entry(m).or_default().push(r);
    }
    // Phase 2: maps the plain bot didn't finish cleanly get seeded variants.
    if o.variants > 1 {
        let jobs: Vec<(String, u64)> = by_map
            .iter()
            .filter(|(_, rs)| !rs[0].clean)
            .flat_map(|(m, _)| (1..o.variants as u64).map(move |seed| (m.clone(), seed)))
            .collect();
        for (m, r) in run_jobs(&o, jobs, o.verbose) {
            by_map.entry(m).or_default().push(r);
        }
    }
    let mut results: Vec<(String, bot::Report)> = Vec::new();
    // The baseline is about the plain bot (seed 0); variants are extra search.
    let mut plain: Vec<(String, Plain)> = Vec::new();
    for (m, mut rs) in by_map {
        let p0 = &rs[0];
        plain.push((m.clone(), Plain { kills: p0.kills, secrets: p0.secrets, exited: p0.exited, noclips: p0.noclip_uses }));
        let n = rs.len();
        let clean = rs.iter().filter(|r| r.clean).count();
        // Errors from any variant count against the map.
        let errors: Vec<String> =
            rs.iter().flat_map(|r| r.errors.iter().map(move |e| if r.seed == 0 { e.clone() } else { format!("seed {}: {e}", r.seed) })).collect();
        // Best variant: clean, then exit, fewest noclips, most kills+secrets, shortest.
        rs.sort_by(|a, b| {
            (b.clean, b.exited, std::cmp::Reverse(b.noclip_uses), b.kills + b.secrets)
                .cmp(&(a.clean, a.exited, std::cmp::Reverse(a.noclip_uses), a.kills + a.secrets))
                .then(a.sim_time.total_cmp(&b.sim_time))
        });
        let mut best = rs.swap_remove(0);
        best.errors = errors;
        best.variants = n as u32;
        best.clean_variants = clean as u32;
        if let (Some(rec), Some(dir)) = (best.recording.take(), &o.routes) {
            let p = bot::route_file(dir, &m);
            match rec.save(&p) {
                Ok(()) => best.route = format!("recorded (seed {})", best.seed),
                Err(e) => best.errors.push(format!("saving route {}: {e}", p.display())),
            }
        }
        if n > 1 {
            eprintln!(
                "{m:<16} best of {n}: seed {} kills {}/{} secrets {}/{} noclip {}{} ({clean} clean)",
                best.seed,
                best.kills,
                best.total_monsters,
                best.secrets,
                best.total_secrets,
                best.noclip_uses,
                if best.exited { " exit" } else { "" }
            );
        }
        results.push((m, best));
    }

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
        for (m, r) in &plain {
            entries.push((m.clone(), entry(r.kills as f64, r.secrets as f64, r.exited, r.noclips as f64)));
        }
        entries.sort();
        let body: Vec<String> = entries.iter().map(|(k, v)| format!("  {}: {v}", json_str(k))).collect();
        let _ = std::fs::write(base, format!("{{\n{}\n}}\n", body.join(",\n")));
        eprintln!("qbot: baseline written to {}", base.display());
    } else if let Some(b) = std::fs::read_to_string(base).ok().and_then(|t| qui::json::parse(&t)) {
        for (m, r) in &plain {
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
                && r.noclips as f64 > nc
            {
                soft.push(format!("{m}: {} noclips (was {nc})", r.noclips));
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
        let opt = bot::Options { skill: 3, max_time: None, verbose: false, routes: None, fresh: false, seed: 0, hints: None };
        let r = bot::run(mount(&base, "id1"), "e1m1", &opt);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.secrets + 1 >= r.total_secrets, "secrets {}/{}", r.secrets, r.total_secrets);
        assert!(r.kills * 10 >= r.total_monsters * 9, "kills {}/{}", r.kills, r.total_monsters);
        assert!(r.exited, "never reached the exit");
    }
}
