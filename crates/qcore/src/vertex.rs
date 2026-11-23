//! Shared particle vertex layout for geometry generation and GPU upload.

/// World-space textured particle vertex, including packed RGBA colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// Instanced normal FTE billboard particle (40 bytes/particle, expanded to 6
/// vertices on the GPU in `fte_quad.vert` with no index buffer).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleQuad {
    pub pos: [f32; 3],
    pub scale: f32,
    pub angle: f32,
    pub color: [u8; 4],
    pub uv: [f32; 4],
}
