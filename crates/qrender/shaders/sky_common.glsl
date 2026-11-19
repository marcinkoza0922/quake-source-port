// Sky lookups shared by sky.glsl and ray-traced reflections (rt.glsl).
// rt.glsl defines SKY_LOD: ray hits have no screen derivatives.
#ifdef SKY_LOD
vec4 sky_tex(uint i, vec2 uv) { return textureLod(textures[nonuniformEXT(i)], uv, SKY_LOD); }
#else
vec4 sky_tex(uint i, vec2 uv) { return tex(i, uv); }
#endif
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
    return sky_tex(face, uv);
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

// Sky colour seen along direction `d` (unnormalised is fine): the skybox
// with wind, or the classic two scrolling layers.
vec3 sky_color(vec3 d) {
    if (fr.sky.y > 0.5) return skybox_wind(normalize(d));
    d.z *= 3.0;
    vec2 uv = normalize(d).xy * (189.0 / 64.0);
    float t = fr.eye.w;
    vec4 solid = sky_tex(fr.skybox1.z, uv + t / 16.0);
    vec4 alpha = sky_tex(fr.skybox1.w, uv + t / 8.0);
    alpha.a *= fr.sky.z;
    return mix(solid.rgb, alpha.rgb, alpha.a);
}
