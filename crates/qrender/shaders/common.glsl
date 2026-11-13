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
    vec4 sky;       // x skyfog, y skybox flag, z sky layer alpha
    uvec4 skybox0;  // rt bk lf ft
    uvec4 skybox1;  // up dn solid-layer alpha-layer
    vec4 alphas;    // water lava slime tele
    vec4 soft;      // x screen dither, y texture dither (r_softemu/r_dither)
    uvec4 softf;    // x world dither level (0 off, 1 coarse, 2 banded lightmap), y alias mode (0 std, 1 dither, 2 affine)
    vec4 water;     // x ripple strength, y caustics strength, z ripple count
    vec4 ripples[32];   // xy centre, z liquid surface height, w start time
    vec4 ripple_amp[8]; // amplitudes, 4 per vec4
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

const uint NO_TEX = 0xffffffffu;

vec4 tex(uint i, vec2 uv) { return texture(textures[nonuniformEXT(i)], uv); }

vec3 apply_fog(vec3 c, vec3 wpos) {
    if (fr.fog.w <= 0.0) return c;
    float z = length(wpos - fr.eye.xyz) * fr.fog.w;
    float f = clamp(exp(-z * z), 0.0, 1.0);
    return mix(fr.fog.rgb, c, f);
}

vec4 unpack_rgba(uint c) { return unpackUnorm4x8(c); }

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
        float dist = length(d);
        float front = age * 85.0;
        float x = dist - front;
        float sig = 5.0 + age * 7.0;
        if (abs(x) > 3.0 * sig) continue;
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

// Caustic light network: two drifting Worley edge layers.
vec2 caustic_hash(vec2 p) {
    p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(p) * 43758.5453);
}
float caustic_layer(vec2 p, float t) {
    vec2 i = floor(p), f = fract(p);
    float d1 = 8.0, d2 = 8.0;
    for (int y = -1; y <= 1; y++)
        for (int x = -1; x <= 1; x++) {
            vec2 o = vec2(x, y);
            vec2 h = caustic_hash(i + o);
            vec2 q = o + 0.5 + 0.38 * sin(t * (0.6 + 0.5 * h.yx) + 6.2831853 * h) - f;
            float d = dot(q, q);
            if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) d2 = d;
        }
    float e = sqrt(d2) - sqrt(d1); // 0 on cell borders
    return pow(1.0 - smoothstep(0.0, 0.26, e), 4.0);
}
float caustic(vec2 p, float t) {
    p += 0.22 * vec2(sin(p.y * 1.7 + t * 0.8), sin(p.x * 1.9 - t * 0.7));
    float a = caustic_layer(p, t);
    float b = caustic_layer(p * 1.31 + vec2(5.2, 1.3), t * 1.15 + 3.0);
    return clamp(0.55 * (a + b) + 0.9 * a * b, 0.0, 1.5);
}
