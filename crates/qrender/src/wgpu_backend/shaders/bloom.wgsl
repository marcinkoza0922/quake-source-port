// r_bloom chain (port of shaders/bloom.frag): mode 0 reads the scene glow
// (plus r_bloom_bright's bright scene colours) into the first level, mode 1
// halves a level with the 13-tap filter, mode 2 adds a 3x3 tent upsample of
// the smaller level, scaled by the weight, onto the next larger (blend One, One).
struct DrawU {
    model: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
    extra: vec4<f32>,
    ix: vec4<u32>,
};
@group(0) @binding(0) var<uniform> du: DrawU;
@group(1) @binding(0) var src: texture_2d<f32>;
@group(1) @binding(1) var scene: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;

@vertex fn vs_bloom(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0);
}

// du.model[0]: target pixel -> source uv scale (xy) and offset (zw)
// du.model[1]: source uv clamp min (xy), max (zw) (this view's region)
// du.model[2]: tap spacing in source uv (xy), mode, bright amount
// du.model[3].x: upsample weight
fn tap(uv0: vec2<f32>) -> vec3<f32> {
    let uv = clamp(uv0, du.model[1].xy, du.model[1].zw);
    var c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    if (du.model[2].z < 0.5 && du.model[2].w > 0.0) {
        let s = textureSampleLevel(scene, samp, uv, 0.0).rgb;
        // bloom.frag bright_part: soft knee on the brightest channel
        c += s * (smoothstep(0.35, 0.9, max(s.r, max(s.g, s.b))) * du.model[2].w);
    }
    return c;
}

@fragment fn fs_bloom(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag.xy * du.model[0].xy + du.model[0].zw;
    let t = du.model[2].xy;
    var c: vec3<f32>;
    if (du.model[2].z > 1.5) {
        c = tap(uv) * 4.0;
        c += (tap(uv + vec2<f32>(t.x, 0.0)) + tap(uv - vec2<f32>(t.x, 0.0)) + tap(uv + vec2<f32>(0.0, t.y)) + tap(uv - vec2<f32>(0.0, t.y))) * 2.0;
        c += tap(uv + t) + tap(uv - t) + tap(uv + vec2<f32>(t.x, -t.y)) + tap(uv + vec2<f32>(-t.x, t.y));
        c *= du.model[3].x / 16.0;
    } else {
        let e = tap(uv);
        let a = tap(uv + vec2<f32>(-2.0, 2.0) * t);
        let b = tap(uv + vec2<f32>(0.0, 2.0) * t);
        let cc = tap(uv + vec2<f32>(2.0, 2.0) * t);
        let d = tap(uv + vec2<f32>(-2.0, 0.0) * t);
        let f = tap(uv + vec2<f32>(2.0, 0.0) * t);
        let g = tap(uv + vec2<f32>(-2.0, -2.0) * t);
        let h = tap(uv + vec2<f32>(0.0, -2.0) * t);
        let i = tap(uv + vec2<f32>(2.0, -2.0) * t);
        let j = tap(uv + vec2<f32>(-1.0, 1.0) * t);
        let k = tap(uv + vec2<f32>(1.0, 1.0) * t);
        let l = tap(uv + vec2<f32>(-1.0, -1.0) * t);
        let m = tap(uv + vec2<f32>(1.0, -1.0) * t);
        c = e * 0.125 + (a + cc + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
    }
    return vec4<f32>(c, 1.0);
}
