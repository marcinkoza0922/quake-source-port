#version 460
#include "common.glsl"
// Multiplies the SSAO target into the scene colour after the opaque draws
// (blend: dst * src). Alpha is left alone by the blend state.
layout(location = 0) in vec2 i_uv;
layout(push_constant) uniform P {
    uint ao_tex;
    uint msaa;
    uint quality;
    uint _pad;
    vec2 ao_per_scene;
    ivec2 ao_max;
} pc;
layout(location = 0) out vec4 o_color;

float linear_depth(float d) {
    const float n = 4.0, f = 65536.0;
    return n * f / (f - d * (f - n));
}

float scene_depth_at(ivec2 px) {
    if (pc.msaa > 1u) return texelFetch(scene_depth_ms, px, 0).r;
    return texelFetch(scene_depth, px, 0).r;
}

// Depth-aware blur/upsample of the AO target around AO coordinate ao_pos using
// exact full-resolution linear depth zc so silhouettes stay sharp.
float ao_at(vec2 ao_pos, float zc) {
    float sum = 0.0, wsum = 0.0;
    if (pc.quality == 0u) {
        ivec2 base = ivec2(floor(ao_pos - 0.5));
        for (int y = 0; y < 2; y++)
            for (int x = 0; x < 2; x++) {
                ivec2 q = clamp(base + ivec2(x, y), ivec2(0), pc.ao_max);
                vec2 t = texelFetch(textures[nonuniformEXT(pc.ao_tex)], q, 0).rg;
                float w = 1.0 / (0.001 + abs(t.g - zc) / max(zc, 1.0) * 50.0);
                sum += t.r * w;
                wsum += w;
            }
    } else {
        ivec2 base = ivec2(floor(ao_pos - 1.5));
        for (int y = 0; y < 4; y++)
            for (int x = 0; x < 4; x++) {
                ivec2 q = clamp(base + ivec2(x, y), ivec2(0), pc.ao_max);
                vec2 t = texelFetch(textures[nonuniformEXT(pc.ao_tex)], q, 0).rg;
                float w = 1.0 / (0.001 + abs(t.g - zc) / max(zc, 1.0) * 50.0);
                sum += t.r * w;
                wsum += w;
            }
    }
    return sum / wsum;
}

void main() {
    ivec2 spx = ivec2(gl_FragCoord.xy);
    float d = scene_depth_at(spx);
    if (d >= 1.0) { o_color = vec4(1.0); return; }
    float zc = linear_depth(d);
    o_color = vec4(vec3(ao_at(gl_FragCoord.xy * pc.ao_per_scene, zc)), 1.0);
}
