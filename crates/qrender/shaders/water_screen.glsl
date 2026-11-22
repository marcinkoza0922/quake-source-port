// Enhanced Water screen effects for post.frag (r_water_refraction,
// r_water_droplets). Ported line for line to wgpu_backend/shaders/screen.wgsl.
// q: 0..1 across the view (y down); p: q with x scaled by the aspect, so
// drops are round and sizes are fractions of the view height.

uint ws_hash(uint x) {
    x ^= x >> 16; x *= 0x7feb352du;
    x ^= x >> 15; x *= 0x846ca68bu;
    x ^= x >> 16;
    return x;
}
vec4 ws_rand4(uint s) {
    uint a = ws_hash(s), b = ws_hash(a ^ 0x9e3779b9u), c = ws_hash(b ^ 0x85ebca6bu), d = ws_hash(c ^ 0xc2b2ae35u);
    return vec4(uvec4(a, b, c, d) >> 8u) * (1.0 / 16777216.0);
}
uint ws_cell(uint seed, ivec2 id, uint layer) {
    return ws_hash(seed ^ ws_hash(uint(id.x) * 73856093u ^ uint(id.y) * 19349663u ^ layer * 83492791u));
}

// Underwater refraction: travelling waves bend the view, wider and slower in
// thicker liquids. Returns the offset in q units (at most `ws_amp`).
vec2 ws_amp(uint kind, float aspect) {
    float a = kind == 3u ? 0.008 : kind == 2u ? 0.006 : 0.006;
    return vec2(a / aspect, a);
}
vec2 ws_wave(vec2 q, float t, uint kind, float aspect) {
    float s = t * (kind == 3u ? 0.4 : kind == 2u ? 0.6 : 1.0);
    vec2 p = vec2(q.x * aspect, q.y);
    vec2 o;
    o.x = sin(p.y * 7.0 + s * 1.7) * 0.5 + sin((p.x + p.y) * 13.0 - s * 2.4) * 0.3 + sin(p.x * 23.0 - p.y * 6.0 + s * 3.5) * 0.2;
    o.y = sin(p.x * 6.0 - s * 1.5) * 0.5 + sin((p.x - p.y) * 11.0 + s * 2.1) * 0.3 + sin(p.y * 25.0 + p.x * 5.0 - s * 3.1) * 0.2;
    return o * ws_amp(kind, aspect);
}

// One drop: refraction offset (p units), coverage, highlight, rim shade.
struct WsDrop { vec2 off; float m; float spec; float rim; };

// st > 1 stretches it vertically (a runner sliding down); h (0..1) picks an
// irregular outline, a random squash and a few lobes: drops on glass are rarely round
void ws_drop(inout WsDrop best, vec2 p, vec2 c, float r, float px, float st, float h) {
    if (r <= 0.0) return;
    vec2 d = (p - c) / r;
    d.y /= st;
    d.x *= 0.75 + 0.5 * fract(h * 7.7);
    d.y *= d.y > 0.0 ? 0.85 : 1.15; // heavier at the bottom
    float l = length(d);
    if (l >= 1.3) return;
    float a = atan(d.y, d.x + 1e-6);
    l *= 1.0 + 0.1 * sin(2.0 * a + h * 6.283) + 0.07 * sin(3.0 * a + h * 17.0) + 0.04 * sin(5.0 * a + h * 29.0);
    if (l >= 1.0) return;
    float m = 1.0 - smoothstep(1.0 - clamp(1.5 * px / r, 0.25, 0.5), 1.0, l);
    if (m <= best.m) return;
    best.m = m;
    // a convex drop shows a smaller, upside-down view of what is behind it
    best.off = -d * r * 2.2;
    best.spec = smoothstep(0.32, 0.05, length(d - vec2(-0.32, -0.42))) * 0.9
              + smoothstep(0.35, 0.0, abs(l - 0.78)) * smoothstep(0.2, 0.9, d.y) * 0.25;
    best.rim = smoothstep(0.55, 1.0, l);
}

// Droplets after leaving water or slime: a film that drains down the view,
// beads left behind that dry out, and runners that slide down with a trail.
// age: seconds since leaving; px: one pixel in p units.
WsDrop ws_droplets(vec2 q, float age, uint seed, bool high, float aspect, float px) {
    WsDrop b = WsDrop(vec2(0.0), 0.0, 0.0, 0.0);
    vec2 p = vec2(q.x * aspect, q.y);
    float fade = 1.0 - smoothstep(0.8, 1.8, age);
    // the film: gone from the top down within 0.4 s
    float edge = age * age * 8.0 - 0.06 + sin(p.x * 13.0 + float(seed & 255u)) * 0.03 + sin(p.x * 31.0) * 0.012;
    if (p.y > edge) {
        float k = clamp(age * 4.0, 0.0, 1.0);
        b.off = vec2(sin(p.y * 17.0 + p.x * 5.0 - age * 9.0), sin(p.x * 15.0 - age * 7.0)) * (0.006 * (1.0 - 0.5 * k));
        b.m = 0.7;
        b.spec = smoothstep(0.03, 0.0, p.y - edge) * 0.3;
        b.rim = 0.15;
        return b;
    }
    // beads (and a finer mist on high quality)
    for (int layer = 0; layer < (high ? 2 : 1); layer++) {
        float cs = layer == 0 ? 1.0 / 11.0 : 1.0 / 26.0;
        ivec2 id = ivec2(floor(p / cs));
        vec4 r = ws_rand4(ws_cell(seed, id, uint(layer)));
        if (r.z < (layer == 0 ? 0.28 : 0.2)) {
            float life = (layer == 0 ? 0.35 : 0.2) + r.w * (layer == 0 ? 0.9 : 0.5);
            float k = clamp(1.0 - age / life, 0.0, 1.0);
            float rad = cs * (0.07 + 0.13 * fract(r.x * 7.31 + r.y * 3.17)) * sqrt(smoothstep(0.0, 0.35, k));
            ws_drop(b, p, (vec2(id) + 0.34 + 0.32 * r.xy) * cs, rad, px, 1.0, fract(r.x * 13.1 + r.w * 7.3));
        }
    }
    // runners: one per column, starting as a bead that breaks away
    for (int layer = 0; layer < (high ? 2 : 1); layer++) {
        float w = layer == 0 ? 1.0 / 6.0 : 1.0 / 4.3;
        float cx = p.x / w + float(layer) * 0.5;
        int col = int(floor(cx));
        vec4 r = ws_rand4(ws_cell(seed, ivec2(col, 0), 7u + uint(layer)));
        if (r.w > 0.35) continue;
        float rad = w * (0.1 + 0.05 * r.z);
        float y0 = 0.05 + 0.55 * r.y;
        float start = 0.1 + 0.5 * fract(r.x * 5.7 + r.z * 2.3);
        float tt = max(age - start, 0.0);
        float y = y0 + 0.15 * tt + 0.6 * tt * tt;
        float phase = r.x * 6.283;
        float x0 = (float(col) + 0.5 + 0.35 * (r.x - 0.5) - float(layer) * 0.5) * w;
        float sway = 0.25 * rad / w;
        float x = x0 + sin(y * 19.0 + phase) * w * sway;
        float st = tt > 0.0 ? 1.0 + min((0.08 + 0.6 * tt) * 0.9, 0.6) : 1.0;
        ws_drop(b, p, vec2(x, y), rad * (1.0 - 0.2 * smoothstep(0.0, 1.5, tt)), px, st, r.z);
        // the trail: tiny beads where it has run
        if (tt > 0.0 && p.y > y0 && p.y < y - rad) {
            float ts = 0.028;
            float ty = floor(p.y / ts);
            vec4 tr = ws_rand4(ws_cell(seed, ivec2(col, int(ty)), 13u + uint(layer)));
            if (tr.x < 0.6) {
                float cy = (ty + 0.5) * ts;
                float trad = rad * (0.22 + 0.22 * tr.y) * (1.0 - smoothstep(0.2, 0.9, tt));
                ws_drop(b, p, vec2(x0 + sin(cy * 19.0 + phase) * w * sway + (tr.z - 0.5) * rad * 0.9, cy), trad, px, 1.0, tr.w);
            }
        }
    }
    b.m *= 0.45 * fade; // drops are mostly see-through
    return b;
}

// Light a pixel seen through a drop: darker rim, highlight, slime tint.
vec3 ws_shade(vec3 c, WsDrop d, uint kind) {
    c *= mix(1.0, 0.9, d.rim * d.m);
    c += d.spec * d.m * 0.12;
    if (kind == 2u) c = mix(c, c * vec3(0.55, 1.0, 0.45) + vec3(0.0, 0.03, 0.0), 0.35 * d.m);
    return c;
}
