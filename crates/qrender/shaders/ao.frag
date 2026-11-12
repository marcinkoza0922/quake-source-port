#version 460
#include "common.glsl"
// Screen-space ambient occlusion (Alchemy/SAO-style obscurance) from scene
// depth. Output: r = visibility (1 = unoccluded), g = view-space depth for the
// depth-aware blur in post.frag.
layout(push_constant) uniform P {
    vec4 proj;   // tan half-fov x, y, scene MSAA samples, world radius
    vec4 size;   // scene viewport w, h, intensity, scene colour texture slot
} pc;
layout(location = 0) out vec4 o_ao;

float linear_depth(float d) {
    const float n = 4.0, f = 65536.0;
    return n * f / (f - d * (f - n));
}
float fetch_depth(ivec2 px) {
    px = clamp(px, ivec2(0), ivec2(pc.size.xy) - 1);
    if (pc.proj.z > 1.0) return texelFetch(scene_depth_ms, px, 0).r;
    return texelFetch(scene_depth, px, 0).r;
}
// View-space position (x right, y down, z forward) of a pixel centre.
vec3 view_pos(ivec2 px, out float d) {
    d = fetch_depth(px);
    float z = linear_depth(d);
    vec2 ndc = (vec2(px) + 0.5) / pc.size.xy * 2.0 - 1.0;
    return vec3(ndc * pc.proj.xy * z, z);
}
vec3 view_pos(ivec2 px) { float d; return view_pos(px, d); }

void main() {
    ivec2 px = ivec2(gl_FragCoord.xy);
    float d;
    vec3 p = view_pos(px, d);
    if (d >= 1.0) { o_ao = vec4(1.0, p.z, 0.0, 0.0); return; }
    // normal from the smaller depth difference on each axis (keeps edges)
    vec3 l = view_pos(px - ivec2(1, 0)), r = view_pos(px + ivec2(1, 0));
    vec3 u = view_pos(px - ivec2(0, 1)), b = view_pos(px + ivec2(0, 1));
    vec3 dx = abs(r.z - p.z) < abs(p.z - l.z) ? r - p : p - l;
    vec3 dy = abs(b.z - p.z) < abs(p.z - u.z) ? b - p : p - u;
    vec3 n = normalize(cross(dy, dx));
    if (dot(n, p) > 0.0) n = -n;

    const int N = 12;
    float radius = pc.proj.w;
    // screen-space radius in pixels
    float rpx = radius / p.z * pc.size.x * 0.5 / pc.proj.x;
    if (rpx < 1.0) { o_ao = vec4(1.0, p.z, 0.0, 0.0); return; }
    rpx = min(rpx, pc.size.y * 0.25);
    float spin = bayer01(px & 3) * 6.2831853 + whitenoise01(vec2(px)) * 0.5;
    float occ = 0.0;
    float r2 = radius * radius;
    for (int i = 0; i < N; i++) {
        float a = (float(i) + 0.5) / float(N);
        float ang = spin + float(i) * 2.3999632; // golden-angle spiral
        float h = a * rpx;
        ivec2 q = px + ivec2(round(vec2(cos(ang), sin(ang)) * h));
        vec3 v = view_pos(q) - p;
        float vv = dot(v, v);
        float vn = dot(v, n);
        // cosine to the sample (minus a depth-scaled bias), fading to 0 at the radius
        float falloff = max(1.0 - vv / r2, 0.0);
        occ += falloff * max(vn * inversesqrt(vv + 1e-4) - 0.1 - 0.0005 * p.z, 0.0);
    }
    float vis = clamp(1.0 - 1.5 * pc.size.z * occ / float(N), 0.0, 1.0);
    // scene alpha masks AO (0 on sky); fade out with fog so fogged distance
    // isn't darkened toward black
    float mask = pc.proj.z > 1.0 ? texelFetch(scene_color_ms, px, 0).a
                                 : texelFetch(textures[nonuniformEXT(uint(pc.size.w))], px, 0).a;
    float fz = length(p) * fr.fog.w;
    float fogf = fr.fog.w > 0.0 ? clamp(exp(-fz * fz), 0.0, 1.0) : 1.0;
    vis = mix(1.0, vis, clamp(mask, 0.0, 1.0) * fogf);
    o_ao = vec4(vis, p.z, 0.0, 0.0);
}
