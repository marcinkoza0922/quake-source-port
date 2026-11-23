#version 460
#include "common.glsl"
#include "rt.glsl"
// Half-resolution single-sampled ray-traced water reflection trace pass.
// Outputs vec4(refl_rgb, 1.0) so water.frag can bilinearly sample with
// r.rgb / r.a un-premultiply and zero dark seams at shorelines.
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    uvec4 m = materials[i_mat];
    uint kind = m.z & 0xfu; // 1 water, 3 slime
    if (kind != 1u && kind != 3u) discard;
    float t = fr.eye.w;
    vec2 slope = vec2(0.0);
    if (fr.water.x > 0.0) {
        int qual = int(fr.water.w + 0.5);
        float dist_fade = water_dist_fade(i_wpos, qual);
        if (dist_fade > 0.0) {
            vec3 n = cross(dFdx(i_wpos), dFdy(i_wpos));
            float horiz = abs(n.z) * inversesqrt(max(dot(n, n), 1e-12));
            if (horiz > 0.7) {
                float k = fr.water.x * dist_fade;
                slope = water_grad(i_wpos, t, 0.5 * min(k, 1.5), 2.5 * k);
            }
        }
    }
    vec3 dp = fwidth(i_wpos);
    vec3 nf = normalize(cross(dFdx(i_wpos), dFdy(i_wpos)));
    vec3 v = fr.eye.xyz - i_wpos;
    float dv = length(v);
    v /= max(dv, 1e-4);
    if (dot(nf, v) < 0.0) nf = -nf;
    vec3 tx = normalize(abs(nf.z) > 0.7 ? vec3(1.0, 0.0, 0.0) - nf * nf.x : vec3(0.0, 0.0, 1.0) - nf * nf.z);
    vec3 ty = cross(nf, tx);
    vec2 wob = sin(i_st.yx * 0.125 + t * vec2(1.3, 1.1)) * 0.012;
    vec3 n = normalize(nf + tx * wob.x + ty * wob.y + vec3(-slope * 0.35 * sign(nf.z), 0.0));
    vec3 r = reflect(-v, n);
    float rn = dot(r, nf);
    if (rn < 0.02) r = normalize(r + nf * (0.02 - rn));
    // Half-res fwidth is 2x full-res; halve cone to keep crisp mip selection.
    float cone = 0.5 * max(dp.x, max(dp.y, dp.z)) / max(dv, 1.0);
    vec3 refl = rt_reflect(i_wpos + nf * 0.5, r, dv, cone);
    o_color = vec4(refl, 1.0);
}
