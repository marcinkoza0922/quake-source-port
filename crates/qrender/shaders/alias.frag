#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_shade;
layout(location = 2) in vec3 i_wpos;
layout(push_constant) uniform P {
    mat4 model; vec4 light; vec4 shadevec; uvec4 poses; uvec4 texs;
} pc;
layout(location = 0) out vec4 o_color;

vec3 ramp(uint row, uint k) {
    uint idx = row < 8u ? row * 16u + k : row * 16u + 15u - k;
    return unpack_rgba(palette[idx]).rgb;
}
void main() {
    vec4 c = tex(pc.texs.x, i_st);
    if ((pc.texs.w & 1u) != 0u && c.a < 0.666) discard;
    vec3 rgb = c.rgb;
    if (pc.texs.z != NO_TEX) {
        vec2 m = tex(pc.texs.z, i_st).rg * 255.0 + 0.5;
        uint top = uint(m.x), bot = uint(m.y);
        bool cm = (pc.texs.w & 4u) != 0u;
        if (top > 0u) rgb += ramp(cm ? pc.poses.z : 1u, top - 1u);
        if (bot > 0u) rgb += ramp(cm ? pc.poses.w : 6u, bot - 1u);
    }
    rgb *= i_shade * 2.0;
    if (pc.texs.y != NO_TEX) rgb += tex(pc.texs.y, i_st).rgb;
    o_color = vec4(apply_fog(rgb, i_wpos), pc.light.a);
}
