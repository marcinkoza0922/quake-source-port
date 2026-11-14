//! Reads the engine's profile JSON (host/prof.rs) and writes the suite's
//! report: per-map tables with deltas against vanilla, bottleneck
//! classification and a ranked hotspot list.

use std::fmt::Write as _;

/// Host phases as host/prof.rs names them.
pub const PHASES: [&str; 6] = ["pre", "input", "server", "client", "view+sound", "render"];

#[derive(Clone, Debug, Default)]
pub struct Run {
    pub fps: f64,
    pub frame_mean: f64,
    pub frame_p99: f64,
    pub frame_max: f64,
    pub fence: f64,
    pub phases: [f64; 6],
    pub phases_p99: [f64; 6],
    pub gpu: Vec<(String, f64)>,
    pub gpu_total: f64,
}

/// The number after `"key":` at or after byte `from`.
fn num(s: &str, from: usize, key: &str) -> Option<(f64, usize)> {
    let pat = format!("\"{key}\":");
    let i = s[from..].find(&pat)? + from + pat.len();
    let t = s[i..].trim_start();
    let off = i + (s[i..].len() - t.len());
    let end = t.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | 'e' | '+'))).unwrap_or(t.len());
    Some((t[..end].parse().ok()?, off + end))
}

/// A top-level number by key (e.g. `mono_start`).
pub fn num_of(s: &str, key: &str) -> Option<f64> {
    num(s, 0, key).map(|v| v.0)
}

impl Run {
    pub fn parse(s: &str) -> Option<Run> {
        let mut r = Run { fps: num(s, 0, "cpu_fps")?.0, ..Run::default() };
        let f = s.find("\"frame_us\"")?;
        r.frame_mean = num(s, f, "mean")?.0;
        r.frame_p99 = num(s, f, "p99")?.0;
        r.frame_max = num(s, f, "max")?.0;
        r.fence = num(s, s.find("\"fence_wait_us\"")?, "mean")?.0;
        let ph = s.find("\"phases_us\"")?;
        for (k, p) in PHASES.iter().enumerate() {
            let at = s[ph..].find(&format!("\"{p}\":"))? + ph;
            r.phases[k] = num(s, at, "mean")?.0;
            r.phases_p99[k] = num(s, at, "p99")?.0;
        }
        if let Some(g) = s.find("\"gpu_us\": {") {
            let body = &s[g + 11..g + 11 + s[g + 11..].find('}')?];
            for kv in body.split(',') {
                let (k, v) = kv.split_once(':')?;
                let k = k.trim().trim_matches('"');
                let v: f64 = v.trim().parse().ok()?;
                match k {
                    "frames" => {}
                    "total" => r.gpu_total = v,
                    _ => r.gpu.push((k.to_string(), v)),
                }
            }
        }
        Some(r)
    }

    /// The run with the median fps.
    pub fn median(mut v: Vec<Run>) -> Option<Run> {
        v.sort_by(|a, b| a.fps.total_cmp(&b.fps));
        let i = v.len() / 2;
        v.into_iter().nth(i)
    }

    /// CPU work in the render phase, excluding the wait on the GPU.
    pub fn render_cpu(&self) -> f64 {
        (self.phases[5] - self.fence).max(0.0)
    }

    /// CPU frame work excluding the fence wait.
    pub fn cpu_work(&self) -> f64 {
        (self.frame_mean - self.fence).max(0.0)
    }

    pub fn bound(&self) -> &'static str {
        if self.fence > 0.3 * self.frame_mean { "GPU" } else { "CPU" }
    }

    fn gpu_top(&self) -> (String, f64) {
        self.gpu.iter().cloned().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or_default()
    }
}

pub struct Result {
    pub scenario: &'static str,
    pub group: &'static str,
    pub what: &'static str,
    pub map: String,
    pub run: Run,
}

fn pct(d: f64, base: f64) -> String {
    if base > 0.0 { format!("{:+.0}%", d / base * 100.0) } else { "-".into() }
}

pub fn write(o: &crate::Opts, results: &[Result]) {
    let mut md = String::new();
    let mode = if o.window { "windowed" } else { "headless (offscreen readback in ui+present)" };
    let _ = writeln!(md, "# qprof report\n\n{}x{}, {mode}, {} s measured after {} s warm-up, sim step 1/{} s, \
        median of {} run(s).\nTimes are µs per frame (means unless noted). `fence` = CPU waiting on the GPU; \
        `render cpu` = render phase minus fence. Bound = GPU when fence > 30% of the frame.\n",
        o.width, o.height, o.secs, o.warmup, o.maxfps, o.runs);
    let mut maps: Vec<&str> = results.iter().map(|r| r.map.as_str()).collect();
    maps.dedup();
    let mut hot: Vec<(f64, String)> = Vec::new();
    for map in &maps {
        let rows: Vec<&Result> = results.iter().filter(|r| r.map == *map).collect();
        let base = rows.iter().find(|r| r.scenario == "vanilla").map(|r| r.run.clone());
        let _ = writeln!(md, "## {map}\n");
        let _ = writeln!(md, "| scenario | group | fps | frame | p99 | Δ frame | server | client | render cpu | fence | GPU | Δ GPU | bound | top GPU stage |");
        let _ = writeln!(md, "|---|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|---|---|");
        for r in &rows {
            let x = &r.run;
            let (dframe, dgpu) = base.as_ref().map_or((String::new(), String::new()), |b| {
                (format!("{:+.0} ({})", x.frame_mean - b.frame_mean, pct(x.frame_mean - b.frame_mean, b.frame_mean)),
                 format!("{:+.0} ({})", x.gpu_total - b.gpu_total, pct(x.gpu_total - b.gpu_total, b.gpu_total)))
            });
            let (gs, gv) = x.gpu_top();
            let _ = writeln!(md, "| {} | {} | {:.0} | {:.0} | {:.0} | {dframe} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {dgpu} | {} | {gs} {gv:.0} |",
                r.scenario, r.group, x.fps, x.frame_mean, x.frame_p99, x.phases[2], x.phases[3], x.render_cpu(), x.fence, x.gpu_total, x.bound());
        }
        let Some(b) = base else { continue };
        // Where each scenario's extra cost went.
        let _ = writeln!(md, "\n### {map}: where the extra time goes (vs vanilla)\n");
        let _ = writeln!(md, "| scenario | Δ CPU work | biggest CPU phase Δ | Δ GPU | biggest GPU stage Δ | server p99 | what |");
        let _ = writeln!(md, "|---|--:|---|--:|---|--:|---|");
        for r in rows.iter().filter(|r| r.scenario != "vanilla") {
            let x = &r.run;
            let mut cpu: Vec<(f64, &str)> = PHASES.iter().enumerate().map(|(k, p)| {
                let d = if k == 5 { x.render_cpu() - b.render_cpu() } else { x.phases[k] - b.phases[k] };
                (d, *p)
            }).collect();
            cpu.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut gpu: Vec<(f64, &str)> = x.gpu.iter().map(|(k, v)| {
                (v - b.gpu.iter().find(|g| g.0 == *k).map_or(0.0, |g| g.1), k.as_str())
            }).collect();
            gpu.sort_by(|a, b| b.0.total_cmp(&a.0));
            let dcpu = x.cpu_work() - b.cpu_work();
            let dgpu = x.gpu_total - b.gpu_total;
            let (c0, g0) = (cpu.first().copied().unwrap_or_default(), gpu.first().copied().unwrap_or_default());
            let _ = writeln!(md, "| {} | {dcpu:+.0} | {} {:+.0} | {dgpu:+.0} | {} {:+.0} | {:.0} | {} |",
                r.scenario, c0.1, c0.0, g0.1, g0.0, x.phases_p99[2], r.what);
            if dcpu > 0.0 {
                hot.push((dcpu, format!("{map}: **{}** costs {dcpu:+.0} µs CPU ({}), mostly `{}` {:+.0}", r.scenario, pct(dcpu, b.cpu_work()), c0.1, c0.0)));
            }
            if dgpu > 0.0 {
                hot.push((dgpu, format!("{map}: **{}** costs {dgpu:+.0} µs GPU ({}), mostly `{}` {:+.0}", r.scenario, pct(dgpu, b.gpu_total), g0.1, g0.0)));
            }
        }
        // Baseline breakdown: what vanilla itself spends time on.
        let _ = writeln!(md, "\n### {map}: vanilla breakdown\n");
        let mut parts: Vec<(f64, String)> = PHASES.iter().enumerate().map(|(k, p)| {
            if k == 5 { (b.render_cpu(), "cpu render (excl. fence)".to_string()) } else { (b.phases[k], format!("cpu {p}")) }
        }).collect();
        parts.extend(b.gpu.iter().map(|(k, v)| (*v, format!("gpu {k}"))));
        parts.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (v, k) in parts.iter().take(8) {
            let _ = writeln!(md, "- {k}: {v:.0} µs");
        }
        let _ = writeln!(md);
    }
    hot.sort_by(|a, b| b.0.total_cmp(&a.0));
    let _ = writeln!(md, "## Hotspot ranking (largest added cost first)\n");
    for (_, h) in hot.iter().take(30) {
        let _ = writeln!(md, "1. {h}");
    }
    let _ = std::fs::write(o.out.join("report.md"), &md);

    let mut js = String::from("[\n");
    for (i, r) in results.iter().enumerate() {
        let x = &r.run;
        let gpu: Vec<String> = x.gpu.iter().map(|(k, v)| format!("\"{k}\": {v:.2}")).collect();
        let ph: Vec<String> = PHASES.iter().zip(x.phases).map(|(k, v)| format!("\"{k}\": {v:.2}")).collect();
        let _ = write!(js, "{}  {{\"scenario\": \"{}\", \"group\": \"{}\", \"map\": \"{}\", \"fps\": {:.1}, \"frame_mean\": {:.2}, \
            \"frame_p99\": {:.2}, \"frame_max\": {:.2}, \"fence\": {:.2}, \"phases\": {{{}}}, \"gpu_total\": {:.2}, \"gpu\": {{{}}}}}",
            if i > 0 { ",\n" } else { "" }, r.scenario, r.group, r.map, x.fps, x.frame_mean, x.frame_p99, x.frame_max, x.fence,
            ph.join(", "), x.gpu_total, gpu.join(", "));
    }
    js.push_str("\n]\n");
    let _ = std::fs::write(o.out.join("report.json"), js);
    println!("{md}");
    eprintln!("qprof: report in {}/report.md", o.out.display());
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_engine_profile() {
        let s = r#"{
  "frames": 3,
  "wall_secs": 1.0,
  "cpu_fps": 500.0,
  "frame_us": {"mean": 2000.00, "p50": 1.0, "p95": 1.0, "p99": 2500.00, "max": 3000.00},
  "fence_wait_us": {"mean": 1000.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0},
  "phases_us": {
    "pre": {"mean": 1.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0},
    "input": {"mean": 2.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0},
    "server": {"mean": 30.00, "p50": 1.0, "p95": 1.0, "p99": 120.0, "max": 1.0},
    "client": {"mean": 4.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0},
    "view+sound": {"mean": 5.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0},
    "render": {"mean": 1900.00, "p50": 1.0, "p95": 1.0, "p99": 1.0, "max": 1.0}
  },
  "gpu_us": {"frames": 3, "total": 12.50, "world": 10.00, "post": 2.50},
  "frame_us_raw": [1,2,3]
}"#;
        let r = super::Run::parse(s).expect("parses");
        assert_eq!((r.fps, r.frame_p99, r.fence), (500.0, 2500.0, 1000.0));
        assert_eq!(r.phases[2], 30.0);
        assert_eq!(r.phases_p99[2], 120.0);
        assert_eq!(r.gpu_total, 12.5);
        assert_eq!(r.gpu, vec![("world".to_string(), 10.0), ("post".to_string(), 2.5)]);
        assert_eq!(r.render_cpu(), 900.0);
        assert_eq!(r.bound(), "GPU");
    }
}
