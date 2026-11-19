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
    soft: vec4<f32>,   // x screen dither, y texture dither (r_softemu/r_dither)
    // x world dither level (0 off, 1 coarse, 2 banded lightmap), y alias mode
    // (0 std, 1 dither, 2 affine), z goggles (1 night vision, 2 thermal)
    softf: vec4<u32>,
    wind: vec4<f32>,   // skybox wind: xyz direction (world), w loop phase
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
// r_enhancedwater (wgpu_backend/water.rs)
struct WaterU {
    params: vec4<f32>,                 // x ripple strength, y caustics, z ripple count, w quality (0 low, 1 high)
    ripples: array<vec4<f32>, 32>,     // xy centre, z liquid surface height, w start time
    amp: array<vec4<f32>, 8>,          // amplitudes, 4 per vec4
};
@group(0) @binding(4) var<uniform> water: WaterU;
@group(0) @binding(5) var caustic_t: texture_2d<f32>;
@group(0) @binding(6) var caustic_s: sampler;
@group(2) @binding(0) var<uniform> du: DrawU;

// Fraction of a surface's own colour left after fog (1 = no fog).
fn fog_factor(wpos: vec3<f32>) -> f32 {
    if (fr.fog.w <= 0.0) { return 1.0; }
    let z = length(wpos - fr.eye.xyz) * fr.fog.w;
    return clamp(exp(-z * z), 0.0, 1.0);
}
fn apply_fog(c: vec3<f32>, wpos: vec3<f32>) -> vec3<f32> {
    if (fr.fog.w <= 0.0) { return c; }
    return mix(fr.fog.rgb, c, fog_factor(wpos));
}

// r_bloom (common.glsl): scene shaders return the colour and what glows; the
// `*_glow` entry points (pipelines built while bloom is on) also write the
// glow to colour attachment 1.
struct Frag {
    @location(0) c: vec4<f32>,
    @location(1) g: vec4<f32>,
};
// Unlit effects glow by brightness, favouring saturated colours.
fn effect_glow(c: vec3<f32>) -> vec3<f32> {
    let hi = max(c.r, max(c.g, c.b));
    let lo = min(c.r, min(c.g, c.b));
    let sat = select(0.0, (hi - lo) / hi, hi > 0.0);
    return c * (smoothstep(0.55, 0.95, hi) * mix(0.35, 1.0, sat));
}
// Glow of an FTE blend mode (fte.frag order): additive modes are light,
// alpha and premultiplied blends glow by brightness, the rest add none.
fn poly_glow(c: vec3<f32>, bm: u32) -> vec3<f32> {
    if (bm == 2u || bm == 3u) { return c; }
    if (bm == 0u || bm == 7u) { return effect_glow(c); }
    return vec3<f32>(0.0);
}

// Add-on goggles: in thermal mode scene shaders write a grey "heat" level
// that the post pass maps to colours (common.glsl thermal/cold_heat).
fn thermal() -> bool { return fr.softf.z == 2u; }
fn luma(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.299, 0.587, 0.114)); }
fn cold_heat(c: vec3<f32>) -> f32 { return 0.05 + 0.2 * luma(c); }

// Noise (ironwail NOISE_FUNCTIONS, as in common.glsl).
// ALU-only 16x16 Bayer matrix, [0, 1).
fn bayer01(c0: vec2<i32>) -> f32 {
    var c = c0 & vec2<i32>(15);
    c.y = c.y ^ c.x;
    var v = u32(c.y | (c.x << 8u));
    v = (v ^ (v << 2u)) & 0x3333u;
    v = (v ^ (v << 1u)) & 0x5555u;
    v = v | (v >> 7u);
    v = reverseBits(v) >> 24u;
    return f32(v) * (1.0 / 256.0);
}
// Hash without Sine (c) 2014 David Hoskins, MIT licence.
fn whitenoise01(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
// Uniform [0, 1] to triangular [-1, 1].
fn tri(x0: f32) -> f32 {
    let orig = x0 * 2.0 - 1.0;
    let signbit = bitcast<u32>(orig) & 0x80000000u;
    let x = sqrt(abs(orig)) - 1.0;
    return bitcast<f32>(bitcast<u32>(x) ^ signbit);
}
// DITHER_NOISE(floor(frag) + 0.5) and SUPPRESS_BANDING at a fragment position.
fn screen_noise(frag: vec2<f32>) -> f32 { return tri(bayer01(vec2<i32>(floor(frag) + 0.5))); }
fn suppress_banding(frag: vec2<f32>) -> f32 { return bayer01(vec2<i32>(frag)) - 0.5; }
