#version 460
#include "common.glsl"
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_st;
layout(location = 2) in vec3 in_lm;
layout(location = 3) in vec2 in_liq;
layout(location = 4) in vec3 in_nrm;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc; // params.x alpha
layout(location = 0) out vec2 o_st;
layout(location = 1) out vec3 o_lm;
layout(location = 2) out vec3 o_wpos;
layout(location = 3) flat out uint o_mat;
layout(location = 4) flat out vec2 o_liq; // liquid surface height, kind
layout(location = 5) out vec3 o_nrm;      // world-space smooth normal (FACENORMALS or plane normal)
void main() {
    vec4 w = pc.model * vec4(in_pos, 1.0);
    o_st = in_st;
    o_lm = in_lm;
    o_wpos = w.xyz;
    o_mat = gl_InstanceIndex;
    o_liq = in_liq;
    o_nrm = mat3(pc.model) * in_nrm;
    gl_Position = fr.viewproj * w;
}
