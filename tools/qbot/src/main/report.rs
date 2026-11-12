use super::*;

pub(super) fn report_json(r: &bot::Report) -> String {
    let errs: Vec<String> = r.errors.iter().map(|e| json_str(e)).collect();
    let assists = r.assists.iter().map(|a| format!(
        "{{\"time\":{:.2},\"cause\":{},\"goal\":{},\"kind\":{},\"classname\":{},\"reason\":{},\"from\":[{},{},{}],\"to\":[{},{},{}],\"door\":{}}}",
        a.time, json_str(a.cause.name()), a.goal.map_or_else(|| "null".into(), |e| e.to_string()),
        json_str(&a.kind), json_str(&a.classname), json_str(&a.reason), a.from.x, a.from.y, a.from.z, a.to.x, a.to.y, a.to.z,
        a.door.map_or_else(|| "null".into(), |e| e.to_string()),
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"kills\": {}, \"total_monsters\": {}, \"secrets\": {}, \"total_secrets\": {}, \"noclip_uses\": {}, \"goals_failed\": {}, \"triggers_missed\": {}, \"monster_noclips\": {}, \"exit_noclips\": {}, \"nav_nodes\": {}, \"jump_links\": {}, \"jumps_completed\": {}, \"jump_failures\": {}, \"lift_detours\": {}, \"prerequisite_actions\": {}, \"has_exit\": {}, \"exited\": {}, \"stranded\": {}, \"route\": {}, \"seed\": {}, \"variants\": {}, \"clean_variants\": {}, \"sim_time\": {:.1}, \"wall_time\": {:.1}, \"errors\": [{}], \"assists\": [{}]}}",
        r.kills,
        r.total_monsters,
        r.secrets,
        r.total_secrets,
        r.noclip_uses,
        r.goals_failed,
        r.triggers_missed,
        r.monster_noclips,
        r.exit_noclips,
        r.nav_nodes,
        r.jump_links,
        r.jumps_completed,
        r.jump_failures,
        r.lift_detours,
        r.prerequisite_actions,
        r.has_exit,
        r.exited,
        r.stranded,
        json_str(&r.route),
        r.seed,
        r.variants,
        r.clean_variants,
        r.sim_time,
        r.wall_time,
        errs.join(", "),
        assists
    )
}

pub(super) fn run_one(o: &Opts, map: &str, seed: u64) -> bot::Report {
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
pub(super) struct Plain {
    pub(super) kills: i32,
    pub(super) secrets: i32,
    pub(super) exited: bool,
    pub(super) noclips: u32,
}

/// Run (map, seed) jobs on `o.jobs` threads, printing each result if `print`.
pub(super) fn run_jobs(o: &Opts, jobs: Vec<(String, u64)>, print: bool) -> Vec<(String, bot::Report)> {
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
