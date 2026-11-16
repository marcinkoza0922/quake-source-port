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
@group(1) @binding(2) var t2: texture_2d<f32>;
@group(1) @binding(3) var s0: sampler;

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
// du.extra: uv rectangle (u0, v0, u1, v1) of the view in the scene texture
// du.color: polyblend; du.params: x gamma, y contrast, z warp clock (< 0 off), w aspect
// du.model[0]: motion blur scale x, y (0 = off), speed blur, motion blur quality
// du.model[1].xy: output size
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
    c = mix(c, du.color.rgb, du.color.a);
    c = pow(max(c * du.params.y, vec3<f32>(0.0)), vec3<f32>(du.params.x));
    return vec4<f32>(c, 1.0);
}
