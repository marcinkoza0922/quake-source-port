#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    uvec4 m = materials[i_mat];
    vec2 size = vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
    vec2 uv = i_st / size;
    vec4 c = tex(m.x, uv);
    if ((m.z & 0x10u) != 0u && c.a < 0.666) discard;
    vec3 light = (m.z & 0x20u) != 0u ? vec3(1.0) : texture(lightmap, i_lm).rgb;
    vec3 rgb = c.rgb * light;
    if (m.y != NO_TEX) rgb += tex(m.y, uv).rgb;
    o_color = vec4(apply_fog(rgb, i_wpos), pc.params.x);
}
