#version 460
#include "common.glsl"
layout(location = 0) in vec2 i_st;
layout(location = 1) in vec3 i_lm;
layout(location = 2) in vec3 i_wpos;
layout(location = 3) flat in uint i_mat;
layout(location = 0) out vec4 o_color;

// Skybox face selection as in Sky_ProjectPoly/Sky_EmitSkyBoxVertex.
vec3 skybox(vec3 d) {
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
    return tex(face, uv).rgb;
}

void main() {
    vec3 d = i_wpos - fr.eye.xyz;
    vec3 rgb;
    if (fr.sky.y > 0.5) {
        rgb = skybox(normalize(d));
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
    o_color = vec4(rgb, 1.0);
}
