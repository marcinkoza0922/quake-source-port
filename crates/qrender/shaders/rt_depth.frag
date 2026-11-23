#version 460
#include "common.glsl"
// Depth-only prepass for half-resolution ray-traced water reflections: writes
// depth only (colour write mask is empty) and honours fence alpha cutouts.
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 4) flat in vec2 i_liq;
layout(location = 5) in vec3 i_nrm;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    uvec4 m = materials[i_mat];
    if ((m.z & 0x10u) != 0u) {
        vec2 size = m.w != 0u ? vec2(m.w & 0xffffu, m.w >> 16) : vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
        if (texture(textures[nonuniformEXT(m.x)], i_st / size).a < 0.666) discard;
    }
}
