#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 4) flat in vec2 i_liq;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
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
    // caustics on submerged faces (not lava); water bluish, slime green
    uint lk = uint(i_liq.y + 0.5);
    if (fr.water.y > 0.0 && (lk & 3u) != 0u && (lk & 3u) != 2u && i_wpos.z < i_liq.x) {
        float depth = i_liq.x - i_wpos.z;
        float t = fr.eye.w;
        // light comes in at a slant, so walls get streaks shifted by depth
        vec2 q = i_wpos.xy + depth * vec2(0.55, 0.35);
        q += water_grad(vec3(q, i_liq.x), t, 0.0, 1.0) * 40.0;
        q = floor(q) + 0.5; // world-unit texels, like the rest of the world
        float cst = caustic(q / 56.0, t * 0.9);
        vec3 n = normalize(cross(dFdx(i_wpos), dFdy(i_wpos)));
        float fade = clamp(depth / 8.0, 0.0, 1.0) * exp(-depth / 384.0)
                   * mix(0.55, 1.0, abs(n.z)) * ((lk & 4u) != 0u ? 0.4 : 1.0);
        vec3 tint = (lk & 3u) == 3u ? vec3(0.55, 1.0, 0.35) : vec3(0.75, 0.92, 1.0);
        rgb += c.rgb * light * tint * (cst * fade * fr.water.y * 1.8);
    }
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
    if (thermal()) {
        float h = cold_heat(c.rgb * light);
        // fullbright texels (torches, lights, lava rock) are hot
        if (m.y != NO_TEX) h += 1.6 * luma(texture(textures[nonuniformEXT(m.y)], uv, bias).rgb);
        rgb = vec3(min(h, 1.0));
    }
    o_color = vec4(rgb, pc.params.x);
}
