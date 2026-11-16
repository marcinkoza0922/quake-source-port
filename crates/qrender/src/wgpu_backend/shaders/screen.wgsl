// 2D overlay (UI) and the post pass (polyblend, contrast, gamma).
struct DrawU {
    model: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
    extra: vec4<f32>,
    ix: vec4<u32>,
};
@group(0) @binding(0) var<uniform> du: DrawU;
@group(1) @binding(0) var t0: texture_2d<f32>;
@group(1) @binding(1) var t1: texture_2d<f32>;
@group(1) @binding(3) var s0: sampler;
// post pass only: the palette and the r_softemu lookup table (post.rs)
@group(1) @binding(4) var palette: texture_2d<f32>;
@group(1) @binding(5) var<storage, read> lut: array<u32>;

struct UiIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct UiOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
// du.color.xy: 1 / output size
@vertex fn vs_ui(v: UiIn) -> UiOut {
    var o: UiOut;
    let p = v.pos * du.color.xy * 2.0 - 1.0;
    o.clip = vec4<f32>(p.x, -p.y, 0.0, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    return o;
}
@fragment fn fs_ui(i: UiOut) -> @location(0) vec4<f32> {
    let c = textureSample(t0, s0, i.uv) * i.color;
    if (c.a <= 0.0) { discard; }
    return c;
}

struct PostOut {
    @builtin(position) clip: vec4<f32>,
    // 0..1 across the view
    @location(0) q: vec2<f32>,
};
@vertex fn vs_post(@builtin(vertex_index) vi: u32) -> PostOut {
    var o: PostOut;
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    o.clip = vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0);
    o.q = q;
    return o;
}
// vkQuake screen_effects.inc SCREEN_EFFECT_FLAG_WATER_WARP (software Quake's
// underwater ripple), inset by the amplitude so edges never sample outside.
// t: warp clock (s); aspect: output width / height.
fn water_warp(p: vec2<f32>, t: f32, aspect: f32) -> vec2<f32> {
    let cycle_x = 3.14159 * 5.0;
    let cycle_y = cycle_x * aspect;
    let amp_x = 1.0 / 300.0;
    let amp_y = amp_x * aspect;
    let x = (p.x + sin(p.y * cycle_x + t) * amp_x) * (1.0 - amp_x * 2.0) + amp_x;
    let y = (p.y + sin(p.x * cycle_y + t) * amp_y) * (1.0 - amp_y * 2.0) + amp_y;
    return vec2<f32>(x, y);
}
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
// Uniform [0, 1] to triangular [-1, 1].
fn tri(x0: f32) -> f32 {
    let orig = x0 * 2.0 - 1.0;
    let signbit = bitcast<u32>(orig) & 0x80000000u;
    let x = sqrt(abs(orig)) - 1.0;
    return bitcast<f32>(bitcast<u32>(x) ^ signbit);
}
// Hash without Sine (c) 2014 David Hoskins, MIT licence.
fn whitenoise01(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
fn scene_at(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(t0, s0, uv, 0.0).rgb;
}
// t1: per-object motion (view uv per frame), scaled to target uv
fn vel_at(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(t1, s0, uv, 0.0).xy * du.model[0].xy;
}
const MB_DIRS = array<vec2<f32>, 8>(
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.70710678, 0.70710678),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(-0.70710678, 0.70710678),
    vec2<f32>(-1.0, 0.0),
    vec2<f32>(-0.70710678, -0.70710678),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(0.70710678, -0.70710678),
);
// post.frag motion_blur: average along the strongest motion nearby, so
// moving objects also smear over the static background around them.
fn motion_blur(c: vec3<f32>, uv: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    let sz = du.model[1].xy;
    let hq = du.model[0].w > 0.5;
    var v = vel_at(uv);
    var best = dot(v * sz, v * sz);
    if (hq && best < 0.25) {
        let r = 24.0 / sz;
        for (var i = 0; i < 8; i++) {
            let t = vel_at(uv + MB_DIRS[i] * r);
            let l = dot(t * sz, t * sz);
            if (l > best) { best = l; v = t; }
        }
    }
    if (best < 0.25) { return c; }
    let l = length(v);
    if (l > 0.06) { v *= 0.06 / l; }
    let n = select(6, 12, hq);
    let j = bayer01(vec2<i32>(frag)) - 0.5;
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < n; i++) {
        let t = (f32(i) + 0.5 + j) / f32(n) - 0.5;
        acc += scene_at(uv - v * t);
    }
    return acc / f32(n);
}
// post.frag speed_blur: sample toward the view centre to stretch scenery
// into peripheral streaks; the inner 40% stays sharp.
fn speed_blur(c: vec3<f32>, uv: vec2<f32>, q: vec2<f32>, amount0: f32) -> vec3<f32> {
    let edge = smoothstep(0.4, 1.1, length((q - 0.5) * 2.0));
    let amount = amount0 * edge;
    if (amount < 0.0001) { return c; }
    let streak = (uv - (du.extra.xy + du.extra.zw) * 0.5) * (0.12 * amount);
    var acc = c;
    for (var i = 1; i <= 8; i++) {
        acc += scene_at(uv - streak * (f32(i) / 8.0));
    }
    return acc / 9.0;
}
// Nearest palette colour of c (GLPalette_Postprocess).
fn palettize(c: vec3<f32>) -> vec3<f32> {
    let q = vec3<u32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * 127.0 + 0.5);
    let i = q.x + q.y * 128u + q.z * 16384u;
    let idx = (lut[i >> 2u] >> ((i & 3u) * 8u)) & 255u;
    return textureLoad(palette, vec2<i32>(i32(idx), 0), 0).rgb;
}
// Daltonize: simulate the deficiency (Viénot/Brettel-style RGB matrices),
// then move the lost difference into channels the viewer can still tell apart.
fn daltonize(c: vec3<f32>, m: u32) -> vec3<f32> {
    var sim = mat3x3<f32>(0.95, 0.0, 0.0, 0.05, 0.43333, 0.475, 0.0, 0.56667, 0.525);
    if (m == 1u) {
        sim = mat3x3<f32>(0.56667, 0.55833, 0.0, 0.43333, 0.44167, 0.24167, 0.0, 0.0, 0.75833);
    } else if (m == 2u) {
        sim = mat3x3<f32>(0.625, 0.70, 0.0, 0.375, 0.30, 0.30, 0.0, 0.0, 0.70);
    }
    let err = c - sim * c;
    let shift = vec3<f32>(0.0, 0.7 * err.r + err.g, 0.7 * err.r + err.b);
    return clamp(c + shift, vec3<f32>(0.0), vec3<f32>(1.0));
}
fn g_luma(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.299, 0.587, 0.114)); }
// Thermal palette: cold blue-grey, through purple and red, to white hot.
fn heat_colour(h: f32) -> vec3<f32> {
    let k = array<vec3<f32>, 6>(vec3<f32>(0.02, 0.03, 0.10), vec3<f32>(0.22, 0.28, 0.40), vec3<f32>(0.50, 0.10, 0.58),
                                vec3<f32>(0.95, 0.18, 0.08), vec3<f32>(1.0, 0.62, 0.05), vec3<f32>(1.0, 1.0, 0.82));
    let x = clamp(h, 0.0, 1.0) * 5.0;
    let i = min(i32(x), 4);
    return mix(k[i], k[i + 1], x - f32(i));
}
const G_RING8 = array<vec2<f32>, 8>(
    vec2<f32>(7.0, 0.0),
    vec2<f32>(4.9497475, 4.9497475),
    vec2<f32>(0.0, 7.0),
    vec2<f32>(-4.9497475, 4.9497475),
    vec2<f32>(-7.0, 0.0),
    vec2<f32>(-4.9497475, -4.9497475),
    vec2<f32>(0.0, -7.0),
    vec2<f32>(4.9497475, -4.9497475),
);
// post.frag goggles. du.ix.z: 1 night vision, 2 thermal (the scene holds
// grey heat levels); du.ix.w glow quality; du.model[2]: gain, noise, time.
fn goggles(c: vec3<f32>, uv: vec2<f32>, q: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    let gain = du.model[2].x;
    let noise = du.model[2].y;
    let time = du.model[2].z;
    let fc = floor(frag);
    let grain = (whitenoise01(fc + fract(time * 7.31) * 517.0) - 0.5) * noise;
    let l = g_luma(c);
    // glow around bright spots: low skips the ring taps, medium takes the 4 diagonals
    var glow = 0.0;
    if (du.ix.w > 0u) {
        let px = 1.0 / vec2<f32>(textureDimensions(t0));
        let n = select(8, 4, du.ix.w == 1u);
        for (var i = 0; i < n; i++) {
            let k = select(i, i * 2 + 1, du.ix.w == 1u);
            glow += max(g_luma(scene_at(uv + G_RING8[k] * px)) - 0.5, 0.0);
        }
        glow /= f32(n);
    }
    let v = (q - 0.5) * vec2<f32>(1.0, 0.8);
    let vignette = smoothstep(0.62, 0.32, length(v));
    if (du.ix.z == 1u) {
        // night vision: amplify (gamma-lift) dark areas, green phosphor
        var a = 1.0 - exp(-l * 7.0 * gain);
        a += glow * 1.6 + grain * 0.14;
        a *= 1.0 - 0.18 * noise * f32(i32(fc.y) % 3 == 0);
        a *= 0.92 + 0.08 * sin(time * 9.0); // faint flicker
        return max(a, 0.0) * vec3<f32>(0.32, 1.0, 0.38) * vignette + vec3<f32>(0.0, 0.015, 0.0);
    }
    let h = 0.1 + (l - 0.1) * gain + glow * 0.8 + grain * 0.06;
    return heat_colour(h) * mix(0.55, 1.0, vignette);
}
// du.extra: uv rectangle (u0, v0, u1, v1) of the view in the scene texture
// du.color: polyblend; du.params: x gamma, y contrast, z warp clock (< 0 off), w aspect
// du.model[0]: motion blur scale x, y (0 = off), speed blur, motion blur quality
// du.model[1]: output size, dither strength, scene pixels per output pixel
// du.ix: softemu mode (0 truecolour, 1 palettize + dither, 2 palettize),
// colour-blind filter, goggles mode, goggles quality
@fragment fn fs_post(i: PostOut) -> @location(0) vec4<f32> {
    var q = i.q;
    if (du.params.z >= 0.0) {
        q = water_warp(q, du.params.z, du.params.w);
    }
    let uv = mix(du.extra.xy, du.extra.zw, q);
    var c = scene_at(uv);
    if (du.model[0].x > 0.0) {
        c = motion_blur(c, uv, i.clip.xy);
    }
    if (du.model[0].z > 0.0) {
        c = speed_blur(c, uv, i.q, du.model[0].z);
    }
    let mode = du.ix.x;
    if (mode != 0u) {
        if (mode == 1u) {
            // noise follows scene pixels
            let nuv = floor(i.clip.xy * du.model[1].w) + 0.5;
            c = sqrt(c);
            c += tri(bayer01(vec2<i32>(nuv))) * du.model[1].z;
            c *= c;
        }
        c = palettize(c);
    }
    if (du.ix.z != 0u) {
        c = goggles(c, uv, i.q, i.clip.xy);
    }
    c = mix(c, du.color.rgb, du.color.a);
    if (du.ix.y != 0u) {
        c = daltonize(c, du.ix.y);
    }
    c = pow(max(c * du.params.y, vec3<f32>(0.0)), vec3<f32>(du.params.x));
    if (mode == 0u) {
        // banding suppression
        c += (bayer01(vec2<i32>(i.clip.xy)) - 0.5) * du.model[1].z;
    }
    return vec4<f32>(c, 1.0);
}
