#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 0) out vec4 o_color;

// Skybox face selection as in Sky_ProjectPoly/Sky_EmitSkyBoxVertex.
vec4 skybox(vec3 d) {
    vec3 a = abs(d);
    uint face; float s; float t; float ma;
    if (a.x >= a.y && a.x >= a.z) {
        ma = a.x;
        if (d.x > 0.0) { face = fr.skybox0.x; s = -d.y; t = d.z; }      // rt
        else           { face = fr.skybox0.z; s = d.y;  t = d.z; }      // lf
    } else if (a.y >= a.z) {
        ma = a.y;
        if (d.y > 0.0) { face = fr.skybox0.y; s = d.x;  t = d.z; }      // bk
        else           { face = fr.skybox0.w; s = -d.x; t = d.z; }      // ft
    } else {
        ma = a.z;
        if (d.z > 0.0) { face = fr.skybox1.x; s = -d.y; t = -d.x; }      // up
        else           { face = fr.skybox1.y; s = -d.y; t = d.x; }     // dn
    }
    vec2 uv = vec2(s, t) / ma * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;
    uv = clamp(uv, 1.0 / 512.0, 1.0 - 1.0 / 512.0);
    return tex(face, uv);
}

// Sky wind (ironwail's sky_cubemap ANIM): two copies of the box drift along
// the wind, cross-faded over the loop, over the still box by their alpha.
vec3 skybox_wind(vec3 d) {
    vec4 base = skybox(d);
    if (fr.wind.xyz == vec3(0.0)) return base.rgb;
    float t1 = fr.wind.w;
    float t2 = fract(t1) - 0.5;
    float blend = abs(t1 * 2.0);
    vec4 layer1 = skybox(d + t1 * fr.wind.xyz);
    vec4 layer2 = skybox(d + t2 * fr.wind.xyz);
    layer1.a *= 1.0 - blend;
    layer2.a *= blend;
    vec4 combined = vec4(layer1.rgb * layer1.a + layer2.rgb * layer2.a, layer1.a + layer2.a);
    return base.rgb * (1.0 - combined.a) + combined.rgb;
}

void main() {
    if (dot(fr.portal_clip.xyz, i_wpos) + fr.portal_clip.w < 0.0) discard;
    vec3 d = i_wpos - fr.eye.xyz;
    vec3 rgb;
    if (fr.sky.y > 0.5) {
        rgb = skybox_wind(normalize(d));
    } else {
        d.z *= 3.0;
        vec2 uv = normalize(d).xy * (189.0 / 64.0);
        float t = fr.eye.w;
        vec4 solid = tex(fr.skybox1.z, uv + t / 16.0);
        vec4 alpha = tex(fr.skybox1.w, uv + t / 8.0);
        alpha.a *= fr.sky.z;
        rgb = mix(solid.rgb, alpha.rgb, alpha.a);
    }
    if (fr.fog.w > 0.0) rgb = mix(rgb, fr.fog.rgb, fr.sky.x);
    if (thermal()) rgb = vec3(0.02);
    o_color = vec4(rgb, 0.0); // alpha 0: no SSAO on sky
    GLOW_OUT(vec4(0.0));
}
