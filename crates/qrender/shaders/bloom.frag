#version 460
#include "common.glsl"
// r_bloom: downsample/upsample chain (Jimenez, "Next Generation Post
// Processing in Call of Duty: Advanced Warfare", 2014). Level 0 is rendered
// directly by the narrow single-sampled glow pass; mode 0 optionally adds
// DarkPlaces-style bright scene colours (r_bloom_bright) onto level 0, mode 1
// halves a level with the 13-tap filter, mode 2 adds a 3x3 tent upsample of
// the smaller level scaled by `weight` onto the next larger one (blend ONE, ONE).
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
    return texture(textures[nonuniformEXT(pc.src)], min(uv, pc.uv_max)).rgb;
}

vec3 scene_bright(vec2 uv) {
    return bright_part(texture(textures[nonuniformEXT(pc.scene)], min(uv, pc.uv_max)).rgb);
}

void main() {
    vec2 uv = gl_FragCoord.xy * pc.uv_scale;
    vec2 t = pc.texel;
    vec3 c;
    if (pc.mode == 0u) {
        // 4-tap box average of bright scene pixels added onto level 0.
        vec2 h = t * 0.5;
        c = (scene_bright(uv + vec2(-h.x, -h.y)) + scene_bright(uv + vec2(h.x, -h.y))
           + scene_bright(uv + vec2(-h.x,  h.y)) + scene_bright(uv + vec2(h.x,  h.y))) * (0.25 * pc.bright);
    } else if (pc.mode == 2u) {
        // 4-tap bilinear tent upsample (exact [-1, +1] tent support via linear filtering).
        vec2 h = t * 0.5;
        c = tap(uv + vec2(-h.x, -h.y)) + tap(uv + vec2(h.x, -h.y))
          + tap(uv + vec2(-h.x,  h.y)) + tap(uv + vec2(h.x,  h.y));
        c *= pc.weight * 0.25;
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
