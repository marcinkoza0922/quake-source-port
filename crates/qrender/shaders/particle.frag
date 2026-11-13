#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
layout(location = 0) out vec4 o_color;
void main() {
    if (dot(i_uv, i_uv) > 1.0) discard;
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    o_color = vec4(apply_fog(i_color.rgb, i_wpos), i_color.a);
}
