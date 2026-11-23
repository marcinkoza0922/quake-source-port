#version 460
#include "common.glsl"
// Extracts sample-0 linear view-space depth into AO mip 0 (R32F), so ao.frag
// reads linear depth directly and builds a cache-friendly SAO mip chain.
layout(push_constant) uniform P {
    vec2 scene_per_ao;
    ivec2 scene_max;
    uint msaa;
} pc;
layout(location = 0) out float o_depth;
void main() {
    ivec2 px = clamp(ivec2((floor(gl_FragCoord.xy) + 0.5) * pc.scene_per_ao), ivec2(0), pc.scene_max);
    float d = pc.msaa > 1u ? texelFetch(scene_depth_ms, px, 0).r : texelFetch(scene_depth, px, 0).r;
    const float n = 4.0, f = 65536.0;
    o_depth = d >= 1.0 ? f : n * f / (f - d * (f - n));
}
