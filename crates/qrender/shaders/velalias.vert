#version 460
#include "common.glsl"
// Alias model positions now and with last frame's transform (same view).
layout(location = 0) in uint in_vert;
layout(location = 1) in vec2 in_st;
layout(push_constant) uniform P {
    mat4 model;
    vec4 prev[3];   // rows of last frame's 3x4 model transform
    uvec4 poses;    // pose1 base, pose2 base, blend (float bits), MSAA samples | flags (bit 16 MD3, bit 17 IQM)
} pc;
layout(location = 0) out vec4 o_cur;
layout(location = 1) out vec4 o_prev;

vec3 skin_iqm_pos(uint vofs, uint bone_base) {
    vec3 p = vec3(
        uintBitsToFloat(alias_verts[vofs + 0u]),
        uintBitsToFloat(alias_verts[vofs + 1u]),
        uintBitsToFloat(alias_verts[vofs + 2u]));
    uint bi = alias_verts[vofs + 6u];
    vec4 bw = unpackUnorm4x8(alias_verts[vofs + 7u]);
    float wsum = bw.x + bw.y + bw.z + bw.w;
    if (wsum <= 0.0) return p;
    bw /= wsum;
    vec4 r0 = vec4(0.0), r1 = vec4(0.0), r2 = vec4(0.0);
    for (uint k = 0u; k < 4u; k++) {
        float w = bw[k];
        if (w <= 0.0) continue;
        uint b = bone_base + ((bi >> (k * 8u)) & 0xffu) * 12u;
        r0 += w * vec4(uintBitsToFloat(alias_verts[b + 0u]), uintBitsToFloat(alias_verts[b + 1u]),
                       uintBitsToFloat(alias_verts[b + 2u]), uintBitsToFloat(alias_verts[b + 3u]));
        r1 += w * vec4(uintBitsToFloat(alias_verts[b + 4u]), uintBitsToFloat(alias_verts[b + 5u]),
                       uintBitsToFloat(alias_verts[b + 6u]), uintBitsToFloat(alias_verts[b + 7u]));
        r2 += w * vec4(uintBitsToFloat(alias_verts[b + 8u]), uintBitsToFloat(alias_verts[b + 9u]),
                       uintBitsToFloat(alias_verts[b + 10u]), uintBitsToFloat(alias_verts[b + 11u]));
    }
    vec4 hp = vec4(p, 1.0);
    return vec3(dot(r0, hp), dot(r1, hp), dot(r2, hp));
}

void main() {
    vec3 pa, pb;
    if ((pc.poses.w & 131072u) != 0u) {
        pa = skin_iqm_pos(in_vert, pc.poses.x);
        pb = pc.poses.x == pc.poses.y ? pa : skin_iqm_pos(in_vert, pc.poses.y);
    } else if ((pc.poses.w & 65536u) != 0u) {
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
