// Shared declarations of the wgpu backend's scene shaders.
struct FrameU {
    viewproj: mat4x4<f32>,
    eye: vec4<f32>,    // xyz eye, w time
    fog: vec4<f32>,    // rgb colour, w density (already /64)
    vright: vec4<f32>,
    vup: vec4<f32>,
    vfwd: vec4<f32>,
    sky: vec4<f32>,    // x skyfog, y skybox flag, z sky layer alpha
    alphas: vec4<f32>, // water lava slime tele
    vel: vec4<f32>,    // velocity pass: scene px per velocity px, max scene px x, y
};
// Per-draw record (dynamic uniform offset).
struct DrawU {
    model: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
    extra: vec4<f32>,
    ix: vec4<u32>,
    prev: mat4x4<f32>, // velocity pass: last frame's transform (DrawU.pad[..16])
};
@group(0) @binding(0) var<uniform> fr: FrameU;
@group(0) @binding(1) var lightmap: texture_2d_array<f32>;
@group(0) @binding(2) var palette: texture_2d<f32>;
@group(0) @binding(3) var lm_samp: sampler;
@group(2) @binding(0) var<uniform> du: DrawU;

fn apply_fog(c: vec3<f32>, wpos: vec3<f32>) -> vec3<f32> {
    if (fr.fog.w <= 0.0) { return c; }
    let z = length(wpos - fr.eye.xyz) * fr.fog.w;
    let f = clamp(exp(-z * z), 0.0, 1.0);
    return mix(fr.fog.rgb, c, f);
}
