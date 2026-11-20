// Sky: two scrolling layers (t4 solid, t5 alpha) or a six-face skybox (t0..t5).
@group(1) @binding(0) var t0: texture_2d<f32>;
@group(1) @binding(1) var t1: texture_2d<f32>;
@group(1) @binding(2) var t2: texture_2d<f32>;
@group(1) @binding(3) var t3: texture_2d<f32>;
@group(1) @binding(4) var t4: texture_2d<f32>;
@group(1) @binding(5) var t5: texture_2d<f32>;
@group(1) @binding(6) var s0: sampler;

struct SkyIn {
    @location(0) pos: vec3<f32>,
    @location(1) st: vec2<f32>,
    @location(2) lm: vec3<f32>,
    @location(3) liq: vec2<f32>,
};
struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) wpos: vec3<f32>,
};
@vertex fn vs_sky(v: SkyIn) -> SkyOut {
    var o: SkyOut;
    let w = du.model * vec4<f32>(v.pos, 1.0);
    o.clip = fr.viewproj * w;
    o.wpos = w.xyz;
    return o;
}
fn skybox(d: vec3<f32>) -> vec4<f32> {
    let a = abs(d);
    var face = 0u;
    var s = 0.0;
    var t = 0.0;
    var ma = 1.0;
    if (a.x >= a.y && a.x >= a.z) {
        ma = a.x;
        if (d.x > 0.0) { face = 0u; s = -d.y; t = d.z; } else { face = 2u; s = d.y; t = d.z; }
    } else if (a.y >= a.z) {
        ma = a.y;
        if (d.y > 0.0) { face = 1u; s = d.x; t = d.z; } else { face = 3u; s = -d.x; t = d.z; }
    } else {
        ma = a.z;
        if (d.z > 0.0) { face = 4u; s = -d.y; t = -d.x; } else { face = 5u; s = -d.y; t = d.x; }
    }
    var uv = vec2<f32>(s, t) / ma * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;
    uv = clamp(uv, vec2<f32>(1.0 / 512.0), vec2<f32>(1.0 - 1.0 / 512.0));
    let c0 = textureSampleLevel(t0, s0, uv, 0.0);
    let c1 = textureSampleLevel(t1, s0, uv, 0.0);
    let c2 = textureSampleLevel(t2, s0, uv, 0.0);
    let c3 = textureSampleLevel(t3, s0, uv, 0.0);
    let c4 = textureSampleLevel(t4, s0, uv, 0.0);
    let c5 = textureSampleLevel(t5, s0, uv, 0.0);
    var r = array<vec4<f32>, 6>(c0, c1, c2, c3, c4, c5);
    return r[face];
}
// Sky wind (ironwail's sky_cubemap ANIM): two drifting copies of the box,
// cross-faded over the loop, over the still box by their alpha.
fn skybox_wind(d: vec3<f32>) -> vec3<f32> {
    let base = skybox(d);
    if (all(fr.wind.xyz == vec3<f32>(0.0))) { return base.rgb; }
    let p1 = fr.wind.w;
    let p2 = fract(p1) - 0.5;
    let blend = abs(p1 * 2.0);
    let l1 = skybox(d + p1 * fr.wind.xyz);
    let l2 = skybox(d + p2 * fr.wind.xyz);
    let a1 = l1.a * (1.0 - blend);
    let a2 = l2.a * blend;
    let rgb = l1.rgb * a1 + l2.rgb * a2;
    return base.rgb * (1.0 - (a1 + a2)) + rgb;
}
@fragment fn fs_sky(i: SkyOut) -> @location(0) vec4<f32> { return sky_frag(i); }
@fragment fn fs_sky_glow(i: SkyOut) -> Frag { return Frag(sky_frag(i), vec4<f32>(0.0)); }
fn sky_frag(i: SkyOut) -> vec4<f32> {
    if ((fr.softf.x & 32u) != 0u) { return vec4<f32>(0.0); }
    var d = i.wpos - fr.eye.xyz;
    var rgb: vec3<f32>;
    if (fr.sky.y > 0.5) {
        rgb = skybox_wind(normalize(d));
    } else {
        d.z *= 3.0;
        let uv = normalize(d).xy * (189.0 / 64.0);
        let t = fr.eye.w;
        let solid = textureSampleLevel(t4, s0, uv + t / 16.0, 0.0);
        var alpha = textureSampleLevel(t5, s0, uv + t / 8.0, 0.0);
        alpha.a *= fr.sky.z;
        rgb = mix(solid.rgb, alpha.rgb, alpha.a);
    }
    if (fr.fog.w > 0.0) { rgb = mix(rgb, fr.fog.rgb, fr.sky.x); }
    if (thermal()) { rgb = vec3<f32>(0.02); }
    // alpha 0: no SSAO on the sky
    return vec4<f32>(rgb, 0.0);
}
