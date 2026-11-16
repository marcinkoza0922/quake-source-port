//! Public types shared by both renderer backends.

/// A `#pragma parameter` of the loaded preset (for menus and configs).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderParam {
    pub id: String,
    pub description: String,
    /// The shader's default, after the preset's own overrides.
    pub initial: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Per-view contribution in µs per profiled frame (including frames where
/// this view was absent). These are a breakdown of the aggregate, not extra work.
/// `frames` counts active scene frames for `gpu_view_times`, or destination
/// draws (including recursion) for `gpu_destination_times`.
#[derive(Clone, Debug)]
pub struct GpuViewTiming {
    pub frames: u64,
    pub stages: Vec<(&'static str, f64)>,
}

