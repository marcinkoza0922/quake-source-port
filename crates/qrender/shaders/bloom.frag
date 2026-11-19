#version 460
#include "common.glsl"
// r_bloom: downsample/upsample chain (Jimenez, "Next Generation Post
// Processing in Call of Duty: Advanced Warfare", 2014). Mode 0 reads the
// scene glow attachment (plus, with r_bloom_bright, DarkPlaces-style bright
// scene colours) into the first level, mode 1 halves a level with the
// 13-tap filter, mode 2 adds a 3x3 tent upsample of the smaller level,
// scaled by `weight`, onto the next larger one (blend ONE, ONE).
layout(push_constant) uniform P {
    vec2 uv_scale;  // gl_FragCoord.xy -> source UV
    vec2 texel;     // one source texel in UV
    vec2 uv_max;    // last valid source UV (this view's region)
    uint src;       // source texture slot
    uint scene;     // linearly sampled scene colour (mode 0)
    uint mode;
    float bright;   // r_bloom_bright: share of bright scene colour (mode 0)
    float weight;   // upsample weight of the smaller level (mode 2)
} pc;
layout(location = 0) out vec4 o_color;

// r_bloom_bright: bright scene colours, faded in over a soft knee on their
// brightest channel (DarkPlaces blooms the whole scene less a small
// r_bloom_colorsubtract; Quake's lighting rarely nears white).
vec3 bright_part(vec3 c) {
    return c * smoothstep(0.35, 0.9, max(c.r, max(c.g, c.b)));
}

vec3 tap(vec2 uv) {
    uv = min(uv, pc.uv_max);
    vec3 c = texture(textures[nonuniformEXT(pc.src)], uv).rgb;
    if (pc.mode == 0u && pc.bright > 0.0)
        c += bright_part(texture(textures[nonuniformEXT(pc.scene)], uv).rgb) * pc.bright;
    return c;
}

void main() {
    vec2 uv = gl_FragCoord.xy * pc.uv_scale;
    vec2 t = pc.texel;
    vec3 c;
    if (pc.mode == 2u) {
        c = tap(uv) * 4.0;
        c += (tap(uv + vec2(t.x, 0.0)) + tap(uv - vec2(t.x, 0.0)) + tap(uv + vec2(0.0, t.y)) + tap(uv - vec2(0.0, t.y))) * 2.0;
        c += tap(uv + t) + tap(uv - t) + tap(uv + vec2(t.x, -t.y)) + tap(uv + vec2(-t.x, t.y));
        c *= pc.weight / 16.0;
    } else {
        vec3 e = tap(uv);
        vec3 a = tap(uv + vec2(-2.0, 2.0) * t), b = tap(uv + vec2(0.0, 2.0) * t), cc = tap(uv + vec2(2.0, 2.0) * t);
        vec3 d = tap(uv + vec2(-2.0, 0.0) * t), f = tap(uv + vec2(2.0, 0.0) * t);
        vec3 g = tap(uv + vec2(-2.0, -2.0) * t), h = tap(uv + vec2(0.0, -2.0) * t), i = tap(uv + vec2(2.0, -2.0) * t);
        vec3 j = tap(uv + vec2(-1.0, 1.0) * t), k = tap(uv + vec2(1.0, 1.0) * t);
        vec3 l = tap(uv + vec2(-1.0, -1.0) * t), m = tap(uv + vec2(1.0, -1.0) * t);
        c = e * 0.125 + (a + cc + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
    }
    o_color = vec4(c, 1.0);
}
