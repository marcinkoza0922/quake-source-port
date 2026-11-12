#version 460
#include "common.glsl"
// FTE scripted particles: pre-built world-space vertices.
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color; // unorm8
layout(location = 0) out vec2 o_uv;
layout(location = 1) out vec4 o_color;
layout(location = 2) out vec3 o_wpos;
void main() {
    o_uv = in_uv;
    o_color = in_color;
    o_wpos = in_pos;
    gl_Position = fr.viewproj * vec4(in_pos, 1.0);
}
