//! qbench's `report.json` and `report.md`.
//!
//! JSON: `{"machine", "demo_set", "runs", "mhruns", "output", "cells": [...]}`;
//! each cell is one (scene, profile, engine, resolution) with its fps runs
//! (no MangoHud), median/min/max, frame-time percentiles (median over the
//! MangoHud runs), the cold-start run's worst frame and frames over 33 ms,
//! and errors. `tools/qmcp` (`compare_qbench`) keys cells by scene,
//! profile, engine and resolution.

use crate::{Case, Cell, Cells, Opts, Res};
use perfkit::json_str;
use perfkit::machine::Machine;
use perfkit::stats::{self, Frames};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Median of each MangoHud run's (p50, p99, max).
fn percentiles(c: &Cell) -> Option<(f64, f64, f64)> {
    let f: Vec<Frames> = c.frame_ms.iter().filter_map(|m| Frames::of(m)).collect();
    let med = |pick: fn(&Frames) -> f64| stats::median(&f.iter().map(pick).collect::<Vec<_>>());
    Some((med(|x| x.p50)?, med(|x| x.p99)?, med(|x| x.max)?))
}

/// (worst frame ms, frames over 33.3 ms) of the cold-start run.
fn cold(c: &Cell) -> Option<(f64, usize)> {
    let ms = c.cold.as_ref()?;
    let worst = ms.iter().copied().fold(0.0f32, f32::max) as f64;
    Some((worst, ms.iter().filter(|&&x| x > 33.3).count()))
}

fn num(x: Option<f64>, digits: usize) -> String {
    x.map_or("null".into(), |v| format!("{v:.digits$}"))
}

pub fn write(o: &Opts, m: &Machine, demo_set: &str, cases: &[Case], cells: &Cells, changed: &[PathBuf]) -> (String, String) {
    (json(o, m, demo_set, cases, cells, changed), markdown(o, m, demo_set, cases, cells, changed))
}

fn json(o: &Opts, m: &Machine, demo_set: &str, cases: &[Case], cells: &Cells, changed: &[PathBuf]) -> String {
    let mut rows = Vec::new();
    for ((ci, engine, res), c) in cells {
        let case = &cases[*ci];
        let (w, h) = res.size();
        let p = percentiles(c);
        let cold = cold(c);
        let range = stats::range(&c.fps);
        rows.push(format!(
            "    {{\"suite\": \"{}\", \"scene\": \"{}\", \"game\": \"{}\", \"map\": \"{}\", \"profile\": \"{}\", \"engine\": \"{engine}\", \
             \"file\": \"{}\", \"width\": {w}, \"height\": {h}, \"display\": \"{}\", \"frames\": {}, \"runs\": {:?}, \
             \"fps_median\": {}, \"fps_min\": {}, \"fps_max\": {}, \"p50_ms\": {}, \"p99_ms\": {}, \"max_ms\": {}, \
             \"cold_worst_ms\": {}, \"cold_over_33ms\": {}, \"errors\": [{}]}}",
            case.suite, case.scene.name, case.scene.game, case.scene.map, case.profile.name(), c.file,
            if *res == Res::P540 { "window" } else { "gamescope" }, c.frames, c.fps,
            num(stats::median(&c.fps), 1), num(range.map(|r| r.0), 1), num(range.map(|r| r.1), 1),
            num(p.map(|p| p.0), 3), num(p.map(|p| p.1), 3), num(p.map(|p| p.2), 3),
            num(cold.map(|c| c.0), 1), cold.map_or("null".into(), |c| c.1.to_string()),
            c.errors.iter().map(|e| json_str(e)).collect::<Vec<_>>().join(", ")
        ));
    }
    let changed: Vec<String> = changed.iter().map(|p| json_str(&p.display().to_string())).collect();
    format!(
        "{{\n  \"machine\": {},\n  \"demo_set\": {},\n  \"runs\": {},\n  \"mhruns\": {},\n  \"output\": \"{}x{}\",\n  \
         \"configs_changed\": [{}],\n  \"cells\": [\n{}\n  ]\n}}\n",
        m.to_json(), json_str(demo_set), o.runs, o.mhruns, o.output.0, o.output.1, changed.join(", "), rows.join(",\n")
    )
}

fn markdown(o: &Opts, m: &Machine, demo_set: &str, cases: &[Case], cells: &Cells, changed: &[PathBuf]) -> String {
    let mut md = String::from("# qbench: Part 1, vanilla ultra\n\n");
    let _ = writeln!(md, "{}\n", m.summary());
    let _ = writeln!(
        md,
        "Demo set `{demo_set}`; {} fps runs + {} MangoHud runs per cell; 960×540 in a window, 3840×2160 in gamescope \
         scaled to {}×{}. fps are medians (min–max); p99 is the median of the MangoHud runs' p99 frame time. \
         **ref +x%** marks a reference faster than ours by more than 5%.\n",
        o.runs, o.mhruns, o.output.0, o.output.1
    );
    let engines = ["ours", "ironwail", "vkquake"];
    for suite in ["main", "fte"] {
        if !cases.iter().any(|c| c.suite == suite) {
            continue;
        }
        let _ = writeln!(md, "## {} suite\n", if suite == "main" { "Main (classic particles)" } else { "FTE particles" });
        for res in [Res::P540, Res::K4] {
            if !o.res.contains(&res) {
                continue;
            }
            let (w, h) = res.size();
            let _ = writeln!(md, "### {w}×{h}\n");
            let _ = writeln!(md, "| scene | ours | ironwail | vkQuake | ours vs fastest ref | p99 ms ours / iw / vk |");
            let _ = writeln!(md, "|---|--:|--:|--:|---|---|");
            for (ci, case) in cases.iter().enumerate().filter(|(_, c)| c.suite == suite) {
                let get = |e: &'static str| cells.get(&(ci, e, res));
                let med = |e: &'static str| get(e).and_then(|c| stats::median(&c.fps));
                let fps = |e: &'static str| match get(e) {
                    None => "–".to_string(),
                    Some(c) => match (stats::median(&c.fps), stats::range(&c.fps)) {
                        (Some(m), Some((lo, hi))) => format!("{m:.0} ({lo:.0}–{hi:.0}){}", if c.file == "full" { " ᶠ" } else { "" }),
                        _ => format!("failed ({})", c.errors.len()),
                    },
                };
                // Only references that played the same file as ours: in the FTE suite
                // ironwail plays the plain .dem without FTE particles, so it isn't comparable.
                let ours_file = get("ours").map(|c| c.file);
                let best_ref = ["ironwail", "vkquake"]
                    .iter()
                    .filter(|e| get(e).map(|c| c.file) == ours_file)
                    .filter_map(|e| med(e).map(|m| (m, *e)))
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                let vs = match (med("ours"), best_ref) {
                    (Some(ours), Some((r, name))) => {
                        let pct = (ours / r - 1.0) * 100.0;
                        if r > ours * 1.05 { format!("**ref +{:.0}%** ({name})", (r / ours - 1.0) * 100.0) } else { format!("{pct:+.0}%") }
                    }
                    _ => "–".into(),
                };
                let p99: Vec<String> = engines.iter().map(|e| get(e).and_then(percentiles).map_or("–".into(), |p| format!("{:.2}", p.1))).collect();
                let _ = writeln!(md, "| {} | {} | {} | {} | {vs} | {} |", case.scene.name, fps("ours"), fps("ironwail"), fps("vkquake"), p99.join(" / "));
            }
            md.push('\n');
        }
    }
    md.push_str("ᶠ = played the `.qsd` with AD's FTE particle messages.\n\n## Cold start (960×540, driver shader cache off)\n\n");
    md.push_str("| scene | profile | ours worst ms (>33 ms) | ironwail | vkQuake |\n|---|---|--:|--:|--:|\n");
    for (ci, case) in cases.iter().enumerate() {
        let c: Vec<String> = engines
            .iter()
            .map(|e| cells.get(&(ci, *e, Res::P540)).and_then(cold).map_or("–".into(), |(w, n)| format!("{w:.1} ({n})")))
            .collect();
        let _ = writeln!(md, "| {} | {} | {} |", case.scene.name, case.profile.name(), c.join(" | "));
    }
    let errors: Vec<String> = cells
        .iter()
        .flat_map(|((ci, e, res), c)| c.errors.iter().map(move |err| format!("- {} {} {:?}: {err}", cases[*ci].scene.name, e, res)))
        .collect();
    if !errors.is_empty() {
        let _ = writeln!(md, "\n## Errors\n\n{}", errors.join("\n"));
    }
    if !changed.is_empty() {
        let _ = writeln!(md, "\n**Configs changed during the run (restored):** {changed:?}");
    }
    md
}
