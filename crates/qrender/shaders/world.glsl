#include "common.glsl"
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
    if (pc.params.y > 0.0) {
        o_color = pc.params.y == 1.0 ? vec4(0.75, 0.95, 1.0, 0.9) : vec4(1.0, 0.32, 0.12, 0.35);
        GLOW_OUT(vec4(0.0));
        return;
    }
    uvec4 m = materials[i_mat];
    // texel coords are in miptex units; an external replacement may differ in size
    vec2 size = m.w != 0u ? vec2(m.w & 0xffffu, m.w >> 16) : vec2(textureSize(textures[nonuniformEXT(m.x)], 0));
    vec2 uv = i_st / size;
    // ironwail world shader DITHER levels: 1 = softemu coarse, 2 = banded lightmap
    uint dither = fr.softf.x & 15u;
    bool fullbright = (fr.softf.x & 16u) != 0u; // r_fullbright
    float bias = dither >= 2u ? -1.0 : dither == 1u ? -0.5 : 0.0;
    vec4 c = texture(textures[nonuniformEXT(m.x)], uv, bias);
    if ((m.z & 0x10u) != 0u && c.a < 0.666) discard;
    vec3 lm = i_lm;
    vec2 lmsize = vec2(textureSize(lightmap, 0).xy) * 16.0;
    if (dither != 0u) // snap to the software renderer's surface-cache texels
        lm.xy = (floor(lm.xy * lmsize) + 0.5) / lmsize;
    if ((fr.softf.x & 32u) != 0u) {
        vec3 visible_light = (m.z & 0x20u) != 0u ? vec3(1.0) : texture(lightmap, lm).rgb;
        o_color = vec4(visible_light, pc.params.x);
        GLOW_OUT(vec4(0.0));
        return;
    }
    vec3 light = (m.z & 0x20u) != 0u || fullbright ? vec3(1.0) : texture(lightmap, lm).rgb;
    // r_shadow_realtime_dlight: dynamic lights per pixel (the lightmaps then
    // leave them out), using smooth FACENORMALS when present (else plane normal).
    bool has_nrm = dot(i_nrm, i_nrm) > 1e-6;
    vec3 nf = has_nrm ? normalize(i_nrm) : normalize(cross(dFdx(i_wpos), dFdy(i_wpos)));
    if ((!has_nrm || (m.z & 0x10u) != 0u) && dot(nf, fr.eye.xyz - i_wpos) < 0.0) nf = -nf;
    if (light_count.x > 0u && (m.z & 0x20u) == 0u && !fullbright) {
        vec3 dl = dyn_lights(i_wpos, nf);
        light = fr.hdr_deluxe.y > 0.0 ? (light + dl) : min(light + dl, vec3(2.0));
    }
    if (dither >= 2u) // 64 light levels, like the colormap
        light = floor(light * (63.0 / 2.0) + 0.5) * (2.0 / 63.0);
    // Directional lightmap (.dlit / LIGHTINGDIR): subtle normal shading & specular
    vec3 spec_add = vec3(0.0);
    if (fr.hdr_deluxe.x > 0.0 && (m.z & 0x20u) == 0u && !fullbright) {
        vec4 dlx = texture(lightmap, vec3(lm.xy, lm.z + fr.hdr_deluxe.x));
        if (dot(dlx.xyz, dlx.xyz) > 1e-4) {
            vec3 L = normalize(dlx.xyz);
            float ndl = max(dot(nf, L), 0.0);
            light *= mix(1.0, 0.75 + 0.5 * ndl, fr.hdr_deluxe.w);
            if (fr.hdr_deluxe.z > 0.0) {
                vec3 V = normalize(fr.eye.xyz - i_wpos);
                vec3 H = normalize(L + V);
                float spec = pow(max(dot(nf, H), 0.0), 24.0) * ndl;
                spec_add = light * (spec * fr.hdr_deluxe.z * (0.35 + 0.65 * luma(c.rgb)));
            }
        }
    }
    vec3 rgb = c.rgb * light + spec_add;
    // caustics on submerged faces (not lava); water bluish, slime green
    uint lk = uint(i_liq.y + 0.5);
    if (fr.water.y > 0.0 && (lk & 3u) != 0u && (lk & 3u) != 2u && i_wpos.z < i_liq.x) {
        int qual = int(fr.water.w + 0.5);
        float dist_fade = water_dist_fade(i_wpos, qual);
        if (dist_fade > 0.0) {
            float depth = i_liq.x - i_wpos.z;
            float t = fr.eye.w;
            // light comes in at a slant, so walls get streaks shifted by depth
            vec2 q = i_wpos.xy + depth * vec2(0.55, 0.35);
            q = floor(q) + 0.5; // world-unit texels, like the rest of the world
            float cst = caustic_tex(q, t * 0.9);
            float fade = clamp(depth / 8.0, 0.0, 1.0) * exp(-depth / 384.0)
                       * mix(0.55, 1.0, abs(nf.z)) * ((lk & 4u) != 0u ? 0.4 : 1.0)
                       * dist_fade;
            vec3 tint = (lk & 3u) == 3u ? vec3(0.55, 1.0, 0.35) : vec3(0.75, 0.92, 1.0);
            rgb += c.rgb * light * tint * (cst * fade * fr.water.y * 1.8);
        }
    }
    vec3 hdr_excess = fr.hdr_deluxe.y > 0.0 ? max(rgb - vec3(1.0), vec3(0.0)) * 0.45 : vec3(0.0);
    vec3 fb = m.y != NO_TEX ? texture(textures[nonuniformEXT(m.y)], uv, bias).rgb : vec3(0.0);
    rgb += fb;
    rgb = apply_fog(fr.hdr_deluxe.y > 0.0 ? hdr_tonemap(rgb) : clamp(rgb, 0.0, 1.0), i_wpos);
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
        h += 1.6 * luma(fb);
        rgb = vec3(min(h, 1.0));
        fb = vec3(0.0);
        hdr_excess = vec3(0.0);
    }
    o_color = vec4(rgb, pc.params.x);
    // fullbright texels (lamps, runes, lava rock) and HDR overbright peaks glow
    GLOW_OUT(vec4((fb + hdr_excess) * fog_factor(i_wpos), pc.params.x));
}
