#version 460
#include "common.glsl"
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(push_constant) uniform P { vec4 color; uint tex; } pc;
layout(location = 0) out vec2 o_uv;
layout(location = 1) out vec3 o_wpos;
void main() {
    o_uv = in_uv;
    o_wpos = in_pos;
    gl_Position = fr.viewproj * vec4(in_pos, 1.0);
}
