#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P {
    vec4 blend;
    float gamma;
    float contrast;
    uint scene;
    float dither;   // r_softemu 1: screen dither; 0: banding suppression
    uint mode;      // 0 truecolour, 1 palettize + dither, 2 palettize
    float scale;    // scene pixels per output pixel (noise follows scene pixels)
    uint colorblind; // r_colorblind: 0 off, 1 protan, 2 deutan, 3 tritan
    uint vel_tex;   // per-object motion target (uv displacement per frame)
    float mbx;      // motion blur scale (0 = off), x and y (reserved rows)
    float mby;
} pc;
layout(location = 0) out vec4 o_color;

// 128^3 palette index LUT, packed 4 per uint after the 256 palette entries.
uint lut(ivec3 c) {
    uint i = uint(c.x + c.y * 128 + c.z * 16384);
    return (palette[256u + (i >> 2)] >> ((i & 3u) * 8u)) & 255u;
}
// Daltonize: simulate the deficiency (Viénot/Brettel-style RGB matrices),
// then move the lost difference into channels the viewer can still tell apart.
vec3 daltonize(vec3 c, uint m) {
    mat3 sim;
    if (m == 1u)      sim = mat3(0.56667, 0.55833, 0.0, 0.43333, 0.44167, 0.24167, 0.0, 0.0, 0.75833);
    else if (m == 2u) sim = mat3(0.625, 0.70, 0.0, 0.375, 0.30, 0.30, 0.0, 0.0, 0.70);
    else              sim = mat3(0.95, 0.0, 0.0, 0.05, 0.43333, 0.475, 0.0, 0.56667, 0.525);
    vec3 err = c - sim * c;
    vec3 shift = vec3(0.0, 0.7 * err.r + err.g, 0.7 * err.r + err.b);
    return clamp(c + shift, 0.0, 1.0);
}
vec2 vel_at(vec2 uv) {
    return texture(textures[nonuniformEXT(pc.vel_tex)], uv).xy * vec2(pc.mbx, pc.mby);
}
// Per-object motion blur: average along the strongest motion nearby, so
// moving objects also smear over the static background around them.
vec3 motion_blur(vec3 c, vec2 uv) {
    vec2 sz = vec2(textureSize(textures[nonuniformEXT(pc.vel_tex)], 0));
    vec2 v = vel_at(uv);
    float best = dot(v * sz, v * sz);
    if (best < 0.25) {
        vec2 r = 24.0 / sz;
        for (int i = 0; i < 8; i++) {
            float a = float(i) * 0.78539816;
            vec2 t = vel_at(uv + vec2(cos(a), sin(a)) * r);
            float l = dot(t * sz, t * sz);
            if (l > best) { best = l; v = t; }
        }
    }
    if (best < 0.25) return c;
    float l = length(v);
    if (l > 0.06) v *= 0.06 / l;
    const int N = 12;
    float j = bayer01(ivec2(gl_FragCoord.xy)) - 0.5;
    vec3 acc = vec3(0.0);
    for (int i = 0; i < N; i++) {
        float t = (float(i) + 0.5 + j) / float(N) - 0.5;
        acc += texture(textures[nonuniformEXT(pc.scene)], uv - v * t).rgb;
    }
    return acc / float(N);
}
void main() {
    vec4 sc = texture(textures[nonuniformEXT(pc.scene)], i_uv);
    vec3 c = sc.rgb;
    if (pc.mbx > 0.0)
        c = motion_blur(c, i_uv);
    if (pc.mode != 0u) {
        if (pc.mode == 1u) {
            vec2 noiseuv = floor(gl_FragCoord.xy * pc.scale) + 0.5;
            c = sqrt(c);
            c += DITHER_NOISE(noiseuv) * pc.dither;
            c *= c;
        }
        // GLPalette_Postprocess: blend/contrast/gamma act on the palette colour
        c = unpack_rgba(palette[lut(ivec3(clamp(c, 0.0, 1.0) * 127.0 + 0.5))]).rgb;
    }
    c = mix(c, pc.blend.rgb, pc.blend.a);
    if (pc.colorblind != 0u)
        c = daltonize(c, pc.colorblind);
    c = pow(max(c * pc.contrast, vec3(0.0)), vec3(pc.gamma));
    if (pc.mode == 0u)
        c += SUPPRESS_BANDING() * pc.dither;
    o_color = vec4(c, 1.0);
}
