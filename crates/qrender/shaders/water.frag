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
    uint kind = m.z & 0xfu; // 1 water 2 lava 3 slime 4 tele
    vec2 st = i_st;
    float shade = 1.0;
    // r_waterripples: the height field bends the classic warp lookup and
    // shades the slopes; teleporters and non-horizontal sheets stay classic
    if (fr.water.x > 0.0 && kind >= 1u && kind <= 3u) {
        vec3 n = cross(dFdx(i_wpos), dFdy(i_wpos));
        float horiz = abs(n.z) * inversesqrt(max(dot(n, n), 1e-12));
        if (horiz > 0.7) {
            float swell = kind == 2u ? 0.5 : 1.0; // lava is thick
            float k = fr.water.x;
            vec2 g = water_grad(i_wpos, t, swell * 0.5 * min(k, 1.5), 2.5 * k);
            st += g * 5.0;
            float slope = dot(g, vec2(0.6, 0.45));
            shade = 1.0 + clamp(slope, -0.6, 0.6) * 1.4 + smoothstep(0.25, 0.7, length(g)) * 0.2;
        }
    }
    st += 8.0 * sin(st.yx * 0.125 + t);
    vec4 c = tex(m.x, st / 64.0);
    float a = kind >= 1u && kind <= 4u ? fr.alphas[kind - 1u] : 1.0;
    o_color = vec4(apply_fog(c.rgb * shade, i_wpos), a * pc.params.x);
}
