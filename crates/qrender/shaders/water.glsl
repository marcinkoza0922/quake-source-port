#include "common.glsl"
#ifdef RT
#include "rt.glsl"
#endif
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(push_constant) uniform P { mat4 model; vec4 params; } pc;
layout(location = 0) out vec4 o_color;
void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    uvec4 m = materials[i_mat];
    float t = fr.eye.w;
    uint kind = m.z & 0xfu; // 1 water 2 lava 3 slime 4 tele
    if (kind == 4u && fr.portal_clip == vec4(0.0)) {
        for (int i = 0; i < 4; i++) {
            if (fr.portal_min[i].w == 0.0 || fr.portal_max[i].w != 1.0) continue;
            if (abs(dot(fr.portal_plane[i].xyz, i_wpos) + fr.portal_plane[i].w) > 0.25) continue;
            if (any(lessThan(i_wpos, fr.portal_min[i].xyz - 0.25)) ||
                any(greaterThan(i_wpos, fr.portal_max[i].xyz + 0.25))) continue;
            vec2 uv = gl_FragCoord.xy * fr.portal_uv.xy;
            vec2 warp = sin(i_st.yx * 0.14 + t * vec2(1.7, -1.3)) * 0.0015;
            vec3 behind = tex(uint(fr.portal_min[i].w), clamp(uv + warp, vec2(0.0), fr.portal_uv.zw)).rgb;
            vec2 cell = floor(i_st / 3.0);
            float seed = whitenoise01(cell);
            float pulse = pow(max(0.0, sin(t * (2.0 + seed * 3.0) + seed * 90.0)), 16.0);
            vec2 p = fract(i_st / 3.0) - 0.5;
            float spark = exp(-dot(p,p) * 40.0) * step(0.9, seed) * (0.15 + 0.85 * pulse);
            float veil = 0.59 + 0.06 * sin(i_st.x * 0.13 + sin(i_st.y * 0.1) + t * 2.0);
            o_color = vec4(behind * veil + vec3(0.8, 0.72, 0.57) * spark, 1.0);
            GLOW_OUT(vec4(vec3(0.8, 0.72, 0.57) * spark * fog_factor(i_wpos), 1.0));
            return;
        }
    }
    vec2 st = i_st;
    vec2 refl_off = vec2(0.0); // planar reflection wobble (screen uv)
    vec2 slope = vec2(0.0);    // ripple slope (ray-traced reflection normal)
    float shade = 1.0;
    float heat = 0.0; // lava only: disturbed crust glows hotter
    // r_waterripples: the height field bends the classic warp lookup and
    // shades the slopes; teleporters and non-horizontal sheets stay classic
    if (fr.water.x > 0.0 && kind >= 1u && kind <= 3u) {
        int qual = int(fr.water.w + 0.5);
        float dist_fade = water_dist_fade(i_wpos, qual);
        if (dist_fade > 0.0) {
            vec3 n = cross(dFdx(i_wpos), dFdy(i_wpos));
            float horiz = abs(n.z) * inversesqrt(max(dot(n, n), 1e-12));
            if (horiz > 0.7) {
                float k = fr.water.x * dist_fade;
                if (kind == 2u) {
                    // lava is thick: a slow heave, viscous swells that glow
                    vec2 g = water_grad(i_wpos, t * 0.3, 0.2 * min(k, 1.5), 0.0);
                    vec3 l = lava_rings(i_wpos, t, k);
                    g += l.xy;
                    st += g * 3.0;
                    shade = 1.0 + clamp(dot(g, vec2(0.6, 0.45)), -0.5, 0.5) * 0.8;
                    heat = l.z;
                } else {
                    vec2 g = water_grad(i_wpos, t, 0.5 * min(k, 1.5), 2.5 * k);
                    st += g * 5.0;
                    refl_off = g * 0.02;
                    slope = g;
                    float slope = dot(g, vec2(0.6, 0.45));
                    shade = 1.0 + clamp(slope, -0.6, 0.6) * 1.4 + smoothstep(0.25, 0.7, length(g)) * 0.2;
                }
            }
        }
    }
    st += 8.0 * sin(st.yx * 0.125 + t);
    vec4 c = tex(m.x, st / 64.0);
    if (heat > 0.0) {
        // toward yellow-white where the crust is broken
        vec3 hot = c.rgb * 1.5 + vec3(0.35, 0.18, 0.02);
        c.rgb = mix(c.rgb, hot, clamp(heat, 0.0, 1.0));
    }
    float a = kind >= 1u && kind <= 4u ? fr.alphas[kind - 1u] : 1.0;
    if (thermal()) {
        float h = kind == 2u ? 0.88 + 0.12 * luma(c.rgb) : cold_heat(c.rgb) * 0.7;
        o_color = vec4(vec3(h), a * pc.params.x);
        GLOW_OUT(vec4(0.0, 0.0, 0.0, a * pc.params.x));
        return;
    }
    vec3 col = apply_fog(c.rgb * shade, i_wpos);
    float alpha = a * pc.params.x;
#ifdef RT
    // r_water_reflections 2: a ray query along the reflection about the
    // face normal bent by the ripple slope and the classic warp, shaded like
    // the raster path and fogged over the whole path (rt.glsl)
    if (fr.refl.x > 0.0 && kind != 2u && kind != 4u && fr.portal_clip == vec4(0.0)) {
        vec3 dp = fwidth(i_wpos);
        vec3 nf = normalize(cross(dFdx(i_wpos), dFdy(i_wpos)));
        vec3 v = fr.eye.xyz - i_wpos;
        float dv = length(v);
        v /= dv;
        if (dot(nf, v) < 0.0) nf = -nf;
        vec3 tx = normalize(abs(nf.z) > 0.7 ? vec3(1.0, 0.0, 0.0) - nf * nf.x : vec3(0.0, 0.0, 1.0) - nf * nf.z);
        vec3 ty = cross(nf, tx);
        vec2 wob = sin(i_st.yx * 0.125 + t * vec2(1.3, 1.1)) * 0.012;
        vec3 n = normalize(nf + tx * wob.x + ty * wob.y + vec3(-slope * 0.35 * sign(nf.z), 0.0));
        vec3 r = reflect(-v, n);
        float rn = dot(r, nf);
        if (rn < 0.02) r = normalize(r + nf * (0.02 - rn));
        float cone = max(dp.x, max(dp.y, dp.z)) / max(dv, 1.0);
        vec3 refl = rt_reflect(i_wpos + nf * 0.5, r, dv, cone);
        float cosv = clamp(dot(v, nf), 0.0, 1.0);
        float fres = 0.2 + 0.8 * pow(1.0 - cosv, 5.0);
        float f = min(fres * fr.refl.x, 0.9);
        col = mix(col, refl, f);
        alpha = max(alpha, f);
    }
#else
    // r_water_reflections: the mirrored view of this sheet's plane (portal
    // slot kind 2), Fresnel-weighted and wobbled by the warp and ripples.
    // Its image is fogged along the reflected path already.
    if (fr.refl.x > 0.0 && kind != 2u && kind != 4u && fr.portal_clip == vec4(0.0)) {
        for (int i = 0; i < 4; i++) {
            if (fr.portal_max[i].w != 2.0) continue;
            if (abs(dot(fr.portal_plane[i].xyz, i_wpos) + fr.portal_plane[i].w) > 0.25) continue;
            if (any(lessThan(i_wpos, fr.portal_min[i].xyz - 0.25)) ||
                any(greaterThan(i_wpos, fr.portal_max[i].xyz + 0.25))) continue;
            vec3 v = normalize(fr.eye.xyz - i_wpos);
            float cosv = clamp(abs(dot(v, fr.portal_plane[i].xyz)), 0.0, 1.0);
            float fres = 0.2 + 0.8 * pow(1.0 - cosv, 5.0);
            float f = min(fres * fr.refl.x, 0.9);
            vec2 wob = sin(i_st.yx * 0.125 + t * vec2(1.3, 1.1)) * 0.0025 + refl_off;
            vec2 uv = clamp(gl_FragCoord.xy * fr.portal_uv.xy + wob, vec2(0.0), fr.portal_uv.zw);
            vec3 refl = tex(uint(fr.portal_min[i].w), uv).rgb;
            col = mix(col, refl, f);
            alpha = max(alpha, f);
            break;
        }
    }
#endif
    o_color = vec4(col, alpha);
    // lava and teleporters are drawn fullbright and glow; hot crust more
    float g = kind == 2u ? 0.55 * (1.0 + heat) : kind == 4u ? 0.5 : 0.0;
    GLOW_OUT(vec4(c.rgb * shade * (g * fog_factor(i_wpos)), a * pc.params.x));
}
