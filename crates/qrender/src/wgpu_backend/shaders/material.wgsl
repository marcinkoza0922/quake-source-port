@group(1) @binding(0) var t0: texture_2d<f32>;
@group(1) @binding(1) var t1: texture_2d<f32>;
@group(1) @binding(2) var t2: texture_2d<f32>;
@group(1) @binding(3) var s0: sampler;

// ---- world (lightmapped brush surfaces) and liquids ----
struct WorldIn {
    @location(0) pos: vec3<f32>,
    @location(1) st: vec2<f32>,
    @location(2) lm: vec3<f32>,
    @location(3) liq: vec2<f32>,
};
struct WorldOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) st: vec2<f32>,
    @location(1) lm: vec3<f32>,
    @location(2) wpos: vec3<f32>,
};
@vertex fn vs_world(v: WorldIn) -> WorldOut {
    var o: WorldOut;
    let w = du.model * vec4<f32>(v.pos, 1.0);
    o.clip = fr.viewproj * w;
    o.st = v.st;
    o.lm = v.lm;
    o.wpos = w.xyz;
    return o;
}
// du.params: x alpha test, y has fullbright, z unlit (fullbright material); du.color.x alpha
@fragment fn fs_world(i: WorldOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(t0));
    let uv = i.st / size;
    let c = textureSample(t0, s0, uv);
    let fb = textureSample(t1, s0, uv);
    let lmc = textureSample(lightmap, lm_samp, i.lm.xy, i32(i.lm.z + 0.5)).rgb * 2.0;
    if (du.params.x > 0.5 && c.a < 0.666) { discard; }
    var light = lmc;
    if (du.params.z > 0.5) { light = vec3<f32>(1.0); }
    var rgb = c.rgb * light + fb.rgb * du.params.y;
    rgb = apply_fog(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), i.wpos);
    return vec4<f32>(rgb, du.color.x);
}
@fragment fn fs_water(i: WorldOut) -> @location(0) vec4<f32> {
    let t = fr.eye.w;
    let st = i.st + 8.0 * sin(i.st.yx * 0.125 + vec2<f32>(t));
    let c = textureSample(t0, s0, st / 64.0);
    return vec4<f32>(apply_fog(c.rgb, i.wpos), du.color.x);
}

// ---- alias models (CPU-posed, world space) ----
struct AliasIn {
    @location(0) pos: vec3<f32>,
    @location(1) st: vec2<f32>,
    @location(2) shade: vec3<f32>,
};
struct AliasOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) st: vec2<f32>,
    @location(1) shade: vec3<f32>,
    @location(2) wpos: vec3<f32>,
};
@vertex fn vs_alias(v: AliasIn) -> AliasOut {
    var o: AliasOut;
    o.clip = fr.viewproj * vec4<f32>(v.pos, 1.0);
    o.st = v.st;
    o.shade = v.shade;
    o.wpos = v.pos;
    return o;
}
fn ramp(row: u32, k: u32) -> vec3<f32> {
    let idx = select(row * 16u + 15u - k, row * 16u + k, row < 8u);
    return textureLoad(palette, vec2<i32>(i32(idx), 0), 0).rgb;
}
// du.params: x alpha test, y has fullbright, z colormapped; du.ix: shirt row, pants row; du.color.w alpha
@fragment fn fs_alias(i: AliasOut) -> @location(0) vec4<f32> {
    let c = textureSample(t0, s0, i.st);
    let fb = textureSample(t1, s0, i.st);
    if (du.params.x > 0.5 && c.a < 0.666) { discard; }
    var rgb = c.rgb;
    if (du.params.z > 0.5) {
        let dims = vec2<f32>(textureDimensions(t2));
        let p = vec2<i32>(clamp(fract(i.st) * dims, vec2<f32>(0.0), dims - 1.0));
        let m = textureLoad(t2, p, 0).rg * 255.0 + 0.5;
        let top = u32(m.x);
        let bot = u32(m.y);
        if (top > 0u) { rgb += ramp(du.ix.x, top - 1u) - ramp(1u, top - 1u); }
        if (bot > 0u) { rgb += ramp(du.ix.y, bot - 1u) - ramp(6u, bot - 1u); }
    }
    rgb = rgb * i.shade * 2.0 + fb.rgb * du.params.y;
    rgb = apply_fog(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), i.wpos);
    return vec4<f32>(rgb, du.color.w);
}

// ---- textured polygons: sprites, particles, FTE particle geometry ----
struct PolyIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct PolyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) wpos: vec3<f32>,
};
@vertex fn vs_poly(v: PolyIn) -> PolyOut {
    var o: PolyOut;
    o.clip = fr.viewproj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    o.wpos = v.pos;
    return o;
}
// du.params.x: alpha test threshold
@fragment fn fs_poly(i: PolyOut) -> @location(0) vec4<f32> {
    let c = textureSample(t0, s0, i.uv) * i.color;
    if (c.a <= du.params.x) { discard; }
    return vec4<f32>(apply_fog(c.rgb, i.wpos), c.a);
}
