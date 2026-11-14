//! Shared particle vertex layout for geometry generation and GPU upload.

/// World-space textured particle vertex, including packed RGBA colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}
