#version 460
#include "common.glsl"
// r_shadow_realtime_dlight: alias models into a cube shadow map face (depth
// only), alpha-tested skins with holes.
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_shade;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) noperspective in vec2 i_st_affine;
layout(location = 4) in vec3 i_nrm;
layout(push_constant) uniform P {
    mat4 model; vec4 light; vec4 shadevec; uvec4 poses; uvec4 texs;
} pc;
void main() {
    if ((pc.texs.w & 1u) != 0u && textureLod(textures[nonuniformEXT(pc.texs.x)], i_st, 0.0).a < 0.666) discard;
}
