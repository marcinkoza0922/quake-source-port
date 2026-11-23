#version 460
#include "common.glsl"
// SAO linear-depth mip downsample (McGuire CSZ_minify rotated-grid tap):
// avoids averaging across silhouettes while bringing far AO taps into L1/L2 cache.
layout(push_constant) uniform P {
    uint src;
    int src_mip;
    ivec2 src_max;
} pc;
layout(location = 0) out float o_depth;
void main() {
    ivec2 p = ivec2(gl_FragCoord.xy);
    ivec2 sp = clamp(p * 2 + ivec2(p.y & 1, p.x & 1), ivec2(0), pc.src_max);
    o_depth = texelFetch(textures[nonuniformEXT(pc.src)], sp, pc.src_mip).r;
}
