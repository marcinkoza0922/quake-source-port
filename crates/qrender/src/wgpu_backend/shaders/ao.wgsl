// Screen-space ambient occlusion: port of the Vulkan backend's ao.frag
// (generation) and aoapply.frag (depth-aware blur multiplied into the scene).
// DEPTH_T / COLOR_T are replaced with the single- or multisampled texture
// types; textureLoad(t, px, 0) reads mip 0 or sample 0 of either.
struct AoU {
    model: mat4x4<f32>, // unused (DrawU layout)
    ao_rect: vec4<f32>, // view in the AO target: x, y, w, h (AO px)
    proj: vec4<f32>,    // tan half-fov x, y; world radius; intensity
    view: vec4<f32>,    // view in the scene target: x, y, w, h (scene px)
    ix: vec4<u32>,      // x quality (0..2)
    fog: vec4<f32>,     // x fog density (already /64)
};
@group(0) @binding(0) var<uniform> u: AoU;
@group(1) @binding(0) var depth_t: DEPTH_T;
@group(1) @binding(1) var tex1: COLOR_T;

@vertex fn vs_full(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let q = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(q.x * 2.0 - 1.0, 1.0 - q.y * 2.0, 0.0, 1.0);
}

fn linear_depth(d: f32) -> f32 {
    let n = 4.0;
    let f = 65536.0;
    return n * f / (f - d * (f - n));
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

// Hash without Sine (c) 2014 David Hoskins, MIT licence.
fn whitenoise01(p: vec2<f32>) -> f32 {
    var p3 = fract(p.xyx * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// AO-local pixel -> scene target pixel (clamped to the view).
fn scene_px(px: vec2<i32>) -> vec2<i32> {
    let per = u.view.zw / u.ao_rect.zw;
    let local = clamp(vec2<i32>((vec2<f32>(px) + 0.5) * per), vec2<i32>(0), vec2<i32>(u.view.zw) - 1);
    return vec2<i32>(u.view.xy) + local;
}

// View-space position (x right, y down, z forward) of an AO pixel centre; w = raw depth.
fn view_pos(px: vec2<i32>) -> vec4<f32> {
    let d = textureLoad(depth_t, scene_px(px), 0);
    let z = linear_depth(d);
    let ndc = (vec2<f32>(px) + 0.5) / u.ao_rect.zw * 2.0 - 1.0;
    return vec4<f32>(ndc * u.proj.xy * z, z, d);
}

// r = visibility (1 = unoccluded), g = view-space depth for the blur.
@fragment fn fs_ao(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(frag.xy) - vec2<i32>(u.ao_rect.xy);
    let pd = view_pos(px);
    let p = pd.xyz;
    if (pd.w >= 1.0) { return vec4<f32>(1.0, p.z, 0.0, 0.0); }
    // normal from the smaller depth difference on each axis (keeps edges)
    let l = view_pos(px - vec2<i32>(1, 0)).xyz;
    let r = view_pos(px + vec2<i32>(1, 0)).xyz;
    let t = view_pos(px - vec2<i32>(0, 1)).xyz;
    let b = view_pos(px + vec2<i32>(0, 1)).xyz;
    let dx = select(p - l, r - p, abs(r.z - p.z) < abs(p.z - l.z));
    let dy = select(p - t, b - p, abs(b.z - p.z) < abs(p.z - t.z));
    var n = normalize(cross(dy, dx));
    if (dot(n, p) > 0.0) { n = -n; }

    let quality = u.ix.x;
    let count = select(select(12, 8, quality == 1u), 6, quality == 0u);
    let radius = u.proj.z;
    // screen-space radius in AO pixels
    var rpx = radius / p.z * u.ao_rect.z * 0.5 / u.proj.x;
    if (rpx < 1.0) { return vec4<f32>(1.0, p.z, 0.0, 0.0); }
    rpx = min(rpx, u.ao_rect.w * 0.25);
    var spin: f32;
    if (quality == 0u) {
        spin = bayer01(px & vec2<i32>(1)) * 6.2831853;
    } else {
        spin = bayer01(px & vec2<i32>(3)) * 6.2831853 + whitenoise01(vec2<f32>(px)) * 0.5;
    }
    var occ = 0.0;
    let r2 = radius * radius;
    for (var i = 0; i < count; i++) {
        let a = (f32(i) + 0.5) / f32(count);
        let ang = spin + f32(i) * 2.3999632; // golden-angle spiral
        let h = a * rpx;
        let q = px + vec2<i32>(round(vec2<f32>(cos(ang), sin(ang)) * h));
        let v = view_pos(q).xyz - p;
        let vv = dot(v, v);
        let vn = dot(v, n);
        // cosine to the sample (minus a depth-scaled bias), fading to 0 at the radius
        let falloff = max(1.0 - vv / r2, 0.0);
        occ += falloff * max(vn * inverseSqrt(vv + 1e-4) - 0.1 - 0.0005 * p.z, 0.0);
    }
    var vis = clamp(1.0 - 1.5 * u.proj.w * occ / f32(count), 0.0, 1.0);
    // scene alpha masks AO (0 on sky); fade out with fog so fogged distance
    // isn't darkened toward black
    let mask = textureLoad(tex1, scene_px(px), 0).a;
    let fz = length(p) * u.fog.x;
    let fogf = select(1.0, clamp(exp(-fz * fz), 0.0, 1.0), u.fog.x > 0.0);
    vis = mix(1.0, vis, clamp(mask, 0.0, 1.0) * fogf);
    return vec4<f32>(vis, p.z, 0.0, 0.0);
}

// Depth-aware blur/upsample of the AO target (tex1) around AO coordinate
// ao_pos using exact full-resolution linear depth zc so silhouettes stay sharp.
fn ao_at(ao_pos: vec2<f32>, zc: f32) -> f32 {
    let taps = select(4, 2, u.ix.x == 0u);
    let base = vec2<i32>(floor(ao_pos - f32(taps / 2) + 0.5));
    let amax = vec2<i32>(u.ao_rect.zw) - 1;
    var sum = 0.0;
    var wsum = 0.0;
    for (var y = 0; y < taps; y++) {
        for (var x = 0; x < taps; x++) {
            let q = clamp(base + vec2<i32>(x, y), vec2<i32>(0), amax) + vec2<i32>(u.ao_rect.xy);
            let t = textureLoad(tex1, q, 0).rg;
            let w = 1.0 / (0.001 + abs(t.g - zc) / max(zc, 1.0) * 50.0);
            sum += t.r * w;
            wsum += w;
        }
    }
    return sum / wsum;
}

// Multiplied into the scene colour (blend dst * src, alpha kept).
@fragment fn fs_apply(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let d = textureLoad(depth_t, vec2<i32>(frag.xy), 0);
    if (d >= 1.0) { return vec4<f32>(1.0); }
    let zc = linear_depth(d);
    let local = frag.xy - u.view.xy;
    return vec4<f32>(vec3<f32>(ao_at(local * u.ao_rect.zw / u.view.zw, zc)), 1.0);
}
