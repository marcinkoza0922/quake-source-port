#version 460
#include "common.glsl"
// Brush entity positions now and with last frame's transform (same view).
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_st;
layout(location = 2) in vec3 in_lm;
layout(push_constant) uniform P {
    mat4 model;
    vec4 prev[3];
    uvec4 misc;     // w: MSAA samples
} pc;
layout(location = 0) out vec4 o_cur;
layout(location = 1) out vec4 o_prev;
void main() {
    vec4 p = vec4(in_pos, 1.0);
    vec4 wp = vec4(dot(pc.prev[0], p), dot(pc.prev[1], p), dot(pc.prev[2], p), 1.0);
    o_cur = fr.viewproj * (pc.model * p);
    o_prev = fr.viewproj * wp;
    gl_Position = o_cur;
}
