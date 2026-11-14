//! Compare measured pairs with the sum of their standalone GPU costs.
use super::{Result, Run};
use std::fmt::Write as _;

pub(super) fn write(md: &mut String, rows: &[&Result], base: &Run) {
    let find = |name| rows.iter().find(|r| r.scenario == name).map(|r| &r.run);
    let mut table = String::new();
    for (pair, effect) in [("supersample-ssao", "ssao"), ("supersample-water", "water")] {
        let (Some(combined), Some(ss), Some(fx)) = (find(pair), find("supersample"), find(effect)) else {
            continue;
        };
        let additive = ss.gpu_total + fx.gpu_total - base.gpu_total;
        // Require all four measurements for a stage; missing timestamps
        // must not masquerade as zero work.
        let stage = |r: &Run, name: &str| r.gpu.iter().find(|(k, _)| k == name).map(|(_, v)| *v);
        let largest = combined.gpu.iter().filter_map(|(name, value)| {
            Some((value - stage(ss, name)? - stage(fx, name)? + stage(base, name)?, name))
        }).max_by(|a, b| a.0.total_cmp(&b.0));
        let stage_text = largest.map_or_else(|| "unavailable".to_string(), |(excess, name)| format!("{name} {excess:+.0}"));
        let _ = writeln!(table, "| {pair} | {additive:.0} | {:.0} | {:+.0} | {stage_text} |",
            combined.gpu_total, combined.gpu_total - additive);
    }
    if !table.is_empty() {
        let _ = writeln!(md, "\n### Enhancement interactions\n\nGPU times in µs. Additive = supersample + effect − vanilla; excess = measured pair − additive. Positive excess suggests amplification; compare with run-to-run noise. Each value comes from the run selected by median fps, not an independent stage median.\n");
        let _ = writeln!(md, "| pair | additive GPU | measured GPU | excess GPU | largest stage excess |\n|---|--:|--:|--:|---|\n{table}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_interaction_above_additive_and_omits_incomplete_pairs() {
        let run = |total, world, post| Run {
            gpu_total: total,
            gpu: vec![("world".into(), world), ("post".into(), post)],
            ..Run::default()
        };
        let result = |scenario, run| Result { scenario, run, group: "enhancement", what: "", map: "test".into() };
        let base = run(100.0, 70.0, 30.0);
        let rows = [
            result("supersample", run(200.0, 170.0, 30.0)),
            result("ssao", run(150.0, 70.0, 80.0)),
            result("supersample-ssao", run(310.0, 180.0, 130.0)),
            result("supersample-water", run(400.0, 300.0, 100.0)),
        ];
        let mut md = String::new();
        write(&mut md, &rows.iter().collect::<Vec<_>>(), &base);
        assert!(md.contains("| supersample-ssao | 250 | 310 | +60 | post +50 |"));
        assert!(!md.contains("supersample-water"));
        // A missing timestamp cannot produce a bogus stage delta.
        let mut rows = rows;
        rows[1].run.gpu.clear();
        md.clear();
        write(&mut md, &rows.iter().collect::<Vec<_>>(), &base);
        assert!(md.contains("+60 | unavailable |"));
        md.clear();
        write(&mut md, &[], &base);
        assert!(md.is_empty());
    }
}
