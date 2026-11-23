#version 460
#include "common.glsl"
// Screen-space ambient occlusion (Alchemy/SAO-style obscurance) from a
// linear-depth mip chain (aosource.frag + aodown.frag).
// Output: r = visibility (1 = unoccluded), g = view-space depth for aoapply.frag.
layout(push_constant) uniform P {
    vec4 proj;   // tan half-fov x, y, scene MSAA samples, world radius
    vec4 size;   // AO viewport w, h, intensity, linear-depth mip chain slot
    vec2 scene_per_ao;
    ivec2 scene_max;
    uint quality;
    uint color_slot; // single-sampled scene colour slot (used when MSAA == 1)
    int max_mip;
    uint _pad;
} pc;
layout(location = 0) out vec4 o_ao;

ivec2 scene_px(ivec2 px) {
    return clamp(ivec2((vec2(px) + 0.5) * pc.scene_per_ao), ivec2(0), pc.scene_max);
}

void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    ivec2 ao_max = ivec2(pc.size.xy) - 1;
    uint depth_slot = uint(pc.size.w);
    vec2 inv_size = 1.0 / pc.size.xy;
    vec2 ndc_scale = 2.0 * pc.proj.xy * inv_size;
    vec2 ndc_bias = -pc.proj.xy + 0.5 * ndc_scale;

    float pz = texelFetch(textures[nonuniformEXT(depth_slot)], clamp(px, ivec2(0), ao_max), 0).r;
    if (pz >= 65500.0) { o_ao = vec4(1.0, pz, 0.0, 0.0); return; }
    vec3 p = vec3((vec2(px) * ndc_scale + ndc_bias) * pz, pz);

    // normal from the smaller depth difference on each axis (keeps edges)
    ivec2 pl = clamp(px - ivec2(1, 0), ivec2(0), ao_max);
    ivec2 pr = clamp(px + ivec2(1, 0), ivec2(0), ao_max);
    ivec2 pu = clamp(px - ivec2(0, 1), ivec2(0), ao_max);
    ivec2 pb = clamp(px + ivec2(0, 1), ivec2(0), ao_max);
    float zl = texelFetch(textures[nonuniformEXT(depth_slot)], pl, 0).r;
    float zr = texelFetch(textures[nonuniformEXT(depth_slot)], pr, 0).r;
    float zu = texelFetch(textures[nonuniformEXT(depth_slot)], pu, 0).r;
    float zb = texelFetch(textures[nonuniformEXT(depth_slot)], pb, 0).r;
    vec3 l = vec3((vec2(pl) * ndc_scale + ndc_bias) * zl, zl);
    vec3 r = vec3((vec2(pr) * ndc_scale + ndc_bias) * zr, zr);
    vec3 u = vec3((vec2(pu) * ndc_scale + ndc_bias) * zu, zu);
    vec3 b = vec3((vec2(pb) * ndc_scale + ndc_bias) * zb, zb);
    vec3 dx = abs(zr - pz) < abs(pz - zl) ? r - p : p - l;
    vec3 dy = abs(zb - pz) < abs(pz - zu) ? b - p : p - u;
    vec3 n = normalize(cross(dy, dx));
    if (dot(n, p) > 0.0) n = -n;

    int N = pc.quality == 0u ? 6 : (pc.quality == 1u ? 8 : 12);
    float radius = pc.proj.w;
    // screen-space radius in AO pixels
    float rpx = radius / pz * pc.size.x * 0.5 / pc.proj.x;
    if (rpx < 1.0) { o_ao = vec4(1.0, pz, 0.0, 0.0); return; }
    rpx = min(rpx, pc.size.y * 0.25);
    float spin = pc.quality == 0u
        ? bayer01(px & 1) * 6.2831853
        : bayer01(px & 3) * 6.2831853 + whitenoise01(vec2(px)) * 0.5;
    // Rotate a unit direction by the golden angle each tap (1 sin/cos total instead of N).
    vec2 dir = vec2(cos(spin), sin(spin));
    const float GA_C = -0.73736888; // cos(2.3999632)
    const float GA_S =  0.67549029; // sin(2.3999632)
    float occ = 0.0;
    float inv_r2 = 1.0 / (radius * radius);
    float inv_N = 1.0 / float(N);
    float bias = 0.1 + 0.0005 * pz;
    for (int i = 0; i < N; i++) {
        float h = (float(i) + 0.5) * inv_N * rpx;
        ivec2 q = clamp(px + ivec2(round(dir * h)), ivec2(0), ao_max);
        int mip = clamp(findMSB(max(int(h), 1)) - 3, 0, pc.max_mip);
        ivec2 qm = clamp(q >> mip, ivec2(0), ao_max >> mip);
        float qz = texelFetch(textures[nonuniformEXT(depth_slot)], qm, mip).r;
        vec3 v = vec3((vec2(q) * ndc_scale + ndc_bias) * qz, qz) - p;
        float vv = dot(v, v);
        float vn = dot(v, n);
        // cosine to the sample (minus a depth-scaled bias), fading to 0 at the radius
        float falloff = max(1.0 - vv * inv_r2, 0.0);
        occ += falloff * max(vn * inversesqrt(vv + 1e-4) - bias, 0.0);
        dir = vec2(dir.x * GA_C - dir.y * GA_S, dir.x * GA_S + dir.y * GA_C);
    }
    float vis = clamp(1.0 - 1.5 * pc.size.z * occ * inv_N, 0.0, 1.0);
    // scene alpha masks AO (0 on sky); fade out with fog so fogged distance
    // isn't darkened toward black
    float mask = pc.proj.z > 1.0
        ? texelFetch(scene_color_ms, scene_px(px), 0).a
        : texelFetch(textures[nonuniformEXT(pc.color_slot)], scene_px(px), 0).a;
    float fz = length(p) * fr.fog.w;
    float fogf = fr.fog.w > 0.0 ? clamp(exp(-fz * fz), 0.0, 1.0) : 1.0;
    vis = mix(1.0, vis, clamp(mask, 0.0, 1.0) * fogf);
    o_ao = vec4(vis, pz, 0.0, 0.0);
}
