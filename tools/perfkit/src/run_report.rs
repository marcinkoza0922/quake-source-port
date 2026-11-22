//! `perfkit run`'s report: every run's numbers, then each feature's cost
//! over its scene's off run (`docs/perf/PLAN.md`, Metrics).

use crate::run_cmd::{Cell, Opts, HITCH_MS, WARM_FRAMES};
use crate::run_keys::Key;
use perfkit::json_str;
use perfkit::machine::Machine;
use perfkit::plan::{PlanScene, Variant};
use perfkit::stats::{self, Frames};
use std::fmt::Write as _;
use std::path::PathBuf;

pub struct Report<'a> {
    pub o: &'a Opts,
    pub machine: &'a Machine,
    pub keys: &'a [Key],
    pub uses: &'a [(&'static PlanScene, Variant, usize)],
    pub cells: &'a [Vec<Cell>],
    pub skipped: &'a [(&'static PlanScene, String)],
    pub changed: &'a [PathBuf],
}

/// Medians of one cell.
#[derive(Clone, Copy, Default)]
pub struct Summary {
    pub runs: usize,
    pub fps: Option<f64>,
    pub mean_ms: Option<f64>,
    /// Max minus min of the per-run mean frame times.
    pub spread_ms: Option<f64>,
    pub p50: Option<f64>,
    pub p99: Option<f64>,
    pub max: Option<f64>,
    pub hitches: Option<f64>,
    pub gpu_us: Option<f64>,
}

fn med(v: impl Iterator<Item = f64>) -> Option<f64> {
    stats::median(&v.collect::<Vec<_>>())
}

pub fn summary(c: &Cell) -> Summary {
    let means: Vec<f64> = c.samples.iter().map(|s| s.mean_ms()).collect();
    let fr = |f: fn(&Frames) -> f64| med(c.samples.iter().filter_map(|s| s.frames.as_ref().map(f)));
    Summary {
        runs: c.samples.len(),
        fps: med(c.samples.iter().map(|s| s.fps)),
        mean_ms: stats::median(&means),
        spread_ms: stats::range(&means).map(|(a, b)| b - a),
        p50: fr(|f| f.p50),
        p99: fr(|f| f.p99),
        max: fr(|f| f.max),
        hitches: med(c.samples.iter().map(|s| s.hitches as f64)),
        gpu_us: med(c.samples.iter().filter_map(|s| s.gpu_us)).or(c.gpu_us),
    }
}

/// A feature run's cost over its off run: µs per frame, % of the off
/// frame, and whether it passes the plan's rule (over 3% and over the
/// larger spread).
pub fn cost(off: &Cell, on: &Cell) -> Option<(f64, f64, bool)> {
    let a: Vec<f64> = off.samples.iter().map(|s| s.mean_ms()).collect();
    let b: Vec<f64> = on.samples.iter().map(|s| s.mean_ms()).collect();
    let (ma, mb) = (stats::median(&a)?, stats::median(&b)?);
    // One run has no spread, so the rule can't apply.
    let sig = a.len() > 1 && b.len() > 1 && stats::significant(&a, &b, 3.0);
    Some(((mb - ma) * 1000.0, (mb / ma - 1.0) * 100.0, sig))
}

fn f(x: Option<f64>, d: usize) -> String {
    x.map_or("–".into(), |v| format!("{v:.d$}"))
}

fn j(x: Option<f64>) -> String {
    x.map_or("null".into(), |v| format!("{v:.4}"))
}

impl Report<'_> {
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        let _ = writeln!(md, "# perfkit run ({})\n\n{}\n", if self.o.full { "full" } else { "quick" }, self.machine.summary());
        let _ = writeln!(
            md,
            "Our engine only, vanilla ultra pins plus every plan feature off (`perfkit::plan::baseline`), {} rounds. \
             Demo runs: timedemo with a MangoHud frame log, the first {WARM_FRAMES} frames left out. Live runs: `-fixedstep`, \
             the engine's profiler over {} s after a {} s warm-up (host frame times; GPU µs per frame). Hitches are frames over {HITCH_MS} ms.\n",
            self.o.runs, perfkit::live::MEASURE_SECS, perfkit::live::MEASURE_AT
        );
        let _ = writeln!(md, "## Costs\n\nFeature on minus the same demo with everything off (median mean frame time; + is slower).\n");
        let _ = writeln!(md, "| scene | tier | res | off ms | on ms | cost µs | cost % | p99 off → on | hitches off → on | GPU µs off → on | significant |");
        let _ = writeln!(md, "|---|---|---|--:|--:|--:|--:|--:|--:|--:|---|");
        for &(s, v, i) in self.uses.iter().filter(|u| u.1 != Variant::Off) {
            let Some(&(_, _, off)) = self.uses.iter().find(|u| u.0.name == s.name && u.1 == Variant::Off) else { continue };
            for (ri, r) in self.o.res.iter().enumerate() {
                let (a, b) = (&self.cells[off][ri], &self.cells[i][ri]);
                let (sa, sb) = (summary(a), summary(b));
                let c = cost(a, b);
                let _ = writeln!(
                    md, "| {} | {} | {} | {} | {} | {} | {} | {} → {} | {} → {} | {} → {} | {} |",
                    s.name, v.name(), r.name(), f(sa.mean_ms, 3), f(sb.mean_ms, 3),
                    c.map_or("–".into(), |c| format!("{:+.0}", c.0)), c.map_or("–".into(), |c| format!("{:+.1}%", c.1)),
                    f(sa.p99, 2), f(sb.p99, 2), f(sa.hitches, 0), f(sb.hitches, 0), f(sa.gpu_us, 0), f(sb.gpu_us, 0),
                    match c {
                        None => "–",
                        Some(_) if a.samples.len() < 2 || b.samples.len() < 2 => "n/a (1 run)",
                        Some(c) => if c.2 { "yes" } else { "no" },
                    }
                );
            }
        }
        let _ = writeln!(md, "\n## Runs\n");
        let _ = writeln!(md, "| run | source | res | runs | fps | mean ms | spread ms | p50 | p99 | max | hitches | GPU µs | errors |");
        let _ = writeln!(md, "|---|---|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|");
        for (k, cells) in self.keys.iter().zip(self.cells) {
            for (r, c) in self.o.res.iter().zip(cells) {
                let s = summary(c);
                let _ = writeln!(
                    md, "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    k.label, k.src.name(), r.name(), s.runs, f(s.fps, 1), f(s.mean_ms, 3), f(s.spread_ms, 3),
                    f(s.p50, 2), f(s.p99, 2), f(s.max, 2), f(s.hitches, 0), f(s.gpu_us, 0), c.errors.len()
                );
            }
        }
        if !self.skipped.is_empty() {
            let _ = writeln!(md, "\n## Not run\n");
            for (s, why) in self.skipped {
                let _ = writeln!(md, "- `{}`: {why}", s.name);
            }
        }
        let errors: Vec<_> = self.keys.iter().zip(self.cells).flat_map(|(k, c)| c.iter().flat_map(move |c| c.errors.iter().map(move |e| (k, e)))).collect();
        if !errors.is_empty() {
            let _ = writeln!(md, "\n## Errors\n");
            for (k, e) in errors {
                let _ = writeln!(md, "- `{}`: {e}", k.label);
            }
        }
        if !self.changed.is_empty() {
            let _ = writeln!(md, "\n**Configs changed during the session (restored):** {:?}", self.changed);
        }
        md
    }

    pub fn json(&self) -> String {
        let mut s = format!("{{\n  \"machine\": {},\n  \"full\": {},\n  \"runs\": [\n", self.machine.to_json(), self.o.full);
        let mut rows = Vec::new();
        for (k, cells) in self.keys.iter().zip(self.cells) {
            for (r, c) in self.o.res.iter().zip(cells) {
                let m = summary(c);
                rows.push(format!(
                    "    {{\"run\": {}, \"demo\": {}, \"res\": \"{}\", \"fps\": {:?}, \"mean_ms\": {:?}, \"fps_median\": {}, \
                     \"mean_ms_median\": {}, \"p99_ms\": {}, \"max_ms\": {}, \"hitches\": {}, \"gpu_us\": {}, \"errors\": [{}]}}",
                    json_str(&k.label), json_str(k.src.name()), r.name(),
                    c.samples.iter().map(|s| s.fps).collect::<Vec<_>>(), c.samples.iter().map(|s| s.mean_ms()).collect::<Vec<_>>(),
                    j(m.fps), j(m.mean_ms), j(m.p99), j(m.max), j(m.hitches), j(m.gpu_us),
                    c.errors.iter().map(|e| json_str(e)).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        s.push_str(&rows.join(",\n"));
        s.push_str("\n  ],\n  \"costs\": [\n");
        let mut rows = Vec::new();
        for &(sc, v, i) in self.uses.iter().filter(|u| u.1 != Variant::Off) {
            let Some(&(_, _, off)) = self.uses.iter().find(|u| u.0.name == sc.name && u.1 == Variant::Off) else { continue };
            for (ri, r) in self.o.res.iter().enumerate() {
                let c = cost(&self.cells[off][ri], &self.cells[i][ri]);
                rows.push(format!(
                    "    {{\"scene\": {}, \"tier\": \"{}\", \"res\": \"{}\", \"cost_us\": {}, \"cost_pct\": {}, \"significant\": {}}}",
                    json_str(sc.name), v.name(), r.name(), j(c.map(|c| c.0)), j(c.map(|c| c.1)), c.is_some_and(|c| c.2)
                ));
            }
        }
        s.push_str(&rows.join(",\n"));
        let _ = write!(
            s, "\n  ],\n  \"skipped\": [{}],\n  \"configs_changed\": [{}]\n}}\n",
            self.skipped.iter().map(|(sc, _)| json_str(sc.name)).collect::<Vec<_>>().join(", "),
            self.changed.iter().map(|p| json_str(&p.display().to_string())).collect::<Vec<_>>().join(", ")
        );
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_cmd::Sample;

    fn cell(ms: &[f64]) -> Cell {
        Cell { samples: ms.iter().map(|&m| Sample::of(1000.0 / m, None)).collect(), ..Cell::default() }
    }

    #[test]
    fn cost_follows_the_plans_rule() {
        let (us, pct, sig) = cost(&cell(&[2.0, 2.01, 1.99]), &cell(&[2.2, 2.21, 2.19])).unwrap();
        assert!((us - 200.0).abs() < 1.0 && (pct - 10.0).abs() < 0.1 && sig);
        let (_, _, sig) = cost(&cell(&[2.0, 2.01, 1.99]), &cell(&[2.02, 2.03, 2.01])).unwrap();
        assert!(!sig);
        assert!(cost(&cell(&[]), &cell(&[2.0])).is_none());
        assert!(!cost(&cell(&[2.0]), &cell(&[3.0])).unwrap().2, "one run each: no spread, not significant");
    }
}
