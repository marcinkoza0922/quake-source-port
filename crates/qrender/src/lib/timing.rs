//! GPU intervals for every scene view and its destinations, plus shared UI/present work.
use crate::*;

pub(super) const VIEW_STAGES: [&str; 14] = [
    "cull", "lightmap", "targets", "destinations", "world", "models",
    "ao-prepare", "ao-generate", "ao-to-apply", "ao-apply", "ao-resume",
    "translucent", "particles+vm", "post",
];
pub(super) const GPU_STAGES: [&str; 15] = [
    "cull", "lightmap", "targets", "destinations", "world", "models",
    "ao-prepare", "ao-generate", "ao-to-apply", "ao-apply", "ao-resume",
    "translucent", "particles+vm", "post", "ui+present",
];
pub(super) const DESTINATION_STAGES: [&str; 5] = ["setup", "world", "models", "particles", "resolve"];
pub(super) const DESTINATION_TS: usize = DESTINATION_STAGES.len() + 1;

pub(super) const VIEW_TS: usize = VIEW_STAGES.len();

pub(super) struct GpuTimes {
    pub pool: vk::QueryPool,
    pub capacity: usize,
    pub written: [usize; FRAMES],
    pub sum_ns: [f64; GPU_STAGES.len()],
    pub frames: u64,
    views: Vec<([f64; VIEW_STAGES.len()], u64)>,
    values: Vec<u64>,
    pub destination_counts: [Vec<usize>; FRAMES],
    destinations: Vec<([f64; DESTINATION_STAGES.len()], u64)>,
}

impl GpuTimes {
    pub fn stride(&self) -> u32 {
        (self.capacity * (VIEW_TS + crate::MAX_PORTAL_VIEWS * DESTINATION_TS) + 2) as u32
    }

    pub fn destination_base(&self, fi: usize, scene: usize) -> u32 {
        fi as u32 * self.stride()
            + (self.capacity * VIEW_TS + 2 + scene * crate::MAX_PORTAL_VIEWS * DESTINATION_TS) as u32
    }

    fn accumulate_destinations(&mut self, scene: usize, values: &[u64], period: f64) {
        self.destinations.resize(self.capacity, ([0.0; DESTINATION_STAGES.len()], 0));
        let (sum, draws) = &mut self.destinations[scene];
        for stamps in values.as_chunks::<DESTINATION_TS>().0 {
            for k in 0..DESTINATION_STAGES.len() {
                sum[k] += stamps[k + 1].saturating_sub(stamps[k]) as f64 * period;
            }
            *draws += 1;
        }
    }

    fn accumulate(&mut self, values: &[u64], period: f64) {
        let count = (values.len() - 2) / VIEW_TS;
        self.views.resize(self.capacity, ([0.0; VIEW_STAGES.len()], 0));
        for vi in 0..count {
            let stamps = &values[vi * VIEW_TS..= (vi + 1) * VIEW_TS];
            let (sum, frames) = &mut self.views[vi];
            for k in 0..VIEW_STAGES.len() {
                let ns = stamps[k + 1].saturating_sub(stamps[k]) as f64 * period;
                sum[k] += ns;
                self.sum_ns[k] += ns;
            }
            *frames += 1;
        }
        self.sum_ns[VIEW_STAGES.len()] += values[values.len() - 1]
            .saturating_sub(values[values.len() - 2]) as f64 * period;
        self.frames += 1;
    }
}

impl Renderer {
    pub(super) fn init_gpu_times(&mut self, views: usize) -> Result<()> {
        if self.gpu_times.is_none() && !self.gpu_times_wanted && std::env::var_os("QSP_GPUTIMES").is_none() {
            return Ok(());
        }
        if self.gpu_times.as_ref().is_some_and(|t| t.capacity >= views) {
            return Ok(());
        }
        // Growing the pool is rare (normally reserve all four local players).
        // Drain both slots before replacing a pool still used by the GPU.
        if self.gpu_times.is_some() {
            self.idle();
            for fi in 0..FRAMES { self.collect_gpu_times(fi); }
        }
        let capacity = views.max(4);
        let ci = vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP)
            .query_count(((capacity * (VIEW_TS + crate::MAX_PORTAL_VIEWS * DESTINATION_TS) + 2) * FRAMES) as u32);
        // SAFETY: valid device; the old pool is retained if allocation fails.
        let pool = unsafe { self.gpu.device.create_query_pool(&ci, None)? };
        if let Some(t) = &mut self.gpu_times {
            // SAFETY: both frame slots completed and were collected above.
            unsafe { self.gpu.device.destroy_query_pool(t.pool, None) };
            t.pool = pool;
            t.capacity = capacity;
        } else {
            self.gpu_times = Some(GpuTimes {
                pool, capacity, written: [0; FRAMES], sum_ns: [0.0; GPU_STAGES.len()],
                frames: 0, views: Vec::new(), values: Vec::new(),
                destination_counts: std::array::from_fn(|_| Vec::new()), destinations: Vec::new(),
            });
        }
        Ok(())
    }

    pub(super) fn collect_gpu_times(&mut self, fi: usize) {
        let period = self.gpu.timestamp_period as f64;
        let Some(t) = &mut self.gpu_times else { return };
        let views = std::mem::take(&mut t.written[fi]);
        if views == 0 { return; }
        // Read only the written prefix: unused view slots have no query result.
        t.values.resize(views * VIEW_TS + 2, 0);
        // SAFETY: this frame's fence has completed; all queried indices were written.
        let result = unsafe {
            self.gpu.device.get_query_pool_results(t.pool, fi as u32 * t.stride(),
                &mut t.values, vk::QueryResultFlags::TYPE_64)
        };
        if result.is_ok() {
            let values = std::mem::take(&mut t.values);
            t.accumulate(&values, period);
            t.values = values;
        }
        for scene in 0..views {
            let count = t.destination_counts[fi][scene];
            if count == 0 { continue; }
            t.values.resize(count * DESTINATION_TS, 0);
            // SAFETY: the frame fence completed; only this scene's written prefix is read.
            let result = unsafe {
                self.gpu.device.get_query_pool_results(t.pool, t.destination_base(fi, scene),
                    &mut t.values, vk::QueryResultFlags::TYPE_64)
            };
            if result.is_ok() {
                let values = std::mem::take(&mut t.values);
                t.accumulate_destinations(scene, &values, period);
                t.values = values;
            }
        }
    }

    /// Destination subintervals per scene, normalized by all profiled GPU frames.
    /// `frames` counts destination draws, including recursive views. These sums
    /// are a subset of `destinations`, never additional aggregate GPU work.
    pub fn gpu_destination_times(&self) -> Vec<GpuViewTiming> {
        let Some(t) = &self.gpu_times else { return Vec::new() };
        let n = t.frames.max(1) as f64 * 1000.0;
        let scenes = t.views.iter().take_while(|(_, frames)| *frames > 0).count();
        t.destinations.iter().take(scenes).map(|(sum, draws)| GpuViewTiming {
            frames: *draws,
            stages: DESTINATION_STAGES.iter().enumerate().map(|(k, name)| (*name, sum[k] / n)).collect(),
        }).collect()
    }

    /// Per-view breakdown since the last `take_gpu_times`, with the same
    /// per-frame denominator as the aggregate. Snapshot before taking totals.
    pub fn gpu_view_times(&self) -> Vec<GpuViewTiming> {
        let Some(t) = &self.gpu_times else { return Vec::new() };
        let n = t.frames.max(1) as f64 * 1000.0;
        t.views.iter().take_while(|(_, frames)| *frames > 0).map(|(sum, frames)| GpuViewTiming {
            frames: *frames,
            stages: VIEW_STAGES.iter().enumerate().map(|(k, name)| (*name, sum[k] / n)).collect(),
        }).collect()
    }

    /// Average µs per GPU stage since the last call, summed across scene views.
    /// UI/present is shared. Resets aggregate and per-view sums together.
    pub fn take_gpu_times(&mut self) -> Option<(u64, Vec<(&'static str, f64)>)> {
        let t = self.gpu_times.as_mut()?;
        let n = t.frames.max(1) as f64 * 1000.0;
        let values = GPU_STAGES.iter().enumerate().map(|(k, name)| (*name, t.sum_ns[k] / n)).collect();
        let frames = t.frames;
        t.sum_ns.fill(0.0);
        t.frames = 0;
        t.views.fill(([0.0; VIEW_STAGES.len()], 0));
        t.destinations.fill(([0.0; DESTINATION_STAGES.len()], 0));
        Some((frames, values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_views_once_and_handles_changing_view_counts() {
        let mut t = GpuTimes { pool: vk::QueryPool::null(), capacity: 4, written: [0; FRAMES],
            sum_ns: [0.0; GPU_STAGES.len()], frames: 0, views: Vec::new(), values: Vec::new(),
            destination_counts: std::array::from_fn(|_| Vec::new()), destinations: Vec::new() };
        let frame = |views: usize| (0..views * VIEW_TS + 2).map(|i| i as u64 * 10).collect::<Vec<_>>();
        t.accumulate(&frame(4), 2.0);
        t.accumulate(&frame(1), 2.0);
        assert_eq!(t.frames, 2);
        assert_eq!(t.views.iter().map(|v| v.1).collect::<Vec<_>>(), [2, 1, 1, 1]);
        for k in 0..VIEW_STAGES.len() {
            assert_eq!(t.sum_ns[k], 100.0);
            assert_eq!(t.sum_ns[k], t.views.iter().map(|v| v.0[k]).sum::<f64>());
        }
        assert_eq!(t.sum_ns[VIEW_STAGES.len()], 40.0, "shared UI counted once per frame");
        assert_eq!(t.sum_ns.iter().sum::<f64>(), 1440.0);
        // Portal queries are indexed by portal id, although recursive views are
        // drawn in reverse order. Never subtract across portal boundaries.
        t.accumulate_destinations(0, &[100, 110, 130, 160, 200, 250, 0, 1, 3, 6, 10, 15], 2.0);
        t.accumulate_destinations(2, &[0, 1, 2, 3, 4, 5], 2.0);
        t.accumulate_destinations(0, &[0, 1, 2, 3, 4, 5], 2.0);
        assert_eq!(t.destinations[0], ([24.0, 46.0, 68.0, 90.0, 112.0], 3));
        assert_eq!(t.destinations[1], ([0.0; 5], 0), "absent scenes keep their index");
        assert_eq!(t.destinations[2], ([2.0; 5], 1));
        assert_eq!(t.sum_ns.iter().sum::<f64>(), 1440.0, "subintervals must not inflate GPU totals");
        assert_eq!(t.destination_base(0, 0), (4 * VIEW_TS + 2) as u32);
        assert_eq!(t.destination_base(0, 3) + (crate::MAX_PORTAL_VIEWS * DESTINATION_TS) as u32, t.stride());
        assert_eq!(t.destination_base(1, 0), t.stride() + t.destination_base(0, 0));
    }
}
