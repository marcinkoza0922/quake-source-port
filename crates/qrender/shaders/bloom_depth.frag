#version 460
#include "common.glsl"
// Downsamples scene depth (sample 0 under MSAA) into the single-sampled
// half/quarter-res bloom depth buffer. Takes the farthest (max) depth in the
// footprint so glowing details on walls never self-occlude at reduced res.
layout(push_constant) uniform P {
    vec2 scale;
    ivec2 max_px;
    uint msaa;
} pc;

float depth_at(ivec2 p) {
    return pc.msaa > 1u ? texelFetch(scene_depth_ms, p, 0).r : texelFetch(scene_depth, p, 0).r;
}

void main() {
    ivec2 p0 = min(ivec2(floor(gl_FragCoord.xy * pc.scale)), pc.max_px);
    ivec2 p1 = min(max(ivec2(floor((gl_FragCoord.xy + 1.0) * pc.scale)) - 1, p0), pc.max_px);
    gl_FragDepth = max(depth_at(p0), depth_at(p1));
}
