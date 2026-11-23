#version 460
#include "common.glsl"
// Narrow bloom glow pass for classic particles.
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
layout(location = 0) out vec4 o_color;

void main() {
    vec4 col = i_color;
    if (fr.sky.w < 1.5) {
        float r = dot(i_uv, i_uv);
        if (r > 1.0) discard;
        col.a *= min(8.0 * (1.0 - r), 1.0);
    }
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    o_color = vec4(effect_glow(col.rgb) * fog_factor(i_wpos), col.a);
}
