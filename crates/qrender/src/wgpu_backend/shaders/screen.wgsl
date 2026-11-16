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
// du.extra: uv rectangle (u0, v0, u1, v1) of the view in the scene texture
// du.color: polyblend; du.params: x gamma, y contrast, z warp clock (< 0 off), w aspect
@fragment fn fs_post(i: PostOut) -> @location(0) vec4<f32> {
    var q = i.q;
    if (du.params.z >= 0.0) {
        q = water_warp(q, du.params.z, du.params.w);
    }
    var c = textureSample(t0, s0, mix(du.extra.xy, du.extra.zw, q)).rgb;
    c = mix(c, du.color.rgb, du.color.a);
    c = pow(max(c * du.params.y, vec3<f32>(0.0)), vec3<f32>(du.params.x));
    return vec4<f32>(c, 1.0);
}
