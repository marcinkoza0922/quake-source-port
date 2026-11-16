// r_softemu palette lookup table (port of convert::palette_lut /
// GLPalette_UpdateLookupTable): the nearest palette index of every 7-bit RGB
// cell, packed 4 per u32 in the order r + g*128 + b*16384. Each invocation
// writes one u32 (4 consecutive red values).
@group(0) @binding(0) var pal: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> lut: array<u32>;
// x: metric (0 naive RGB, 1 Riemersma on linear RGB, 2 weighted OKLab)
@group(0) @binding(2) var<uniform> metric: vec4<u32>;

var<workgroup> cands: array<vec3<f32>, 256>;

fn srgb_to_linear(v: vec3<f32>) -> vec3<f32> {
    return select(v / 12.92, pow((v + 0.055) / 1.055, vec3<f32>(2.4)), v > vec3<f32>(0.04045));
}
fn cbrt(x: f32) -> f32 {
    return select(0.0, pow(x, 1.0 / 3.0), x > 0.0);
}
fn oklab(c: vec3<f32>) -> vec3<f32> {
    let l = cbrt(0.41222146 * c.x + 0.53633255 * c.y + 0.051445995 * c.z);
    let m = cbrt(0.2119035 * c.x + 0.6806995 * c.y + 0.10739696 * c.z);
    let s = cbrt(0.08830246 * c.x + 0.28171885 * c.y + 0.6299787 * c.z);
    return vec3<f32>(
        0.21045426 * l + 0.7936178 * m - 0.004072047 * s,
        1.9779985 * l - 2.4285922 * m + 0.4505937 * s,
        0.025904037 * l + 0.78277177 * m - 0.80867577 * s,
    );
}
fn norm(c: vec3<f32>) -> vec3<f32> {
    var v = c;
    if (metric.x >= 1u) { v = srgb_to_linear(v); }
    if (metric.x >= 2u) { v = oklab(v); }
    return v;
}
fn dist(a: vec3<f32>, b: vec3<f32>) -> f32 {
    var d = b - a;
    if (metric.x == 1u) {
        let rmean = (a.x + b.x) * 0.5;
        return d.x * d.x * (2.0 + rmean) + d.y * d.y * 4.0 + d.z * d.z * (3.0 - rmean);
    }
    if (metric.x == 2u) {
        d.x *= 1.25;
        d.z *= 1.5;
    }
    return dot(d, d);
}

@compute @workgroup_size(64)
fn cs_lut(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    for (var k = li; k < 256u; k += 64u) {
        // palette bytes, exactly as the CPU table sees them
        let p = round(textureLoad(pal, vec2<i32>(i32(k), 0), 0).rgb * 255.0);
        cands[k] = norm(p / 255.0);
    }
    workgroupBarrier();
    let word = gid.x;
    if (word >= 128u * 128u * 128u / 4u) { return; }
    var packed = 0u;
    for (var j = 0u; j < 4u; j++) {
        let i = word * 4u + j;
        let cell = vec3<u32>(i & 127u, (i >> 7u) & 127u, i >> 14u);
        let t = norm(vec3<f32>((cell << vec3<u32>(1u)) + (cell >> vec3<u32>(6u))) / 255.0);
        var best = 0u;
        var bd = 3.4e38;
        for (var k = 0u; k < 256u; k++) {
            let d = dist(t, cands[k]);
            if (d < bd) { bd = d; best = k; }
        }
        packed |= best << (j * 8u);
    }
    lut[word] = packed;
}
