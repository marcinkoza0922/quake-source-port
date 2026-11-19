#version 460
#include "common.glsl"
// r_shadow_realtime_dlight: world and brush entities into a cube shadow map
// face (depth only). Fence holes let light through.
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 4) flat in vec2 i_liq;
void main() {
    uvec4 m = materials[i_mat];
    if ((m.z & 0x10u) != 0u) {
        vec2 size = m.w != 0u ? vec2(m.w & 0xffffu, m.w >> 16) : vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
        if (textureLod(textures[nonuniformEXT(m.x)], i_st / size, 0.0).a < 0.666) discard;
    }
}
