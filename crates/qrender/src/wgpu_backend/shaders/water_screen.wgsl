// Enhanced Water screen effects (r_water_refraction, r_water_droplets): a
// port of shaders/water_screen.glsl, appended to screen.wgsl for fs_post.
// q: 0..1 across the view (y down); p: q with x scaled by the aspect.

fn ws_hash(x0: u32) -> u32 {
    var x = x0;
    x ^= x >> 16u; x *= 0x7feb352du;
    x ^= x >> 15u; x *= 0x846ca68bu;
    x ^= x >> 16u;
    return x;
}
fn ws_rand4(s: u32) -> vec4<f32> {
    let a = ws_hash(s);
    let b = ws_hash(a ^ 0x9e3779b9u);
    let c = ws_hash(b ^ 0x85ebca6bu);
    let d = ws_hash(c ^ 0xc2b2ae35u);
    return vec4<f32>(vec4<u32>(a, b, c, d) >> vec4<u32>(8u)) * (1.0 / 16777216.0);
}
fn ws_cell(seed: u32, id: vec2<i32>, layer: u32) -> u32 {
    return ws_hash(seed ^ ws_hash(u32(id.x) * 73856093u ^ u32(id.y) * 19349663u ^ layer * 83492791u));
}

fn ws_amp(kind: u32, aspect: f32) -> vec2<f32> {
    let a = select(0.006, 0.008, kind == 3u);
    return vec2<f32>(a / aspect, a);
}
fn ws_wave(q: vec2<f32>, t: f32, kind: u32, aspect: f32) -> vec2<f32> {
    let s = t * select(select(1.0, 0.6, kind == 2u), 0.4, kind == 3u);
    let p = vec2<f32>(q.x * aspect, q.y);
    let o = vec2<f32>(
        sin(p.y * 7.0 + s * 1.7) * 0.5 + sin((p.x + p.y) * 13.0 - s * 2.4) * 0.3 + sin(p.x * 23.0 - p.y * 6.0 + s * 3.5) * 0.2,
        sin(p.x * 6.0 - s * 1.5) * 0.5 + sin((p.x - p.y) * 11.0 + s * 2.1) * 0.3 + sin(p.y * 25.0 + p.x * 5.0 - s * 3.1) * 0.2,
    );
    return o * ws_amp(kind, aspect);
}

struct WsDrop { off: vec2<f32>, m: f32, spec: f32, rim: f32 };

// st > 1 stretches it vertically (a runner sliding down); h (0..1) picks an
// irregular outline, a random squash and a few lobes: drops on glass are rarely round
fn ws_drop(best: ptr<function, WsDrop>, p: vec2<f32>, c: vec2<f32>, r: f32, px: f32, st: f32, h: f32) {
    if (r <= 0.0) { return; }
    let rel = p - c;
    // Fast AABB reject before division and lobe trig.
    if (abs(rel.x) >= r * 1.8 || abs(rel.y) >= r * st * 1.6) { return; }
    var d = rel / r;
    d.y /= st;
    d.x *= 0.75 + 0.5 * fract(h * 7.7);
    d.y *= select(1.15, 0.85, d.y > 0.0);
    var l = length(d);
    if (l >= 1.3) { return; }
    let a = atan2(d.y, d.x + 1e-6);
    l *= 1.0 + 0.1 * sin(2.0 * a + h * 6.283) + 0.07 * sin(3.0 * a + h * 17.0) + 0.04 * sin(5.0 * a + h * 29.0);
    if (l >= 1.0) { return; }
    let m = 1.0 - smoothstep(1.0 - clamp(1.5 * px / r, 0.25, 0.5), 1.0, l);
    if (m <= (*best).m) { return; }
    (*best).m = m;
    (*best).off = -d * r * 2.2;
    (*best).spec = smoothstep(0.32, 0.05, length(d - vec2<f32>(-0.32, -0.42))) * 0.9
                 + smoothstep(0.35, 0.0, abs(l - 0.78)) * smoothstep(0.2, 0.9, d.y) * 0.25;
    (*best).rim = smoothstep(0.55, 1.0, l);
}

fn ws_droplets(q: vec2<f32>, age: f32, seed: u32, high: bool, aspect: f32, px: f32) -> WsDrop {
    var b = WsDrop(vec2<f32>(0.0), 0.0, 0.0, 0.0);
    let p = vec2<f32>(q.x * aspect, q.y);
    let fade = 1.0 - smoothstep(0.8, 1.8, age);
    let edge = age * age * 8.0 - 0.06 + sin(p.x * 13.0 + f32(seed & 255u)) * 0.03 + sin(p.x * 31.0) * 0.012;
    if (p.y > edge) {
        let k = clamp(age * 4.0, 0.0, 1.0);
        b.off = vec2<f32>(sin(p.y * 17.0 + p.x * 5.0 - age * 9.0), sin(p.x * 15.0 - age * 7.0)) * (0.006 * (1.0 - 0.5 * k));
        b.m = 0.7;
        b.spec = smoothstep(0.03, 0.0, p.y - edge) * 0.3;
        b.rim = 0.15;
        return b;
    }
    let layers = select(1, 2, high);
    for (var layer = 0; layer < layers; layer++) {
        let cs = select(1.0 / 26.0, 1.0 / 11.0, layer == 0);
        let id = vec2<i32>(floor(p / cs));
        let r = ws_rand4(ws_cell(seed, id, u32(layer)));
        if (r.z < select(0.2, 0.28, layer == 0)) {
            let life = select(0.2, 0.35, layer == 0) + r.w * select(0.5, 0.9, layer == 0);
            let k = clamp(1.0 - age / life, 0.0, 1.0);
            let rad = cs * (0.07 + 0.13 * fract(r.x * 7.31 + r.y * 3.17)) * sqrt(smoothstep(0.0, 0.35, k));
            ws_drop(&b, p, (vec2<f32>(id) + 0.34 + 0.32 * r.xy) * cs, rad, px, 1.0, fract(r.x * 13.1 + r.w * 7.3));
        }
    }
    for (var layer = 0; layer < layers; layer++) {
        let w = select(1.0 / 4.3, 1.0 / 6.0, layer == 0);
        let cx = p.x / w + f32(layer) * 0.5;
        let col = i32(floor(cx));
        let r = ws_rand4(ws_cell(seed, vec2<i32>(col, 0), 7u + u32(layer)));
        if (r.w > 0.35) { continue; }
        let rad = w * (0.1 + 0.05 * r.z);
        let y0 = 0.05 + 0.55 * r.y;
        let start = 0.1 + 0.5 * fract(r.x * 5.7 + r.z * 2.3);
        let tt = max(age - start, 0.0);
        let y = y0 + 0.15 * tt + 0.6 * tt * tt;
        let phase = r.x * 6.283;
        let x0 = (f32(col) + 0.5 + 0.35 * (r.x - 0.5) - f32(layer) * 0.5) * w;
        let sway = 0.25 * rad / w;
        let x = x0 + sin(y * 19.0 + phase) * w * sway;
        let st = select(1.0, 1.0 + min((0.08 + 0.6 * tt) * 0.9, 0.6), tt > 0.0);
        ws_drop(&b, p, vec2<f32>(x, y), rad * (1.0 - 0.2 * smoothstep(0.0, 1.5, tt)), px, st, r.z);
        if (tt > 0.0 && p.y > y0 && p.y < y - rad) {
            let ts = 0.028;
            let ty = floor(p.y / ts);
            let tr = ws_rand4(ws_cell(seed, vec2<i32>(col, i32(ty)), 13u + u32(layer)));
            if (tr.x < 0.6) {
                let cy = (ty + 0.5) * ts;
                let trad = rad * (0.22 + 0.22 * tr.y) * (1.0 - smoothstep(0.2, 0.9, tt));
                ws_drop(&b, p, vec2<f32>(x0 + sin(cy * 19.0 + phase) * w * sway + (tr.z - 0.5) * rad * 0.9, cy), trad, px, 1.0, tr.w);
            }
        }
    }
    b.m *= 0.45 * fade;
    return b;
}

fn ws_shade(c0: vec3<f32>, d: WsDrop, kind: u32) -> vec3<f32> {
    var c = c0 * mix(1.0, 0.9, d.rim * d.m);
    c += d.spec * d.m * 0.12;
    if (kind == 2u) {
        c = mix(c, c * vec3<f32>(0.55, 1.0, 0.45) + vec3<f32>(0.0, 0.03, 0.0), 0.35 * d.m);
    }
    return c;
}
