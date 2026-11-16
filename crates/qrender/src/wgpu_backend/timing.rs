//! GPU pass timings (`QSP_GPUTIMES=1`, `prof_start`): timestamps written at
//! the start and end of each render pass (the only placement WebGPU allows),
//! resolved into a ring of read-back buffers and mapped asynchronously, so a
//! frame's numbers arrive a frame or more later. Passes cover all split
//! views at once, so there is no per-view breakdown.

use super::Renderer;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Stages, one per kind of render pass. "scene" is the whole scene unless
/// SSAO splits it, when the draws after AO count as "translucent".
pub(super) const STAGES: [&str; 7] = ["scene", "ao-generate", "ao-apply", "translucent", "soft-particles", "velocity", "post+ui"];
pub(super) const SCENE: usize = 0;
pub(super) const AO_GENERATE: usize = 1;
pub(super) const AO_APPLY: usize = 2;
pub(super) const TRANSLUCENT: usize = 3;
pub(super) const SOFT: usize = 4;
pub(super) const VELOCITY: usize = 5;
pub(super) const POST: usize = 6;

/// Timestamps per frame (two per pass).
const MAX_TS: u32 = 32;
/// Frames whose timestamps can be in flight.
const SLOTS: usize = 3;

const FREE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;

struct Slot {
    buf: wgpu::Buffer,
    /// Stage of each timestamp pair written into `buf`.
    stages: Vec<u8>,
    state: Arc<AtomicU8>,
}

pub(super) struct GpuTimes {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    /// Ring slot of the frame being recorded (None: all slots in flight).
    cur: Option<usize>,
    /// Stages of the passes recorded so far this frame.
    passes: RefCell<Vec<u8>>,
    /// Nanoseconds per timestamp tick.
    period: f64,
    sum_ns: [f64; STAGES.len()],
    frames: u64,
}

impl GpuTimes {
    fn new(dev: &wgpu::Device, period: f32) -> GpuTimes {
        let queries = dev.create_query_set(&wgpu::QuerySetDescriptor { label: Some("gpu times"), ty: wgpu::QueryType::Timestamp, count: MAX_TS });
        let buffer = |usage| {
            dev.create_buffer(&wgpu::BufferDescriptor { label: Some("gpu times"), size: MAX_TS as u64 * 8, usage, mapped_at_creation: false })
        };
        let resolve = buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
        let slots = (0..SLOTS)
            .map(|_| Slot {
                buf: buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
                stages: Vec::new(),
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        GpuTimes {
            queries,
            resolve,
            slots,
            cur: None,
            passes: RefCell::new(Vec::new()),
            period: period as f64,
            sum_ns: [0.0; STAGES.len()],
            frames: 0,
        }
    }

    /// Adds every mapped slot's intervals and frees it.
    fn collect(&mut self) {
        for s in &mut self.slots {
            match s.state.load(Ordering::Acquire) {
                MAPPED => {
                    if let Ok(data) = s.buf.slice(..).get_mapped_range() {
                        let stamps: Vec<u64> =
                            data.as_chunks::<8>().0.iter().take(s.stages.len() * 2).map(|b| u64::from_le_bytes(*b)).collect();
                        for (pair, &stage) in stamps.as_chunks::<2>().0.iter().zip(&s.stages) {
                            self.sum_ns[stage as usize] += pair[1].saturating_sub(pair[0]) as f64 * self.period;
                        }
                        self.frames += 1;
                    }
                    s.buf.unmap();
                    s.state.store(FREE, Ordering::Release);
                }
                FAILED => s.state.store(FREE, Ordering::Release),
                _ => {}
            }
        }
    }

    /// Average µs per stage since the last call; resets the sums.
    fn take(&mut self) -> (u64, Vec<(&'static str, f64)>) {
        let n = self.frames.max(1) as f64 * 1000.0;
        let values = STAGES.iter().zip(self.sum_ns).map(|(name, ns)| (*name, ns / n)).collect();
        let frames = std::mem::take(&mut self.frames);
        self.sum_ns = [0.0; STAGES.len()];
        (frames, values)
    }
}

/// Timestamp writes of a render pass from [`Renderer::pass_times`].
pub(super) fn writes(ts: &Option<(wgpu::QuerySet, u32)>) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
    ts.as_ref().map(|(q, i)| wgpu::RenderPassTimestampWrites {
        query_set: q,
        beginning_of_pass_write_index: Some(*i),
        end_of_pass_write_index: Some(*i + 1),
    })
}

impl Renderer {
    /// Starts timing a frame: collects finished frames, then picks a free
    /// read-back slot (none free: this frame isn't timed).
    pub(super) fn begin_gpu_times(&mut self) {
        let wanted = self.gpu_times_wanted || std::env::var_os("QSP_GPUTIMES").is_some();
        if self.gpu_times.is_none() {
            if !wanted || !self.dev.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
                return;
            }
            self.gpu_times = Some(GpuTimes::new(&self.dev, self.queue.get_timestamp_period()));
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.dev.poll(wgpu::PollType::Poll);
        let Some(t) = &mut self.gpu_times else { return };
        t.collect();
        t.passes.get_mut().clear();
        t.cur = t.slots.iter().position(|s| s.state.load(Ordering::Acquire) == FREE);
    }

    /// Query set and first index for the timestamps of the next pass of `stage`.
    pub(super) fn pass_times(&self, stage: usize) -> Option<(wgpu::QuerySet, u32)> {
        let t = self.gpu_times.as_ref()?;
        t.cur?;
        let mut p = t.passes.borrow_mut();
        let first = p.len() as u32 * 2;
        if first + 2 > MAX_TS {
            return None;
        }
        p.push(stage as u8);
        Some((t.queries.clone(), first))
    }

    /// Resolves this frame's timestamps into its read-back slot.
    pub(super) fn resolve_gpu_times(&self, enc: &mut wgpu::CommandEncoder) {
        let Some(t) = &self.gpu_times else { return };
        let (Some(cur), n) = (t.cur, t.passes.borrow().len() as u32 * 2) else { return };
        if n == 0 {
            return;
        }
        enc.resolve_query_set(&t.queries, 0..n, &t.resolve, 0);
        enc.copy_buffer_to_buffer(&t.resolve, 0, &t.slots[cur].buf, 0, n as u64 * 8);
    }

    /// Maps the slot written by the frame just submitted.
    pub(super) fn submit_gpu_times(&mut self) {
        let Some(t) = &mut self.gpu_times else { return };
        let Some(cur) = t.cur.take() else { return };
        let stages = std::mem::take(t.passes.get_mut());
        if stages.is_empty() {
            return;
        }
        let s = &mut t.slots[cur];
        s.stages = stages;
        s.state.store(MAPPING, Ordering::Release);
        let state = s.state.clone();
        s.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
        });
    }

    pub fn gpu_destination_times(&self) -> Vec<crate::GpuViewTiming> {
        Vec::new()
    }

    /// Passes draw every split view at once: no per-view breakdown.
    pub fn gpu_view_times(&self) -> Vec<crate::GpuViewTiming> {
        Vec::new()
    }

    /// Average µs per GPU stage since the last call (frames whose
    /// timestamps have been read back); None without timestamp support or
    /// before timing was requested.
    pub fn take_gpu_times(&mut self) -> Option<(u64, Vec<(&'static str, f64)>)> {
        let t = self.gpu_times.as_mut()?;
        t.collect();
        Some(t.take())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if std::env::var_os("QSP_GPUTIMES").is_none() {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.dev.poll(wgpu::PollType::wait_indefinitely());
        let Some(t) = &mut self.gpu_times else { return };
        t.collect();
        let frames = t.frames;
        let (_, values) = t.take();
        eprintln!("GPU times over {frames} frames (avg us/frame):");
        for (name, us) in &values {
            eprintln!("  {name:<14} {us:8.1}");
        }
        eprintln!("  {:<14} {:8.1}", "total", values.iter().map(|v| v.1).sum::<f64>());
    }
}
