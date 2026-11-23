#version 460
#include "common.glsl"
// Narrow bloom glow pass for glowing liquids (lava and teleporters only).
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    uvec4 m = materials[i_mat];
    uint kind = m.z & 0xfu; // 2 lava, 4 tele
    if (kind != 2u && kind != 4u) discard;
    float t = fr.eye.w;
    if (kind == 4u && fr.portal_clip == vec4(0.0)) {
        for (int i = 0; i < 4; i++) {
            if (fr.portal_min[i].w == 0.0 || fr.portal_max[i].w != 1.0) continue;
            if (abs(dot(fr.portal_plane[i].xyz, i_wpos) + fr.portal_plane[i].w) > 0.25) continue;
            if (any(lessThan(i_wpos, fr.portal_min[i].xyz - 0.25)) ||
                any(greaterThan(i_wpos, fr.portal_max[i].xyz + 0.25))) continue;
            vec2 cell = floor(i_st / 3.0);
            float seed = whitenoise01(cell);
            float pulse = pow(max(0.0, sin(t * (2.0 + seed * 3.0) + seed * 90.0)), 16.0);
            vec2 p = fract(i_st / 3.0) - 0.5;
            float spark = exp(-dot(p, p) * 40.0) * step(0.9, seed) * (0.15 + 0.85 * pulse);
            o_color = vec4(vec3(0.8, 0.72, 0.57) * spark * fog_factor(i_wpos), 1.0);
            return;
        }
    }
    vec2 st = i_st;
    float shade = 1.0;
    float heat = 0.0;
    if (fr.water.x > 0.0 && kind == 2u) {
        int qual = int(fr.water.w + 0.5);
        float dist_fade = water_dist_fade(i_wpos, qual);
        if (dist_fade > 0.0) {
            vec3 n = cross(dFdx(i_wpos), dFdy(i_wpos));
            float horiz = abs(n.z) * inversesqrt(max(dot(n, n), 1e-12));
            if (horiz > 0.7) {
                float k = fr.water.x * dist_fade;
                vec2 g = water_grad(i_wpos, t * 0.3, 0.2 * min(k, 1.5), 0.0);
                vec3 l = lava_rings(i_wpos, t, k);
                g += l.xy;
                st += g * 3.0;
                shade = 1.0 + clamp(dot(g, vec2(0.6, 0.45)), -0.5, 0.5) * 0.8;
                heat = l.z;
            }
        }
    }
    st += 8.0 * sin(st.yx * 0.125 + t);
    vec4 c = tex(m.x, st / 64.0);
    if (heat > 0.0) {
        vec3 hot = c.rgb * 1.5 + vec3(0.35, 0.18, 0.02);
        c.rgb = mix(c.rgb, hot, clamp(heat, 0.0, 1.0));
    }
    float a = fr.alphas[kind - 1u];
    float g = kind == 2u ? 0.55 * (1.0 + heat) : 0.5;
    o_color = vec4(c.rgb * shade * (g * fog_factor(i_wpos)), a * pc.params.x);
}
