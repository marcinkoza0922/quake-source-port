#version 460
#include "common.glsl"
// Narrow bloom glow pass for alias models with fullbright skins (pc.texs.y != NO_TEX).
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_shade;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) noperspective in vec2 i_st_affine;
layout(location = 4) in vec3 i_nrm;
layout(push_constant) uniform P {
    mat4 model; vec4 light; vec4 shadevec; uvec4 poses; uvec4 texs;
} pc;
layout(location = 0) out vec4 o_color;

vec4 stex(uint i, vec2 st, bool affine) {
    if (affine) {
        vec2 sz = vec2(textureSize(textures[nonuniformEXT(i)], 0));
        return textureLod(textures[nonuniformEXT(i)], st - 0.5 / sz, 0.0);
    }
    return tex(i, st);
}

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    bool affine = fr.softf.y == 2u;
    vec2 st = affine ? i_st_affine : i_st;
    bool holey = (pc.texs.w & 1u) != 0u;
    bool md3_alpha = (pc.texs.w & (32u | 64u)) != 0u && !holey;
    float out_a = pc.light.a;
    if (holey || md3_alpha) {
        float ca = stex(pc.texs.x, st, affine).a;
        if (holey && ca < 0.666) discard;
        if (md3_alpha) out_a *= ca;
    }
    vec3 fb = stex(pc.texs.y, st, affine).rgb;
    o_color = vec4(fb * fog_factor(i_wpos), out_a);
}
