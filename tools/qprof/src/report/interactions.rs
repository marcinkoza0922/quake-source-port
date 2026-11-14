//! Compare measured pairs with the sum of their standalone GPU costs.
use super::{Result, Run};
use std::fmt::Write as _;

pub(super) fn write(md: &mut String, rows: &[&Result], base: &Run) {
    let find = |name| rows.iter().find(|r| r.scenario == name).map(|r| &r.run);
    let mut table = String::new();
    for (pair, first, effect) in [
        ("supersample-ssao", "supersample", "ssao"),
        ("supersample-water", "supersample", "water"),
        ("msaa8-renderscale200", "msaa8", "renderscale200"),
        ("supersample-renderscale200", "supersample", "renderscale200"),
    ] {
        let (Some(combined), Some(ss), Some(fx)) = (find(pair), find(first), find(effect)) else {
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
        let _ = writeln!(md, "\n### Enhancement interactions\n\nGPU times in µs. Additive = first standalone + second standalone − vanilla; excess = measured pair − additive. Positive excess suggests amplification; compare with run-to-run noise. Each value comes from the run selected by median fps, not an independent stage median.\n");
        let _ = writeln!(md, "| pair | additive GPU | measured GPU | excess GPU | largest stage excess |\n|---|--:|--:|--:|---|\n{table}");
    }
    let mut deltas = String::new();
    for (label, before, after) in [
        ("8x MSAA at 100%", "vanilla", "msaa8"),
        ("8x MSAA at 200%", "renderscale200", "msaa8-renderscale200"),
        ("sample shading at 100% (8x MSAA)", "msaa8", "supersample"),
        ("sample shading at 200% (8x MSAA)", "msaa8-renderscale200", "supersample-renderscale200"),
        ("200% scale (1 sample)", "vanilla", "renderscale200"),
        ("200% scale (8x MSAA)", "msaa8", "msaa8-renderscale200"),
        ("200% scale (8x sample shading)", "supersample", "supersample-renderscale200"),
    ] {
        if let (Some(a), Some(b)) = (find(before), find(after)) {
            let _ = writeln!(deltas, "| {label} | {before} → {after} | {:+.0} | {:+.0} |",
                b.frame_mean - a.frame_mean, b.gpu_total - a.gpu_total);
        }
    }
    if !deltas.is_empty() {
        let _ = writeln!(md, "\n### MSAA, sample shading and render scale\n\nControlled setting differences in µs; sample shading is measured with MSAA held at 8x. With gl_lodbias auto, vid_fsaamode also changes mip bias from 0 to −1.5, so its delta includes that sampler change. 200% scale quadruples scene pixels. These are diagnostic setting costs, not optimisation speedups.\n\n| change | comparison | Δ frame | Δ GPU |\n|---|---|--:|--:|\n{deltas}");
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

    #[test]
    fn separates_msaa_sample_shading_and_scale_costs() {
        let result = |scenario, gpu_total| Result {
            scenario, run: Run { gpu_total, frame_mean: gpu_total + 10.0, ..Run::default() },
            group: "enhancement", what: "", map: "test".into(),
        };
        let rows = [
            result("vanilla", 100.0), result("msaa8", 130.0),
            result("supersample", 170.0), result("renderscale200", 200.0),
            result("msaa8-renderscale200", 280.0),
            result("supersample-renderscale200", 440.0),
        ];
        let mut md = String::new();
        write(&mut md, &rows.iter().collect::<Vec<_>>(), &rows[0].run);
        assert!(md.contains("| msaa8-renderscale200 | 230 | 280 | +50 | unavailable |"));
        assert!(md.contains("| supersample-renderscale200 | 270 | 440 | +170 | unavailable |"));
        assert!(md.contains("| sample shading at 100% (8x MSAA) | msaa8 → supersample | +40 | +40 |"));
        assert!(md.contains("| sample shading at 200% (8x MSAA) | msaa8-renderscale200 → supersample-renderscale200 | +160 | +160 |"));
        md.clear();
        write(&mut md, &[&rows[0], &rows[1]], &rows[0].run);
        assert!(md.contains("| 8x MSAA at 100% | vanilla → msaa8 | +30 | +30 |"));
        assert!(!md.contains("sample shading at 200% (8x MSAA) |"));
    }
}
