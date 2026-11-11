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
    // ironwail world shader DITHER levels: 1 = softemu coarse, 2 = banded lightmap
    uint dither = fr.softf.x;
    float bias = dither >= 2u ? -1.0 : dither == 1u ? -0.5 : 0.0;
    vec4 c = texture(textures[nonuniformEXT(m.x)], uv, bias);
    if ((m.z & 0x10u) != 0u && c.a < 0.666) discard;
    vec3 lm = i_lm;
    vec2 lmsize = vec2(textureSize(lightmap, 0).xy) * 16.0;
    if (dither != 0u) // snap to the software renderer's surface-cache texels
        lm.xy = (floor(lm.xy * lmsize) + 0.5) / lmsize;
    vec3 light = (m.z & 0x20u) != 0u ? vec3(1.0) : texture(lightmap, lm).rgb;
    if (dither >= 2u) // 64 light levels, like the colormap
        light = floor(light * (63.0 / 2.0) + 0.5) * (2.0 / 63.0);
    vec3 rgb = c.rgb * light;
    if (m.y != NO_TEX) rgb += texture(textures[nonuniformEXT(m.y)], uv, bias).rgb;
    rgb = apply_fog(clamp(rgb, 0.0, 1.0), i_wpos);
    if (dither == 1u) {
        vec3 dpos = fwidth(i_wpos);
        float farblend = clamp(max(dpos.x, max(dpos.y, dpos.z)) * 0.5 - 0.125, 0.0, 1.0);
        farblend *= farblend;
        rgb = sqrt(rgb);
        float luma = dot(rgb, vec3(.25, .625, .125));
        float nearnoise = tri(whitenoise01(lm.xy * lmsize)) * luma * fr.soft.y;
        float farnoise = fr.fog.w > 0.0 ? SCREEN_SPACE_NOISE() * fr.soft.x : 0.0;
        rgb += mix(nearnoise, farnoise, farblend);
        rgb *= rgb;
    } else if (dither == 0u) {
        rgb += SUPPRESS_BANDING() * fr.soft.x;
    }
    o_color = vec4(rgb, pc.params.x);
}
