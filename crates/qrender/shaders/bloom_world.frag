#version 460
#include "common.glsl"
// Narrow bloom glow pass for world & brush surfaces: emits fullbright texels
// (and HDR lightmap excess when active) directly into bloom level 0.
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 4) flat in vec2 i_liq;
layout(location = 5) in vec3 i_nrm;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    uvec4 m = materials[i_mat];
    vec2 size = m.w != 0u ? vec2(m.w & 0xffffu, m.w >> 16) : vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
    vec2 uv = i_st / size;
    uint dither = fr.softf.x & 15u;
    float bias = dither >= 2u ? -1.0 : dither == 1u ? -0.5 : 0.0;
    vec4 c = vec4(0.0);
    bool need_base = (m.z & 0x10u) != 0u || fr.hdr_deluxe.y > 0.0;
    if (need_base) {
        c = texture(textures[nonuniformEXT(m.x)], uv, bias);
        if ((m.z & 0x10u) != 0u && c.a < 0.666) discard;
    }
    vec3 fb = m.y != NO_TEX ? texture(textures[nonuniformEXT(m.y)], uv, bias).rgb : vec3(0.0);
    vec3 hdr_excess = vec3(0.0);
    if (fr.hdr_deluxe.y > 0.0) {
        bool fullbright = (fr.softf.x & 16u) != 0u;
        vec3 light = (m.z & 0x20u) != 0u || fullbright ? vec3(1.0) : texture(lightmap, i_lm).rgb;
        hdr_excess = max(c.rgb * light - vec3(1.0), vec3(0.0)) * 0.45;
    }
    o_color = vec4((fb + hdr_excess) * fog_factor(i_wpos), pc.params.x);
}
