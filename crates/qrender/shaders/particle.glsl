#include "common.glsl"
layout(location = 0) in vec2 i_uv;
layout(location = 1) in vec4 i_color;
layout(location = 2) in vec3 i_wpos;
layout(location = 0) out vec4 o_color;
void main() {
    vec4 col = i_color;
    if (fr.sky.w < 1.5) {
        // GLQuake round dot with an antialiased rim (R_ParticleTextureLookup, sharpness 8)
        float r = dot(i_uv, i_uv);
        if (r > 1.0) discard;
        col.a *= min(8.0 * (1.0 - r), 1.0);
    }
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    if (thermal()) {
        o_color = vec4(vec3(0.55 + 0.45 * luma(col.rgb)), col.a);
        GLOW_OUT(vec4(0.0, 0.0, 0.0, col.a));
        return;
    }
    o_color = vec4(apply_fog(col.rgb, i_wpos), col.a);
    GLOW_OUT(vec4(effect_glow(col.rgb) * fog_factor(i_wpos), col.a));
}
