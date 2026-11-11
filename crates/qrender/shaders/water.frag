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
    float t = fr.eye.w;
    vec2 st = i_st + 8.0 * sin(i_st.yx * 0.125 + t);
    vec4 c = tex(m.x, st / 64.0);
    uint kind = m.z & 0xfu; // 1 water 2 lava 3 slime 4 tele
    float a = kind >= 1u && kind <= 4u ? fr.alphas[kind - 1u] : 1.0;
    o_color = vec4(apply_fog(c.rgb, i_wpos), a * pc.params.x);
}
