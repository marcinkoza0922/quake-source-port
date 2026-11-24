//! The HUD's Performance Metrics elements (`crate::hud`): frame rate,
//! frame time (average and worst frame), process CPU and memory
//! (`qcore::procstat`), GPU time per frame (the renderer's pass
//! timestamps) and video memory (VK_EXT_memory_budget). Sampled every half
//! second while the layout has any of them.

use super::*;
use crate::hud::kind::Kind;
use qcore::procstat::{self, ProcStat};

/// Seconds between samples.
const PERIOD: f64 = 0.5;

#[derive(Default)]
pub(super) struct Metrics {
    /// Frame time summed this period, frames, worst frame.
    acc: f64,
    frames: u32,
    worst: f32,
    /// Process snapshot at the last sample, and when it was taken.
    proc_prev: ProcStat,
    proc_at: f64,
    // shown values
    fps: f32,
    frame_ms: f32,
    worst_ms: f32,
    cpu: Option<f32>,
    rss: Option<u64>,
    gpu_ms: Option<f32>,
    vram: Option<(u64, u64)>,
}

/// Which metrics are on.
#[derive(Clone, Copy, Default)]
struct Shown {
    fps: bool,
    frame: bool,
    cpu: bool,
    ram: bool,
    gpu: bool,
    vram: bool,
}

impl Shown {
    fn any(self) -> bool {
        self.fps || self.frame || self.cpu || self.ram || self.gpu || self.vram
    }
}

fn mib(bytes: u64) -> u64 {
    (bytes + (1 << 19)) >> 20
}

impl Host {
    fn metrics_shown(&self) -> Shown {
        let on = |k: Kind| self.hud.layout.elems.iter().any(|e| e.kind == k);
        Shown {
            fps: on(Kind::Fps),
            frame: on(Kind::FrameTime),
            cpu: on(Kind::Cpu),
            ram: on(Kind::Ram),
            gpu: on(Kind::Gpu),
            vram: on(Kind::Vram),
        }
    }

    /// Counts a frame of `ft` seconds; samples everything each period.
    pub(super) fn update_metrics(&mut self, ft: f32) {
        let shown = self.metrics_shown();
        if !shown.any() {
            self.metrics.frames = 0;
            self.metrics.acc = 0.0;
            return;
        }
        if shown.gpu {
            self.r.want_gpu_times(true);
        }
        let m = &mut self.metrics;
        m.acc += f64::from(ft);
        m.frames += 1;
        m.worst = m.worst.max(ft);
        if m.acc < PERIOD {
            return;
        }
        m.fps = (f64::from(m.frames) / m.acc) as f32;
        m.frame_ms = (m.acc * 1000.0 / f64::from(m.frames)) as f32;
        m.worst_ms = m.worst * 1000.0;
        (m.acc, m.frames, m.worst) = (0.0, 0, 0.0);
        if shown.cpu || shown.ram {
            let now = procstat::sample();
            let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
            m.cpu = procstat::cpu_percent(&m.proc_prev, &now, self.realtime - m.proc_at, cores);
            m.rss = now.rss;
            (m.proc_prev, m.proc_at) = (now, self.realtime);
        }
        // the profiler (prof_start) owns the GPU timestamps while it records
        if shown.gpu && self.prof.is_none() {
            m.gpu_ms = self.r.take_gpu_times().filter(|(frames, _)| *frames > 0).map(|(_, stages)| (stages.iter().map(|s| s.1).sum::<f64>() / 1000.0) as f32);
        }
        if shown.vram {
            m.vram = self.r.vram();
        }
    }

    /// The metric elements' values, `Kind::Fps` order.
    pub(super) fn metric_values(&self) -> [String; 6] {
        let m = &self.metrics;
        let na = || "n/a".to_string();
        [
            format!("{:.0}", m.fps),
            format!("{:.1} ms, max {:.1}", m.frame_ms, m.worst_ms),
            m.cpu.map_or_else(na, |c| format!("{c:.0}%")),
            m.rss.map_or_else(na, |r| format!("{} MB", mib(r))),
            m.gpu_ms.map_or_else(na, |g| format!("{g:.1} ms")),
            m.vram.map_or_else(na, |(used, budget)| format!("{}/{} MB", mib(used), mib(budget))),
        ]
    }
}
