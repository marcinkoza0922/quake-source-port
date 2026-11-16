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
    // liquid surface height above and kind (WorldVertex::liquid)
    @location(3) @interpolate(flat) liq: vec2<f32>,
};
@vertex fn vs_world(v: WorldIn) -> WorldOut {
    var o: WorldOut;
    let w = du.model * vec4<f32>(v.pos, 1.0);
    o.clip = fr.viewproj * w;
    o.st = v.st;
    o.lm = v.lm;
    o.wpos = w.xyz;
    o.liq = v.liq;
    return o;
}

// ---- enhanced water: ports of common.glsl's water_grad, lava_rings,
// water_dist_fade and caustic_tex ----
fn ripple_amp(i: i32) -> f32 {
    return water.amp[i >> 2u][i & 3];
}
// Liquid surface slope (dh/dx, dh/dy) at world point p on a surface at
// height p.z: optional gentle swell plus the expanding ripple rings.
fn water_grad(p: vec3<f32>, t: f32, swell: f32, rings: f32) -> vec2<f32> {
    var g = vec2<f32>(0.0);
    if (swell > 0.0) {
        // a few long, slow directional waves
        let w = array<vec4<f32>, 3>(vec4<f32>(0.80, 0.60, 0.090, 1.3),
                                    vec4<f32>(-0.45, 0.89, 0.130, -1.7),
                                    vec4<f32>(0.31, -0.95, 0.210, 2.1));
        for (var i = 0; i < 3; i++) {
            let ph = dot(w[i].xy, p.xy) * w[i].z + t * w[i].w;
            g += w[i].xy * (cos(ph) * w[i].z * 0.9 / (1.0 + f32(i)));
        }
        g *= swell;
    }
    let n = select(0, i32(water.params.z), rings > 0.0);
    var gr = vec2<f32>(0.0);
    for (var i = 0; i < n; i++) {
        let r = water.ripples[i];
        let age = t - r.w;
        if (age < 0.0 || age > 2.5 || abs(p.z - r.z) > 24.0) { continue; }
        let d = p.xy - r.xy;
        let front = age * 85.0;
        let sig = 5.0 + age * 7.0;
        let reach = 3.0 * sig;
        let max_r = front + reach;
        if (abs(d.x) > max_r || abs(d.y) > max_r) { continue; }
        let d2 = dot(d, d);
        let min_r = max(front - reach, 0.0);
        if (d2 > max_r * max_r || d2 < min_r * min_r) { continue; }
        let dist = sqrt(d2);
        let x = dist - front;
        let env = exp(-x * x / (2.0 * sig * sig));
        let fade = 1.0 - age / 2.5;
        let amp = ripple_amp(i) * fade * fade / (1.0 + front * 0.03);
        let k = 0.4;
        // h = amp * env * cos(k x); slope along the ring's radius
        let dh = amp * env * (-k * sin(k * x) - cos(k * x) * x / (sig * sig));
        gr += dh * d / max(dist, 1e-3);
    }
    return g + gr * rings;
}
// Lava: one slow, broad swell per event that glows as it passes.
// Returns (dh/dx, dh/dy, heat).
fn lava_rings(p: vec3<f32>, t: f32, rings: f32) -> vec3<f32> {
    let n = select(0, i32(water.params.z), rings > 0.0);
    var g = vec2<f32>(0.0);
    var heat = 0.0;
    for (var i = 0; i < n; i++) {
        let r = water.ripples[i];
        let age = t - r.w;
        if (age < 0.0 || age > 2.5 || abs(p.z - r.z) > 24.0) { continue; }
        let d = p.xy - r.xy;
        // slows as it spreads: ~40 u/s at first, ~60 units total
        let front = 60.0 * (1.0 - exp(-age * 0.9));
        let sig = 8.0 + age * 5.0;
        let max_r = front + 3.0 * sig;
        if (abs(d.x) > max_r || abs(d.y) > max_r) { continue; }
        let d2 = dot(d, d);
        if (d2 > max_r * max_r) { continue; }
        let dist = sqrt(d2);
        let x = dist - front;
        let env = exp(-x * x / (2.0 * sig * sig));
        let fade = 1.0 - age / 2.5;
        let amp = ripple_amp(i) * fade * fade;
        let dh = -3.0 * amp * env * x / (sig * sig);
        g += dh * d / max(dist, 1e-3);
        // molten core exposed at the impact, cooling quickly, plus the ridge
        let core = exp(-d2 / (2.0 * 12.0 * 12.0)) * exp(-age * 1.8);
        heat += amp * (0.35 * env + 0.6 * core);
    }
    return vec3<f32>(g * rings, min(heat * rings, 1.5));
}
// Distance fade for water ripples and caustics by quality tier (0 low, 1 high).
fn water_dist_fade(wpos: vec3<f32>, qual: i32) -> f32 {
    let fade_d = select(1024.0, 640.0, qual == 0);
    let max_d = select(1536.0, 1024.0, qual == 0);
    let delta = wpos - fr.eye.xyz;
    let d2 = dot(delta, delta);
    if (d2 <= fade_d * fade_d) { return 1.0; }
    if (d2 >= max_d * max_d) { return 0.0; }
    return 1.0 - smoothstep(fade_d, max_d, sqrt(d2));
}
// Tiling caustic texture (two scrolling Worley layers).
fn caustic_tex(q: vec2<f32>, t: f32) -> f32 {
    let uv = q / (56.0 * 4.0);
    let a = textureSampleLevel(caustic_t, caustic_s, uv + vec2<f32>(t * 0.035, t * 0.024), 0.0).r;
    let b = textureSampleLevel(caustic_t, caustic_s, uv * 1.31 + vec2<f32>(0.37, 0.19) + vec2<f32>(-t * 0.029, t * 0.038), 0.0).g;
    return clamp(0.55 * (a + b) + 0.9 * a * b, 0.0, 1.5);
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
    var rgb = c.rgb * light;
    // caustics on submerged faces (not lava); water bluish, slime green
    let nf = cross(dpdx(i.wpos), dpdy(i.wpos));
    let lk = u32(i.liq.y + 0.5);
    if (water.params.y > 0.0 && (lk & 3u) != 0u && (lk & 3u) != 2u && i.wpos.z < i.liq.x) {
        let dist_fade = water_dist_fade(i.wpos, i32(water.params.w + 0.5));
        if (dist_fade > 0.0) {
            let depth = i.liq.x - i.wpos.z;
            // light comes in at a slant, so walls get streaks shifted by depth
            let q = floor(i.wpos.xy + depth * vec2<f32>(0.55, 0.35)) + 0.5;
            let cst = caustic_tex(q, fr.eye.w * 0.9);
            let n = normalize(nf);
            let fade = clamp(depth / 8.0, 0.0, 1.0) * exp(-depth / 384.0) * mix(0.55, 1.0, abs(n.z))
                * select(1.0, 0.4, (lk & 4u) != 0u) * dist_fade;
            let tint = select(vec3<f32>(0.75, 0.92, 1.0), vec3<f32>(0.55, 1.0, 0.35), (lk & 3u) == 3u);
            rgb += c.rgb * light * tint * (cst * fade * water.params.y * 1.8);
        }
    }
    rgb += fb.rgb * du.params.y;
    rgb = apply_fog(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), i.wpos);
    return vec4<f32>(rgb, du.color.x);
}
// du.params.w: liquid kind (1 water, 2 lava, 3 slime, 4 tele)
@fragment fn fs_water(i: WorldOut) -> @location(0) vec4<f32> {
    let t = fr.eye.w;
    let kind = u32(du.params.w + 0.5);
    let nf = cross(dpdx(i.wpos), dpdy(i.wpos));
    var st = i.st;
    var shade = 1.0;
    var heat = 0.0; // lava only: disturbed crust glows hotter
    // r_waterripples: the height field bends the classic warp lookup and
    // shades the slopes; teleporters and non-horizontal sheets stay classic
    if (water.params.x > 0.0 && kind >= 1u && kind <= 3u) {
        let dist_fade = water_dist_fade(i.wpos, i32(water.params.w + 0.5));
        let horiz = abs(nf.z) * inverseSqrt(max(dot(nf, nf), 1e-12));
        if (dist_fade > 0.0 && horiz > 0.7) {
            let k = water.params.x * dist_fade;
            if (kind == 2u) {
                // lava is thick: a slow heave, viscous swells that glow
                let l = lava_rings(i.wpos, t, k);
                let g = water_grad(i.wpos, t * 0.3, 0.2 * min(k, 1.5), 0.0) + l.xy;
                st += g * 3.0;
                shade = 1.0 + clamp(dot(g, vec2<f32>(0.6, 0.45)), -0.5, 0.5) * 0.8;
                heat = l.z;
            } else {
                let g = water_grad(i.wpos, t, 0.5 * min(k, 1.5), 2.5 * k);
                st += g * 5.0;
                let slope = dot(g, vec2<f32>(0.6, 0.45));
                shade = 1.0 + clamp(slope, -0.6, 0.6) * 1.4 + smoothstep(0.25, 0.7, length(g)) * 0.2;
            }
        }
    }
    st += 8.0 * sin(st.yx * 0.125 + vec2<f32>(t));
    var c = textureSample(t0, s0, st / 64.0).rgb;
    if (heat > 0.0) {
        // toward yellow-white where the crust is broken
        let hot = c * 1.5 + vec3<f32>(0.35, 0.18, 0.02);
        c = mix(c, hot, clamp(heat, 0.0, 1.0));
    }
    return vec4<f32>(apply_fog(c * shade, i.wpos), du.color.x);
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
// fte.frag fog: alpha blending fogs toward the fog colour, additive and
// modulating modes toward black (du.params.y = 1)
fn poly_fog(c: vec4<f32>, wpos: vec3<f32>) -> vec4<f32> {
    if (fr.fog.w <= 0.0) { return c; }
    let z = length(wpos - fr.eye.xyz) * fr.fog.w;
    let f = clamp(exp(-z * z), 0.0, 1.0);
    return vec4<f32>(mix(fr.fog.rgb * (1.0 - du.params.y), c.rgb, f), c.a);
}
// du.params.x: alpha test threshold; y: fog toward black
@fragment fn fs_poly(i: PolyOut) -> @location(0) vec4<f32> {
    let c = textureSample(t0, s0, i.uv) * i.color;
    if (c.a <= du.params.x) { discard; }
    return poly_fog(c, i.wpos);
}
