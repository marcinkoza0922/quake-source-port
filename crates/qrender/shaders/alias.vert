#version 460
#include "common.glsl"
#include "anorms.glsl"
layout(location = 0) in uint in_vert;
layout(location = 1) in vec2 in_st;
layout(push_constant) uniform P {
    mat4 model;     // includes scale/scale_origin
    vec4 light;     // rgb, alpha
    vec4 shadevec;  // xyz, blend
    uvec4 poses;    // pose1 base, pose2 base, shirt row, pants row
    uvec4 texs;     // base, fb, cmap, flags
} pc;
layout(location = 0) out vec2 o_st;
layout(location = 1) out vec3 o_shade;
layout(location = 2) out vec3 o_wpos;
layout(location = 3) noperspective out vec2 o_st_affine; // r_softemu_mdl_warp
layout(location = 4) out vec3 o_nrm; // world-space normal (per-pixel dynamic lights)

float shadedot(vec3 n) {
    float d = dot(n, pc.shadevec.xyz);
    return d < 0.0 ? 1.0 + d * (13.0 / 44.0) : 1.0 + d;
}
void main() {
    // flag 16: a CPU-built pose in the per-frame buffer (ragdolls)
    // flag 32: packed 16-bit MD3 pose (2 u32s per vertex)
    bool dyn = (pc.texs.w & 16u) != 0u;
    bool md3 = !dyn && (pc.texs.w & 32u) != 0u;
    vec3 pa, pb, na, nb;
    if (md3) {
        uint va = in_vert * 2u;
        uint a0 = alias_verts[pc.poses.x + va];
        uint a1 = alias_verts[pc.poses.x + va + 1u];
        uint b0 = alias_verts[pc.poses.y + va];
        uint b1 = alias_verts[pc.poses.y + va + 1u];
        pa = vec3(float(a0 & 0xffffu), float((a0 >> 16) & 0xffffu), float(a1 & 0xffffu)) - 32768.0;
        pb = vec3(float(b0 & 0xffffu), float((b0 >> 16) & 0xffffu), float(b1 & 0xffffu)) - 32768.0;
        const float K = 2.0 * 3.14159265 / 255.0;
        vec2 sa = vec2((a1 >> 16) & 0xffu, (a1 >> 24) & 0xffu) * K;
        vec2 sb = vec2((b1 >> 16) & 0xffu, (b1 >> 24) & 0xffu) * K;
        na = vec3(cos(sa.y) * sin(sa.x), sin(sa.y) * sin(sa.x), cos(sa.x));
        nb = vec3(cos(sb.y) * sin(sb.x), sin(sb.y) * sin(sb.x), cos(sb.x));
    } else {
        uint a = dyn ? dyn_verts[pc.poses.x + in_vert] : alias_verts[pc.poses.x + in_vert];
        uint b = dyn ? a : alias_verts[pc.poses.y + in_vert];
        pa = vec3(a & 0xffu, (a >> 8) & 0xffu, (a >> 16) & 0xffu);
        pb = vec3(b & 0xffu, (b >> 8) & 0xffu, (b >> 16) & 0xffu);
        na = ANORMS[min(a >> 24, 161u)];
        nb = ANORMS[min(b >> 24, 161u)];
    }
    float blend = pc.shadevec.w;
    vec3 p = mix(pa, pb, blend);
    float sh = mix(shadedot(na), shadedot(nb), blend);
    vec4 w = pc.model * vec4(p, 1.0);
    o_st = in_st;
    o_st_affine = in_st;
    o_shade = (pc.texs.w & 2u) != 0u ? vec3(1.0) : pc.light.rgb * sh;
    o_wpos = w.xyz;
    o_nrm = transpose(inverse(mat3(pc.model))) * mix(na, nb, blend);
    gl_Position = fr.viewproj * w;
}
