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

float shadedot(vec3 n) {
    float d = dot(n, pc.shadevec.xyz);
    return d < 0.0 ? 1.0 + d * (13.0 / 44.0) : 1.0 + d;
}
void main() {
    // flag 16: a CPU-built pose in the per-frame buffer (ragdolls)
    bool dyn = (pc.texs.w & 16u) != 0u;
    uint a = dyn ? dyn_verts[pc.poses.x + in_vert] : alias_verts[pc.poses.x + in_vert];
    uint b = dyn ? a : alias_verts[pc.poses.y + in_vert];
    vec3 pa = vec3(a & 0xffu, (a >> 8) & 0xffu, (a >> 16) & 0xffu);
    vec3 pb = vec3(b & 0xffu, (b >> 8) & 0xffu, (b >> 16) & 0xffu);
    float blend = pc.shadevec.w;
    vec3 p = mix(pa, pb, blend);
    float sh = mix(shadedot(ANORMS[min(a >> 24, 161u)]), shadedot(ANORMS[min(b >> 24, 161u)]), blend);
    vec4 w = pc.model * vec4(p, 1.0);
    o_st = in_st;
    o_st_affine = in_st;
    o_shade = (pc.texs.w & 2u) != 0u ? vec3(1.0) : pc.light.rgb * sh;
    o_wpos = w.xyz;
    gl_Position = fr.viewproj * w;
}
