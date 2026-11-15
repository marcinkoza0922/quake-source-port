#version 460
#include "common.glsl"
// Alias model positions now and with last frame's transform (same view).
layout(location = 0) in uint in_vert;
layout(location = 1) in vec2 in_st;
layout(push_constant) uniform P {
    mat4 model;
    vec4 prev[3];   // rows of last frame's 3x4 model transform
    uvec4 poses;    // pose1 base, pose2 base, blend (float bits), MSAA samples
} pc;
layout(location = 0) out vec4 o_cur;
layout(location = 1) out vec4 o_prev;
void main() {
    vec3 pa, pb;
    if ((pc.poses.w & 65536u) != 0u) {
        uint va = in_vert * 2u;
        uint a0 = alias_verts[pc.poses.x + va];
        uint a1 = alias_verts[pc.poses.x + va + 1u];
        uint b0 = alias_verts[pc.poses.y + va];
        uint b1 = alias_verts[pc.poses.y + va + 1u];
        pa = vec3(float(a0 & 0xffffu), float((a0 >> 16) & 0xffffu), float(a1 & 0xffffu)) - 32768.0;
        pb = vec3(float(b0 & 0xffffu), float((b0 >> 16) & 0xffffu), float(b1 & 0xffffu)) - 32768.0;
    } else {
        uint a = alias_verts[pc.poses.x + in_vert];
        uint b = alias_verts[pc.poses.y + in_vert];
        pa = vec3(a & 0xffu, (a >> 8) & 0xffu, (a >> 16) & 0xffu);
        pb = vec3(b & 0xffu, (b >> 8) & 0xffu, (b >> 16) & 0xffu);
    }
    vec4 p = vec4(mix(pa, pb, uintBitsToFloat(pc.poses.z)), 1.0);
    vec4 w = pc.model * p;
    vec4 wp = vec4(dot(pc.prev[0], p), dot(pc.prev[1], p), dot(pc.prev[2], p), 1.0);
    o_cur = fr.viewproj * w;
    o_prev = fr.viewproj * wp;
    gl_Position = o_cur;
}
