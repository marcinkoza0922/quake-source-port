//! Frame profiler for tools/qprof: `prof_start` begins recording wall-clock
//! CPU time per host-frame phase (plus the renderer's fence wait and GPU
//! pass timestamps); `prof_stop <file.json>` writes the summary and the raw
//! per-frame totals. Costs nothing while not recording.

use std::time::Instant;

/// Host frame phases, in the order `Host::frame` runs them.
pub(super) const PHASES: [&str; 6] = ["pre", "input", "server", "client", "view+sound", "render"];

/// CLOCK_MONOTONIC seconds (what `Instant` wraps on Linux), so tools can
/// line profiles up with `perf record -k CLOCK_MONOTONIC`. 0 if unknown.
fn monotonic_secs(t: Instant) -> f64 {
    let d = format!("{t:?}");
    let num = |k: &str| -> Option<f64> {
        let i = d.find(k)? + k.len();
        d[i..].trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    };
    match (num("tv_sec:"), num("tv_nsec:")) {
        (Some(s), Some(n)) => s + n / 1e9,
        _ => 0.0,
    }
}

const COUNTERS: [&str; 5] = ["live", "beams", "decals", "vertices", "indices"];

pub(super) struct Profiler {
    particles: Vec<[usize; 5]>,
    started: Instant,
    last: Instant,
    cur: [f64; PHASES.len()],
    /// Per frame: phase µs, then total µs, then fence wait µs.
    frames: Vec<[f64; PHASES.len() + 2]>,
}

impl Profiler {
    pub(super) fn new() -> Self {
        let now = Instant::now();
        Profiler { particles: Vec::with_capacity(16384), started: now, last: now, cur: [0.0; PHASES.len()], frames: Vec::with_capacity(16384) }
    }

    pub(super) fn begin(&mut self) {
        self.last = Instant::now();
        self.cur = [0.0; PHASES.len()];
    }

    /// Ends phase `k` (time since the previous mark).
    pub(super) fn mark(&mut self, k: usize) {
        let now = Instant::now();
        self.cur[k] += (now - self.last).as_secs_f64() * 1e6;
        self.last = now;
    }

    pub(super) fn end(&mut self, fence_wait_ns: u64, particles: [usize; 5]) {
        let mut row = [0.0; PHASES.len() + 2];
        row[..PHASES.len()].copy_from_slice(&self.cur);
        row[PHASES.len()] = self.cur.iter().sum();
        row[PHASES.len() + 1] = fence_wait_ns as f64 / 1000.0;
        self.frames.push(row);
        self.particles.push(particles);
    }

    /// JSON report; `gpu` is (frames, [(stage, avg µs)]).
    pub(super) fn report(&self, gpu: Option<(u64, Vec<(&'static str, f64)>)>) -> String {
        let n = self.frames.len();
        let col = |k: usize| -> Vec<f64> { self.frames.iter().map(|r| r[k]).collect() };
        let stats = |mut v: Vec<f64>| -> String {
            if v.is_empty() {
                return "null".into();
            }
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            v.sort_by(f64::total_cmp);
            let pct = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
            format!(
                "{{\"mean\": {mean:.2}, \"p50\": {:.2}, \"p95\": {:.2}, \"p99\": {:.2}, \"max\": {:.2}}}",
                pct(0.5), pct(0.95), pct(0.99), pct(1.0)
            )
        };
        let total = col(PHASES.len());
        let wall: f64 = total.iter().sum::<f64>() / 1e6;
        let mut s = format!(
            "{{\n  \"frames\": {n},\n  \"mono_start\": {:.6},\n  \"mono_end\": {:.6},\n  \"wall_secs\": {:.3},\n  \"cpu_fps\": {:.1},\n  \"frame_us\": {},\n  \"fence_wait_us\": {},\n  \"phases_us\": {{\n",
            monotonic_secs(self.started),
            monotonic_secs(Instant::now()),
            self.started.elapsed().as_secs_f64(),
            if wall > 0.0 { n as f64 / wall } else { 0.0 },
            stats(total.clone()),
            stats(col(PHASES.len() + 1)),
        );
        let ph: Vec<String> = PHASES.iter().enumerate().map(|(k, p)| format!("    \"{p}\": {}", stats(col(k)))).collect();
        s.push_str(&ph.join(",\n"));
        s.push_str("\n  },\n  \"gpu_us\": ");
        match gpu {
            Some((frames, v)) if frames > 0 => {
                let t: f64 = v.iter().map(|x| x.1).sum();
                let parts: Vec<String> = v.iter().map(|(k, us)| format!("\"{k}\": {us:.2}")).collect();
                s.push_str(&format!("{{\"frames\": {frames}, \"total\": {t:.2}, {}}}", parts.join(", ")));
            }
            _ => s.push_str("null"),
        }
        let counters: Vec<String> = COUNTERS.iter().enumerate().map(|(k, name)| {
            let values = self.particles.iter().map(|row| row[k] as f64).collect();
            format!("\"{name}\": {}", stats(values))
        }).collect();
        s.push_str(&format!(",\n  \"fte_particles\": {{{}}}", counters.join(", ")));
        let raw: Vec<String> = total.iter().map(|v| format!("{v:.0}")).collect();
        s.push_str(&format!(",\n  \"frame_us_raw\": [{}]\n}}\n", raw.join(",")));
        s
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn report_shape() {
        let mut p = super::Profiler::new();
        for _ in 0..3 {
            p.begin();
            for k in 0..super::PHASES.len() {
                p.mark(k);
            }
            p.end(5000, [10, 2, 3, 40, 60]);
        }
        let r = p.report(Some((3, vec![("world", 10.0), ("post", 2.5)])));
        assert!(r.contains("\"frames\": 3"));
        assert!(r.contains("\"fence_wait_us\": {\"mean\": 5.00"));
        assert!(r.contains("\"total\": 12.50"));
        assert!(r.contains("\"render\""));
        assert!(r.contains("\"live\": {\"mean\": 10.00"));
        assert!(r.contains("\"vertices\": {\"mean\": 40.00"));
    }
}
