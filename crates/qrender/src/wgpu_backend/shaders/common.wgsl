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
// r_shadow_realtime_dlight (rtlight.rs, wgpu_backend/shadows.rs): dynamic
// lights lit per pixel; vp[layer] renders/reads cube face layer % 6 of
// light layer / 6 in shadow_t.
struct RtLight {
    pos_radius: vec4<f32>,     // origin, radius
    color_minlight: vec4<f32>, // colour, minlight
    info: vec4<i32>,           // x first shadow layer (-1: unshadowed)
};
struct Lights {
    count: vec4<u32>,          // x lights, y shadow filter (0..2), z face size
    lights: array<RtLight, 32>,
    vp: array<mat4x4<f32>, 48>,
};
@group(0) @binding(7) var<uniform> rt: Lights;
@group(0) @binding(8) var shadow_t: texture_depth_2d_array;
@group(0) @binding(9) var shadow_s: sampler_comparison;

// Fraction of light `i` reaching `p` (common.glsl light_visibility).
fn light_visibility(i: u32, p: vec3<f32>, n: vec3<f32>, tl: vec3<f32>) -> f32 {
    let base = rt.lights[i].info.x;
    if (base < 0) { return 1.0; }
    let size = f32(rt.count.z);
    let q = p + n * (0.5 + 3.0 * length(tl) / size);
    let d = q - rt.lights[i].pos_radius.xyz;
    let a = abs(d);
    var face = select(5, 4, d.z >= 0.0);
    if (a.x >= a.y && a.x >= a.z) { face = select(1, 0, d.x >= 0.0); }
    else if (a.y >= a.z) { face = select(3, 2, d.y >= 0.0); }
    let layer = base + face;
    let c = rt.vp[layer] * vec4<f32>(q, 1.0);
    let ndc = c.xyz / c.w;
    let ts = 1.0 / size;
    // NDC y is up, texture v down (wgpu clip space)
    let uv = clamp(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), vec2<f32>(ts), vec2<f32>(1.0 - ts));
    let filt = rt.count.y;
    if (filt == 0u) { return textureSampleCompareLevel(shadow_t, shadow_s, uv, layer, ndc.z); }
    var s = 0.0;
    if (filt == 1u) {
        for (var k = 0; k < 4; k++) {
            let o = vec2<f32>(f32(k & 1), f32(k >> 1u)) - 0.5;
            s += textureSampleCompareLevel(shadow_t, shadow_s, uv + o * ts, layer, ndc.z);
        }
        return s * 0.25;
    }
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompareLevel(shadow_t, shadow_s, uv + vec2<f32>(f32(x), f32(y)) * ts * 1.25, layer, ndc.z);
        }
    }
    return s / 9.0;
}
// Per-pixel dynamic light in lightmap units (common.glsl dyn_lights).
fn dyn_lights(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < rt.count.x; i++) {
        let tl = rt.lights[i].pos_radius.xyz - p;
        let dist = length(tl);
        let a = rt.lights[i].pos_radius.w - dist;
        if (a <= rt.lights[i].color_minlight.w) { continue; }
        let ndl = dot(n, tl) / max(dist, 1e-3);
        let facing = (0.7 + 0.3 * max(ndl, 0.0)) * smoothstep(-0.1, 0.1, ndl);
        if (facing <= 0.0) { continue; }
        sum += rt.lights[i].color_minlight.rgb * (a / 128.0 * facing * light_visibility(i, p, n, tl));
    }
    return sum;
}

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
