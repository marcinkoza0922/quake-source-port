#version 460
#include "common.glsl"
// Preserve sample 0 exactly; resolving depth or alpha would change AO edges.
layout(push_constant) uniform P {
    vec2 scene_per_ao;
    ivec2 scene_max;
} pc;
layout(location = 0) out vec2 o_source;
void main() {
    ivec2 px = clamp(ivec2(gl_FragCoord.xy * pc.scene_per_ao), ivec2(0), pc.scene_max);
    o_source = vec2(texelFetch(scene_depth_ms, px, 0).r,
                    texelFetch(scene_color_ms, px, 0).a);
}
