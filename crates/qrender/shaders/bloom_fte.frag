#version 460
#include "common.glsl"
// Narrow bloom glow pass for glowing FTE particle batches and gun portal rims.
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
layout(push_constant) uniform P {
    vec4 fogmix; // y: soft inv fade, w: blend mode (0 Blend, 2 AddA, 3 AddC, 7 Premul, 8 GunPortal)
    uint tex;
    uint msaa;
    vec2 scale;
    ivec2 max_px;
} pc;
layout(location = 0) out vec4 o_color;

float linear_depth(float d) {
    const float n = 4.0, f = 65536.0;
    return n * f / (f - d * (f - n));
}

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    if (pc.fogmix.w == 8.0) {
        vec2 q = i_uv * 2.0 - 1.0;
        float d = length(q);
        float aa = fwidth(d) * 1.5;
        if (d > 1.0) discard;
        float rim = smoothstep(0.84 - aa, 0.9, d);
        vec3 glow = mix(i_color.rgb, vec3(1.0), 0.35 * (1.0 - smoothstep(0.9, 1.0, d)));
        float alpha = 1.0 - smoothstep(1.0 - aa, 1.0, d);
        o_color = vec4(glow * (rim * 0.6), alpha);
        return;
    }
    vec4 c = tex(pc.tex, i_uv) * i_color;
    int bm = int(pc.fogmix.w);
    if (pc.fogmix.y > 0.0) {
        ivec2 px = min(ivec2(gl_FragCoord.xy * pc.scale), pc.max_px);
        float d = pc.msaa > 1u ? texelFetch(scene_depth_ms, px, 0).r : texelFetch(scene_depth, px, 0).r;
        float pz = dot(i_wpos - fr.eye.xyz, fr.vfwd.xyz);
        float fade = d < 1.0 ? clamp((linear_depth(d) - pz) * pc.fogmix.y, 0.0, 1.0) : 1.0;
        if (bm == 3 || bm == 7) c.rgb *= fade;
        if (bm == 0 || bm == 2 || bm == 7) c.a *= fade;
    }
    vec3 glow = (bm == 2 || bm == 3) ? c.rgb : effect_glow(c.rgb);
    o_color = vec4(glow * fog_factor(i_wpos), c.a);
}
