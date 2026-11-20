// Shared declarations for all graphics shaders (set 0 is global).
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0, std140) uniform Frame {
    vec4 portal_clip;
    vec4 portal_plane[4];
    vec4 portal_min[4];
    vec4 portal_max[4];
    mat4 viewproj;
    vec4 eye;       // xyz eye, w time
    vec4 fog;       // rgb colour, w density (already /64)
    vec4 vright;    // view right
    vec4 vup;       // view up
    vec4 vfwd;      // view forward
    vec4 sky;       // x skyfog, y skybox flag, z sky layer alpha, w particle style (1 round, 2 square)
    uvec4 skybox0;  // rt bk lf ft
    uvec4 skybox1;  // up dn solid-layer alpha-layer
    vec4 alphas;    // water lava slime tele
    vec4 soft;      // x screen dither, y texture dither (r_softemu/r_dither)
    uvec4 softf;    // x world dither level (0 off, 1 coarse, 2 banded lightmap), y alias mode (0 std, 1 dither, 2 affine), z goggles, w caustic tex
    vec4 water;     // x ripple strength, y caustics strength, z ripple count, w quality (0 low, 1 high)
    vec4 portal_uv; // xy gl_FragCoord -> portal UV scale, zw valid UV clamp max
    vec4 vel_uv;    // xy scene px per vel px, zw scene max texel coords
    vec4 wind;      // skybox wind: xyz direction (world), w loop phase
    vec4 ripples[32];   // xy centre, z liquid surface height, w start time
    vec4 ripple_amp[8]; // amplitudes, 4 per vec4
    vec4 refl;          // x planar reflection strength (0 off)
    uvec4 teleport_fx;  // x teleporter impact ripple count
    vec4 teleport_ripples[12]; // xyz impact position, w birth time
    vec4 hdr_deluxe;    // x deluxe layer offset (0 off), y hdr tonemap (0/1), z specular, w normal mod
} fr;

layout(set = 0, binding = 1) uniform sampler2D textures[];
layout(set = 0, binding = 2) readonly buffer AliasVerts { uint alias_verts[]; };
layout(set = 0, binding = 3) uniform sampler2DArray lightmap;
layout(set = 0, binding = 4) readonly buffer Materials { uvec4 materials[]; };
layout(set = 0, binding = 5) readonly buffer Palette { uint palette[]; };
layout(set = 0, binding = 6) uniform sampler2D scene_depth;
layout(set = 0, binding = 7) uniform sampler2DMS scene_depth_ms;
layout(set = 0, binding = 8) uniform sampler2DMS scene_color_ms;
layout(set = 0, binding = 9) readonly buffer DynVerts { uint dyn_verts[]; };
// r_shadow_realtime_dlight (frame/dlights.rs): dynamic lights lit per pixel.
// shadow_vp[layer] renders/reads cube face `layer % 6` of light `layer / 6`.
struct RtLight {
    vec4 pos_radius;     // origin, radius
    vec4 color_minlight; // colour, minlight
    ivec4 info;          // x first shadow layer (-1: unshadowed)
};
layout(set = 0, binding = 10, std430) readonly buffer Lights {
    uvec4 light_count;   // x lights, y shadow filter (0 low, 1 medium, 2 high), z face size
    RtLight lights[32];
    mat4 shadow_vp[48];
};
layout(set = 0, binding = 11) uniform sampler2DArrayShadow shadow_maps;

const uint NO_TEX = 0xffffffffu;

vec4 tex(uint i, vec2 uv) { return texture(textures[nonuniformEXT(i)], uv); }

// Fraction of a surface's own colour left after fog (1 = no fog).
float fog_factor(vec3 wpos) {
    if (fr.fog.w <= 0.0) return 1.0;
    float z = length(wpos - fr.eye.xyz) * fr.fog.w;
    return clamp(exp(-z * z), 0.0, 1.0);
}

vec3 apply_fog(vec3 c, vec3 wpos) {
    if (fr.fog.w <= 0.0) return c;
    return mix(fr.fog.rgb, c, fog_factor(wpos));
}

// r_bloom: the scene pipelines' *_glow.frag variants (built while bloom is
// on) also write what glows to colour attachment 1.
#ifdef GLOW
layout(location = 1) out vec4 o_glow;
#define GLOW_OUT(v) o_glow = (v)
#else
#define GLOW_OUT(v)
#endif
// Unlit effects (particles, sprites) glow by
// brightness, favouring saturated colours: fire and sparks glow, grey smoke,
// bubbles and dark blood barely do.
vec3 effect_glow(vec3 c) {
    float hi = max(c.r, max(c.g, c.b));
    float lo = min(c.r, min(c.g, c.b));
    float sat = hi > 0.0 ? (hi - lo) / hi : 0.0;
    return c * (smoothstep(0.55, 0.95, hi) * mix(0.35, 1.0, sat));
}

vec4 unpack_rgba(uint c) { return unpackUnorm4x8(c); }

// Hue-preserving soft HDR shoulder above 0.85 so LIT2 / LIGHTING_E5BGR9
// overbright lighting rolls off smoothly instead of clipping to white.
vec3 hdr_tonemap(vec3 c) {
    vec3 p = max(c, vec3(0.0));
    float m = max(p.r, max(p.g, p.b));
    if (m <= 0.85) return p;
    float excess = m - 0.85;
    float mapped = 0.85 + 0.15 * (excess / (excess + 0.15));
    return p * (mapped / m);
}

// Fraction of light `i` reaching `p` (normal `n`, `tl` = light - p): its
// cube shadow map, filtered by quality. A ray-traced test can replace this.
float light_visibility(int i, vec3 p, vec3 n, vec3 tl) {
    int base = lights[i].info.x;
    if (base < 0) return 1.0;
    float size = float(light_count.z);
    // normal offset of ~1.5 shadow texels against acne (texel = 2 d / size)
    vec3 q = p + n * (0.5 + 3.0 * length(tl) / size);
    vec3 d = q - lights[i].pos_radius.xyz;
    vec3 a = abs(d);
    int face = a.x >= a.y && a.x >= a.z ? (d.x >= 0.0 ? 0 : 1)
             : a.y >= a.z ? (d.y >= 0.0 ? 2 : 3) : (d.z >= 0.0 ? 4 : 5);
    int layer = base + face;
    vec4 c = shadow_vp[layer] * vec4(q, 1.0);
    vec3 ndc = c.xyz / c.w;
    float ts = 1.0 / size;
    vec2 uv = clamp(ndc.xy * 0.5 + 0.5, vec2(ts), vec2(1.0 - ts));
    vec4 tc = vec4(uv, float(layer), ndc.z);
    uint q8 = light_count.y;
    if (q8 == 0u) return texture(shadow_maps, tc);
    float s = 0.0;
    if (q8 == 1u) {
        for (int k = 0; k < 4; k++) {
            vec2 o = vec2(k & 1, k >> 1) - 0.5;
            s += texture(shadow_maps, vec4(uv + o * ts, tc.zw));
        }
        return s * 0.25;
    }
    for (int y = -1; y <= 1; y++)
        for (int x = -1; x <= 1; x++)
            s += texture(shadow_maps, vec4(uv + vec2(x, y) * ts * 1.25, tc.zw));
    return s / 9.0;
}

// Per-pixel dynamic light at `p` with normal `n`, in lightmap units (1 =
// normal light): Quake's (radius - distance) / 128 above minlight, mostly
// independent of facing as in the lightmap version, fading out behind.
vec3 dyn_lights(vec3 p, vec3 n) {
    vec3 sum = vec3(0.0);
    for (uint i = 0u; i < light_count.x; i++) {
        vec3 tl = lights[i].pos_radius.xyz - p;
        float dist = length(tl);
        float a = lights[i].pos_radius.w - dist;
        if (a <= lights[i].color_minlight.w) continue;
        float ndl = dot(n, tl) / max(dist, 1e-3);
        // Quake's lightmap distance (radius minus plane and in-plane distance)
        // is longer than this one; a mild facing term evens it out on floors
        float facing = (0.7 + 0.3 * max(ndl, 0.0)) * smoothstep(-0.1, 0.1, ndl);
        if (facing <= 0.0) continue;
        float vis = light_visibility(int(i), p, n, tl);
        sum += lights[i].color_minlight.rgb * (a / 128.0 * facing * vis);
    }
    return sum;
}

// Add-on goggles (fr.softf.z: 1 night vision, 2 thermal). In thermal mode
// scene shaders write a grey "heat" level that post.frag maps to colours.
bool thermal() { return fr.softf.z == 2u; }
float luma(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }
float cold_heat(vec3 c) { return 0.05 + 0.2 * luma(c); }

// Noise (ironwail NOISE_FUNCTIONS).
// ALU-only 16x16 Bayer matrix, [0, 1).
float bayer01(ivec2 coord) {
    coord &= 15;
    coord.y ^= coord.x;
    uint v = uint(coord.y | (coord.x << 8));
    v = (v ^ (v << 2)) & 0x3333u;
    v = (v ^ (v << 1)) & 0x5555u;
    v |= v >> 7;
    v = bitfieldReverse(v) >> 24;
    return float(v) * (1.0 / 256.0);
}
float bayer(ivec2 coord) { return bayer01(coord) - 0.5; }
// Hash without Sine (c) 2014 David Hoskins, MIT licence.
float whitenoise01(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * .1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
// Uniform [0, 1] to triangular [-1, 1].
float tri(float x) {
    float orig = x * 2.0 - 1.0;
    uint signbit = floatBitsToUint(orig) & 0x80000000u;
    x = sqrt(abs(orig)) - 1.0;
    return uintBitsToFloat(floatBitsToUint(x) ^ signbit);
}
#define DITHER_NOISE(uv) tri(bayer01(ivec2(uv)))
#define SCREEN_SPACE_NOISE() DITHER_NOISE(floor(gl_FragCoord.xy) + 0.5)
#define SUPPRESS_BANDING() bayer(ivec2(gl_FragCoord.xy))

// Liquid surface slope (dh/dx, dh/dy) at world point p on a surface at
// height p.z: optional gentle swell plus the expanding ripple rings.
vec2 water_grad(vec3 p, float t, float swell, float rings) {
    vec2 g = vec2(0.0);
    if (swell > 0.0) {
        // a few long, slow directional waves
        const vec4 W[3] = vec4[3](vec4(0.80, 0.60, 0.090, 1.3),
                                  vec4(-0.45, 0.89, 0.130, -1.7),
                                  vec4(0.31, -0.95, 0.210, 2.1));
        for (int i = 0; i < 3; i++) {
            float ph = dot(W[i].xy, p.xy) * W[i].z + t * W[i].w;
            g += W[i].xy * (cos(ph) * W[i].z * 0.9 / (1.0 + float(i)));
        }
        g *= swell;
    }
    int n = rings > 0.0 ? int(fr.water.z) : 0;
    vec2 gr = vec2(0.0);
    for (int i = 0; i < n; i++) {
        vec4 r = fr.ripples[i];
        float age = t - r.w;
        if (age < 0.0 || age > 2.5 || abs(p.z - r.z) > 24.0) continue;
        vec2 d = p.xy - r.xy;
        float front = age * 85.0;
        float sig = 5.0 + age * 7.0;
        float reach = 3.0 * sig;
        float max_r = front + reach;
        if (abs(d.x) > max_r || abs(d.y) > max_r) continue;
        float d2 = dot(d, d);
        float min_r = max(front - reach, 0.0);
        if (d2 > max_r * max_r || d2 < min_r * min_r) continue;
        float dist = sqrt(d2);
        float x = dist - front;
        float env = exp(-x * x / (2.0 * sig * sig));
        float fade = (1.0 - age / 2.5);
        float amp = fr.ripple_amp[i >> 2][i & 3] * fade * fade / (1.0 + front * 0.03);
        const float k = 0.4;
        // h = amp * env * cos(k x); slope along the ring's radius
        float dh = amp * env * (-k * sin(k * x) - cos(k * x) * x / (sig * sig));
        gr += dh * d / max(dist, 1e-3);
    }
    return g + gr * rings;
}

// Lava ripples: thick, viscous molten rock instead of water. Each event is a
// single slow, broad, non-oscillating swell rolling outward (no ring train),
// and the disturbance breaks the crust: the centre flares hot and the swell
// glows as it passes. Returns (dh/dx, dh/dy, heat).
vec3 lava_rings(vec3 p, float t, float rings) {
    int n = rings > 0.0 ? int(fr.water.z) : 0;
    vec2 g = vec2(0.0);
    float heat = 0.0;
    for (int i = 0; i < n; i++) {
        vec4 r = fr.ripples[i];
        float age = t - r.w;
        if (age < 0.0 || age > 2.5 || abs(p.z - r.z) > 24.0) continue;
        vec2 d = p.xy - r.xy;
        // slows as it spreads: ~40 u/s at first, ~60 units total
        float front = 60.0 * (1.0 - exp(-age * 0.9));
        float sig = 8.0 + age * 5.0;
        float max_r = front + 3.0 * sig;
        if (abs(d.x) > max_r || abs(d.y) > max_r) continue;
        float d2 = dot(d, d);
        if (d2 > max_r * max_r) continue;
        float dist = sqrt(d2);
        float x = dist - front;
        float env = exp(-x * x / (2.0 * sig * sig));
        float fade = 1.0 - age / 2.5;
        float amp = fr.ripple_amp[i >> 2][i & 3] * fade * fade;
        // h = 3 * amp * env: one smooth ridge, slope along the radius
        float dh = -3.0 * amp * env * x / (sig * sig);
        g += dh * d / max(dist, 1e-3);
        // molten core exposed at the impact, cooling quickly, plus the ridge
        float core = exp(-d2 / (2.0 * 12.0 * 12.0)) * exp(-age * 1.8);
        heat += amp * (0.35 * env + 0.6 * core);
    }
    return vec3(g * rings, min(heat * rings, 1.5));
}

// Distance fade for water ripples and caustics by quality tier (0 low, 1 high).
float water_dist_fade(vec3 wpos, int qual) {
    float fade_d = qual == 0 ? 640.0 : 1024.0;
    float max_d  = qual == 0 ? 1024.0 : 1536.0;
    vec3 delta = wpos - fr.eye.xyz;
    float d2 = dot(delta, delta);
    if (d2 <= fade_d * fade_d) return 1.0;
    if (d2 >= max_d * max_d) return 0.0;
    return 1.0 - smoothstep(fade_d, max_d, sqrt(d2));
}

// Precomputed tiling caustic texture (two scrolling Worley layers).
float caustic_tex(vec2 q, float t) {
    uint slot = fr.softf.w;
    if (slot == NO_TEX) return 0.0;
    vec2 uv = q / (56.0 * 4.0);
    float a = tex(slot, uv + vec2(t * 0.035, t * 0.024)).r;
    float b = tex(slot, uv * 1.31 + vec2(0.37, 0.19) + vec2(-t * 0.029, t * 0.038)).g;
    return clamp(0.55 * (a + b) + 0.9 * a * b, 0.0, 1.5);
}
