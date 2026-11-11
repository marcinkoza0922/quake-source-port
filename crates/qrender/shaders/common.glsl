// Shared declarations for all graphics shaders (set 0 is global).
#extension GL_EXT_nonuniform_qualifier : require

layout(set = 0, binding = 0, std140) uniform Frame {
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
} fr;

layout(set = 0, binding = 1) uniform sampler2D textures[];
layout(set = 0, binding = 2) readonly buffer AliasVerts { uint alias_verts[]; };
layout(set = 0, binding = 3) uniform sampler2DArray lightmap;
layout(set = 0, binding = 4) readonly buffer Materials { uvec4 materials[]; };
layout(set = 0, binding = 5) readonly buffer Palette { uint palette[]; };

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
