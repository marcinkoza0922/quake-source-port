#include "common.glsl"
#include "sky_common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 0) out vec4 o_color;

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    if ((fr.softf.x & 32u) != 0u) {
        o_color = vec4(0.0);
        GLOW_OUT(vec4(0.0));
        return;
    }
    vec3 rgb = sky_color(i_wpos - fr.eye.xyz);
    if (fr.fog.w > 0.0) rgb = mix(rgb, fr.fog.rgb, fr.sky.x);
    if (thermal()) rgb = vec3(0.02);
    o_color = vec4(rgb, 0.0); // alpha 0: no SSAO on sky
    GLOW_OUT(vec4(0.0));
}
