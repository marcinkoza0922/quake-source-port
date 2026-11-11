#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_shade;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) noperspective in vec2 i_st_affine;
layout(push_constant) uniform P {
    mat4 model; vec4 light; vec4 shadevec; uvec4 poses; uvec4 texs;
} pc;
layout(location = 0) out vec4 o_color;

vec3 ramp(uint row, uint k) {
    uint idx = row < 8u ? row * 16u + k : row * 16u + 15u - k;
    return unpack_rgba(palette[idx]).rgb;
}
// ironwail alias modes: 0 standard, 1 dither (softemu coarse), 2 affine
// (r_softemu_mdl_warp: software-renderer texture warping, no mips).
vec4 stex(uint i, vec2 st, bool affine) {
    if (affine) {
        vec2 sz = vec2(textureSize(textures[nonuniformEXT(i)], 0));
        return textureLod(textures[nonuniformEXT(i)], st - 0.5 / sz, 0.0);
    }
    return tex(i, st);
}
void main() {
    uint mode = fr.softf.y;
    bool affine = mode == 2u;
    vec2 st = affine ? i_st_affine : i_st;
    vec4 c = stex(pc.texs.x, st, affine);
    if ((pc.texs.w & 1u) != 0u && c.a < 0.666) discard;
    vec3 rgb = c.rgb;
    // colour translation: shift masked texels from the default ramps (shirt
    // row 1, pants row 6) to the entity's rows; untranslated models skip this
    if (pc.texs.z != NO_TEX && (pc.texs.w & 4u) != 0u) {
        vec2 m = stex(pc.texs.z, st, affine).rg * 255.0 + 0.5;
        uint top = uint(m.x), bot = uint(m.y);
        if (top > 0u) rgb += ramp(pc.poses.z, top - 1u) - ramp(1u, top - 1u);
        if (bot > 0u) rgb += ramp(pc.poses.w, bot - 1u) - ramp(6u, bot - 1u);
    }
    rgb *= i_shade * 2.0;
    if (pc.texs.y != NO_TEX) rgb += stex(pc.texs.y, st, affine).rgb;
    rgb = apply_fog(clamp(rgb, 0.0, 1.0), i_wpos);
    if (mode != 0u) {
        if (fr.fog.w > 0.0) {
            rgb = sqrt(rgb);
            rgb += SCREEN_SPACE_NOISE() * fr.soft.x;
            rgb *= rgb;
        }
    } else {
        rgb += SUPPRESS_BANDING() * fr.soft.x;
    }
    o_color = vec4(rgb, pc.light.a);
}
