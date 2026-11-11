#version 460
#include "common.glsl"
layout(location = 0) in vec3 in_pos;   // per instance
layout(location = 1) in vec4 in_color; // per instance (unorm8)
layout(location = 0) out vec2 o_uv;
layout(location = 1) out vec4 o_color;
layout(location = 2) out vec3 o_wpos;
const vec2 Q[6] = vec2[6](vec2(-1,-1), vec2(1,-1), vec2(1,1), vec2(-1,-1), vec2(1,1), vec2(-1,1));
void main() {
    vec2 q = Q[gl_VertexIndex];
    float d = dot(in_pos - fr.eye.xyz, fr.vfwd.xyz);
    float scale = d < 20.0 ? 1.0 : 1.0 + d * 0.004;
    vec3 p = in_pos + (fr.vright.xyz * q.x + fr.vup.xyz * q.y) * scale * 0.75;
    o_uv = q;
    o_color = in_color;
    o_wpos = p;
    gl_Position = fr.viewproj * vec4(p, 1.0);
}
