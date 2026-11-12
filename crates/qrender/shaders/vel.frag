#version 460
#include "common.glsl"
// Per-object motion: screen-space displacement since the last frame (in UV
// units), only where this surface is the visible one in the scene depth.
layout(location = 0) in vec4 i_cur;
layout(location = 1) in vec4 i_prev;
layout(push_constant) uniform P { layout(offset = 124) uint samples; } pc;
layout(location = 0) out vec4 o_vel;

float linear_depth(float d) {
    const float n = 4.0, f = 65536.0;
    return n * f / (f - d * (f - n));
}
void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    float d = pc.samples > 1u ? texelFetch(scene_depth_ms, px, 0).r : texelFetch(scene_depth, px, 0).r;
    if (linear_depth(gl_FragCoord.z) > linear_depth(d) * 1.02 + 2.0) discard;
    vec2 c = i_cur.xy / i_cur.w, p = i_prev.xy / i_prev.w;
    o_vel = vec4((c - p) * 0.5, 0.0, 0.0);
}
